use anyhow::{bail, Result};

/// Build-time feature gate check for MLIR integration.
pub fn is_enabled() -> bool { cfg!(feature = "mlir") }

/// Lower a NIR graph to a minimal textual MLIR-like representation.
/// This is a stub intended to be replaced with a true MLIR dialect.
pub fn lower_to_mlir(g: &nc_nir::Graph) -> Result<String> {
    if !is_enabled() {
        bail!("mlir feature is disabled; build with feature 'mlir'");
    }
    let mut out = String::new();
    out.push_str(&format!("module @{} attributes {{nir_version = \"{}\"}} {{\n", g.name, nc_nir::VERSION));
    for p in &g.populations {
        out.push_str(&format!(
            "  %{} = \"nir.population\"() {{name = \"{}\", size = {}}} : () -> none\n",
            p.name, p.name, p.size
        ));
    }
    for c in &g.connections {
        out.push_str(&format!(
            "  \"nir.connect\"() {{pre = \"{}\", post = \"{}\", weight = {:.6}, delay_ms = {:.6}}} : () -> none\n",
            c.pre, c.post, c.weight, c.delay_ms
        ));
    }
    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_reports_status() {
        // Should compile regardless of whether the feature is enabled.
        let _ = is_enabled();
    }

    #[cfg(feature = "mlir")]
    #[test]
    fn lower_stub_compiles() {
        let mut g = nc_nir::Graph::new("t");
        g.populations.push(nc_nir::Population {
            name: "a".into(),
            size: 1,
            model: "LIF".into(),
            params: serde_json::json!({}),
        });
        let s = lower_to_mlir(&g).unwrap();
        assert!(s.contains("module @t"));
        assert!(s.contains("nir.population"));
    }
}
