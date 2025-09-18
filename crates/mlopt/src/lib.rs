use anyhow::Result;

pub fn version() -> &'static str { "0.0.1" }

pub trait CostModel {
    fn predict_latency_ms(&self, graph: &nc_nir::Graph) -> Result<f64>;
    fn predict_energy_mj(&self, graph: &nc_nir::Graph) -> Result<f64>;
}

pub struct NoOpCostModel;

impl CostModel for NoOpCostModel {
    fn predict_latency_ms(&self, _graph: &nc_nir::Graph) -> Result<f64> { Ok(0.0) }
    fn predict_energy_mj(&self, _graph: &nc_nir::Graph) -> Result<f64> { Ok(0.0) }
}

pub trait MappingSearch {
    fn propose(&mut self, graph: &nc_nir::Graph) -> Result<String>;
    fn feedback(&mut self, score: f64);
}

pub struct GreedySearchStub {
    last_score: Option<f64>,
}

impl GreedySearchStub {
    pub fn new() -> Self { Self { last_score: None } }
}

impl MappingSearch for GreedySearchStub {
    fn propose(&mut self, _graph: &nc_nir::Graph) -> Result<String> {
        Ok("identity".to_string())
    }
    fn feedback(&mut self, score: f64) {
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
