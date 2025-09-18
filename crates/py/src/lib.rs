use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

// Rust API always available
pub fn version() -> &'static str { "0.0.1" }
pub fn list_targets() -> Vec<&'static str> { nc_hal::builtin_targets().to_vec() }

pub fn import_nir_json_str(s: &str) -> Result<nc_nir::Graph> {
    let g = nc_nir::Graph::from_json_str(s)?;
    Ok(g)
}

pub fn import_nir_yaml_str(s: &str) -> Result<nc_nir::Graph> {
    let g = nc_nir::Graph::from_yaml_str(s)?;
    Ok(g)
}

pub fn compile_stub(target: &str) -> Result<String> {
    // Placeholder compile path
    Ok(format!("compile: target={}", target))
}

/// Compile NIR from JSON string for a specific target (feature-gated backends).
pub fn compile_nir_json_str(target: &str, json: &str) -> Result<String> {
    let mut g = nc_nir::Graph::from_json_str(json)?;
    g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    g.ensure_version_tag();
    let manifest_path = std::path::PathBuf::from(format!("targets/{}.toml", target));
    let manifest = nc_hal::parse_target_manifest_path(&manifest_path)?;
    nc_hal::validate_manifest(&manifest)?;

    match target {
        "truenorth" => {
            #[cfg(feature = "backend-truenorth")]
            { return nc_backend_truenorth::compile(&g, &manifest); }
            #[cfg(not(feature = "backend-truenorth"))]
            { anyhow::bail!("backend 'truenorth' not enabled; build python crate with feature 'backend-truenorth'"); }
        }
        "dynaps" => {
            #[cfg(feature = "backend-dynaps")]
            { return nc_backend_dynaps::compile(&g, &manifest); }
            #[cfg(not(feature = "backend-dynaps"))]
            { anyhow::bail!("backend 'dynaps' not enabled; build python crate with feature 'backend-dynaps'"); }
        }
        other => anyhow::bail!("unsupported target '{other}'"),
    }
}

/// Compile NIR from YAML string for a specific target (feature-gated backends).
pub fn compile_nir_yaml_str(target: &str, yaml: &str) -> Result<String> {
    let mut g = nc_nir::Graph::from_yaml_str(yaml)?;
    g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    g.ensure_version_tag();
    let manifest_path = std::path::PathBuf::from(format!("targets/{}.toml", target));
    let manifest = nc_hal::parse_target_manifest_path(&manifest_path)?;
    nc_hal::validate_manifest(&manifest)?;

    match target {
        "truenorth" => {
            #[cfg(feature = "backend-truenorth")]
            { return nc_backend_truenorth::compile(&g, &manifest); }
            #[cfg(not(feature = "backend-truenorth"))]
            { anyhow::bail!("backend 'truenorth' not enabled; build python crate with feature 'backend-truenorth'"); }
        }
        "dynaps" => {
            #[cfg(feature = "backend-dynaps")]
            { return nc_backend_dynaps::compile(&g, &manifest); }
            #[cfg(not(feature = "backend-dynaps"))]
            { anyhow::bail!("backend 'dynaps' not enabled; build python crate with feature 'backend-dynaps'"); }
        }
        other => anyhow::bail!("unsupported target '{other}'"),
    }
}


pub fn simulate_stub(sim: &str) -> Result<String> {
    Ok(format!("simulate: simulator={}", sim))
}

/// Summarize a JSONL profiling file into CSV metrics: metric,count,avg,min,max
pub fn profile_summary_jsonl(path: &str) -> Result<String> {
    let file = File::open(path)?;
    let rdr = BufReader::new(file);
    let mut stats: HashMap<String, (usize, f64, f64, f64)> = HashMap::new(); // count,sum,min,max
    for line in rdr.lines() {
        if let Ok(l) = line {
            if l.trim().is_empty() { continue; }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) {
                let metric = v.get("metric").and_then(|m| m.as_str()).unwrap_or("unknown");
                let value = v.get("value").and_then(|x| x.as_f64()).unwrap_or(0.0);
                let e = stats.entry(metric.to_string())
                    .or_insert((0, 0.0, f64::INFINITY, f64::NEG_INFINITY));
                e.0 += 1;
                e.1 += value;
                if value < e.2 { e.2 = value; }
                if value > e.3 { e.3 = value; }
            }
        }
    }
    let mut out = String::from("metric,count,avg,min,max\n");
    for (m, (c, sum, min, max)) in stats {
        let avg = if c > 0 { sum / c as f64 } else { 0.0 };
        out.push_str(&format!("{},{},{:.4},{:.4},{:.4}\n", m, c, avg, min, max));
    }
    Ok(out)
}

/// Deploy stub (placeholder for runtime-backed deployment)
pub fn deploy_stub(target: &str) -> Result<String> {
    Ok(format!("deploy: target={}", target))
}

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(all(feature = "python"))]
#[pymodule]
fn neuro_compiler(_py: Python, m: &PyModule) -> PyResult<()> {
    #[pyfn(m)]
    fn version_py() -> &'static str { version() }
    #[pyfn(m)]
    fn list_targets_py() -> Vec<&'static str> { list_targets() }
    #[pyfn(m)]
    fn import_json_py(s: &str) -> PyResult<String> {
        let g = import_nir_json_str(s).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(g.name)
    }
    #[pyfn(m)]
    fn import_yaml_py(s: &str) -> PyResult<String> {
        let g = import_nir_yaml_str(s).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(g.name)
    }
    #[pyfn(m)]
    fn compile_py(target: &str) -> PyResult<String> {
        compile_stub(target).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn simulate_py(simulator: &str) -> PyResult<String> {
        simulate_stub(simulator).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn profile_summary_py(path: &str) -> PyResult<String> {
        profile_summary_jsonl(path).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn deploy_py(target: &str) -> PyResult<String> {
        deploy_stub(target).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn compile_nir_json_py(target: &str, json: &str) -> PyResult<String> {
        compile_nir_json_str(target, json).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn compile_nir_yaml_py(target: &str, yaml: &str) -> PyResult<String> {
        compile_nir_yaml_str(target, yaml).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    Ok(())
}
