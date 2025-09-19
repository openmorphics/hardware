use anyhow::Result;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;

pub fn compile(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<String> {
    graph.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    nc_hal::validate_manifest(manifest)?;

    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| telemetry::profiling::Appender::open(p).ok());
    #[cfg(feature = "telemetry")]
    let _timer = {
        if let Some(a) = app.as_ref() {
            let mut labels = BTreeMap::new();
            labels.insert("backend".to_string(), "akida".to_string());
            labels.insert("target".to_string(), manifest.name.clone());
            labels.insert("graph".to_string(), graph.name.clone());
            Some(a.start_timer("backend.compile_ms", labels))
        } else {
            None
        }
    };

    let artifact = format!("compiled:{}:{}", manifest.name, graph.name);

    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let l = telemetry::labels::backend(&graph.name, "akida", Some(&manifest.name));
        let _ = a.counter("graph.populations", graph.populations.len() as f64, l.clone());
        let _ = a.counter("graph.connections", graph.connections.len() as f64, l.clone());
        let _ = a.counter("graph.probes", graph.probes.len() as f64, l);
    }

    Ok(artifact)
}
