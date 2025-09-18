use anyhow::{bail, Result};
pub use nc_nir as nir;
use nc_hal as hal;
use std::fs;
use std::path::{Path, PathBuf};

pub trait Pass {
    fn name(&self) -> &str;
    fn run(&self, g: nir::Graph) -> Result<nir::Graph>;
}

pub struct NoOpPass;
impl Pass for NoOpPass {
    fn name(&self) -> &str { "no-op" }
    fn run(&self, g: nir::Graph) -> Result<nir::Graph> { Ok(g) }
}

pub struct ValidatePass;
impl Pass for ValidatePass {
    fn name(&self) -> &str { "validate" }
    fn run(&self, g: nir::Graph) -> Result<nir::Graph> {
        g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(g)
    }
}

pub struct QuantizeWeightsPass {
    pub bits: u32,
}

impl QuantizeWeightsPass {
    fn quantize(w: f32, bits: u32) -> f32 {
        // Uniform symmetric quantization onto [-1,1] with 2^bits levels
        let levels: u32 = if bits >= 31 { u32::MAX } else { 1u32 << bits };
        let l_minus_1 = (levels.saturating_sub(1)) as f32;
        let l_minus_1 = if l_minus_1 <= 0.0 { 1.0 } else { l_minus_1 };
        let w_clamped = w.clamp(-1.0, 1.0);
        let step = 2.0 / l_minus_1;
        ((w_clamped + 1.0) / step).round() * step - 1.0
    }
}

impl Pass for QuantizeWeightsPass {
    fn name(&self) -> &str { "quantize" }
    fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        for c in &mut g.connections {
            c.weight = Self::quantize(c.weight, self.bits);
        }
        Ok(g)
    }
}

fn extract_caps_from_graph(g: &nir::Graph) -> Option<hal::Capabilities> {
    if let Some(p) = g.attributes.get("hal_manifest_path").and_then(|v| v.as_str()) {
        if let Ok(m) = hal::parse_target_manifest_path(p) {
            return m.capabilities.clone();
        }
    }
    None
}

pub struct PartitionPass;
impl Pass for PartitionPass {
    fn name(&self) -> &str { "partition" }
    fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        let mut strategy = "naive";
        let mut parts: usize = 1;
        let mut assignment: Vec<(String, usize)> = Vec::new();
        let mut violations: Vec<serde_json::Value> = Vec::new();

        if let Some(caps) = extract_caps_from_graph(&g) {
            strategy = "cap-aware";
            let max_neurons = caps.max_neurons_per_core.unwrap_or(0) as usize;
            let max_syn = caps.max_synapses_per_core.unwrap_or(0) as usize;

            let total_neurons: usize = g.populations.iter().map(|p| p.size as usize).sum();
            let total_synapses: usize = g.connections.len();

            let parts_by_neurons = if max_neurons > 0 { (total_neurons + max_neurons - 1) / max_neurons } else { 1 };
            let parts_by_syn = if max_syn > 0 { (total_synapses + max_syn - 1) / max_syn } else { 1 };
            parts = parts_by_neurons.max(parts_by_syn).max(1);

            // Greedy size-balanced assignment
            let mut buckets: Vec<usize> = vec![0; parts];
            let mut pops: Vec<(String, usize)> = g.populations.iter().map(|p| (p.name.clone(), p.size as usize)).collect();
            pops.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
            for (name, sz) in pops {
                let mut idx = 0usize;
                let mut min_val = buckets[0];
                for i in 1..parts {
                    if buckets[i] < min_val {
                        min_val = buckets[i];
                        idx = i;
                    }
                }
                if max_neurons > 0 && sz > max_neurons {
                    violations.push(serde_json::json!({
                        "code": "POP_EXCEEDS_MAX_NEURONS_PER_CORE",
                        "population": name,
                        "size": sz,
                        "max": max_neurons
                    }));
                }
                buckets[idx] += sz;
                assignment.push((name, idx));
            }
        } else {
            // Naive: single part, trivial assignment
            parts = 1;
            assignment = g.populations.iter().map(|p| (p.name.clone(), 0usize)).collect();
        }

        let assignment_json: Vec<serde_json::Value> = assignment
            .iter()
            .map(|(pop, part)| serde_json::json!({ "population": pop, "part": part }))
            .collect();

        let meta = serde_json::json!({
            "parts": parts as u32,
            "strategy": strategy,
            "assignment": assignment_json,
            "violations": violations,
        });
        g.attributes.insert("partition".to_string(), meta);
        Ok(g)
    }
}

