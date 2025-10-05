use anyhow::Result;
use std::collections::HashMap;
#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;
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
    Ok(format!("compile: target={target}"))
}

/// Compile NIR from JSON string for a specific target (feature-gated backends).
pub fn compile_nir_json_str(target: &str, json: &str) -> Result<String> {
    let mut g = nc_nir::Graph::from_json_str(json)?;
    g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    g.ensure_version_tag();
    let manifest_path = std::path::PathBuf::from(format!("targets/{target}.toml"));
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
        // RISC-V targets (compile-only from Python by default).
        // Runtime execution via QEMU/Renode is controlled out-of-process with env:
        //   NC_RISCV_QEMU_RUN=1  -> attempt run if toolchains are present
        //   NC_RISCV_QEMU_RUN=0  -> compile-only (unit tests use this)
        "riscv64gcv_linux" | "riscv32imac_bare" | "riscv64gc_ctrl" => {
            #[cfg(feature = "backend-riscv")]
            { return nc_backend_riscv::compile(&g, &manifest); }
            #[cfg(not(feature = "backend-riscv"))]
            { anyhow::bail!("backend 'riscv' not enabled; build python crate with feature 'backend-riscv'"); }
        }
        other => anyhow::bail!("unsupported target '{other}'"),
    }
}

/// Compile NIR from YAML string for a specific target (feature-gated backends).
pub fn compile_nir_yaml_str(target: &str, yaml: &str) -> Result<String> {
    let mut g = nc_nir::Graph::from_yaml_str(yaml)?;
    g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    g.ensure_version_tag();
    let manifest_path = std::path::PathBuf::from(format!("targets/{target}.toml"));
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
        // RISC-V targets (compile-only from Python by default).
        // Runtime execution via QEMU/Renode is controlled out-of-process with env:
        //   NC_RISCV_QEMU_RUN=1  -> attempt run if toolchains are present
        //   NC_RISCV_QEMU_RUN=0  -> compile-only (unit tests use this)
        "riscv64gcv_linux" | "riscv32imac_bare" | "riscv64gc_ctrl" => {
            #[cfg(feature = "backend-riscv")]
            { return nc_backend_riscv::compile(&g, &manifest); }
            #[cfg(not(feature = "backend-riscv"))]
            { anyhow::bail!("backend 'riscv' not enabled; build python crate with feature 'backend-riscv'"); }
        }
        other => anyhow::bail!("unsupported target '{other}'"),
    }
}

/// Compile NIR from a string (auto-detect JSON vs YAML) for a specific target.
pub fn compile_nir_str(target: &str, s: &str) -> Result<String> {
    let t = s.trim_start();
    if t.starts_with('{') || t.starts_with('[') {
        compile_nir_json_str(target, s)
    } else {
        // try YAML first; if it fails, fallback to JSON
        compile_nir_yaml_str(target, s).or_else(|_| compile_nir_json_str(target, s))
    }
}


pub fn simulate_stub(sim: &str) -> Result<String> {
    Ok(format!("simulate: simulator={sim}"))
}

