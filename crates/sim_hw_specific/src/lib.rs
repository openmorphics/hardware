#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;

pub fn stub() -> &'static str {
    #[cfg(feature = "telemetry")]
    {
        if let Ok(p) = std::env::var("NC_PROFILE_JSONL") {
            if let Ok(a) = telemetry::profiling::Appender::open(p) {
                let mut labels = BTreeMap::new();
                labels.insert("simulator".to_string(), "hw".to_string());
                let _ = a.counter("sim.stub_calls", 1.0, labels);
            }
        }
    }
    "ok"
}
