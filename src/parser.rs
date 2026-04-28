use crate::state::{DerivedStats, Track, TrackBounds, TrajectoryPoint, TrajectoryProfile};
use quick_xml::events::Event;
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;

const TPE_V1: &[u8] = b"http://www.garmin.com/xmlschemas/TrackPointExtension/v1";
const TPE_V2: &[u8] = b"http://www.garmin.com/xmlschemas/TrackPointExtension/v2";

#[derive(Debug)]
pub enum GpxError {
    Xml(String),
    NoPoints,
    CyclingMissingHr,
}

impl std::fmt::Display for GpxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(msg) => write!(f, "GPX 解析失败: {}", msg),
            Self::NoPoints => write!(f, "GPX 文件不含任何轨迹点"),
            Self::CyclingMissingHr => write!(f, "cycling 默认数据必须含 hr 字段"),
        }
    }
}

impl std::error::Error for GpxError {}

#[derive(Copy, Clone, PartialEq, Eq)]
enum TextTarget {
    None,
    Ele,
    Time,
    Hr,
    Cad,
    Power,
    Speed,
    TrkName,
}

fn is_tpe_ns(ns: &ResolveResult<'_>) -> bool {
    if let ResolveResult::Bound(Namespace(uri)) = ns {
        *uri == TPE_V1 || *uri == TPE_V2
    } else {
        false
    }
}

pub fn parse_gpx(xml: &str) -> Result<Track, GpxError> {
    let mut reader = NsReader::from_str(xml);
    reader.trim_text(true);

    let mut points: Vec<TrajectoryPoint> = Vec::with_capacity(10_000);
    let mut route_name = String::new();
    let mut buf: Vec<u8> = Vec::new();

    let mut in_trk = false;
    let mut in_trkpt = false;
    let mut in_tpe = false;
    let mut cur = TrajectoryPoint::blank();
    let mut text_target = TextTarget::None;
    let mut source_index_counter: usize = 0;

    loop {
        let res = reader.read_resolved_event_into(&mut buf);
        match res {
            Err(e) => return Err(GpxError::Xml(e.to_string())),
            Ok((_ns, Event::Eof)) => break,

            Ok((ns, Event::Start(e))) => {
                let local = e.local_name();
                let lname = local.as_ref();
                if !in_trkpt {
                    match lname {
                        b"trk" => in_trk = true,
                        b"trkpt" => {
                            in_trkpt = true;
                            cur = TrajectoryPoint::blank();
                            cur.source_index = source_index_counter;
                            source_index_counter += 1;
                            for attr_res in e.attributes() {
                                let Ok(attr) = attr_res else { continue };
                                let key_local = attr.key.local_name();
                                let val = attr.unescape_value().unwrap_or_default();
                                match key_local.as_ref() {
                                    b"lat" => cur.lat = val.parse().unwrap_or(0.0),
                                    b"lon" => cur.lon = val.parse().unwrap_or(0.0),
                                    _ => {}
                                }
                            }
                        }
                        b"name" if in_trk && route_name.is_empty() => {
                            text_target = TextTarget::TrkName;
                        }
                        _ => {}
                    }
                } else {
                    match lname {
                        b"ele" => text_target = TextTarget::Ele,
                        b"time" => text_target = TextTarget::Time,
                        b"TrackPointExtension" if is_tpe_ns(&ns) => in_tpe = true,
                        b"hr" if in_tpe && is_tpe_ns(&ns) => text_target = TextTarget::Hr,
                        b"cad" if in_tpe && is_tpe_ns(&ns) => text_target = TextTarget::Cad,
                        b"power" if in_tpe && is_tpe_ns(&ns) => text_target = TextTarget::Power,
                        b"speed" if in_tpe && is_tpe_ns(&ns) => text_target = TextTarget::Speed,
                        _ => {}
                    }
                }
            }

            Ok((_ns, Event::Text(t))) => {
                let txt = t.unescape().unwrap_or_default();
                match text_target {
                    TextTarget::TrkName => route_name = txt.to_string(),
                    TextTarget::Ele => cur.ele_m = txt.parse().ok(),
                    TextTarget::Time => cur.timestamp_ms = parse_iso_time(&txt).unwrap_or(0),
                    TextTarget::Hr => cur.heart_rate_bpm = txt.parse().ok(),
                    TextTarget::Cad => cur.cadence_rpm = txt.parse().ok(),
                    TextTarget::Power => cur.power_w = txt.parse().ok(),
                    TextTarget::Speed => cur.speed_mps = txt.parse().ok(),
                    TextTarget::None => {}
                }
                text_target = TextTarget::None;
            }

            Ok((ns, Event::End(e))) => {
                let local = e.local_name();
                let lname = local.as_ref();
                match lname {
                    b"trk" => in_trk = false,
                    b"trkpt" if in_trkpt => {
                        points.push(std::mem::replace(&mut cur, TrajectoryPoint::blank()));
                        in_trkpt = false;
                        in_tpe = false;
                    }
                    b"TrackPointExtension" if is_tpe_ns(&ns) => in_tpe = false,
                    _ => {}
                }
            }

            _ => {}
        }
        buf.clear();
    }

    if points.is_empty() {
        return Err(GpxError::NoPoints);
    }
    let any_hr = points.iter().any(|p| p.heart_rate_bpm.is_some());
    if !any_hr {
        return Err(GpxError::CyclingMissingHr);
    }

    fill_speed_from_distance(&mut points);
    let stats = compute_stats(&points);

    Ok(Track {
        points,
        stats,
        route_name,
        profile: TrajectoryProfile::Cycling,
    })
}

