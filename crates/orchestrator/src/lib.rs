use anyhow::Result;
pub use nc_nir as nir;

#[derive(Debug, Clone)]
pub struct PartitionPlan {
    pub parts: usize,
}

pub fn partition(_g: &nir::Graph, _targets: &[&str]) -> Result<PartitionPlan> {
    Ok(PartitionPlan { parts: 1 })
}

pub fn version() -> &'static str { "0.0.1" }
