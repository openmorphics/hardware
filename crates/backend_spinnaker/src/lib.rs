use anyhow::Result;

pub fn compile(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<String> {
    graph.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    nc_hal::validate_manifest(manifest)?;
    Ok(format!("compiled:{}:{}", manifest.name, graph.name))
}
