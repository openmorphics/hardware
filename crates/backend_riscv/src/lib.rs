use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use nc_passes::{Pass, PassManager, PipelineConfig, DumpFormat};
use serde_json::json;

/// Lightweight RISC-V-specific no-op passes to form a pipeline stage names.
struct RvLowerPass;
impl Pass for RvLowerPass {
    fn name(&self) -> &str { "rv-lower" }
    fn run(&self, g: nc_nir::Graph) -> Result<nc_nir::Graph> { Ok(g) }
}
struct RvLayoutPass;
impl Pass for RvLayoutPass {
    fn name(&self) -> &str { "rv-layout" }
    fn run(&self, mut g: nc_nir::Graph) -> Result<nc_nir::Graph> {
        let caps = g.attributes.get("caps_riscv");
        let align = caps.and_then(|v| v.get("align_bytes")).and_then(|x| x.as_u64()).unwrap_or(16);
        let vec_ok = caps.and_then(|v| v.get("vector_available")).and_then(|x| x.as_bool()).unwrap_or(false);
        let qbits = caps.and_then(|v| v.get("quant_bits_default")).and_then(|x| x.as_u64()).unwrap_or(8);
        let meta = json!({
            "align_bytes": align,
            "vector_available": vec_ok,
            "quant_bits_default": qbits
        });
        g.attributes.insert("rv_layout".to_string(), meta);
        Ok(g)
    }
}
struct RvSchedulePass;
impl Pass for RvSchedulePass {
    fn name(&self) -> &str { "rv-schedule" }
    fn run(&self, mut g: nc_nir::Graph) -> Result<nc_nir::Graph> {
        let vec_ok = g.attributes
            .get("rv_layout")
            .and_then(|v| v.get("vector_available"))
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let fused = if vec_ok { vec!["op_fuse_vadd_vmul"] } else { vec!["op_fuse_scalar"] };
        let meta = json!({
            "threads": 1,
            "fused_stages": fused
        });
        g.attributes.insert("rv_schedule".to_string(), meta);
        Ok(g)
    }
}

struct RvVectorizePass;
impl Pass for RvVectorizePass {
    fn name(&self) -> &str { "rv-vectorize" }
    fn run(&self, mut g: nc_nir::Graph) -> Result<nc_nir::Graph> {
        let vec_ok = g.attributes
            .get("rv_layout")
            .and_then(|v| v.get("vector_available"))
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let meta = json!({ "enabled": vec_ok });
        g.attributes.insert("rv_vectorize".to_string(), meta);
        Ok(g)
    }
}

struct RvBaremetalTuningPass;
impl Pass for RvBaremetalTuningPass {
    fn name(&self) -> &str { "rv-baremetal-tuning" }
    fn run(&self, mut g: nc_nir::Graph) -> Result<nc_nir::Graph> {
        let caps = g.attributes.get("caps_riscv");
        let has_c = caps.and_then(|v| v.get("has_c")).and_then(|x| x.as_bool()).unwrap_or(false);
        let meta = json!({
            "size_optimized": true,
            "use_compressed": has_c
        });
        g.attributes.insert("rv_bare_tuning".to_string(), meta);
        Ok(g)
    }
}

struct RvControlPlanePass;
impl Pass for RvControlPlanePass {
    fn name(&self) -> &str { "rv-control-plane-driver" }
    fn run(&self, mut g: nc_nir::Graph) -> Result<nc_nir::Graph> {
        let caps = g.attributes.get("caps_riscv");
        let mmio = caps.and_then(|v| v.get("mmio_supported")).and_then(|x| x.as_bool()).unwrap_or(false);
        let mmio_w = caps.and_then(|v| v.get("mmio_width_bits")).and_then(|x| x.as_u64());
        let dma = caps.and_then(|v| v.get("dma_supported")).and_then(|x| x.as_bool()).unwrap_or(false);
        let dma_alignment = caps.and_then(|v| v.get("dma_alignment")).and_then(|x| x.as_u64());
        let meta = json!({
            "mmio_supported": mmio,
            "mmio_width_bits": mmio_w,
            "dma_supported": dma,
            "dma_alignment": dma_alignment
        });
        g.attributes.insert("rv_ctrl_plane".to_string(), meta);
        Ok(g)
    }
}

