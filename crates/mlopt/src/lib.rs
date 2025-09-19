use anyhow::Result;
#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;

pub fn version() -> &'static str { "0.0.1" }

pub trait CostModel {
    fn predict_latency_ms(&self, graph: &nc_nir::Graph) -> Result<f64>;
    fn predict_energy_mj(&self, graph: &nc_nir::Graph) -> Result<f64>;
}

pub struct NoOpCostModel;

impl CostModel for NoOpCostModel {
    fn predict_latency_ms(&self, _graph: &nc_nir::Graph) -> Result<f64> {
        #[cfg(feature = "telemetry")]
        let app = std::env::var("NC_PROFILE_JSONL")
            .ok()
            .and_then(|p| telemetry::profiling::Appender::open(p).ok());
        #[cfg(feature = "telemetry")]
        let _t = {
            if let Some(a) = app.as_ref() {
                let mut labels = BTreeMap::new();
                labels.insert("graph".to_string(), _graph.name.clone());
                labels.insert("kind".to_string(), "latency".to_string());
                Some(a.start_timer("mlopt.cost_predict_ms", labels))
            } else { None }
        };
        Ok(0.0)
    }
    fn predict_energy_mj(&self, _graph: &nc_nir::Graph) -> Result<f64> {
        #[cfg(feature = "telemetry")]
        let app = std::env::var("NC_PROFILE_JSONL")
            .ok()
            .and_then(|p| telemetry::profiling::Appender::open(p).ok());
        #[cfg(feature = "telemetry")]
        let _t = {
            if let Some(a) = app.as_ref() {
                let mut labels = BTreeMap::new();
                labels.insert("graph".to_string(), _graph.name.clone());
                labels.insert("kind".to_string(), "energy".to_string());
                Some(a.start_timer("mlopt.cost_predict_ms", labels))
            } else { None }
        };
        Ok(0.0)
    }
}

pub trait MappingSearch {
    fn propose(&mut self, graph: &nc_nir::Graph) -> Result<String>;
    fn feedback(&mut self, score: f64);
}

#[derive(Default)]
pub struct GreedySearchStub {
    last_score: Option<f64>,
}

impl GreedySearchStub {
    pub fn new() -> Self { Self { last_score: None } }
}

impl MappingSearch for GreedySearchStub {
    fn propose(&mut self, _graph: &nc_nir::Graph) -> Result<String> {
        #[cfg(feature = "telemetry")]
        let app = std::env::var("NC_PROFILE_JSONL")
            .ok()
            .and_then(|p| telemetry::profiling::Appender::open(p).ok());
        #[cfg(feature = "telemetry")]
        let _t = {
            if let Some(a) = app.as_ref() {
                let mut labels = BTreeMap::new();
                labels.insert("graph".to_string(), _graph.name.clone());
                Some(a.start_timer("mlopt.search.propose_ms", labels))
            } else { None }
        };
        Ok("identity".to_string())
    }
    fn feedback(&mut self, score: f64) {
        #[cfg(feature = "telemetry")]
        if let (Ok(p), true) = (std::env::var("NC_PROFILE_JSONL"), true) {
            if let Ok(a) = telemetry::profiling::Appender::open(p) {
                let labels = BTreeMap::new();
                let _ = a.counter("mlopt.search.feedback", score, labels);
            }
        }
        self.last_score = Some(score);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cost_model_noop() {
        let cm = NoOpCostModel;
        let g = nc_nir::Graph::new("g");
        assert_eq!(cm.predict_latency_ms(&g).unwrap(), 0.0);
        assert_eq!(cm.predict_energy_mj(&g).unwrap(), 0.0);
    }

    #[test]
    fn search_stub() {
        let mut s = GreedySearchStub::new();
        let g = nc_nir::Graph::new("g");
        let p = s.propose(&g).unwrap();
        assert_eq!(p, "identity");
        s.feedback(1.23);
    }
}
