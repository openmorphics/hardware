//! Orchestrator graph metrics over NIR graphs.
//! Computes simple structural metrics used by partition planning and tests.

use crate::nir;
use std::collections::HashMap;

/// Structural metrics over a NIR graph (population-level).
#[derive(Debug, Clone, PartialEq)]
pub struct GraphMetrics {
    /// Number of populations (nodes).
    pub node_count: usize,
    /// Number of projections (edges).
    pub edge_count: usize,
    /// Average fan-in over populations (sum_in / node_count).
    pub avg_fanin: f64,
    /// Average fan-out over populations (sum_out / node_count).
    pub avg_fanout: f64,
    /// Maximum fan-in across populations.
    pub max_fanin: usize,
    /// Maximum fan-out across populations.
    pub max_fanout: usize,
}

/// Compute structural metrics from a NIR graph.
///
/// Semantics:
/// - Nodes correspond to populations.
/// - Edges correspond to connections (projections).
/// - Fan-in/out counts are at the population granularity.
pub fn compute_metrics(g: &nir::Graph) -> GraphMetrics {
    let node_count = g.populations.len();
    let edge_count = g.connections.len();

    // Initialize maps with all population names to ensure zeros are counted.
    let mut fan_in: HashMap<&str, usize> = HashMap::with_capacity(node_count);
    let mut fan_out: HashMap<&str, usize> = HashMap::with_capacity(node_count);
    for p in &g.populations {
        fan_in.insert(p.name.as_str(), 0);
        fan_out.insert(p.name.as_str(), 0);
    }

    for c in &g.connections {
        if let Some(x) = fan_out.get_mut(c.pre.as_str()) {
            *x += 1;
        }
        if let Some(x) = fan_in.get_mut(c.post.as_str()) {
            *x += 1;
        }
    }

    let mut sum_in: usize = 0;
    let mut sum_out: usize = 0;
    let mut max_in: usize = 0;
    let mut max_out: usize = 0;

    for p in &g.populations {
        let fi = *fan_in.get(p.name.as_str()).unwrap_or(&0);
        let fo = *fan_out.get(p.name.as_str()).unwrap_or(&0);
        sum_in += fi;
        sum_out += fo;
        if fi > max_in {
            max_in = fi;
        }
        if fo > max_out {
            max_out = fo;
        }
    }

    let denom = node_count.max(1) as f64; // avoid div-by-zero
    GraphMetrics {
        node_count,
        edge_count,
        avg_fanin: (sum_in as f64) / denom,
        avg_fanout: (sum_out as f64) / denom,
        max_fanin: max_in,
        max_fanout: max_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn metrics_on_chain_three_nodes() {
        // Build a simple chain: p0 -> p1 -> p2
        let g = nir::fixtures::chain(&[10, 20, 30]);
        let m = compute_metrics(&g);
        assert_eq!(m.node_count, 3);
        assert_eq!(m.edge_count, 2);
        // In a chain of N nodes, sum of fanin = edges = N-1; avg = (N-1)/N
        assert!(approx_eq(m.avg_fanin, 2.0 / 3.0));
        assert!(approx_eq(m.avg_fanout, 2.0 / 3.0));
        assert_eq!(m.max_fanin, 1);
        assert_eq!(m.max_fanout, 1);
    }

    #[test]
    fn metrics_on_star_center_to_spokes() {
        // center -> s0..s4
        let g = nir::fixtures::star(32, 8, 5, 0.5, 1.0);
        let m = compute_metrics(&g);
        assert_eq!(m.node_count, 6); // 1 center + 5 spokes
        assert_eq!(m.edge_count, 5);
        // Sum of fanin = edges (all at spokes); avg = edges/nodes
        assert!(approx_eq(m.avg_fanin, 5.0 / 6.0));
        // Fanout concentrated at center: max_fanout = 5
        assert_eq!(m.max_fanout, 5);
        // Max fanin is 1 (each spoke has exactly one incoming)
        assert_eq!(m.max_fanin, 1);
    }

    #[test]
    fn metrics_on_empty_graph_is_safe() {
        let g = nir::Graph::new("empty");
        let m = compute_metrics(&g);
        assert_eq!(m.node_count, 0);
        assert_eq!(m.edge_count, 0);
        assert!(approx_eq(m.avg_fanin, 0.0));
        assert!(approx_eq(m.avg_fanout, 0.0));
        assert_eq!(m.max_fanin, 0);
        assert_eq!(m.max_fanout, 0);
    }
}