/// Compile NIR to a RISC-V artifact. In M1 this emits a scalar Linux user-mode C program
/// for RV64GCV and best-effort builds it; if NC_RISCV_QEMU_RUN=1 and qemu/cc toolchains
/// are present, it will run under qemu-user and capture JSONL to NC_PROFILE_JSONL or
/// to {out_dir}/profile.jsonl.
pub fn compile(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<String> {
    // Validate input IR and target manifest
    graph.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    nc_hal::validate_manifest(manifest)?;

    // Optional telemetry profiling
    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| nc_telemetry::profiling::Appender::open(p).ok());

    #[cfg(feature = "telemetry")]
    let _timer = {
        if let Some(a) = app.as_ref() {
            let labels = nc_telemetry::labels::backend(&graph.name, "riscv", Some(&manifest.name));
            Some(a.start_timer("backend.compile_ms", labels))
        } else {
            None
        }
    };

    let out_dir = default_out_dir(&graph.name, &manifest.name);
    if !out_dir.exists() {
        let _ = fs::create_dir_all(&out_dir);
    }
    let mut warnings: Vec<String> = Vec::new();

    // Run RISC-V pass pipeline by profile and dump JSON after each stage; collect metadata for README
    let mut meta_lines: Vec<String> = Vec::new();
    if let Ok(s) = graph.to_json_string() {
        match nc_nir::Graph::from_json_str(&s) {
            Ok(mut g_owned) => {
                // Attach RISC-V caps summary for passes to consume
                let caps = manifest.capabilities.as_ref();
                let vector_available = caps.and_then(|c| c.has_vector).unwrap_or(false);
                let align_bytes = caps.and_then(|c| c.cacheline_bytes).unwrap_or(16);
                let quant_bits_default = caps
                    .and_then(|c| c.weight_precisions.as_ref()
                    .and_then(|v| v.iter().min().copied()))
                    .unwrap_or(8);
                let has_c = caps.and_then(|c| c.has_c).unwrap_or(false);
                let mmio_supported = caps.and_then(|c| c.mmio_supported).unwrap_or(false);
                let mmio_width_bits = caps.and_then(|c| c.mmio_width_bits);
                let dma_supported = caps.and_then(|c| c.dma_supported).unwrap_or(false);
                let dma_alignment = caps.and_then(|c| c.dma_alignment);

                let caps_json = serde_json::json!({
                    "vector_available": vector_available,
                    "align_bytes": align_bytes,
                    "quant_bits_default": quant_bits_default,
                    "has_c": has_c,
                    "mmio_supported": mmio_supported,
                    "mmio_width_bits": mmio_width_bits,
                    "dma_supported": dma_supported,
                    "dma_alignment": dma_alignment
                });
                g_owned.attributes.insert("caps_riscv".to_string(), caps_json);

                let profile = manifest.capabilities
                    .as_ref()
                    .and_then(|c| c.profile.as_deref())
                    .unwrap_or_else(|| {
                        if manifest.name.contains("linux") { "linux_user" }
                        else if manifest.name.contains("ctrl") { "control_plane" }
                        else { "bare_metal" }
                    });

                let mut pm = PassManager::new();
                pm.add_pass(nc_passes::ValidatePass);
                pm.add_pass(RvLowerPass);
                pm.add_pass(RvLayoutPass);
                match profile {
                    "linux_user" => {
                        pm.add_pass(RvSchedulePass);
                        pm.add_pass(RvVectorizePass);
                    }
                    "bare_metal" => {
                        pm.add_pass(RvBaremetalTuningPass);
                    }
                    "control_plane" => {
                        pm.add_pass(RvControlPlanePass);
                    }
                    _ => {
                        pm.add_pass(RvSchedulePass);
                    }
                }
                let cfg = PipelineConfig {
                    dump_dir: Some(out_dir.join("passes")),
                    dump_formats: vec![DumpFormat::Json],
                    ..Default::default()
                };
                match pm.run_with_config(g_owned, &cfg) {
                    Ok(g_after) => {
                        if let Some(v) = g_after.attributes.get("rv_layout") {
                            if let Some(al) = v.get("align_bytes").and_then(|x| x.as_u64()) {
                                meta_lines.push(format!("align_bytes={al}"));
                            }
                            if let Some(q) = v.get("quant_bits_default").and_then(|x| x.as_u64()) {
                                meta_lines.push(format!("quant_bits_default={q}"));
                            }
                            if let Some(vec_ok) = v.get("vector_available").and_then(|x| x.as_bool()) {
                                meta_lines.push(format!("vector_available={vec_ok}"));
                            }
                        }
                        if let Some(v) = g_after.attributes.get("rv_schedule") {
                            if let Some(fs) = v.get("fused_stages").and_then(|x| x.as_array()) {
                                let names: Vec<String> = fs.iter().filter_map(|e| e.as_str().map(|s| s.to_string())).collect();
                                if !names.is_empty() {
                                    meta_lines.push(format!("fused_stages={}", names.join("+")));
                                }
                            }
                            if let Some(t) = v.get("threads").and_then(|x| x.as_u64()) {
                                meta_lines.push(format!("threads={t}"));
                            }
                        }
                        if let Some(v) = g_after.attributes.get("rv_bare_tuning") {
                            if let Some(sz) = v.get("size_optimized").and_then(|x| x.as_bool()) {
                                meta_lines.push(format!("size_optimized={sz}"));
                            }
                            if let Some(uc) = v.get("use_compressed").and_then(|x| x.as_bool()) {
                                meta_lines.push(format!("use_compressed={uc}"));
                            }
                        }
                        if let Some(v) = g_after.attributes.get("rv_ctrl_plane") {
                            if let Some(mmio) = v.get("mmio_supported").and_then(|x| x.as_bool()) {
                                meta_lines.push(format!("mmio_supported={mmio}"));
                            }
                            if let Some(d) = v.get("dma_supported").and_then(|x| x.as_bool()) {
                                meta_lines.push(format!("dma_supported={d}"));
                            }
                        }
                    }
                    Err(e) => {
                        warnings.push(format!("pass pipeline failed: {e}"));
                    }
                }
            }
            Err(e) => warnings.push(format!("graph roundtrip (json) failed: {e}")),
        }
    } else {
        warnings.push("graph serialization to JSON failed".into());
    }

    // Emit scalar runtime only for Linux user profile targets; keep others as stubs.
    let is_linux_user = manifest
        .capabilities
        .as_ref()
        .and_then(|c| c.profile.as_deref())
        .map(|p| p == "linux_user")
        .unwrap_or_else(|| manifest.name.contains("linux"));

    if is_linux_user {
        if let Err(e) = emit_scalar_linux_rv64(&out_dir, graph, manifest) {
            warnings.push(format!("emit failed: {e}"));
        }

        // Best-effort build and optional run; write WARN.txt instead of failing compile.
        match build_rv64_linux_binary(&out_dir) {
            Ok(exe) => {
                if std::env::var("NC_RISCV_QEMU_RUN").ok().as_deref() == Some("1") {
                    if let Err(e) = run_qemu_and_capture(&exe, &out_dir) {
                        warnings.push(format!("qemu run failed: {e}"));
                    }
                }
            }
            Err(e) => {
                warnings.push(format!("{e}"));
            }
        }
    } else {
        // Stub emission for bare-metal/control-plane profiles: create README only.
        let mut readme = fs::File::create(out_dir.join("README.txt"))
            .context("create README.txt for non-linux profile")?;
        writeln!(
            readme,
            "RISC-V backend stub: profile '{:?}' currently emits no code; pipeline ran.",
            manifest.capabilities.as_ref().and_then(|c| c.profile.as_deref())
        )?;
    }

    // Append pass metadata to README
    if !meta_lines.is_empty() {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_dir.join("README.txt"))
            .context("append README metadata")?;
        writeln!(f, "Pass metadata:")?;
        for l in &meta_lines {
            writeln!(f, "- {l}")?;
        }
    }

    // Append pass metadata to README
    if !meta_lines.is_empty() {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_dir.join("README.txt"))
            .context("append README metadata")?;
        writeln!(f, "Pass metadata:")?;
        for l in &meta_lines {
            writeln!(f, "- {l}")?;
        }
    }

    if !warnings.is_empty() {
        let _ = fs::write(out_dir.join("WARN.txt"), warnings.join("\n"));
    }

    // Return artifact descriptor (path for convenience)
    let artifact = format!("artifact:{}", out_dir.to_string_lossy());

    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let l = nc_telemetry::labels::backend(&graph.name, "riscv", Some(&manifest.name));
        let _ = a.counter("graph.populations", graph.populations.len() as f64, l.clone());
        let _ = a.counter("graph.connections", graph.connections.len() as f64, l.clone());
        let _ = a.counter("graph.probes", graph.probes.len() as f64, l);
    }

    Ok(artifact)
}

