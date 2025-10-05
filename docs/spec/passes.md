# Pass Catalog

Status: v0.1-draft
Intent: Define deterministic, composable passes operating over NIR, producing artifacts required by hardware backends.

Guiding principles
- Deterministic output given fixed inputs and seeds
- Validate early, diagnose precisely, and preserve debuggability via stable textual dumps
- Make each pass responsible for a single concern; use an explicit pipeline descriptor

Shared interfaces
- Input: NIR (as specified in [nir.md](nir.md))
- Output: either annotated NIR (preferred for early passes) or pass-specific sidecar artifacts (JSON) colocated under target/ or test fixtures
- Diagnostics: machine-parseable codes plus human-friendly messages
- Seeds: all randomized heuristics receive an explicit seed to ensure reproducibility

Passes

1) ValidateNIR (optional but recommended)
- Goal: ensure NIR invariants (ids, shapes, dtypes, delays, quant metadata)
- Output: success/failure with error list (NIR_E*)
- Invariants checked: see [nir.md](nir.md) Validation invariants
- Artifacts: validation report (JSON) with error codes and locations

2) LowerToKernels
- Goal: map high-level NIR entities into backend-agnostic kernel ops with explicit inputs/outputs/params
- Input: NIR
- Output: NIR + kernels metadata:
  - kernels: [{ id, op_type, inputs, outputs, params, device_class }]
  - mapping: population/projection ids → kernel ids
- Invariants:
  - No implicit type conversions; all casts are explicit
  - Deterministic ordering of kernels (topological, then stable tie-break)
- Artifacts: kernels.json (golden-able), textual dumps for diffing

3) MemoryLayoutAndQuant
- Goal: assign concrete layouts (row-major/CSR/etc), memory regions (on/off-chip), and finalize quantization
- Input: NIR + kernels metadata
- Output: annotations:
  - tensor_layouts: tensor_id → { layout, strides, region }
  - quant_finals: tensor_id → { scheme, scale, zero_point, bitwidth }
- Invariants:
  - All tensors are assigned exactly one region and layout
  - Quant metadata must match storage dtype
- Artifacts: layout.json, quant.json; golden snapshots required

4) KernelFusionAndScheduling
- Goal: (conservative) fuse kernels to reduce traffic and produce a legal execution schedule
- Input: kernels + layout/quant annotations
- Output: schedule: [{ time_slot, kernel_id(s), constraints }], fusion_groups, and resource_usage summary
- Invariants:
  - Resource budgets not exceeded; no cycles; obey delay semantics from [nir.md](nir.md)
  - Deterministic ordering within time_slot and across slots
- Artifacts: schedule.json; perf counters (if simulated)

Pipeline descriptor (YAML example)
```yaml
version: 0.1
seed: 42
passes:
  - validate_nir
  - lower_to_kernels
  - memory_layout_and_quant
  - kernel_fusion_and_scheduling
artifacts:
  dir: target/pipeline-out
  keep_text_dumps: true
```

Diagnostics and metrics
- All passes emit structured diagnostics with codes, severity, and source locations
- Each pass records runtime, kernel counts, memory footprints; exported as JSON lines for profiling

Backends
- Backends must publish their capability constraints and mapping expectations in [backends.md](backends.md)