fn fill_speed_from_distance(points: &mut [TrajectoryPoint]) {
    if points.len() < 2 {
        return;
    }
    let n = points.len();
    let mut speeds: Vec<f32> = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = if i == 0 { (0, 1) } else { (i - 1, i) };
        let p1 = &points[a];
        let p2 = &points[b];
        let dt_ms = p2.timestamp_ms - p1.timestamp_ms;
        let s = if dt_ms <= 0 {
            0.0
        } else {
            let d = haversine(p1.lat, p1.lon, p2.lat, p2.lon);
            (d / (dt_ms as f64 / 1000.0)) as f32
        };
        speeds.push(s.max(0.0));
    }
    let win: usize = 5;
    for (i, point) in points.iter_mut().enumerate() {
        let lo = i.saturating_sub(win / 2);
        let hi = (i + win / 2 + 1).min(n);
        let mut sum = 0.0_f32;
        let mut cnt = 0u32;
        for s in &speeds[lo..hi] {
            sum += *s;
            cnt += 1;
        }
        let smoothed = if cnt > 0 { sum / cnt as f32 } else { 0.0 };
        point.speed_mps = Some(smoothed);
    }
}

fn compute_stats(points: &[TrajectoryPoint]) -> DerivedStats {
    let mut s = DerivedStats::default();
    if points.is_empty() {
        return s;
    }

    s.track_bounds = TrackBounds {
        lat_min: points[0].lat,
        lat_max: points[0].lat,
        lon_min: points[0].lon,
        lon_max: points[0].lon,
    };
    s.elevation_min_m = points[0].ele_m.unwrap_or(0.0);
    s.elevation_max_m = points[0].ele_m.unwrap_or(0.0);
    s.ele_min = s.elevation_min_m;
    s.ele_max = s.elevation_max_m;
    s.speed_min_mps = points[0].speed_mps.unwrap_or(0.0);
    s.speed_max_mps = s.speed_min_mps;
    s.hr_min = points[0].heart_rate_bpm.unwrap_or(u16::MAX);
    s.hr_max = points[0].heart_rate_bpm.unwrap_or(0);
    s.cad_min = points[0].cadence_rpm.unwrap_or(u16::MAX);
    s.cad_max = points[0].cadence_rpm.unwrap_or(0);

    let mut total_dist = 0.0_f64;
    let mut hr_sum: u64 = 0;
    let mut hr_cnt: u64 = 0;
    let mut prev_ele: Option<f32> = points[0].ele_m;
    let mut gain: f32 = 0.0;

    for (i, p) in points.iter().enumerate() {
        s.track_bounds.lat_min = s.track_bounds.lat_min.min(p.lat);
        s.track_bounds.lat_max = s.track_bounds.lat_max.max(p.lat);
        s.track_bounds.lon_min = s.track_bounds.lon_min.min(p.lon);
        s.track_bounds.lon_max = s.track_bounds.lon_max.max(p.lon);

        if let Some(e) = p.ele_m {
            s.ele_min = s.ele_min.min(e);
            s.ele_max = s.ele_max.max(e);
            s.elevation_min_m = s.ele_min;
            s.elevation_max_m = s.ele_max;
            if let Some(prev) = prev_ele {
                let d = e - prev;
                if d > 0.0 {
                    gain += d;
                }
            }
            prev_ele = Some(e);
        }

        if let Some(speed) = p.speed_mps {
            s.speed_min_mps = s.speed_min_mps.min(speed);
            s.speed_max_mps = s.speed_max_mps.max(speed);
        }

        if let Some(hr) = p.heart_rate_bpm {
            hr_sum += hr as u64;
            hr_cnt += 1;
            s.hr_min = s.hr_min.min(hr);
            s.hr_max = s.hr_max.max(hr);
            s.observed_max_hr = s.observed_max_hr.max(hr);
        }
        if let Some(cad) = p.cadence_rpm {
            s.cad_min = s.cad_min.min(cad);
            s.cad_max = s.cad_max.max(cad);
        }

        if i > 0 {
            let prev = &points[i - 1];
            total_dist += haversine(prev.lat, prev.lon, p.lat, p.lon);
        }
    }

    s.distance_m_total = total_dist as f32;
    s.duration_ms_total =
        points.last().unwrap().timestamp_ms - points.first().unwrap().timestamp_ms;
    s.elevation_gain_m = gain;
    s.avg_hr = if hr_cnt > 0 {
        hr_sum as f32 / hr_cnt as f32
    } else {
        0.0
    };
    s.speed_max_mps_ceil = s.speed_max_mps.ceil();
    if s.hr_min == u16::MAX {
        s.hr_min = 0;
    }
    if s.cad_min == u16::MAX {
        s.cad_min = 0;
    }
    s
}

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0_f64;
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dphi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().asin()
}