fn default_out_dir(graph_name: &str, target_name: &str) -> PathBuf {
    PathBuf::from(format!("target/{target_name}-{graph_name}"))
}

#[allow(clippy::uninlined_format_args)]
fn emit_scalar_linux_rv64(out_dir: &Path, graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<()> {
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }
    let graph_name = &graph.name;
    let target_name = &manifest.name;

    // Derive simple metadata for comments (mirrors rv-layout/schedule defaults)
    let caps = manifest.capabilities.as_ref();
    let align_bytes = caps.and_then(|c| c.cacheline_bytes).unwrap_or(16);
    let quant_bits_default = caps
        .and_then(|c| c.weight_precisions.as_ref().and_then(|v| v.iter().min().copied()))
        .unwrap_or(8);
    let fused_stage = if caps.and_then(|c| c.has_vector).unwrap_or(false) {
        "op_fuse_vadd_vmul"
    } else {
        "op_fuse_scalar"
    };

    let c_src = format!(
r#"#include <stdio.h>
#include <stdint.h>
#include <time.h>

/* RISC-V pass metadata (from pipeline/config):
 *  - align_bytes={align}
 *  - quant_bits_default={qbits}
 *  - fused_stages={fused}
 */

static inline uint64_t now_ns() {{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}}

int main(void) {{
    const char* graph = "{graph}";
    const char* backend = "riscv";
    const char* isa = "rv64gcv";
    const char* simulator = "qemu";
    uint64_t t0 = now_ns();
    volatile uint64_t acc = 0;
    for (int i = 0; i < 100000; ++i) {{ acc += (uint64_t)i; }}
    uint64_t t1 = now_ns();
    double step_ns = (double)(t1 - t0);
    printf("{{\"metric\":\"kernel.step_ns\",\"value\":%.0f,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n", step_ns, graph, backend, isa, simulator);
    printf("{{\"metric\":\"events.processed\",\"value\":%d,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n", 100000, graph, backend, isa, simulator);
    (void)acc;
    return 0;
}}
"#,
        graph = graph_name,
        align = align_bytes,
        qbits = quant_bits_default,
        fused = fused_stage
    );

    let main_c = out_dir.join("main.c");
    let mut f = fs::File::create(&main_c)?;
    f.write_all(c_src.as_bytes())?;
    f.flush()?;

    // Write a simple README for artifact provenance
    let mut readme = fs::File::create(out_dir.join("README.txt"))?;
    writeln!(
        readme,
        "Generated RV64 Linux scalar runtime for graph '{graph_name}' targeting '{target_name}'.",
    )?;
    Ok(())
}

