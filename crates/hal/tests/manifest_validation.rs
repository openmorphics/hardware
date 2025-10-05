
use nc_hal::{parse_target_manifest_str, validate_manifest};
use anyhow::Result;

#[test]
fn hal_weight_precisions_contains_zero() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "X"
        version = "1"
        [capabilities]
        weight_precisions = [0]
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("weight_precisions"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_vector_true_missing_vlen_bits_max() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        has_vector = true
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("vlen_bits_max"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_vector_true_zero_vlen_bits_max() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        has_vector = true
        vlen_bits_max = 0
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("vlen_bits_max"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_zvl_bits_min_gt_vlen_bits_max() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        vlen_bits_max = 128
        zvl_bits_min = 256
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("zvl_bits_min") && msg.contains("vlen_bits_max"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_zvl_or_vlen_not_multiple_of_8() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        vlen_bits_max = 64
        zvl_bits_min = 20
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("multiples of 8"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_mmio_supported_missing_base_addr() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        mmio_supported = true
        mmio_width_bits = 32
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("mmio_base_addr"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_mmio_supported_bad_width() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        mmio_supported = true
        mmio_base_addr = 1024
        mmio_width_bits = 16
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("mmio_width_bits") && (msg.contains("32") || msg.contains("64")), "msg={msg}");
    Ok(())
}

#[test]
fn hal_dma_alignment_not_power_of_two() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        dma_supported = true
        dma_alignment = 24
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("dma_alignment") && msg.contains("power-of-two"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_invalid_endianness() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "X"
        version = "1"
        [capabilities]
        endianness = "middle"
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("endianness") && (msg.contains("little") || msg.contains("big")), "msg={msg}");
    Ok(())
}

#[test]
fn hal_cacheline_bytes_invalid() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "X"
        version = "1"
        [capabilities]
        cacheline_bytes = 0
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("cacheline_bytes") && msg.contains("power-of-two"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_page_size_bytes_not_power_of_two() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "X"
        version = "1"
        [capabilities]
        page_size_bytes = 3000
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("page_size_bytes") && msg.contains("power-of-two"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_invalid_code_model() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        code_model = "large"
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("code_model"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_rv32_bad_abi_not_ilp32() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        isa = "rv32imac"
        abi = "lp64"
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("ilp32"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_rv64_bad_abi_not_lp64() -> Result<()> {
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        isa = "rv64gc"
        abi = "ilp32"
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("lp64"), "msg={msg}");
    Ok(())
}

#[test]
fn hal_vector_true_but_isa_lacks_v() -> Result<()> {
    // Provide vlen_bits_max to bypass earlier vector-length check and trigger the 'isa lacks v' error.
    let s = r#"
        name = "t"
        vendor = "v"
        family = "RISC-V"
        version = "1"
        [capabilities]
        has_vector = true
        vlen_bits_max = 128
        isa = "r64gc"
    "#;
    let m = parse_target_manifest_str(s)?;
    let err = validate_manifest(&m).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("has_vector") && msg.contains("isa") && msg.contains("'v'"), "msg={msg}");
    Ok(())
}