pub struct PlacementPass;
impl Pass for PlacementPass {
    fn name(&self) -> &str { "placement" }
    fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        // Derive partition assignment
        let parts = g.attributes.get("partition").and_then(|v| v.get("parts")).and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let mut pop_to_part: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        if let Some(assign) = g.attributes.get("partition").and_then(|v| v.get("assignment")).and_then(|v| v.as_array()) {
            for a in assign {
                if let (Some(pop), Some(part)) = (a.get("population").and_then(|x| x.as_str()), a.get("part").and_then(|x| x.as_u64())) {
                    pop_to_part.insert(pop.to_string(), part as usize);
                }
            }
        } else {
            for p in &g.populations {
                pop_to_part.insert(p.name.clone(), 0usize);
            }
        }

        // Count resources per part
        let mut neurons_per_part = vec![0usize; parts];
        for p in &g.populations {
            let part = *pop_to_part.get(&p.name).unwrap_or(&0usize);
            neurons_per_part[part] += p.size as usize;
        }
        let mut syn_per_part = vec![0usize; parts];
        for c in &g.connections {
            let pre_part = *pop_to_part.get(&c.pre).unwrap_or(&0usize);
            let post_part = *pop_to_part.get(&c.post).unwrap_or(&0usize);
            if pre_part == post_part {
                syn_per_part[pre_part] += 1;
            }
        }

        // Simple memory model (placeholder)
        let neuron_mem_kib = 1usize;
        let syn_mem_kib = 0usize;

        let caps = extract_caps_from_graph(&g);
        let core_mem_cap = caps.as_ref().and_then(|c| c.core_memory_kib).map(|v| v as usize);
        let max_fan_in = caps.as_ref().and_then(|c| c.max_fan_in).map(|v| v as usize);
        let max_fan_out = caps.as_ref().and_then(|c| c.max_fan_out).map(|v| v as usize);

        let mut violations: Vec<serde_json::Value> = Vec::new();
        for part in 0..parts {
            let mem = neurons_per_part[part] * neuron_mem_kib + syn_per_part[part] * syn_mem_kib;
            if let Some(cap) = core_mem_cap {
                if mem > cap {
                    violations.push(serde_json::json!({
                        "code": "CORE_MEMORY_EXCEEDED",
                        "part": part,
                        "estimate_kib": mem,
                        "cap_kib": cap
                    }));
                }
            }
        }

        // Fan-in/out checks per population
        let mut fan_in: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut fan_out: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for c in &g.connections {
            *fan_out.entry(c.pre.clone()).or_insert(0) += 1;
            *fan_in.entry(c.post.clone()).or_insert(0) += 1;
        }
        for p in &g.populations {
            if let Some(cap) = max_fan_in {
                if let Some(v) = fan_in.get(&p.name) {
                    if *v > cap {
                        violations.push(serde_json::json!({
                            "code": "MAX_FAN_IN_EXCEEDED",
                            "population": p.name,
                            "fan_in": v,
                            "cap": cap
                        }));
                    }
                }
            }
            if let Some(cap) = max_fan_out {
                if let Some(v) = fan_out.get(&p.name) {
                    if *v > cap {
                        violations.push(serde_json::json!({
                            "code": "MAX_FAN_OUT_EXCEEDED",
                            "population": p.name,
                            "fan_out": v,
                            "cap": cap
                        }));
                    }
                }
            }
        }

        let status = if violations.is_empty() { "ok" } else { "violations" };
        let meta = serde_json::json!({
            "status": status,
            "parts": parts,
            "neurons_per_part": neurons_per_part,
            "synapses_per_part": syn_per_part,
            "violations": violations
        });
        g.attributes.insert("placement".to_string(), meta);
        Ok(g)
    }
}

pub struct RoutingPass;
impl Pass for RoutingPass {
    fn name(&self) -> &str { "routing" }
    fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        let meta = serde_json::json!({ "status": "ok" });
        g.attributes.insert("routing".to_string(), meta);
        Ok(g)
    }
}

pub struct TimingPass;
impl Pass for TimingPass {
    fn name(&self) -> &str { "timing" }
    fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        // Extremely rough placeholder: 0.01ms per connection
        let est_latency_ms = (g.connections.len() as f64) * 0.01f64;
        let meta = serde_json::json!({ "est_latency_ms": est_latency_ms });
        g.attributes.insert("timing".to_string(), meta);
        Ok(g)
    }
}

pub struct ResourceCheckPass;
impl Pass for ResourceCheckPass {
    fn name(&self) -> &str { "resource-check" }
    fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        // Compute simple fan-in per post population and attach as attributes
        let mut fan_in: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for c in &g.connections {
            *fan_in.entry(c.post.clone()).or_insert(0) += 1;
        }
        let summary: Vec<serde_json::Value> = fan_in
            .iter()
            .map(|(k, v)| serde_json::json!({ "population": k, "fan_in": v }))
            .collect();
        let meta = serde_json::json!({ "fan_in": summary });
        g.attributes.insert("resource_check".to_string(), meta);
        Ok(g)
    }
}