fn detect_tool(candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if Command::new("sh").arg("-lc").arg(format!("command -v {c}")).status().ok()?.success() {
            return Some((*c).to_string());
        }
    }
    None
}

fn build_rv64_linux_binary(out_dir: &Path) -> Result<PathBuf> {
    let main_c = out_dir.join("main.c");
    let exe = out_dir.join("prog-rv64");
    // Prefer GCC cross, then Clang cross
    if let Some(cc) = detect_tool(&["riscv64-linux-gnu-gcc"]) {
        let status = Command::new(cc)
            .arg("-O2")
            .arg("-static") // may fail on some setups; fall back handled below
            .arg("-o").arg(&exe)
            .arg(&main_c)
            .status()
            .context("invoke riscv64-linux-gnu-gcc")?;
        if status.success() {
            return Ok(exe);
        }
        // Retry without -static
        let status2 = Command::new("riscv64-linux-gnu-gcc")
            .arg("-O2")
            .arg("-o").arg(&exe)
            .arg(&main_c)
            .status()
            .context("invoke riscv64-linux-gnu-gcc (dynamic)")?;
        if status2.success() {
            return Ok(exe);
        }
    }
    if let Some(clang) = detect_tool(&["clang"]) {
        let status = Command::new(clang)
            .arg("--target=riscv64-unknown-linux-gnu")
            .arg("-O2")
            .arg("-o").arg(&exe)
            .arg(&main_c)
            .status()
            .context("invoke clang --target=riscv64-unknown-linux-gnu")?;
        if status.success() {
            return Ok(exe);
        }
    }
    // No toolchain or build failed; return an error to caller to handle optionally
    anyhow::bail!("no suitable RISC-V cross toolchain found to build {:?}", exe)
}

