use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub static RECORDING: OnceLock<RecordingShared> = OnceLock::new();

pub struct RecordingShared {
    pub inner: Mutex<Option<RecordingInner>>,
}

pub struct RecordingInner {
    pub start_time: Instant,
    pub events: Vec<(f64, Vec<u8>)>,
}

impl RecordingShared {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn start(&self) {
        let mut guard = self.inner.lock().unwrap();
        *guard = Some(RecordingInner {
            start_time: Instant::now(),
            events: Vec::new(),
        });
    }

    pub fn stop(&self) -> Option<(f64, Vec<(f64, Vec<u8>)>)> {
        let mut guard = self.inner.lock().unwrap();
        let state = guard.take()?;
        let elapsed = state.start_time.elapsed().as_secs_f64();
        Some((elapsed, state.events))
    }

    pub fn is_recording(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }

    pub fn push_event(&self, staging: &[u8]) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(ref mut state) = *guard {
            let ts = state.start_time.elapsed().as_secs_f64();
            state.events.push((ts, staging.to_vec()));
        }
    }
}

pub fn write_cast_file(
    path: &PathBuf,
    width: u16,
    height: u16,
    duration: f64,
    shell: &str,
    term: &str,
    events: &[(f64, Vec<u8>)],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(path)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let header = serde_json::json!({
        "version": 2,
        "width": width,
        "height": height,
        "timestamp": timestamp,
        "duration": duration,
        "env": {
            "SHELL": shell,
            "TERM": term,
        }
    });
    writeln!(f, "{}", serde_json::to_string(&header).unwrap())?;

    for (ts, data) in events {
        let line = serde_json::json!([ts, "o", &String::from_utf8_lossy(data)]);
        writeln!(f, "{}", serde_json::to_string(&line).unwrap())?;
    }

    f.flush()?;
    Ok(())
}

pub fn format_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let sec_total = now.as_secs();
    let s = sec_total % 60;
    let m = (sec_total / 60) % 60;
    let h = (sec_total / 3600) % 24;
    let days = (sec_total / 86400) as i64;
    let mut remaining = days;
    let mut y = 1970i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let months = [
        31,
        28 + is_leap(y) as i64,
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u32;
    for md in &months {
        if remaining < *md {
            break;
        }
        remaining -= *md;
        mo += 1;
    }
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        y,
        mo,
        remaining + 1,
        h,
        m,
        s
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn default_recording_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(dir).join("SYNAPSE_").join("recordings")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".cache")
            .join("SYNAPSE_")
            .join("recordings")
    } else {
        PathBuf::from("/tmp").join("SYNAPSE_").join("recordings")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_shared_start_stop() {
        let rs = RecordingShared::new();
        assert!(!rs.is_recording());
        rs.start();
        assert!(rs.is_recording());
        let result = rs.stop();
        assert!(result.is_some());
        assert!(!rs.is_recording());
    }

    #[test]
    fn test_push_events() {
        let rs = RecordingShared::new();
        rs.start();
        rs.push_event(b"hello");
        rs.push_event(b"world");
        let (duration, events) = rs.stop().unwrap();
        assert!(duration > 0.0);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].1, b"hello");
        assert_eq!(events[1].1, b"world");
    }

    #[test]
    fn test_write_cast_file() {
        let dir = std::env::temp_dir().join("synapse_test_record");
        let path = dir.join("test.cast");
        let events = vec![(0.1, b"hello\n".to_vec()), (0.3, b"world\n".to_vec())];
        write_cast_file(&path, 80, 24, 0.5, "/bin/bash", "xterm-256color", &events).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"version\":2"));
        assert!(content.contains("\"width\":80"));
        assert!(content.contains("hello"));
        assert!(content.contains("world"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
