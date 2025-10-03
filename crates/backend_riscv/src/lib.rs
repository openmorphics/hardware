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
        let vec_bytes = if vec_ok { 64 } else { 16 };
        let meta = json!({
            "align_bytes": align,
            "vector_available": vec_ok,
            "vector_bytes": vec_bytes,
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
        let vlen = g.attributes
            .get("rv_layout")
            .and_then(|v| v.get("vector_bytes"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let meta = json!({ "enabled": vec_ok, "vlen_bytes": vlen });
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

    // Determine profile with backward-compatible fallback
    let profile = manifest.capabilities
        .as_ref()
        .and_then(|c| c.profile.as_deref())
        .unwrap_or_else(|| {
            if manifest.name.contains("linux") { "linux_user" }
            else if manifest.name.contains("ctrl") { "control_plane" }
            else { "bare_metal" }
        });

    // Dispatch by profile
    let artifact = match profile {
        "linux_user" => compile_linux_user(graph, manifest, &out_dir)?,
        "bare_metal" => compile_bare_metal(graph, manifest, &out_dir)?,
        "control_plane" => compile_control_plane(graph, manifest, &out_dir)?,
        _ => compile_linux_user(graph, manifest, &out_dir)?,
    };

    // Telemetry counters (unchanged)
    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let l = nc_telemetry::labels::backend(&graph.name, "riscv", Some(&manifest.name));
        let _ = a.counter("graph.populations", graph.populations.len() as f64, l.clone());
        let _ = a.counter("graph.connections", graph.connections.len() as f64, l.clone());
        let _ = a.counter("graph.probes", graph.probes.len() as f64, l);
    }

    Ok(artifact)
}

/// Run the RISC-V pass pipeline appropriate to `profile`, dump JSON, and collect README metadata.
/// Best-effort: records warnings instead of failing the compile.
fn run_pipeline_and_collect_meta(
    graph: &nc_nir::Graph,
    manifest: &nc_hal::TargetManifest,
    out_dir: &Path,
    profile: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
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

    meta_lines
}

/// Compile the linux_user profile (existing logic preserved).
fn compile_linux_user(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest, out_dir: &Path) -> Result<String> {
    let mut warnings: Vec<String> = Vec::new();

    let meta_lines = run_pipeline_and_collect_meta(graph, manifest, out_dir, "linux_user", &mut warnings);

    if let Err(e) = emit_linux_rv64_runtime(out_dir, graph, manifest) {
        warnings.push(format!("emit failed: {e}"));
    }

    match build_rv64_linux_binary(out_dir) {
        Ok(exe) => {
            if std::env::var("NC_RISCV_QEMU_RUN").ok().as_deref() == Some("1") {
                if let Err(e) = run_qemu_and_capture(&exe, out_dir) {
                    warnings.push(format!("qemu run failed: {e}"));
                }
            }
        }
        Err(e) => {
            warnings.push(format!("{e}"));
        }
    }

    // README + metadata
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

    Ok(format!("artifact:{}", out_dir.to_string_lossy()))
}

/// Compile the bare_metal profile: emit crt0.S/linker.ld/main.c, best-effort build, and optional QEMU-system run.
fn compile_bare_metal(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest, out_dir: &Path) -> Result<String> {
    let mut warnings: Vec<String> = Vec::new();

    let meta_lines = run_pipeline_and_collect_meta(graph, manifest, out_dir, "bare_metal", &mut warnings);

    if let Err(e) = emit_bare_metal_runtime(out_dir, graph, manifest) {
        warnings.push(format!("emit (bare-metal) failed: {e}"));
    } else {
        match build_rv32_bare_metal_binary(out_dir) {
            Ok(elf) => {
                if std::env::var("NC_RISCV_QEMU_RUN").ok().as_deref() == Some("1") {
                    if let Err(e) = run_qemu_system_and_capture(&elf, out_dir) {
                        warnings.push(format!("qemu-system run failed: {e}"));
                    }
                }
            }
            Err(e) => warnings.push(format!("{e}")),
        }
    }

    // README + metadata
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

    Ok(format!("artifact:{}", out_dir.to_string_lossy()))
}

/// Compile the control_plane profile: emit Renode artifacts and run simulation.
fn compile_control_plane(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest, out_dir: &Path) -> Result<String> {
    let mut warnings: Vec<String> = Vec::new();

    let meta_lines = run_pipeline_and_collect_meta(graph, manifest, out_dir, "control_plane", &mut warnings);

    // Emit control-plane artifacts (main.c, .repl, .py, .resc)
    if let Err(e) = emit_control_plane_runtime(out_dir, graph, manifest) {
        warnings.push(format!("emit_control_plane_runtime failed: {e}"));
    } else {
        // Build the Linux binary (control-plane is also a linux_user binary)
        match build_rv64_linux_binary(out_dir) {
            Ok(exe) => {
                // If NC_RISCV_QEMU_RUN=1, run Renode simulation
                if std::env::var("NC_RISCV_QEMU_RUN").ok().as_deref() == Some("1") {
                    if let Err(e) = run_renode_and_capture(&exe, out_dir) {
                        warnings.push(format!("renode run failed: {e}"));
                    }
                }
            }
            Err(e) => {
                warnings.push(format!("build failed: {e}"));
            }
        }
    }

    // README + metadata
    let mut readme = fs::File::create(out_dir.join("README.txt"))
        .context("create README.txt for control_plane profile")?;
    writeln!(
        readme,
        "Generated RISC-V control-plane artifacts for graph '{}' with Renode simulation.",
        graph.name
    )?;

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

    Ok(format!("artifact:{}", out_dir.to_string_lossy()))
}

/// Emit bare-metal templates: linker.ld, crt0.S, and main.c (polled UART at 0x10000000).
fn emit_bare_metal_runtime(out_dir: &Path, graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<()> {
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }

    let graph_name = &graph.name;
    let _target_name = &manifest.name;

    // linker.ld
    let ld = r#"
OUTPUT_ARCH(riscv)
ENTRY(_start)

MEMORY
{
  RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 4M
}

SECTIONS
{
  .text : {
    KEEP(*(.init))
    *(.text*)
    *(.rodata*)
  } > RAM

  .data : {
    *(.data*)
  } > RAM

  .bss (NOLOAD) : {
    __bss_start__ = .;
    *(.bss*)
    *(COMMON)
    __bss_end__ = .;
  } > RAM

  . = ORIGIN(RAM) + LENGTH(RAM);
  __stack_top = .;
}
"#;
    fs::write(out_dir.join("linker.ld"), ld)?;

    // crt0.S
    let crt0 = r#"
    .section .init
    .globl _start
_start:
    la  sp, __stack_top

    /* Zero .bss */
    la  a0, __bss_start__
    la  a1, __bss_end__
1:
    beq a0, a1, 2f
    sw  zero, 0(a0)
    addi a0, a0, 4
    blt a0, a1, 1b
2:
    /* Jump to C main */
    call main

3:
    wfi
    j 3b
"#;
    fs::write(out_dir.join("crt0.S"), crt0)?;

    // main.c (bare-metal, polled UART at 0x10000000, QEMU exit via 0x100000)
    let c_src = format!(r#"
#include <stdint.h>
#include <stddef.h>

#define UART0_BASE      0x10000000UL
#define UART_THR        (UART0_BASE + 0x00)
#define UART_LSR        (UART0_BASE + 0x05)
#define LSR_THRE        0x20

#define QEMU_FINISHER_BASE 0x00100000UL
#define QEMU_FINISHER_PASS 0x5555

static inline void mmio_write8(uintptr_t addr, uint8_t val) {{ *(volatile uint8_t*)addr = val; }}
static inline uint8_t mmio_read8(uintptr_t addr) {{ return *(volatile uint8_t*)addr; }}

static void uart_putc(char c) {{
    /* wait for THR empty */
    while ((mmio_read8(UART_LSR) & LSR_THRE) == 0) {{ }}
    mmio_write8(UART_THR, (uint8_t)c);
}}

static void uart_puts(const char* s) {{
    while (*s) {{
        uart_putc(*s++);
    }}
}}

static void print_u32(uint32_t x) {{
    char buf[11]; // max 10 digits + NUL
    int i = 0;
    if (x == 0) {{ uart_putc('0'); return; }}
    while (x > 0 && i < 10) {{
        uint32_t q = x / 10;
        uint32_t r = x - q * 10;
        buf[i++] = (char)('0' + r);
        x = q;
    }}
    while (i--) uart_putc(buf[i]);
}}

static inline uint32_t rdcycle(void) {{
    uint32_t x; __asm__ volatile("csrr %0, cycle" : "=r"(x)); return x;
}}
static inline uint32_t rdinstret(void) {{
    uint32_t x; __asm__ volatile("csrr %0, instret" : "=r"(x)); return x;
}}

static inline void qemu_exit(uint32_t code) {{
    volatile uint32_t* fin = (volatile uint32_t*)QEMU_FINISHER_BASE;
    /* Encode status: (code<<16) | PASS */
    *fin = (code << 16) | QEMU_FINISHER_PASS;
}}

int main(void) {{
    const char* graph = "{graph_name}";
    const char* backend = "riscv";
    const char* isa = "rv32imac";
    const char* simulator = "qemu";

    volatile uint32_t acc = 0;
    uint32_t c0 = rdcycle();
    uint32_t i0 = rdinstret();

    for (uint32_t i = 0; i < 100000; ++i) {{ acc += i; }}

    uint32_t c1 = rdcycle();
    uint32_t i1 = rdinstret();
    uint32_t dc = c1 - c0;
    uint32_t di = i1 - i0;

    /* JSONL lines */
    uart_puts("{{\"metric\":\"kernel.step_ns\",\"value\":"); print_u32(dc); uart_puts(",\"labels\":{{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}}}\\n");

    uart_puts("{{\"metric\":\"events.processed\",\"value\":"); print_u32(100000u); uart_puts(",\"labels\":{{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}}}\\n");

    uart_puts("{{\"metric\":\"cpu.cycle\",\"value\":"); print_u32(dc); uart_puts(",\"labels\":{{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}}}\\n");

    uart_puts("{{\"metric\":\"cpu.instret\",\"value\":"); print_u32(di); uart_puts(",\"labels\":{{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}}}\\n");

    (void)acc;
    qemu_exit(0);
    for(;;) {{ }}
    return 0;
}}
"#);
    fs::write(out_dir.join("main.c"), c_src.as_bytes())?;

    // Provenance
    let mut readme = fs::File::create(out_dir.join("README.txt"))?;
    writeln!(
        readme,
        "Generated RV32IMAC bare-metal runtime for graph '{graph_name}' (UART @ 0x10000000, QEMU finisher @ 0x00100000)."
    )?;

    Ok(())
}

/// Best-effort build of the bare-metal firmware. Returns path to ELF on success.
fn build_rv32_bare_metal_binary(out_dir: &Path) -> Result<PathBuf> {
    let linker = out_dir.join("linker.ld");
    let crt0 = out_dir.join("crt0.S");
    let main_c = out_dir.join("main.c");
    let elf = out_dir.join("firmware.elf");

    let mut warn_lines: Vec<String> = Vec::new();

    let append_build_info = |tool: &str| {
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_dir.join("README.txt"))
        {
            let _ = writeln!(f, "Build (bare-metal): toolchain={tool}, flags=-Os -ffreestanding -nostdlib -nostartfiles -march=rv32imac -mabi=ilp32");
        }
    };

    if let Some(cc) = detect_tool(&["riscv64-unknown-elf-gcc"]) {
        let status = Command::new(&cc)
            .arg("-Os")
            .arg("-ffreestanding")
            .arg("-nostdlib")
            .arg("-nostartfiles")
            .arg("-Wl,-Map=firmware.map")
            .arg("-T").arg(&linker)
            .arg("-march=rv32imac")
            .arg("-mabi=ilp32")
            .arg("-o").arg(&elf)
            .arg(&crt0)
            .arg(&main_c)
            .status()
            .context("invoke riscv64-unknown-elf-gcc (bare-metal)")?;
        if status.success() {
            append_build_info("gcc-elf");
            return Ok(elf);
        } else {
            warn_lines.push("riscv64-unknown-elf-gcc returned non-zero status".into());
        }
    } else {
        warn_lines.push("toolchain not found: riscv64-unknown-elf-gcc".into());
    }

    if !warn_lines.is_empty() {
        let _ = fs::write(out_dir.join("WARN.txt"), warn_lines.join("\n"));
    }
    anyhow::bail!("no suitable RISC-V bare-metal toolchain found to build {:?}", elf)
}

/// Run QEMU system emulator and capture UART stdout to profile.jsonl (or NC_PROFILE_JSONL).
fn run_qemu_system_and_capture(elf: &Path, out_dir: &Path) -> Result<()> {
    let qemu = detect_tool(&["qemu-system-riscv32"]).ok_or_else(|| anyhow::anyhow!("qemu-system-riscv32 not found"))?;
    let output = Command::new(qemu)
        .arg("-nographic")
        .arg("-machine").arg("virt")
        .arg("-bios").arg("none")
        .arg("-no-reboot")
        .arg("-kernel").arg(elf)
        .output()
        .context("running qemu-system-riscv32")?;

    let dest = if let Ok(p) = std::env::var("NC_PROFILE_JSONL") {
        PathBuf::from(p)
    } else {
        out_dir.join("profile.jsonl")
    };
    fs::write(&dest, output.stdout)?;
    Ok(())
}

fn default_out_dir(graph_name: &str, target_name: &str) -> PathBuf {
    PathBuf::from(format!("target/{target_name}-{graph_name}"))
}

#[allow(clippy::uninlined_format_args)]
fn emit_linux_rv64_runtime(out_dir: &Path, graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<()> {
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

    // Try to read vectorization metadata from in-graph attributes first
    let mut vector_enabled = graph.attributes
        .get("rv_vectorize")
        .and_then(|v| v.get("enabled"))
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let mut vlen_bytes: u64 = graph.attributes
        .get("rv_vectorize")
        .and_then(|v| v.get("vlen_bytes"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    // Fall back to pass dumps if the input graph doesn't carry pass attributes
    if !vector_enabled || vlen_bytes == 0 {
        let passes_dir = out_dir.join("passes");
        if let Ok(rd) = fs::read_dir(&passes_dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().contains("rv-vectorize") {
                    if let Ok(s) = fs::read_to_string(e.path()) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                            if let Some(rv) = val.get("rv_vectorize") {
                                if let Some(b) = rv.get("enabled").and_then(|x| x.as_bool()) {
                                    vector_enabled = b;
                                }
                                if let Some(vb) = rv.get("vlen_bytes").and_then(|x| x.as_u64()) {
                                    vlen_bytes = vb;
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    let c_src = format!(
r#"#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>

/* RISC-V pass metadata (from pipeline/config):
 *  - align_bytes={align}
 *  - quant_bits_default={qbits}
 *  - fused_stages={fused}
 *  - rvv_enabled={rvv}
 *  - vlen_bytes={vlen}
 */

/* Conditionally include RVV intrinsics */
#if defined(__riscv_vector)
  #include <riscv_vector.h>
#endif

static inline uint64_t now_ns() {{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}}

/* Optional RISC-V counters: rdcycle/rdinstret stubs when not building for RISC-V */
#if defined(__riscv) || defined(__riscv_xlen)
static inline uint64_t rdcycle(void) {{ uint64_t x; __asm__ volatile("rdcycle %0" : "=r"(x)); return x; }}
static inline uint64_t rdinstret(void) {{ uint64_t x; __asm__ volatile("rdinstret %0" : "=r"(x)); return x; }}
#else
static inline uint64_t rdcycle(void) {{ return 0ull; }}
static inline uint64_t rdinstret(void) {{ return 0ull; }}
#endif

int main(void) {{
    const char* graph = "{graph}";
    const char* backend = "riscv";
    const char* isa = "rv64gcv";
    const char* simulator = "qemu";

    uint64_t c0 = rdcycle();
    uint64_t i0 = rdinstret();

    uint64_t t0 = now_ns();

    /* Workloop: vectorized sum-reduction with scalar fallback */
#if defined(__riscv_vector)
    size_t n = 100000;
    uint64_t* data = (uint64_t*)malloc(n * sizeof(uint64_t));
    if (!data) return 1;
    for (size_t ii = 0; ii < n; ++ii) {{ data[ii] = (uint64_t)ii; }}

    size_t ii = 0;
    size_t vl1 = vsetvl_e64m1(1);
    vuint64m1_t v_acc = vmv_v_x_u64m1(0, vl1);
    while (ii < n) {{
        size_t vl = vsetvl_e64m8(n - ii);
        vuint64m8_t v_data = vle64_v_u64m8(&data[ii], vl);
        v_acc = vredsum_vs_u64m8_u64m1(v_data, v_acc, vl);
        ii += vl;
    }}
    uint64_t sum = vmv_x_s_u64m1_u64(v_acc);
    volatile uint64_t acc = sum;
    free(data);
#else
    volatile uint64_t acc = 0;
    for (size_t i = 0; i < 100000; ++i) {{ acc += (uint64_t)i; }}
#endif

    uint64_t t1 = now_ns();

    uint64_t c1 = rdcycle();
    uint64_t i1 = rdinstret();

    double step_ns = (double)(t1 - t0);
    printf("{{\"metric\":\"kernel.step_ns\",\"value\":%.0f,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n", step_ns, graph, backend, isa, simulator);
    printf("{{\"metric\":\"events.processed\",\"value\":%d,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n", 100000, graph, backend, isa, simulator);
    printf("{{\"metric\":\"cpu.cycle\",\"value\":%llu,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n",
           (unsigned long long)(c1 - c0), graph, backend, isa, simulator);
    printf("{{\"metric\":\"cpu.instret\",\"value\":%llu,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n",
           (unsigned long long)(i1 - i0), graph, backend, isa, simulator);
    (void)acc;
    return 0;
}}
"#,
        graph = graph_name,
        align = align_bytes,
        qbits = quant_bits_default,
        fused = fused_stage,
        rvv = vector_enabled,
        vlen = vlen_bytes
    );

    let main_c = out_dir.join("main.c");
    let mut f = fs::File::create(&main_c)?;
    f.write_all(c_src.as_bytes())?;
    f.flush()?;

    // Write a simple README for artifact provenance
    let mut readme = fs::File::create(out_dir.join("README.txt"))?;
    writeln!(
        readme,
        "Generated RV64 Linux runtime (guarded RVV + scalar) for graph '{graph_name}' targeting '{target_name}'.",
    )?;
    writeln!(
        readme,
        "RVV intent: enabled={}, vlen_bytes={}",
        vector_enabled, vlen_bytes
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

    // Determine whether to attempt RVV build from pass dumps and feature gate.
    let mut try_vector = false;
    if cfg!(feature = "riscv-v") {
        let passes_dir = out_dir.join("passes");
        if let Ok(rd) = fs::read_dir(&passes_dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().contains("rv-vectorize") {
                    if let Ok(s) = fs::read_to_string(e.path()) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                            if let Some(rv) = val.get("rv_vectorize") {
                                try_vector = rv
                                    .get("enabled")
                                    .and_then(|x| x.as_bool())
                                    .unwrap_or(false);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    let mut warn_lines: Vec<String> = Vec::new();

    // Helper to append build info
    let append_build_info = |tool: &str, built_with_vector: bool| {
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_dir.join("README.txt"))
        {
            let _ = writeln!(
                f,
                "Build: toolchain={tool}, vector_intent={}, built_with_vector={}",
                if cfg!(feature = "riscv-v") { "true" } else { "false" },
                built_with_vector
            );
        }
    };

    // Prefer GCC cross, then Clang cross
    if let Some(cc) = detect_tool(&["riscv64-linux-gnu-gcc"]) {
        if try_vector {
            // Try GCC with RVV (static, then dynamic)
            let status = Command::new(&cc)
                .arg("-O2")
                .arg("-static")
                .arg("-march=rv64gcv")
                .arg("-o")
                .arg(&exe)
                .arg(&main_c)
                .status()
                .context("invoke riscv64-linux-gnu-gcc (vector, static)")?;
            if status.success() {
                append_build_info("gcc", true);
                return Ok(exe);
            }
            let status2 = Command::new(&cc)
                .arg("-O2")
                .arg("-march=rv64gcv")
                .arg("-o")
                .arg(&exe)
                .arg(&main_c)
                .status()
                .context("invoke riscv64-linux-gnu-gcc (vector, dynamic)")?;
            if status2.success() {
                append_build_info("gcc", true);
                return Ok(exe);
            }
            warn_lines.push("rvv: GCC vector build failed, falling back to scalar".into());
        }

        // Scalar fallback (static then dynamic)
        let status3 = Command::new(&cc)
            .arg("-O2")
            .arg("-static")
            .arg("-o")
            .arg(&exe)
            .arg(&main_c)
            .status()
            .context("invoke riscv64-linux-gnu-gcc (scalar, static)")?;
        if status3.success() {
            append_build_info("gcc", false);
            if !warn_lines.is_empty() {
                let _ = fs::write(out_dir.join("WARN.txt"), warn_lines.join("\n"));
            }
            return Ok(exe);
        }
        let status4 = Command::new(&cc)
            .arg("-O2")
            .arg("-o")
            .arg(&exe)
            .arg(&main_c)
            .status()
            .context("invoke riscv64-linux-gnu-gcc (scalar, dynamic)")?;
        if status4.success() {
            append_build_info("gcc", false);
            if !warn_lines.is_empty() {
                let _ = fs::write(out_dir.join("WARN.txt"), warn_lines.join("\n"));
            }
            return Ok(exe);
        }
    }

    if let Some(clang) = detect_tool(&["clang"]) {
        if try_vector {
            let status = Command::new(&clang)
                .arg("--target=riscv64-unknown-linux-gnu")
                .arg("-O2")
                .arg("-march=rv64gcv")
                .arg("-o")
                .arg(&exe)
                .arg(&main_c)
                .status()
                .context("invoke clang --target=riscv64-unknown-linux-gnu (vector)")?;
            if status.success() {
                append_build_info("clang", true);
                return Ok(exe);
            }
            warn_lines.push("rvv: Clang vector build failed, falling back to scalar".into());
        }

        let status2 = Command::new(&clang)
            .arg("--target=riscv64-unknown-linux-gnu")
            .arg("-O2")
            .arg("-o")
            .arg(&exe)
            .arg(&main_c)
            .status()
            .context("invoke clang --target=riscv64-unknown-linux-gnu (scalar)")?;
        if status2.success() {
            append_build_info("clang", false);
            if !warn_lines.is_empty() {
                let _ = fs::write(out_dir.join("WARN.txt"), warn_lines.join("\n"));
            }
            return Ok(exe);
        }
    }

    // No toolchain or build failed; write warnings if any and return an error
    if !warn_lines.is_empty() {
        let _ = fs::write(out_dir.join("WARN.txt"), warn_lines.join("\n"));
    }
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

/// Emit control-plane artifacts: main.c, accelerator.repl, accelerator.py, run.resc
fn emit_control_plane_runtime(out_dir: &Path, graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<()> {
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }

    let graph_name = &graph.name;
    let caps = manifest.capabilities.as_ref();
    let mmio_base_addr = caps.and_then(|c| c.mmio_base_addr).unwrap_or(0x40000000);

    // Extract MMIO and DMA properties from rv_ctrl_plane pass attributes if available
    // Fall back to manifest capabilities if pass data not available
    let _mmio_supported = caps.and_then(|c| c.mmio_supported).unwrap_or(false);
    let dma_supported = caps.and_then(|c| c.dma_supported).unwrap_or(false);

    // Generate main.c - Linux user-space program using mmap to access MMIO
    let main_c = format!(r#"#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <time.h>
#include <errno.h>
#include <string.h>

#define MMIO_BASE_ADDR  0x{mmio_base:08x}UL
#define MMIO_SIZE       0x1000UL  // 4KB region

// Accelerator register offsets
#define ACCEL_CTRL      0x00  // Control register
#define ACCEL_STATUS    0x04  // Status register
#define DMA_ADDR        0x08  // DMA address register
#define DMA_LEN         0x0C  // DMA length register

// Control register bits
#define CTRL_START      (1 << 0)
#define CTRL_RESET      (1 << 1)

// Status register bits
#define STATUS_DONE     (1 << 0)
#define STATUS_BUSY     (1 << 1)

static inline uint64_t now_ns() {{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}}

int main(void) {{
    const char* graph = "{graph_name}";
    const char* backend = "riscv";
    const char* isa = "rv64gc";
    const char* simulator = "renode";

    printf("Starting control-plane test for graph '%s'\\n", graph);

    // Open /dev/mem for MMIO access
    int mem_fd = open("/dev/mem", O_RDWR | O_SYNC);
    if (mem_fd < 0) {{
        fprintf(stderr, "Failed to open /dev/mem: %s\\n", strerror(errno));
        fprintf(stderr, "Note: This program requires root privileges or UIO driver\\n");
        return 1;
    }}

    // Map MMIO region
    volatile uint32_t* mmio_base = (volatile uint32_t*)mmap(
        NULL, MMIO_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, mem_fd, MMIO_BASE_ADDR);
    
    if (mmio_base == MAP_FAILED) {{
        fprintf(stderr, "Failed to mmap MMIO region: %s\\n", strerror(errno));
        close(mem_fd);
        return 1;
    }}

    uint64_t t0 = now_ns();

    printf("Mapped MMIO region at 0x%lx\\n", MMIO_BASE_ADDR);

    // Reset accelerator
    printf("Resetting accelerator...\\n");
    mmio_base[ACCEL_CTRL/4] = CTRL_RESET;
    usleep(1000); // 1ms delay
    mmio_base[ACCEL_CTRL/4] = 0;

    // Configure DMA (dummy operation)
    if ({dma_supported}) {{
        printf("Configuring DMA...\\n");
        mmio_base[DMA_ADDR/4] = 0x80000000; // Dummy DMA address
        mmio_base[DMA_LEN/4] = 1024;        // Dummy DMA length
    }}

    // Start accelerator operation
    printf("Starting accelerator operation...\\n");
    mmio_base[ACCEL_CTRL/4] = CTRL_START;

    // Poll for completion
    int timeout = 1000; // 1000ms timeout
    uint32_t status;
    do {{
        status = mmio_base[ACCEL_STATUS/4];
        if (status & STATUS_DONE) {{
            break;
        }}
        usleep(1000); // 1ms delay
        timeout--;
    }} while (timeout > 0);

    uint64_t t1 = now_ns();

    if (timeout == 0) {{
        printf("Operation timed out!\\n");
    }} else {{
        printf("Operation completed successfully\\n");
    }}

    // Read final status
    status = mmio_base[ACCEL_STATUS/4];
    printf("Final status: 0x%08x\\n", status);

    double step_ns = (double)(t1 - t0);

    // Output telemetry in JSONL format
    printf("{{\"metric\":\"kernel.step_ns\",\"value\":%.0f,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n",
           step_ns, graph, backend, isa, simulator);
    printf("{{\"metric\":\"events.processed\",\"value\":%d,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n",
           1, graph, backend, isa, simulator);
    printf("{{\"metric\":\"mmio.operations\",\"value\":%d,\"labels\":{{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}}}\\n",
           {dma_supported} ? 5 : 3, graph, backend, isa, simulator);

    // Cleanup
    munmap((void*)mmio_base, MMIO_SIZE);
    close(mem_fd);

    return 0;
}}
"#,
        mmio_base = mmio_base_addr,
        graph_name = graph_name,
        dma_supported = if dma_supported { "1" } else { "0" }
    );

    fs::write(out_dir.join("main.c"), main_c)?;

    // Generate accelerator.repl - Renode platform description
    let repl_content = format!(r#"using sysbus
mach create "snn_system"

machine LoadPlatformDescription @platforms/cpus/riscv64.repl

# Add our custom accelerator peripheral
sysbus Redirect 0x{mmio_base_addr:08x} 0x1000 sysbus.snn_accelerator

# Create the accelerator
python "exec(open('accelerator.py').read())"
machine PyDevFromFile @accelerator.py 0x{mmio_base_addr:08x} 0x1000 True "snn_accelerator"

showAnalyzer sysbus.uart0
"#);

    fs::write(out_dir.join("accelerator.repl"), repl_content)?;

    // Generate accelerator.py - Python model for the SNN_Accelerator peripheral
    let py_content = r#"from Antmicro.Renode.Peripherals import IDoubleWordPeripheral
from Antmicro.Renode.Peripherals.Bus import BusRangeRegistration
from Antmicro.Renode.Core import Range

class SNNAccelerator(IDoubleWordPeripheral):
    def __init__(self):
        self.ctrl_reg = 0
        self.status_reg = 0
        self.dma_addr_reg = 0
        self.dma_len_reg = 0
        self.operation_active = False
        
    def ReadDoubleWord(self, offset):
        if offset == 0x00:  # ACCEL_CTRL
            return self.ctrl_reg
        elif offset == 0x04:  # ACCEL_STATUS
            return self.status_reg
        elif offset == 0x08:  # DMA_ADDR
            return self.dma_addr_reg
        elif offset == 0x0C:  # DMA_LEN
            return self.dma_len_reg
        else:
            print(f"[SNN_Accelerator] Read from unknown offset 0x{offset:02x}")
            return 0
            
    def WriteDoubleWord(self, offset, value):
        if offset == 0x00:  # ACCEL_CTRL
            self.ctrl_reg = value
            if value & 0x1:  # CTRL_START
                print("[SNN_Accelerator] Accelerator control register written: START operation")
                self.operation_active = True
                self.status_reg |= 0x2  # Set BUSY bit
                # Simulate operation completion
                self.status_reg |= 0x1  # Set DONE bit
                self.status_reg &= ~0x2  # Clear BUSY bit
                print("[SNN_Accelerator] Operation completed")
            elif value & 0x2:  # CTRL_RESET
                print("[SNN_Accelerator] Accelerator control register written: RESET")
                self.status_reg = 0
                self.operation_active = False
        elif offset == 0x04:  # ACCEL_STATUS (read-only, but allow writes for testing)
            print(f"[SNN_Accelerator] Status register write attempted: 0x{value:08x}")
        elif offset == 0x08:  # DMA_ADDR
            self.dma_addr_reg = value
            print(f"[SNN_Accelerator] DMA transfer configured: addr=0x{value:08x}")
        elif offset == 0x0C:  # DMA_LEN
            self.dma_len_reg = value
            print(f"[SNN_Accelerator] DMA transfer configured: len={value}")
        else:
            print(f"[SNN_Accelerator] Write to unknown offset 0x{offset:02x}: 0x{value:08x}")

    def Reset(self):
        self.ctrl_reg = 0
        self.status_reg = 0
        self.dma_addr_reg = 0
        self.dma_len_reg = 0
        self.operation_active = False
        print("[SNN_Accelerator] Reset")
"#;

    fs::write(out_dir.join("accelerator.py"), py_content)?;

    // Generate run.resc - Renode script to load and run simulation
    let resc_content = r#"mach create
machine LoadPlatformDescription @accelerator.repl

$bin?=@prog-rv64

macro reset
"""
    sysbus LoadELF $bin
"""

runMacro $reset

# Enable logging for our accelerator
logLevel 3 snn_accelerator

start
"#;

    fs::write(out_dir.join("run.resc"), resc_content)?;

    Ok(())
}

/// Run Renode simulation and capture output
fn run_renode_and_capture(exe: &Path, out_dir: &Path) -> Result<()> {
    // Check if renode is available
    let renode = detect_tool(&["renode"]).ok_or_else(|| anyhow::anyhow!("renode not found in PATH"))?;
    
    // Copy the executable to the expected name
    let prog_rv64 = out_dir.join("prog-rv64");
    if exe != prog_rv64.as_path() {
        fs::copy(exe, &prog_rv64).context("copy executable for renode")?;
    }

    // Run Renode with the script
    let resc_script = out_dir.join("run.resc");
    let output = Command::new(&renode)
        .arg("--disable-xwt")
        .arg("--console")
        .arg("--script")
        .arg(&resc_script)
        .arg("--execute")
        .arg("quit")
        .current_dir(out_dir)
        .output()
        .context("running renode")?;

    // Capture output to log file
    let log_path = out_dir.join("renode.log");
    fs::write(&log_path, &output.stdout)?;

    // Also capture stderr for debugging
    if !output.stderr.is_empty() {
        let stderr_path = out_dir.join("renode_stderr.log");
        fs::write(&stderr_path, &output.stderr)?;
    }

    // Extract JSONL telemetry from stdout if present
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let mut jsonl_lines = Vec::new();
    for line in stdout_str.lines() {
        if line.trim().starts_with("{") && line.contains("\"metric\"") {
            jsonl_lines.push(line);
        }
    }

    if !jsonl_lines.is_empty() {
        let dest = if let Ok(p) = std::env::var("NC_PROFILE_JSONL") {
            PathBuf::from(p)
        } else {
            out_dir.join("profile.jsonl")
        };
        fs::write(&dest, jsonl_lines.join("\n"))?;
    }

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
    fn pipeline_profile_bare_emit() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv32imac_bare.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv32imac_bare manifest");
        let g = nc_nir::Graph::new("bare");
        let artifact = compile(&g, &m).expect("compile ok");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
        assert!(out_dir.join("README.txt").exists(), "README expected for bare_metal profile");
        assert!(out_dir.join("main.c").exists(), "main.c expected for bare_metal profile");
        assert!(out_dir.join("crt0.S").exists(), "crt0.S expected for bare_metal profile");
        assert!(out_dir.join("linker.ld").exists(), "linker.ld expected for bare_metal profile");
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
    fn pipeline_profile_ctrl_emit() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv64gc_ctrl.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv64gc_ctrl manifest");
        let g = nc_nir::Graph::new("ctrl");
        let artifact = compile(&g, &m).expect("compile ok");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
        assert!(out_dir.join("README.txt").exists(), "README expected for control_plane profile");
        assert!(out_dir.join("main.c").exists(), "main.c should exist for control_plane profile");
        assert!(out_dir.join("accelerator.repl").exists(), "accelerator.repl should be generated");
        assert!(out_dir.join("accelerator.py").exists(), "accelerator.py should be generated");
        assert!(out_dir.join("run.resc").exists(), "run.resc should be generated");
        
        // Verify main.c contains control-plane specific content
        let main_c_content = fs::read_to_string(out_dir.join("main.c")).unwrap_or_default();
        assert!(main_c_content.contains("mmap"), "main.c should contain mmap for MMIO");
        assert!(main_c_content.contains("MMIO_BASE_ADDR"), "main.c should define MMIO_BASE_ADDR");
        
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
        emit_linux_rv64_runtime(&out_dir, &g, &m).unwrap();
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

    #[test]
    fn qemu_system_bare_metal_smoke_if_available() {
        // Guard on required tools
        if detect_tool(&["riscv64-unknown-elf-gcc"]).is_none() { return; }
        if detect_tool(&["qemu-system-riscv32"]).is_none() { return; }

        // Use riscv32imac_bare manifest
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv32imac_bare.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv32imac_bare manifest");
        let g = nc_nir::Graph::new("bmqemu");

        // Request QEMU run
        std::env::set_var("NC_RISCV_QEMU_RUN", "1");
        let artifact = compile(&g, &m).expect("compile ok");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
        let elf = out_dir.join("firmware.elf");
        assert!(elf.exists(), "expected ELF to be built at {elf:?}");

        // If profile.jsonl exists, validate content schema
        let jsonl = out_dir.join("profile.jsonl");
        if let Ok(content) = std::fs::read_to_string(&jsonl) {
            assert!(content.contains("\"backend\":\"riscv\""));
            assert!(content.contains("\"kernel.step_ns\""));
        }
    }

    #[cfg(feature = "riscv-v")]
    #[test]
    fn qemu_user_vector_smoke_if_available() {
        // Only run when a compatible toolchain exists
        if detect_tool(&["riscv64-linux-gnu-gcc"]).is_none() && detect_tool(&["clang"]).is_none() {
            return;
        }
        let g = nc_nir::Graph::new("gqemu_vec");
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
        emit_linux_rv64_runtime(&out_dir, &g, &m).unwrap();

        // Assert generated main.c contains the RVV guard so we know vector path was emitted
        let main_c_s = fs::read_to_string(out_dir.join("main.c")).unwrap_or_default();
        assert!(main_c_s.contains("__riscv_vector"), "main.c should contain RVV guard");
        // Optionally check header is referenced
        assert!(main_c_s.contains("riscv_vector.h"), "main.c should reference riscv_vector.h");
    }

    #[test]
    fn renode_control_plane_smoke_if_available() {
        // Skip if toolchain not present
        if detect_tool(&["riscv64-linux-gnu-gcc"]).is_none() && detect_tool(&["clang"]).is_none() {
            return;
        }
        // Skip if renode not present
        if detect_tool(&["renode"]).is_none() {
            return;
        }

        // Use the real riscv64gc_ctrl manifest
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_dir = PathBuf::from(manifest_dir);
        let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
        let path = ws_root.join("targets").join("riscv64gc_ctrl.toml");
        let m = nc_hal::parse_target_manifest_path(&path).expect("parse riscv64gc_ctrl manifest");
        let g = nc_nir::Graph::new("ctrl");

        // Request Renode run
        std::env::set_var("NC_RISCV_QEMU_RUN", "1");
        let artifact = compile(&g, &m).expect("compile ok");
        let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));

        // Verify control-plane artifacts were generated
        assert!(out_dir.join("main.c").exists(), "main.c should exist for control_plane profile");
        assert!(out_dir.join("accelerator.repl").exists(), "accelerator.repl should be generated");
        assert!(out_dir.join("accelerator.py").exists(), "accelerator.py should be generated");
        assert!(out_dir.join("run.resc").exists(), "run.resc should be generated");

        // Verify main.c contains MMIO operations
        let main_c_content = fs::read_to_string(out_dir.join("main.c")).unwrap_or_default();
        assert!(main_c_content.contains("mmap"), "main.c should contain mmap for MMIO access");
        assert!(main_c_content.contains("MMIO_BASE_ADDR"), "main.c should define MMIO_BASE_ADDR");
        assert!(main_c_content.contains("ACCEL_CTRL"), "main.c should define accelerator registers");

        // Verify Renode log was created (if simulation ran)
        let log_exists = out_dir.join("renode.log").exists();
        if log_exists {
            let log_content = fs::read_to_string(out_dir.join("renode.log")).unwrap_or_default();
            // Check for expected output from Python peripheral model
            assert!(
                log_content.contains("Accelerator control register written") ||
                log_content.contains("SNN_Accelerator"),
                "renode.log should contain accelerator peripheral output"
            );

            // If profile.jsonl exists, validate telemetry content
            let jsonl_path = out_dir.join("profile.jsonl");
            if jsonl_path.exists() {
                let jsonl_content = fs::read_to_string(&jsonl_path).unwrap_or_default();
                assert!(jsonl_content.contains("\"backend\":\"riscv\""), "JSONL should contain riscv backend");
                assert!(jsonl_content.contains("\"simulator\":\"renode\""), "JSONL should contain renode simulator");
                assert!(jsonl_content.contains("\"kernel.step_ns\""), "JSONL should contain timing metrics");
            }
        }
    }
}
