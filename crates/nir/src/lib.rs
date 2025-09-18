use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Population {
    pub name: String,
    pub size: u32,
    pub model: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub pre: String,
    pub post: String,
    #[serde(default)]
    pub weight: f32,
    #[serde(default)]
    pub delay_ms: f32,
    #[serde(default)]
    pub plasticity: Option<PlasticityRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    pub target: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Dialect {
    Event,
    Dataflow,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlasticityKind {
    STDP,
    Hebbian,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlasticityRule {
    pub kind: PlasticityKind,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ValidationError(pub String);

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NIR validation error: {}", self.0)
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub name: String,
    #[serde(default)]
    pub populations: Vec<Population>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub probes: Vec<Probe>,
    #[serde(default)]
    pub dialect: Option<Dialect>,
    #[serde(default)]
    pub attributes: IndexMap<String, serde_json::Value>,
}

impl Graph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            populations: Vec::new(),
            connections: Vec::new(),
            probes: Vec::new(),
            dialect: None,
            attributes: IndexMap::new(),
        }
    }

    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
    pub fn to_yaml_string(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }
    pub fn from_yaml_str(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }

    #[cfg(feature = "bin")]
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    #[cfg(feature = "bin")]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Validate structural integrity of the graph.
    /// Checks:
    /// - population names unique and non-empty; size > 0; model non-empty
    /// - connections' pre/post exist; weight/delay finite; delay_ms >= 0
    /// - probes target an existing population; kind non-empty
    pub fn validate(&self) -> Result<(), ValidationError> {
        let mut names: HashSet<String> = HashSet::new();
        for p in &self.populations {
            if p.name.trim().is_empty() {
                return Err(ValidationError("population name cannot be empty".into()));
            }
            if !names.insert(p.name.clone()) {
                return Err(ValidationError(format!("duplicate population '{}'", p.name)));
            }
            if p.size == 0 {
                return Err(ValidationError(format!("population '{}' has size 0", p.name)));
            }
            if p.model.trim().is_empty() {
                return Err(ValidationError(format!("population '{}' missing model", p.name)));
            }
        }
        for c in &self.connections {
            if !names.contains(&c.pre) {
                return Err(ValidationError(format!("connection pre '{}' not found", c.pre)));
            }
            if !names.contains(&c.post) {
                return Err(ValidationError(format!("connection post '{}' not found", c.post)));
            }
            if !c.weight.is_finite() {
                return Err(ValidationError(format!("connection {}->{} has non-finite weight", c.pre, c.post)));
            }
            if !c.delay_ms.is_finite() || c.delay_ms < 0.0 {
                return Err(ValidationError(format!(
                    "connection {}->{} has invalid delay_ms {}",
                    c.pre, c.post, c.delay_ms
                )));
            }
        }
        for pr in &self.probes {
            if pr.kind.trim().is_empty() {
                return Err(ValidationError("probe kind cannot be empty".into()));
            }
            if !names.contains(&pr.target) {
                return Err(ValidationError(format!(
                    "probe target '{}' not found among populations",
                    pr.target
                )));
            }
        }
        Ok(())
    }

    /// Ensure the 'nir_version' attribute is present with the current VERSION.
    pub fn ensure_version_tag(&mut self) {
        if !self.attributes.contains_key("nir_version") {
            self.attributes.insert("nir_version".to_string(), serde_json::json!(VERSION));
        }
    }
}

pub const VERSION: &str = "0.0.1";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_graph() {
        let g = Graph::new("test");
        assert_eq!(g.name, "test");
    }

    #[test]
    fn json_roundtrip() {
        let mut g = Graph::new("json");
        g.populations.push(Population {
            name: "pop".into(),
            size: 10,
            model: "LIF".into(),
            params: serde_json::json!({ "tau": 10.0 }),
        });
        let s = g.to_json_string().unwrap();
        let g2 = Graph::from_json_str(&s).unwrap();
        assert_eq!(g2.name, "json");
        assert_eq!(g2.populations.len(), 1);
    }

    #[test]
    fn yaml_roundtrip() {
        let mut g = Graph::new("yaml");
        g.connections.push(Connection {
            pre: "a".into(),
            post: "b".into(),
            weight: 0.5,
            delay_ms: 1.0,
            plasticity: None,
        });
        let s = g.to_yaml_string().unwrap();
        let g2 = Graph::from_yaml_str(&s).unwrap();
        assert_eq!(g2.name, "yaml");
        assert_eq!(g2.connections.len(), 1);
    }

    #[test]
    fn validate_ok() {
        let mut g = Graph::new("v");
        g.populations.push(Population {
            name: "a".into(),
            size: 1,
            model: "LIF".into(),
            params: serde_json::json!({}),
        });
        g.populations.push(Population {
            name: "b".into(),
            size: 2,
            model: "LIF".into(),
            params: serde_json::json!({}),
        });
        g.connections.push(Connection {
            pre: "a".into(),
            post: "b".into(),
            weight: 0.1,
            delay_ms: 0.0,
            plasticity: None,
        });
        g.validate().unwrap();
    }

    #[test]
    fn validate_bad_conn() {
        let mut g = Graph::new("bad");
        g.populations.push(Population {
            name: "only".into(),
            size: 1,
            model: "LIF".into(),
            params: serde_json::json!({}),
        });
        g.connections.push(Connection {
            pre: "missing".into(),
            post: "only".into(),
            weight: 1.0,
            delay_ms: 0.0,
            plasticity: None,
        });
        assert!(g.validate().is_err());
    }

    #[test]
    fn version_tag() {
        let mut g = Graph::new("ver");
        assert!(g.attributes.get("nir_version").is_none());
        g.ensure_version_tag();
        assert_eq!(g.attributes.get("nir_version").and_then(|v| v.as_str()), Some(VERSION));
    }

    #[cfg(feature = "bin")]
    #[test]
    fn bin_roundtrip() {
        let g = Graph::new("bin");
        let bytes = g.to_bytes().unwrap();
        let g2 = Graph::from_bytes(&bytes).unwrap();
        assert_eq!(g2.name, "bin");
    }
}
