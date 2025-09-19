use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use nc_nir as nir;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;

pub fn emit_artifacts(g: &nir::Graph, out_dir: &Path) -> Result<PathBuf> {
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }

    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| telemetry::profiling::Appender::open(p).ok());

    #[cfg(feature = "telemetry")]
    let _timer = {
        if let Some(a) = app.as_ref() {
            let labels = telemetry::labels::simulator(&g.name, "arbor");
            Some(a.start_timer("sim.emit_ms", labels))
        } else {
            None
        }
    };

    let summary = serde_json::json!({
        "simulator": "arbor",
        "name": g.name,
        "populations": g.populations.len(),
        "connections": g.connections.len(),
        "probes": g.probes.len()
    });
    fs::write(out_dir.join("model_summary.json"), serde_json::to_string_pretty(&summary)?)?;
    fs::write(out_dir.join("RUN.txt"), format!("simulator: arbor\nname: {}\n", g.name))?;

    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let l = telemetry::labels::simulator(&g.name, "arbor");
        let _ = a.counter("graph.populations", g.populations.len() as f64, l.clone());
        let _ = a.counter("graph.connections", g.connections.len() as f64, l.clone());
        let _ = a.counter("graph.probes", g.probes.len() as f64, l);
    }

    Ok(out_dir.to_path_buf())
}

pub fn stub() -> &'static str { "ok" }
