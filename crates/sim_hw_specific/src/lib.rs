#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;
use anyhow::Result;
use std::fs;
use std::path::Path;

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

/// Emit minimal artifacts for a hardware-specific simulation run.
/// Writes RUN.txt and a simple model_summary.txt under out_dir.
pub fn emit_artifacts(g: &nc_nir::Graph, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    #[cfg(feature = "telemetry")]
    {
        if let Ok(p) = std::env::var("NC_PROFILE_JSONL") {
            if let Ok(a) = telemetry::profiling::Appender::open(p) {
                let mut labels = BTreeMap::new();
                labels.insert("simulator".to_string(), "hw".to_string());
                labels.insert("graph".to_string(), g.name.clone());
                let _ = a.counter("sim.hw_runs", 1.0, labels);
            }
        }
    }
    fs::write(out_dir.join("RUN.txt"), format!("hw simulate run for {}\n", g.name))?;
    let summary = format!(
        "graph={}, populations={}, connections={}\n",
        g.name, g.populations.len(), g.connections.len()
    );
    fs::write(out_dir.join("model_summary.txt"), summary)?;
    Ok(())
}