pub enum DumpFormat {
    Json,
    Yaml,
    #[cfg(feature = "bin")]
    Bin,
}

pub struct PipelineConfig {
    pub passes: Vec<String>,
    pub dump_dir: Option<PathBuf>,
    pub dump_formats: Vec<DumpFormat>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            passes: vec!["noop".into()],
            dump_dir: None,
            dump_formats: vec![DumpFormat::Json],
        }
    }
}

pub struct PassManager {
    passes: Vec<Box<dyn Pass>>,
}

impl PassManager {
    pub fn new() -> Self { Self { passes: Vec::new() } }
    pub fn add_pass<P: Pass + 'static>(&mut self, p: P) { self.passes.push(Box::new(p)); }

    pub fn run(&self, mut g: nir::Graph) -> Result<nir::Graph> {
        for p in &self.passes {
            g = p.run(g)?;
        }
        Ok(g)
    }

    pub fn run_with_config(&self, mut g: nir::Graph, cfg: &PipelineConfig) -> Result<nir::Graph> {
        let mut idx = 0usize;
        for p in &self.passes {
            g = p.run(g)?;
            if let Some(dir) = &cfg.dump_dir {
                dump_graph(&g, dir, idx, p.name(), &cfg.dump_formats)?;
            }
            idx += 1;
        }
        Ok(g)
    }
}

fn dump_graph(g: &nir::Graph, dir: &Path, idx: usize, pass: &str, fmts: &[DumpFormat]) -> Result<()> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    let base = format!("{:02}_{}", idx, pass.replace('/', "_"));
    for f in fmts {
        match f {
            DumpFormat::Json => {
                let s = g.to_json_string().map_err(|e| anyhow::anyhow!(e))?;
                fs::write(dir.join(format!("{base}.json")), s)?;
            }
            DumpFormat::Yaml => {
                let s = g.to_yaml_string().map_err(|e| anyhow::anyhow!(e))?;
                fs::write(dir.join(format!("{base}.yaml")), s)?;
            }
            #[cfg(feature = "bin")]
            DumpFormat::Bin => {
                let b = g.to_bytes().map_err(|e| anyhow::anyhow!(e))?;
                fs::write(dir.join(format!("{base}.bin")), b)?;
            }
        }
    }
    Ok(())
}

/// Build a pipeline by pass names (string identifiers)
pub fn build_pipeline(pm: &mut PassManager, names: &[String]) -> Result<()> {
    for n in names {
        match n.as_str() {
            "noop" | "no-op" => pm.add_pass(NoOpPass),
            "validate" => pm.add_pass(ValidatePass),
            "quantize4" => pm.add_pass(QuantizeWeightsPass { bits: 4 }),
            "quantize8" => pm.add_pass(QuantizeWeightsPass { bits: 8 }),
            "quantize16" => pm.add_pass(QuantizeWeightsPass { bits: 16 }),
            "partition" => pm.add_pass(PartitionPass),
            "placement" => pm.add_pass(PlacementPass),
            "routing" => pm.add_pass(RoutingPass),
            "timing" => pm.add_pass(TimingPass),
            "resource-check" | "resource_check" => pm.add_pass(ResourceCheckPass),
            other => bail!("unknown pass '{other}'"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_noop_pipeline() {
        let g = nir::Graph::new("t");
        let mut pm = PassManager::new();
        pm.add_pass(NoOpPass);
        let out = pm.run(g).unwrap();
        assert_eq!(out.name, "t");
    }

    #[test]
    fn run_validate_pipeline() {
        let g = nir::Graph::new("t2");
        let mut pm = PassManager::new();
        pm.add_pass(ValidatePass);
        let out = pm.run(g).unwrap();
        assert_eq!(out.name, "t2");
    }

    #[test]
    fn run_quantize_pipeline() {
        let mut g = nir::Graph::new("tq");
        g.populations.push(nir::Population { name: "a".into(), size: 1, model: "LIF".into(), params: serde_json::json!({}) });
        g.populations.push(nir::Population { name: "b".into(), size: 1, model: "LIF".into(), params: serde_json::json!({}) });
        g.connections.push(nir::Connection { pre: "a".into(), post: "b".into(), weight: 0.1234, delay_ms: 0.0, plasticity: None });
        let mut pm = PassManager::new();
        pm.add_pass(ValidatePass);
        pm.add_pass(QuantizeWeightsPass { bits: 8 });
        let out = pm.run(g).unwrap();
        assert_eq!(out.name, "tq");
        assert!(out.connections[0].weight.is_finite());
        assert!(out.connections[0].weight >= -1.0 && out.connections[0].weight <= 1.0);
    }
}
