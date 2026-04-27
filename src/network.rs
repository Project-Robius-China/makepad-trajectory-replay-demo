use crate::parser::parse_gpx;
use crate::state::Track;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

pub enum FetchResult {
    Success(Track),
    Fallback(String),
}

pub fn spawn_fetch_worker() -> (Receiver<FetchResult>, std::thread::ThreadId) {
    let (tx, rx) = channel();
    let handle = std::thread::Builder::new()
        .name("trajectory-fetch".into())
        .spawn(move || {
            let result = try_fetch();
            let _ = tx.send(result);
        })
        .expect("spawn fetch worker");
    let id = handle.thread().id();
    (rx, id)
}

fn try_fetch() -> FetchResult {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(3))
        .user_agent("makepad-trajectory-replay-demo/0.1 (+github.com/Project-Robius-China)")
        .build();

    let manifest_api = "https://api.github.com/repos/Project-Robius-China/trajectory-replay-data/contents/manifest.json?ref=main";
    let manifest_resp = match agent
        .get(manifest_api)
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r,
        Err(e) => return FetchResult::Fallback(format!("manifest req: {}", e)),
    };
    if manifest_resp.status() != 200 {
        return FetchResult::Fallback(format!(
            "manifest status {}",
            manifest_resp.status()
        ));
    }
    let manifest_meta = match manifest_resp.into_string() {
        Ok(s) => s,
        Err(e) => return FetchResult::Fallback(format!("manifest read: {}", e)),
    };

    let download_url = match extract_string_field(&manifest_meta, "download_url") {
        Some(u) => u,
        None => return FetchResult::Fallback("manifest no download_url".into()),
    };

    let manifest_text = match agent.get(&download_url).call() {
        Ok(r) => match r.into_string() {
            Ok(s) => s,
            Err(e) => return FetchResult::Fallback(format!("manifest body: {}", e)),
        },
        Err(e) => return FetchResult::Fallback(format!("manifest dl: {}", e)),
    };

    let default_dataset = match extract_string_field(&manifest_text, "default_dataset") {
        Some(s) => s,
        None => return FetchResult::Fallback("no default_dataset".into()),
    };

    let raw_url = format!(
        "https://raw.githubusercontent.com/Project-Robius-China/trajectory-replay-data/main/{}",
        default_dataset
    );
    let gpx_text = match agent.get(&raw_url).call() {
        Ok(r) => match r.into_string() {
            Ok(s) => s,
            Err(e) => return FetchResult::Fallback(format!("gpx read: {}", e)),
        },
        Err(e) => return FetchResult::Fallback(format!("gpx dl: {}", e)),
    };

    match parse_gpx(&gpx_text) {
        Ok(t) => FetchResult::Success(t),
        Err(e) => FetchResult::Fallback(format!("gpx parse: {}", e)),
    }
}

fn extract_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let idx = text.find(&needle)?;
    let rest = &text[idx + needle.len()..];
    let colon = rest.find(':')?;
    let after_colon = &rest[colon + 1..];
    let q1 = after_colon.find('"')?;
    let after_q1 = &after_colon[q1 + 1..];
    let q2 = find_unescaped_quote(after_q1)?;
    Some(after_q1[..q2].to_string())
}

fn find_unescaped_quote(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            return Some(i);
        }
        i += 1;
    }
    None
}
