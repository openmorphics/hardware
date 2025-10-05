# NIR (Neuromorphic Intermediate Representation)

Status: v0.1-draft
Audience: compiler/backend authors, frontend importers, tooling integrators.

Purpose
- Provide a stable, deterministic IR to represent spiking/rate/hybrid neural workloads.
- Serve as the contract between frontends, the orchestrator/partitioner, compiler passes, and hardware backends.

Core concepts
- Network: top-level container with global metadata (name, version, dt, units).
- Population: homogeneous group of neurons with shared neuron_type and parameter schema.
  - Fields: id (string), size (u32), neuron_type (string), params (object), resource_hints (optional).
- Projection: directed connectivity between a source and a destination population.
  - Fields: id (string), src (population id), dst (population id), connectivity (dense|sparse), weights, delays, plasticity, transmission (spike|rate|analog), params (object).
  - Connectivity:
    - dense: shape [dst.size, src.size] (row-major dst×src) or compact block specs
    - sparse: coordinate list with (dst_index, src_index, w[, delay]) sorted by dst_index then src_index
- Signal: the value kind carried along projections (e.g., spike, rate, analog current).

Types and shapes
- Scalar types: i8, i16, i32, u8, u16, u32, f16, bf16, f32.
- Shapes: [] (scalar), [N], [N,M]; all shapes are explicit, row-major unless declared otherwise.
- Quantization:
  - Per-field quant: { scheme: affine|symmetric|power_of_two, scale: f32, zero_point: i32, bitwidth: 4|8|16 }
  - Quant scopes: population.params.*, projection.weights, projection.params.* (explicitly declared).
- Units:
  - time: seconds (s); dt (global) is a positive f32 in seconds; delays are non-negative integer ticks where delay_ticks = round(delay_seconds/dt).

Timing and scheduling semantics
- Discrete-time global step of size dt. At step t:
  1) Populations update intrinsic state given inputs scheduled for t (post-delay).
  2) Emitted outputs are staged on projections with their per-edge/per-projection delay.
  3) Plasticity updates (if enabled) applied after state/output update.
- Hybrid (event + sampled) allowed by declaring transmission=spike|rate|analog per projection.
- Determinism requirements:
  - Given fixed inputs, params, and seed, all reductions must be ordered; any parallel reductions must be made deterministic (e.g., sort indices, Kahan sum or fixed order).

Resources and placement hints (optional)
- resource_hints on populations: { memory_estimate_bytes, compute_class: "cpu"|"gpu"|"neuro", colocate_with: [pop_id], shard_preference: "split_src"|"split_dst"|null }.
- resource_hints on projections: { prefer_on_chip: bool, fanin_limit: u32, route_budget: u32 }.

Serialization
- Format: JSON (normative) and YAML (informative). All ids are strings; enums are lower_snake_case.
- Top-level object:
  - version: "0.1"
  - dt: f32 seconds
  - populations: [Population]
  - projections: [Projection]
  - metadata: object (free-form)
- Example (truncated):
  ```json
  {
    "version": "0.1",
    "dt": 0.001,
    "populations": [
      { "id": "input", "size": 1024, "neuron_type": "source", "params": {} },
      { "id": "l1", "size": 2048, "neuron_type": "lif", "params": { "tau_m": 0.02, "v_th": 1.0 } }
    ],
    "projections": [
      {
        "id": "p_in_l1",
        "src": "input",
        "dst": "l1",
        "connectivity": "sparse",
        "transmission": "spike",
        "weights": { "type": "f16", "layout": "csr", "values": [[0,1,0.5],[1,7,-0.2]] },
        "delays": { "ticks": 1 },
        "plasticity": { "rule": "static" },
        "params": {}
      }
    ],
    "metadata": { "name": "toy" }
  }
  ```
- Field naming is stable; additions must be backward compatible (new optional fields).

Validation invariants
- Unique ids across populations and projections.
- Projections reference existing populations.
- Shapes and dtypes match declared neuron/projection schemas.
- dt > 0, delay_ticks ≥ 0 and within backend-declared limits when known.
- No negative weights if the backend backend_disallows_negative_weights=true (when declared later in mapping).
- Quantization metadata must match tensor dtypes (e.g., 8-bit quant implies i8/u8 storage).
- Deterministic ordering: sparse coordinates are sorted by (dst, src).

Error taxonomy (prefix: NIR_E*)
- NIR_E001 MissingPopulationRef
- NIR_E002 InvalidShapeOrDType
- NIR_E003 DelayOutOfRange
- NIR_E004 QuantizationMismatch
- NIR_E005 NonDeterministicReduction
- NIR_E006 UnsupportedTransmissionMode
- NIR_E007 PlasticityRuleUnsupported

Versioning and compatibility
- Document-level: version string (semantic: MAJOR.MINOR).
- Field-level: new optional fields permitted; removals or semantic changes require MAJOR bump.

Pass boundaries (for compiler authors)
- Consumers should rely on the contracts defined in [passes.md](passes.md) for:
  - Lowering to kernel-level ops
  - Memory layout and quantization decisions
  - Fusion and scheduling artifacts
- Backends must publish capability constraints in [backends.md](backends.md) and ensure mapping is total or produce actionable diagnostics.

Conformance and determinism tests
- Provide golden JSON/YAML roundtrips; enforce sorting of sparse coords; seed must produce the same kernel order and schedule when combined with pass pipeline configuration.