fn parse_iso_time(s: &str) -> Option<i64> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    let hour: u32 = s.get(11..13)?.parse().ok()?;
    let minute: u32 = s.get(14..16)?.parse().ok()?;
    let second: u32 = s.get(17..19)?.parse().ok()?;

    let mut idx = 19;
    let mut millis: u32 = 0;
    if bytes.get(idx).copied() == Some(b'.') {
        idx += 1;
        let mut count = 0u32;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() && count < 3 {
            millis = millis * 10 + (bytes[idx] - b'0') as u32;
            idx += 1;
            count += 1;
        }
        for _ in count..3 {
            millis *= 10;
        }
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
    }

    let mut tz_offset_sec: i64 = 0;
    if idx < bytes.len() {
        match bytes[idx] {
            b'Z' | b'z' => {}
            b'+' | b'-' => {
                let sign: i64 = if bytes[idx] == b'-' { -1 } else { 1 };
                idx += 1;
                if idx + 4 <= bytes.len() {
                    let hh: i64 = s.get(idx..idx + 2)?.parse().ok()?;
                    let mm: i64 = if bytes.get(idx + 2).copied() == Some(b':') {
                        s.get(idx + 3..idx + 5)?.parse().ok()?
                    } else {
                        s.get(idx + 2..idx + 4)?.parse().ok()?
                    };
                    tz_offset_sec = sign * (hh * 3600 + mm * 60);
                }
            }
            _ => {}
        }
    }

    let days = days_from_civil(year, month, day);
    let secs =
        days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64 - tz_offset_sec;
    Some(secs * 1000 + millis as i64)
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let mp = if m > 2 { m as i64 - 3 } else { m as i64 + 9 };
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_gpx() {
        let xml = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:ns3="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
  <trk>
    <name>Test Route</name>
    <trkseg>
      <trkpt lat="35.0" lon="-121.0">
        <ele>10.0</ele>
        <time>2019-10-22T14:36:00.000Z</time>
        <extensions><ns3:TrackPointExtension><ns3:hr>110</ns3:hr><ns3:cad>30</ns3:cad></ns3:TrackPointExtension></extensions>
      </trkpt>
      <trkpt lat="35.0001" lon="-121.0001">
        <ele>11.0</ele>
        <time>2019-10-22T14:36:01.000Z</time>
        <extensions><ns3:TrackPointExtension><ns3:hr>112</ns3:hr></ns3:TrackPointExtension></extensions>
      </trkpt>
    </trkseg>
  </trk>
</gpx>
"#;
        let track = parse_gpx(xml).expect("parse ok");
        assert_eq!(track.points.len(), 2);
        assert_eq!(track.route_name, "Test Route");
        assert_eq!(track.points[0].heart_rate_bpm, Some(110));
        assert_eq!(track.points[0].cadence_rpm, Some(30));
        assert_eq!(track.points[1].heart_rate_bpm, Some(112));
        assert_eq!(track.points[1].cadence_rpm, None);
        assert!(track.points[0].speed_mps.unwrap() >= 0.0);
    }

    #[test]
    fn parses_real_bundled_gpx() {
        let xml = include_str!("../assets/cycling-track.gpx");
        let track = parse_gpx(xml).expect("real GPX must parse");
        assert!(
            track.points.len() > 1000,
            "expected several thousand trkpts, got {}",
            track.points.len()
        );
        let with_hr = track
            .points
            .iter()
            .filter(|p| p.heart_rate_bpm.is_some())
            .count();
        assert!(with_hr > 0, "real cycling GPX must have hr on most points");
        assert!(track.stats.observed_max_hr > 100);
        assert!(track.stats.distance_m_total > 1000.0);
        assert!(track.stats.duration_ms_total > 60_000);
        assert!(track.stats.speed_max_mps > 0.0);
    }

    #[test]
    fn rejects_when_no_hr() {
        let xml = r#"<?xml version="1.0"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1">
  <trk><name>X</name><trkseg>
    <trkpt lat="35.0" lon="-121.0"><ele>10</ele><time>2019-10-22T14:36:00.000Z</time></trkpt>
    <trkpt lat="35.0001" lon="-121.0001"><ele>11</ele><time>2019-10-22T14:36:01.000Z</time></trkpt>
  </trkseg></trk>
</gpx>"#;
        match parse_gpx(xml) {
            Err(GpxError::CyclingMissingHr) => {}
            other => panic!("expected CyclingMissingHr, got {:?}", other),
        }
    }
}
