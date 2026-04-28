#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TrajectoryProfile {
    #[default]
    Cycling,
    Running,
    Hiking,
    Walking,
    Transit,
    Travel,
    Flight,
}

impl TrajectoryProfile {
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Cycling => "骑行",
            Self::Running => "跑步",
            Self::Hiking => "徒步",
            Self::Walking => "步行",
            Self::Transit => "通勤",
            Self::Travel => "旅行",
            Self::Flight => "飞行",
        }
    }

    pub fn from_manifest_str(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "hiking" => Self::Hiking,
            "walking" => Self::Walking,
            "transit" => Self::Transit,
            "travel" => Self::Travel,
            "flight" => Self::Flight,
            _ => Self::Cycling,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    Network,
    LocalFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    Idle,
    Fetching,
    Success,
    Fallback,
}

#[derive(Debug, Clone)]
pub struct TrajectoryPoint {
    pub lat: f64,
    pub lon: f64,
    pub ele_m: Option<f32>,
    pub timestamp_ms: i64,
    pub speed_mps: Option<f32>,
    pub heart_rate_bpm: Option<u16>,
    pub cadence_rpm: Option<u16>,
    pub power_w: Option<u16>,
    pub heading_deg: Option<f32>,
    pub altitude_m: Option<f32>,
    pub transport_mode: Option<String>,
    pub route_label: Option<String>,
    pub source_index: usize,
}

impl TrajectoryPoint {
    pub fn blank() -> Self {
        Self {
            lat: 0.0,
            lon: 0.0,
            ele_m: None,
            timestamp_ms: 0,
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_w: None,
            heading_deg: None,
            altitude_m: None,
            transport_mode: None,
            route_label: None,
            source_index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrackBounds {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
}

#[derive(Debug, Clone, Default)]
pub struct DerivedStats {
    pub distance_m_total: f32,
    pub duration_ms_total: i64,
    pub elevation_min_m: f32,
    pub elevation_max_m: f32,
    pub elevation_gain_m: f32,
    pub observed_max_hr: u16,
    pub avg_hr: f32,
    pub speed_min_mps: f32,
    pub speed_max_mps: f32,
    pub speed_max_mps_ceil: f32,
    pub track_bounds: TrackBounds,
    pub hr_min: u16,
    pub hr_max: u16,
    pub cad_min: u16,
    pub cad_max: u16,
    pub ele_min: f32,
    pub ele_max: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Track {
    pub points: Vec<TrajectoryPoint>,
    pub stats: DerivedStats,
    pub route_name: String,
    pub profile: TrajectoryProfile,
}

#[derive(Debug, Clone, Copy)]
pub struct UserProfile {
    pub max_hr: u16,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self { max_hr: 195 }
    }
}

#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub profile: TrajectoryProfile,
    pub playback_progress: f32,
    pub current_trkpt_index: usize,
    pub current_speed_mps: f32,
    pub current_hr_bpm: Option<u16>,
    pub current_ele_m: Option<f32>,
    pub current_cad_rpm: Option<u16>,
    pub playback_speed: f32,
    pub is_paused: bool,
    pub data_source: DataSource,
    pub network_state: NetworkState,
    pub contract_guard_active: bool,
    pub walked_segment_ratio: f32,
    pub scrubber_echo_phase: f32,
    pub network_state_entered_at_secs: f64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            profile: TrajectoryProfile::Cycling,
            playback_progress: 0.0,
            current_trkpt_index: 0,
            current_speed_mps: 0.0,
            current_hr_bpm: None,
            current_ele_m: None,
            current_cad_rpm: None,
            playback_speed: 4.0,
            is_paused: false,
            data_source: DataSource::LocalFallback,
            network_state: NetworkState::Idle,
            contract_guard_active: false,
            walked_segment_ratio: 0.0,
            scrubber_echo_phase: 0.0,
            network_state_entered_at_secs: 0.0,
        }
    }
}

impl PlaybackState {
    pub fn sync_status_text(&self) -> &'static str {
        match self.network_state {
            NetworkState::Idle | NetworkState::Fetching => "同步中...",
            NetworkState::Success => "已同步",
            NetworkState::Fallback => "本地缓存",
        }
    }

    pub fn apply_progress(&mut self, track: &Track, progress: f32) {
        let p = progress.clamp(0.0, 1.0);
        self.playback_progress = p;
        self.walked_segment_ratio = p;
        if track.points.is_empty() {
            return;
        }
        let n = track.points.len();
        let idx = ((p * (n - 1) as f32).round() as usize).min(n - 1);
        self.current_trkpt_index = idx;
        let pt = &track.points[idx];
        self.current_speed_mps = pt.speed_mps.unwrap_or(0.0);
        self.current_hr_bpm = pt.heart_rate_bpm;
        self.current_ele_m = pt.ele_m;
        self.current_cad_rpm = pt.cadence_rpm;
    }
}

pub fn effective_max_hr(user: &UserProfile, track: &Track) -> u16 {
    user.max_hr.max(track.stats.observed_max_hr)
}

pub fn five_minute_window_avg_hr(track: &Track, center_index: usize) -> Option<f32> {
    if track.points.is_empty() {
        return None;
    }
    let center = center_index.min(track.points.len() - 1);
    let center_ts = track.points[center].timestamp_ms;
    let half_window_ms: i64 = 150_000;
    let mut sum: u32 = 0;
    let mut count: u32 = 0;
    for p in &track.points {
        if (p.timestamp_ms - center_ts).abs() <= half_window_ms {
            if let Some(hr) = p.heart_rate_bpm {
                sum += hr as u32;
                count += 1;
            }
        }
    }
    if count == 0 {
        None
    } else {
        Some(sum as f32 / count as f32)
    }
}
