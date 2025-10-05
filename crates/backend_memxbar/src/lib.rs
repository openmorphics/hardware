use anyhow::Result;
use std::collections::BTreeMap;
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
            labels.insert("backend".to_string(), "memxbar".to_string());
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
        let l = telemetry::labels::backend(&graph.name, "memxbar", Some(&manifest.name));
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
            name = "memxbar"
            vendor = "Generic"
            family = "MemXBar"
            version = "1"
            [capabilities]
            weight_precisions = [8]
            max_neurons_per_core = 1
            max_synapses_per_core = 1
            time_resolution_ns = 1
        "#).unwrap();
        let out = compile(&g, &m).expect("compile ok");
        assert!(out.starts_with("compiled:"));
    }
}
