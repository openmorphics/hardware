pub fn init() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
}

/// Initialize OpenTelemetry exporter (stub; no-op).
pub fn init_otel(_endpoint: Option<&str>) -> anyhow::Result<()> {
    // Future: configure an OTLP exporter when feature-flagged.
    Ok(())
}

pub mod profiling {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::fs::File;
    use std::io::{BufRead, Write};
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    /// JSONL Profile record schema (one record per line).
    /// Fields:
    /// - ts_ms: epoch milliseconds
    /// - metric: metric name (e.g., "latency_ms", "spikes_per_sec")
    /// - value: numeric value
    /// - labels: key/value tags (backend="loihi2", chip="0", etc.)
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProfileRecord {
        pub ts_ms: u64,
        pub metric: String,
        pub value: f64,
        #[serde(default)]
        pub labels: BTreeMap<String, String>,
    }

    /// Emit an array of profile records as JSON Lines (one JSON object per line).
    pub fn emit_profile_jsonl<P: AsRef<Path>>(path: P, records: &[ProfileRecord]) -> Result<()> {
        let mut f = File::create(path)?;
        for r in records {
            let line = serde_json::to_string(r)?;
            writeln!(f, "{line}")?;
        }
        Ok(())
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Appender writes ProfileRecord lines to a JSONL file and provides timer/counter helpers.
    pub struct Appender {
        file: Arc<Mutex<File>>,
    }

    impl Appender {
        pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            let f = File::create(path)?;
            Ok(Self { file: Arc::new(Mutex::new(f)) })
        }

        pub fn log(&self, rec: &ProfileRecord) -> Result<()> {
            let line = serde_json::to_string(rec)?;
            let mut guard = self.file.lock().expect("poisoned lock");
            writeln!(&mut *guard, "{line}")?;
            Ok(())
        }

        pub fn start_timer(&self, metric: impl Into<String>, labels: BTreeMap<String, String>) -> TimerGuard {
            TimerGuard {
                start: Instant::now(),
                metric: metric.into(),
                labels,
                file: self.file.clone(),
            }
        }

        pub fn counter(&self, metric: impl Into<String>, value: f64, labels: BTreeMap<String, String>) -> Result<()> {
            let rec = ProfileRecord {
                ts_ms: now_ms(),
                metric: metric.into(),
                value,
                labels,
            };
            self.log(&rec)
        }
    }

    /// A guard that records elapsed time to the JSONL on drop.
    pub struct TimerGuard {
        start: Instant,
        metric: String,
        labels: BTreeMap<String, String>,
        file: Arc<Mutex<File>>,
    }

    impl Drop for TimerGuard {
        fn drop(&mut self) {
            let elapsed_ms = self.start.elapsed().as_secs_f64() * 1000.0;
            let rec = ProfileRecord {
                ts_ms: now_ms(),
                metric: self.metric.clone(),
                value: elapsed_ms,
                labels: std::mem::take(&mut self.labels),
            };
            if let Ok(mut guard) = self.file.lock() {
                let _ = writeln!(&mut *guard, "{}", serde_json::to_string(&rec).unwrap_or_default());
            }
        }
    }

    /// Summarize a JSONL file of ProfileRecord objects into (count,sum,min,max) per metric.
    pub fn summarize_jsonl<P: AsRef<Path>>(path: P) -> Result<std::collections::HashMap<String, (usize, f64, f64, f64)>> {
        let f = std::fs::File::open(path)?;
        let rdr = std::io::BufReader::new(f);
        let mut stats: std::collections::HashMap<String, (usize, f64, f64, f64)> = std::collections::HashMap::new();
        for line in rdr.lines() {
            if let Ok(l) = line {
                if l.trim().is_empty() { continue; }
                if let Ok(rec) = serde_json::from_str::<ProfileRecord>(&l) {
                    let e = stats.entry(rec.metric.clone())
                        .or_insert((0, 0.0, f64::INFINITY, f64::NEG_INFINITY));
                    e.0 += 1;
                    e.1 += rec.value;
                    if rec.value < e.2 { e.2 = rec.value; }
                    if rec.value > e.3 { e.3 = rec.value; }
                }
            }
        }
        Ok(stats)
    }
}


#[cfg(test)]
mod tests_profile {
    use super::profiling::{emit_profile_jsonl, ProfileRecord};
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn emit_jsonl_file() {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let mut labels = BTreeMap::new();
        labels.insert("target".to_string(), "loihi2".to_string());
        let recs = vec![
            ProfileRecord { ts_ms: ts, metric: "latency_ms".into(), value: 1.23, labels: labels.clone() },
            ProfileRecord { ts_ms: ts + 1, metric: "spikes_per_sec".into(), value: 45678.0, labels },
        ];
        let mut path = std::env::temp_dir();
        path.push("nc_profile_test.jsonl");
        let _ = std::fs::remove_file(&path);
        emit_profile_jsonl(&path, &recs).expect("emit profile jsonl");
        let data = std::fs::read_to_string(&path).expect("read jsonl");
        assert!(data.lines().count() >= 2, "expected at least 2 JSONL records");
    }
}