/// Simulate NIR from JSON string for a specified simulator, writing artifacts to out_dir (or default).
pub fn simulate_nir_json_str(simulator: &str, json: &str, out_dir: Option<&str>) -> Result<String> {
    let mut g = nc_nir::Graph::from_json_str(json)?;
    g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    g.ensure_version_tag();

    let out_path = match out_dir {
        Some(p) => std::path::PathBuf::from(p),
        None => std::path::PathBuf::from(format!("target/sim-{simulator}-py-out")),
    };
    // Keep the variable marked as used even when all simulator features are disabled
    let _ = &out_path;

    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| nc_telemetry::profiling::Appender::open(p).ok());

    #[cfg(feature = "telemetry")]
    let mut labels = BTreeMap::new();
    #[cfg(feature = "telemetry")]
    {
        labels.insert("simulator".to_string(), simulator.to_string());
        labels.insert("graph".to_string(), g.name.clone());
    }

    #[cfg(feature = "telemetry")]
    let __timer_emit = app.as_ref().map(|a| a.start_timer("py.simulate.emit_ms", labels.clone()));

    let emit_result: anyhow::Result<()> = match simulator {
        "neuron" => {
            #[cfg(feature = "sim-neuron")]
            {
                nc_sim_neuron::emit_artifacts(&g, &out_path)?;
                Ok(())
            }
            #[cfg(not(feature = "sim-neuron"))]
            {
                Err(anyhow::anyhow!("simulator 'neuron' not enabled; build python crate with feature 'sim-neuron'"))
            }
        }
        "coreneuron" => {
            #[cfg(feature = "sim-coreneuron")]
            {
                nc_sim_coreneuron::emit_artifacts(&g, &out_path)?;
                Ok(())
            }
            #[cfg(not(feature = "sim-coreneuron"))]
            {
                Err(anyhow::anyhow!("simulator 'coreneuron' not enabled; build python crate with feature 'sim-coreneuron'"))
            }
        }
        "arbor" => {
            #[cfg(feature = "sim-arbor")]
            {
                nc_sim_arbor::emit_artifacts(&g, &out_path)?;
                Ok(())
            }
            #[cfg(not(feature = "sim-arbor"))]
            {
                Err(anyhow::anyhow!("simulator 'arbor' not enabled; build python crate with feature 'sim-arbor'"))
            }
        }
        other => {
            anyhow::bail!("unsupported simulator '{other}'");
        }
    };

    emit_result?;

    #[cfg(feature = "telemetry")]
    {
        if let Some(a) = &app {
            let _ = a.counter("graph.populations", g.populations.len() as f64, labels.clone());
            let _ = a.counter("graph.connections", g.connections.len() as f64, labels.clone());
            let _ = a.counter("graph.probes", g.probes.len() as f64, labels.clone());
        }
    }

    Ok(out_path.to_string_lossy().to_string())
}

/// Simulate NIR from YAML string for a specified simulator, writing artifacts to out_dir (or default).
pub fn simulate_nir_yaml_str(simulator: &str, yaml: &str, out_dir: Option<&str>) -> Result<String> {
    let mut g = nc_nir::Graph::from_yaml_str(yaml)?;
    g.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    g.ensure_version_tag();

    let out_path = match out_dir {
        Some(p) => std::path::PathBuf::from(p),
        None => std::path::PathBuf::from(format!("target/sim-{simulator}-py-out")),
    };
    // Keep the variable marked as used even when all simulator features are disabled
    let _ = &out_path;

    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| nc_telemetry::profiling::Appender::open(p).ok());

    #[cfg(feature = "telemetry")]
    let mut labels = BTreeMap::new();
    #[cfg(feature = "telemetry")]
    {
        labels.insert("simulator".to_string(), simulator.to_string());
        labels.insert("graph".to_string(), g.name.clone());
    }

    #[cfg(feature = "telemetry")]
    let __timer_emit = app.as_ref().map(|a| a.start_timer("py.simulate.emit_ms", labels.clone()));

    let emit_result: anyhow::Result<()> = match simulator {
        "neuron" => {
            #[cfg(feature = "sim-neuron")]
            {
                nc_sim_neuron::emit_artifacts(&g, &out_path)?;
                Ok(())
            }
            #[cfg(not(feature = "sim-neuron"))]
            {
                Err(anyhow::anyhow!("simulator 'neuron' not enabled; build python crate with feature 'sim-neuron'"))
            }
        }
        "coreneuron" => {
            #[cfg(feature = "sim-coreneuron")]
            {
                nc_sim_coreneuron::emit_artifacts(&g, &out_path)?;
                Ok(())
            }
            #[cfg(not(feature = "sim-coreneuron"))]
            {
                Err(anyhow::anyhow!("simulator 'coreneuron' not enabled; build python crate with feature 'sim-coreneuron'"))
            }
        }
        "arbor" => {
            #[cfg(feature = "sim-arbor")]
            {
                nc_sim_arbor::emit_artifacts(&g, &out_path)?;
                Ok(())
            }
            #[cfg(not(feature = "sim-arbor"))]
            {
                Err(anyhow::anyhow!("simulator 'arbor' not enabled; build python crate with feature 'sim-arbor'"))
            }
        }
        other => {
            anyhow::bail!("unsupported simulator '{other}'");
        }
    };

    emit_result?;

    #[cfg(feature = "telemetry")]
    {
        if let Some(a) = &app {
            let _ = a.counter("graph.populations", g.populations.len() as f64, labels.clone());
            let _ = a.counter("graph.connections", g.connections.len() as f64, labels.clone());
            let _ = a.counter("graph.probes", g.probes.len() as f64, labels.clone());
        }
    }

    Ok(out_path.to_string_lossy().to_string())
}

