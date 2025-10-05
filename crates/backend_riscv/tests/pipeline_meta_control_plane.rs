use anyhow::Result;
use std::path::PathBuf;
use std::fs;

#[test]
fn pipeline_meta_control_plane() -> Result<()> {
    // Disable external runners/tools
    std::env::set_var("NC_RISCV_QEMU_RUN", "0");

    // Workspace root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("workspace root");

    // Load example NIR graph
    let nir_path = ws_root.join("examples").join("nir").join("simple.json");
    let nir = fs::read_to_string(&nir_path)?;
    let g = nc_nir::Graph::from_json_str(&nir)?;

    // Load RISC-V control_plane manifest
    let manifest_path = ws_root.join("targets").join("riscv64gc_ctrl.toml");
    let m = nc_hal::parse_target_manifest_path(&manifest_path)?;

    // Compile and normalize artifact path
    let artifact = nc_backend_riscv::compile(&g, &m)?;
    let out_dir = PathBuf::from(artifact.trim_start_matches("artifact:"));
    assert!(out_dir.is_dir(), "artifact dir missing: {out_dir:?}");

    // Verify passes directory JSON dumps exist
    let passes = out_dir.join("passes");
    assert!(passes.is_dir(), "passes dir missing: {passes:?}");
    let mut has_json = false;
    if let Ok(rd) = fs::read_dir(&passes) {
        for e in rd.flatten() {
            if e.path().extension().and_then(|s| s.to_str()) == Some("json") {
                has_json = true;
                break;
            }
        }
    }
    assert!(has_json, "no JSON dumps found in passes dir: {passes:?}");

    // README.txt should exist unconditionally for control_plane profile
    let readme_path = out_dir.join("README.txt");
    let readme = fs::read_to_string(&readme_path).unwrap_or_default();
    if readme.is_empty() {
        // Accept WARN.txt existence but surface for debugging if README is empty
        let warn_path = out_dir.join("WARN.txt");
        let warn = fs::read_to_string(&warn_path).unwrap_or_default();
        panic!("README.txt missing or empty for control_plane.\nWARN.txt:\n{warn}");
    }

    // Presence-only checks (values not strictly asserted)
    assert!(readme.contains("mmio_supported="), "README missing mmio_supported=: \n{readme}");
    assert!(readme.contains("dma_supported="), "README missing dma_supported=: \n{readme}");

    // Do not fail if WARN.txt exists (acceptable when toolchains are missing)
    Ok(())
}