fn run_qemu_and_capture(exe: &Path, out_dir: &Path) -> Result<()> {
    let qemu = detect_tool(&["qemu-riscv64"]).ok_or_else(|| anyhow::anyhow!("qemu-riscv64 not found"))?;
    let output = Command::new(qemu)
        .arg(exe)
        .output()
        .context("running qemu-riscv64")?;
    // Decide JSONL destination
    let dest = if let Ok(p) = std::env::var("NC_PROFILE_JSONL") {
        PathBuf::from(p)
    } else {
        out_dir.join("profile.jsonl")
    };
    fs::write(&dest, output.stdout)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn compile_smoke() {
        let g = nc_nir::Graph::new("g");
        let m = nc_hal::parse_target_manifest_str(r#"
            name = "riscv64gcv_linux"
            vendor = "Generic"
            family = "RISC-V"
            version = "1"
            [capabilities]
            weight_precisions = [8]
            max_neurons_per_core = 1
            max_synapses_per_core = 1
            time_resolution_ns = 1
        "#).unwrap();
        let _ = compile(&g, &m).unwrap();
    }

    #[test]
    fn pipeline_integration_smoke() {
        // Use the real manifest file for riscv64gcv_linux and ensure main.c is emitted
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv64gcv_linux.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv64gcv_linux manifest");
        let g = nc_nir::Graph::new("pipe");
        let artifact = compile(&g, &m).expect("compile ok");
        assert!(artifact.contains("artifact:target/"), "artifact string: {artifact}");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
        assert!(out_dir.join("main.c").exists(), "expected main.c to exist for linux_user profile");

        // Ensure pass dumps exist with expected keys
        let passes = out_dir.join("passes");
        let layout_json = passes.join("02_rv-layout.json");
        let sched_json = passes.join("03_rv-schedule.json");
        assert!(layout_json.exists(), "rv-layout dump missing");
        assert!(sched_json.exists(), "rv-schedule dump missing");
        let layout_s = fs::read_to_string(&layout_json).unwrap_or_default();
        let sched_s = fs::read_to_string(&sched_json).unwrap_or_default();
        assert!(layout_s.contains("\"rv_layout\"") || layout_s.contains("rv_layout"), "rv_layout key not found");
        assert!(layout_s.contains("align_bytes"), "align_bytes key not found");
        assert!(sched_s.contains("\"rv_schedule\"") || sched_s.contains("rv_schedule"), "rv_schedule key not found");
    }

    #[test]
    fn pipeline_profile_bare_stub() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv32imac_bare.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv32imac_bare manifest");
        let g = nc_nir::Graph::new("bare");
        let artifact = compile(&g, &m).expect("compile ok");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
        assert!(out_dir.join("README.txt").exists(), "README expected for bare_metal profile");
        assert!(!out_dir.join("main.c").exists(), "main.c must not exist for bare_metal profile");
        // check pass dump includes bare tuning
        let passes = out_dir.join("passes");
        let mut found = false;
        if let Ok(entries) = std::fs::read_dir(&passes) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().contains("rv-baremetal-tuning") {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "rv-baremetal-tuning dump not found in {passes:?}");
    }

    #[test]
    fn pipeline_profile_ctrl_stub() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv64gc_ctrl.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv64gc_ctrl manifest");
        let g = nc_nir::Graph::new("ctrl");
        let artifact = compile(&g, &m).expect("compile ok");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
        assert!(out_dir.join("README.txt").exists(), "README expected for control_plane profile");
        assert!(!out_dir.join("main.c").exists(), "main.c must not exist for control_plane profile");
        // check pass dump includes control-plane driver
        let passes = out_dir.join("passes");
        let mut found = false;
        if let Ok(entries) = std::fs::read_dir(&passes) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().contains("rv-control-plane-driver") {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "rv-control-plane-driver dump not found in {passes:?}");
    }

    #[test]
    fn qemu_user_smoke_if_available() {
        // Skip if toolchain or qemu not present
        if detect_tool(&["qemu-riscv64"]).is_none() {
            return;
        }
        if detect_tool(&["riscv64-linux-gnu-gcc"]).is_none() && detect_tool(&["clang"]).is_none() {
            return;
        }
        let g = nc_nir::Graph::new("gqemu");
        let m = nc_hal::parse_target_manifest_str(r#"
            name = "riscv64gcv_linux"
            vendor = "Generic"
            family = "RISC-V"
            version = "1"
            [capabilities]
            weight_precisions = [8,16,32]
            max_neurons_per_core = 100
            max_synapses_per_core = 100
            time_resolution_ns = 1
        "#).unwrap();

        let out_dir = default_out_dir(&g.name, &m.name);
        emit_scalar_linux_rv64(&out_dir, &g, &m).unwrap();
        if let Ok(exe) = build_rv64_linux_binary(&out_dir) {
            // Ensure we write to a temp JSONL inside out_dir
            let jsonl = out_dir.join("profile.jsonl");
            std::env::set_var("NC_PROFILE_JSONL", &jsonl);
            run_qemu_and_capture(&exe, &out_dir).unwrap();
            let content = fs::read_to_string(&jsonl).unwrap_or_default();
            assert!(content.contains("\"backend\":\"riscv\""));
            assert!(content.contains("\"kernel.step_ns\""));
        }
    }
}