/// Simulate NIR from a string (auto-detect JSON vs YAML) for a specified simulator.
pub fn simulate_nir_str(simulator: &str, s: &str, out_dir: Option<&str>) -> Result<String> {
    let t = s.trim_start();
    if t.starts_with('{') || t.starts_with('[') {
        simulate_nir_json_str(simulator, s, out_dir)
    } else {
        // try YAML first; if it fails, fallback to JSON
        simulate_nir_yaml_str(simulator, s, out_dir)
            .or_else(|_| simulate_nir_json_str(simulator, s, out_dir))
    }
}

/// Summarize a JSONL profiling file into CSV metrics: metric,count,avg,min,max
pub fn profile_summary_jsonl(path: &str) -> Result<String> {
    let file = File::open(path)?;
    let rdr = BufReader::new(file);
    let mut stats: HashMap<String, (usize, f64, f64, f64)> = HashMap::new(); // count,sum,min,max
    for l in rdr.lines().map_while(Result::ok) {
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
    let mut out = String::from("metric,count,avg,min,max\n");
    for (m, (c, sum, min, max)) in stats {
        let avg = if c > 0 { sum / c as f64 } else { 0.0 };
        out.push_str(&format!("{m},{c},{avg:.4},{min:.4},{max:.4}\n"));
    }
    Ok(out)
}

/// Deploy stub (placeholder for runtime-backed deployment)
pub fn deploy_stub(target: &str) -> Result<String> {
    Ok(format!("deploy: target={target}"))
}

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
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
    #[pyfn(m)]
    fn simulate_nir_json_py(simulator: &str, json: &str, out_dir: Option<&str>) -> PyResult<String> {
        simulate_nir_json_str(simulator, json, out_dir)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn simulate_nir_yaml_py(simulator: &str, yaml: &str, out_dir: Option<&str>) -> PyResult<String> {
        simulate_nir_yaml_str(simulator, yaml, out_dir)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn compile_nir_str_py(target: &str, s: &str) -> PyResult<String> {
        compile_nir_str(target, s).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    #[pyfn(m)]
    fn simulate_nir_str_py(simulator: &str, s: &str, out_dir: Option<&str>) -> PyResult<String> {
        simulate_nir_str(simulator, s, out_dir).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Feature-gated Python API test: compile-only for RISC-V (no external tools)
    #[cfg(all(feature = "backend-riscv", feature = "python"))]
    #[test]
    fn py_compile_riscv64gcv_linux_compile_only() {
        std::env::set_var("NC_RISCV_QEMU_RUN", "0"); // ensure no QEMU/Renode invocation
        let nir = std::fs::read_to_string("examples/nir/simple.json").expect("read NIR");
        pyo3::prepare_freethreaded_python();
        pyo3::Python::with_gil(|py| {
            let m = pyo3::types::PyModule::new(py, "neuro_compiler").expect("module new");
            // Initialize the in-process module with all #[pyfn] exports
            crate::neuro_compiler(py, m).expect("init module");
            let f = m.getattr("compile_nir_str_py").expect("get compile_nir_str_py");
            let art: String = f.call1(("riscv64gcv_linux", nir.as_str()))
                .expect("call ok")
                .extract()
                .expect("extract str");
            if art.starts_with("artifact:") {
                let dir = PathBuf::from(art.trim_start_matches("artifact:"));
                assert!(dir.exists(), "artifact dir should exist: {}", dir.display());
            } else {
                assert!(PathBuf::from(&art).exists(), "artifact path should exist: {}", art);
            }
        });
    }

    // Negative test when RISC-V backend feature is NOT enabled
    #[cfg(not(feature = "backend-riscv"))]
    #[test]
    fn riscv_backend_disabled_has_clear_error() {
        let nir = std::fs::read_to_string("examples/nir/simple.json").expect("read NIR");
        let err = compile_nir_str("riscv64gcv_linux", &nir).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("backend 'riscv' not enabled"), "error: {s}");
        assert!(s.contains("backend-riscv"), "error: {s}");
    }
}
