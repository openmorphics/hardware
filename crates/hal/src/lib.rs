use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::bail;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("invalid manifest field: {field} ({msg})")]
    InvalidField { field: &'static str, msg: &'static str },
}

#[derive(Debug, Clone)]
pub struct TargetDescriptor {
    pub name: &'static str,
    pub vendor: &'static str,
}

/// Names of built-in targets with manifest stubs under the `targets/` directory.
pub fn builtin_targets() -> &'static [&'static str] {
    &[
        "loihi2",
        "truenorth",
        "akida",
        "spinnaker2",
        "neurogrid",
        "dynaps",
        "memxbar",
        "custom_asic",
    ]
}

#[derive(Debug, Clone, Deserialize)]
pub struct Capabilities {
    pub on_chip_learning: Option<bool>,
    pub weight_precisions: Option<Vec<u32>>,
    pub max_neurons_per_core: Option<u32>,
    pub max_synapses_per_core: Option<u32>,
    pub time_resolution_ns: Option<u64>,

    // Expanded descriptor fields (optional; extend manifests incrementally)
    pub supports_sparse: Option<bool>,
    pub neuron_models: Option<Vec<String>>,
    pub max_fan_in: Option<u32>,
    pub max_fan_out: Option<u32>,
    pub core_memory_kib: Option<u32>,
    pub interconnect_bandwidth_mbps: Option<u32>,
    pub analog: Option<bool>,
    pub on_chip_plasticity_rules: Option<Vec<String>>,

    // Resource and traffic modeling (optional; used by mapping passes)
    /// Approximate KiB required per neuron on this target
    pub neuron_mem_kib_per: Option<f64>,
    /// Approximate KiB required per synapse on this target
    pub syn_mem_kib_per: Option<f64>,
    /// Bytes per spike/event transferred across interconnect
    pub bytes_per_event: Option<u32>,
    /// Default spike rate per connection (Hz) used for coarse bandwidth estimates
    pub default_spike_rate_hz: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetManifest {
    pub name: String,
    pub vendor: String,
    pub family: String,
    pub version: String,
    pub notes: Option<String>,
    pub capabilities: Option<Capabilities>,
}

pub fn parse_target_manifest_str(s: &str) -> Result<TargetManifest, anyhow::Error> {
    let m: TargetManifest = toml::from_str(s)?;
    Ok(m)
}

pub fn parse_target_manifest_path<P: AsRef<Path>>(path: P) -> Result<TargetManifest, anyhow::Error> {
    let data = fs::read_to_string(path)?;
    parse_target_manifest_str(&data)
}

pub fn load_manifests_from_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<(PathBuf, TargetManifest)>, anyhow::Error> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("toml") {
            let m = parse_target_manifest_path(&p)?;
            out.push((p, m));
        }
    }
    Ok(out)
}

/// Validate a target manifest for basic consistency.
pub fn validate_manifest(m: &TargetManifest) -> anyhow::Result<()> {
    if m.name.trim().is_empty() {
        bail!("manifest.name must be non-empty");
    }
    if m.vendor.trim().is_empty() {
        bail!("manifest.vendor must be non-empty");
    }
    if let Some(c) = &m.capabilities {
        if let Some(wp) = &c.weight_precisions {
            if wp.contains(&0) {
                bail!("capabilities.weight_precisions entries must be > 0");
            }
        }
        if let Some(v) = c.max_neurons_per_core {
            if v == 0 {
                bail!("capabilities.max_neurons_per_core must be > 0");
            }
        }
        if let Some(v) = c.max_synapses_per_core {
            if v == 0 {
                bail!("capabilities.max_synapses_per_core must be > 0");
            }
        }
        if let Some(v) = c.time_resolution_ns {
            if v == 0 {
                bail!("capabilities.time_resolution_ns must be > 0");
            }
        }
        if let Some(v) = c.max_fan_in {
            if v == 0 {
                bail!("capabilities.max_fan_in must be > 0");
            }
        }
        if let Some(v) = c.max_fan_out {
            if v == 0 {
                bail!("capabilities.max_fan_out must be > 0");
            }
        }
        if let Some(v) = c.core_memory_kib {
            if v == 0 {
                bail!("capabilities.core_memory_kib must be > 0");
            }
        }
        if let Some(v) = c.interconnect_bandwidth_mbps {
            if v == 0 {
                bail!("capabilities.interconnect_bandwidth_mbps must be > 0");
            }
        }
        if let Some(v) = c.neuron_mem_kib_per {
            if v <= 0.0 {
                bail!("capabilities.neuron_mem_kib_per must be > 0");
            }
        }
        if let Some(v) = c.syn_mem_kib_per {
            if v <= 0.0 {
                bail!("capabilities.syn_mem_kib_per must be > 0");
            }
        }
        if let Some(v) = c.bytes_per_event {
            if v == 0 {
                bail!("capabilities.bytes_per_event must be > 0");
            }
        }
        if let Some(v) = c.default_spike_rate_hz {
            if v <= 0.0 {
                bail!("capabilities.default_spike_rate_hz must be > 0");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_str() {
        let s = r#"
            name = "loihi2"
            vendor = "Intel"
            family = "Loihi"
            version = "2"
            notes = "Stub"
        "#;
        let m = parse_target_manifest_str(s).unwrap();
        assert_eq!(m.name, "loihi2");
        assert_eq!(m.vendor, "Intel");
    }
}


#[cfg(test)]
mod tests_builtin {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn builtin_targets_nonempty() {
        assert!(!builtin_targets().is_empty());
    }

    #[test]
    fn validate_all_targets_dir() {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("workspace root");
        let dir = ws_root.join("targets");
        let manis = super::load_manifests_from_dir(&dir).expect("load manifests");
        assert!(!manis.is_empty(), "no target manifests found");
        for (_p, m) in manis {
            super::validate_manifest(&m).expect("manifest valid");
        }
    }
}

#[cfg(test)]
mod tests_validate {
    use super::*;
    #[test]
    fn validate_manifest_ok() {
        let s = r#"
            name = "x"
            vendor = "v"
            family = "F"
            version = "1"
            [capabilities]
            on_chip_learning = true
            weight_precisions = [8, 16]
            max_neurons_per_core = 1024
            max_synapses_per_core = 65536
            time_resolution_ns = 1000
        "#;
        let m = parse_target_manifest_str(s).unwrap();
        validate_manifest(&m).unwrap();
    }

    #[test]
    fn validate_manifest_bad_weight_precision() {
        let s = r#"
            name = "x"
            vendor = "v"
            family = "F"
            version = "1"
            [capabilities]
            weight_precisions = [0]
        "#;
        let m = parse_target_manifest_str(s).unwrap();
        assert!(validate_manifest(&m).is_err());
    }
}
