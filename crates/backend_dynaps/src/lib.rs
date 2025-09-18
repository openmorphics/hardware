use anyhow::Result;

fn quantize_weight(w: f32, bits: u32) -> f32 {
    let levels: u32 = if bits >= 31 { u32::MAX } else { 1u32 << bits };
    let l_minus_1 = (levels.saturating_sub(1)) as f32;
    let l_minus_1 = if l_minus_1 <= 0.0 { 1.0 } else { l_minus_1 };
    let w_clamped = w.clamp(-1.0, 1.0);
    let step = 2.0 / l_minus_1;
    ((w_clamped + 1.0) / step).round() * step - 1.0
}

pub fn compile(graph: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> Result<String> {
    graph.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    nc_hal::validate_manifest(manifest)?;

    let bits = manifest
        .capabilities
        .as_ref()
        .and_then(|c| c.weight_precisions.as_ref())
        .and_then(|v| v.iter().max().copied())
        .unwrap_or(8);

    let conns: Vec<serde_json::Value> = graph
        .connections
        .iter()
        .map(|c| {
            let q = quantize_weight(c.weight, bits);
            serde_json::json!({
                "pre": c.pre,
                "post": c.post,
                "weight_q": q,
                "bits": bits
            })
        })
        .collect();

    let obj = serde_json::json!({
        "target": manifest.name,
        "graph": graph.name,
        "connections": conns
    });
    Ok(serde_json::to_string_pretty(&obj)?)
}
