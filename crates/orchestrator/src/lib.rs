use anyhow::Result;
pub use nc_nir as nir;
#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;

#[derive(Debug, Clone)]
pub struct PartitionPlan {
    pub parts: usize,
}

pub fn partition(_g: &nir::Graph, _targets: &[&str]) -> Result<PartitionPlan> {
    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| telemetry::profiling::Appender::open(p).ok());

    #[cfg(feature = "telemetry")]
    let _timer = {
        if let Some(a) = app.as_ref() {
            let mut labels = BTreeMap::new();
            labels.insert("graph".to_string(), _g.name.clone());
            labels.insert("targets".to_string(), _targets.join(","));
            Some(a.start_timer("orchestrator.partition_ms", labels))
        } else { None }
    };

    // Stub plan
    let plan = PartitionPlan { parts: 1 };

    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let mut l = BTreeMap::new();
        l.insert("graph".to_string(), _g.name.clone());
        let _ = a.counter("orchestrator.parts", plan.parts as f64, l);
    }
    Ok(plan)
}

pub fn version() -> &'static str { "0.0.1" }
