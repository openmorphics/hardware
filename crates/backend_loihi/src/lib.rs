use anyhow::Result;

pub fn compile(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<String> {
    // Basic validation
    graph.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    nc_hal::validate_manifest(manifest)?;

    // Optional telemetry profiling
    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| nc_telemetry::profiling::Appender::open(p).ok());
    #[cfg(feature = "telemetry")]
    let _timer = {
        if let Some(a) = app.as_ref() {
            let labels = nc_telemetry::labels::backend(&graph.name, "loihi", Some(&manifest.name));
            Some(a.start_timer("backend.compile_ms", labels))
        } else {
            None
        }
    };

    // Return a simple artifact descriptor for now
    let artifact = format!("compiled:{}:{}", manifest.name, graph.name);

    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let l = nc_telemetry::labels::backend(&graph.name, "loihi", Some(&manifest.name));
        let _ = a.counter("graph.populations", graph.populations.len() as f64, l.clone());
        let _ = a.counter("graph.connections", graph.connections.len() as f64, l.clone());
        let _ = a.counter("graph.probes", graph.probes.len() as f64, l);
    }

    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compile_smoke() {
        let g = nc_nir::Graph::new("g");
        let m = nc_hal::parse_target_manifest_str(r#"
            name = "loihi2"
            vendor = "Intel"
            family = "Loihi"
            version = "2"
            [capabilities]
            weight_precisions = [8]
            max_neurons_per_core = 1
            max_synapses_per_core = 1
            time_resolution_ns = 1
        "#).unwrap();
        let _ = compile(&g, &m).unwrap();
    }
}
