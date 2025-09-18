use anyhow::Result;

pub fn compile(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<String> {
    // Basic validation
    graph.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    nc_hal::validate_manifest(manifest)?;
    // Return a simple artifact descriptor for now
    Ok(format!("compiled:{}:{}", manifest.name, graph.name))
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
