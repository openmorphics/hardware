# Backend Authoring Guide

This document is a practical guide for adding or extending a hardware backend in the universal neuromorphic compiler.

References (code)
- IR (NIR): [crates/nir/src/lib.rs](crates/nir/src/lib.rs)
- HAL (manifests/capabilities): [crates/hal/src/lib.rs](crates/hal/src/lib.rs)
- Passes and pipeline: [crates/passes/src/lib.rs](crates/passes/src/lib.rs)
- Example backend crates:
  - Loihi: [crates/backend_loihi/src/lib.rs](crates/backend_loihi/src/lib.rs)
  - TrueNorth: [crates/backend_truenorth/src/lib.rs](crates/backend_truenorth/src/lib.rs)
  - DYNAPs: [crates/backend_dynaps/src/lib.rs](crates/backend_dynaps/src/lib.rs)
  - Akida: [crates/backend_akida/src/lib.rs](crates/backend_akida/src/lib.rs)
  - SpiNNaker: [crates/backend_spinnaker/src/lib.rs](crates/backend_spinnaker/src/lib.rs)
  - Neurogrid: [crates/backend_neurogrid/src/lib.rs](crates/backend_neurogrid/src/lib.rs)
  - MemXbar: [crates/backend_memxbar/src/lib.rs](crates/backend_memxbar/src/lib.rs)
  - Custom ASIC: [crates/backend_custom_asic/src/lib.rs](crates/backend_custom_asic/src/lib.rs)
- CLI integration (compile/manifest loading): [crates/cli/src/main.rs](crates/cli/src/main.rs)
- Telemetry (timers/counters/labels): [crates/telemetry/src/lib.rs](crates/telemetry/src/lib.rs)
- Label schema: [docs/metrics/labels.md](docs/metrics/labels.md)
- Pipeline tutorial: [docs/tutorials/pipeline.md](docs/tutorials/pipeline.md)

## 1. Backend responsibilities

A backend provides a unified compile entrypoint that takes:
- A validated NIR graph (with version tag ensured).
- A target manifest (HAL) describing hardware capabilities.

It produces an artifact (string path, JSON blob, or other structured artifact) representing a hardware-ready configuration, or a preliminary artifact in early stages.

Interface (pattern used across backends)
```rust
pub fn compile(g: &nc_nir::Graph, manifest: &nc_hal::TargetManifest) -> anyhow::Result<String> { /* ... */ }
```

Build-time feature flags gate each backend so users can produce minimal builds (see the CLI and Python crates for mapping features to optional dependencies).

## 2. HAL-driven compilation

A target manifest captures constraints and modeling hints:

Key capabilities (subset)
- max_neurons_per_core, max_synapses_per_core
- max_fan_in, max_fan_out
- core_memory_kib, interconnect_bandwidth_mbps
- time_resolution_ns
- neuron_mem_kib_per, syn_mem_kib_per
- bytes_per_event, default_spike_rate_hz
- analog and on_chip_plasticity_rules (booleans/lists describing support)

Authoring workflow:
1) Define/extend a manifest file under [targets/](targets) (e.g., targets/my_chip.toml).
2) Ensure it parses/validates with [crates/hal/src/lib.rs](crates/hal/src/lib.rs).
3) In your backend compile(), interpret these fields to decide mapping strategies:
   - Quantization modes and weight formats.
   - Partitioning/placement limits (if not already enforced by earlier passes).
   - Routing/bandwidth checks specific to your interconnect.
   - Timing discretization (ticks) based on time_resolution_ns.

Backends should trust earlier capability-aware passes for coarse feasibility (partition/placement/routing/resource-check), but must still validate critical assumptions and emit meaningful diagnostics.

## 3. Mapping stages (typical)

Although much of the mapping is handled by passes, many backends still need target-specific work:

- Neuron model mapping:
  - Map NIR neuron models (e.g., "LIF") to the backend’s hardware neuron configurations and parameter encodings.
  - Handle parameter quantization/ranges.
- Synapse/weight encoding:
  - Apply bit-width quantization; pack formats per backend requirements.
  - Respect sign/scale conventions and sparsity structures if supported.
- Memory layout:
  - Allocate core memory regions for neuron/synapse states based on neuron_mem_kib_per and syn_mem_kib_per, validated against core_memory_kib.
- Routing artifacts:
  - Transform inter-part links into physical routes (if required by your artifact format) or use pass outputs as hints.
- Timing:
  - Convert connection delays to hardware ticks using time_resolution_ns; ensure bounds/rounding are legal.

## 4. Artifacts

Backends should emit artifacts in a stable form. Recommended patterns:
- A top-level directory with:
  - manifest.json (summary: target, graph, resource usage, version)
  - mapping.json or mapping.bin (encoded network for the device)
  - README.txt with version/CLI used and quick-start instructions

Return the artifact directory or primary file path from compile() so the caller prints a succinct “compile ok: …” message.

## 5. Telemetry integration

Standardize labels to keep metrics queryable:
- Use telemetry timers to measure compile duration:
  ```rust
  #[cfg(feature = "telemetry")]
  let timer = app.as_ref().map(|a| a.start_timer("backend.compile_ms",
      nc_telemetry::labels::backend(&g.name, "my_backend", Some(&manifest.name))));
  ```
- Emit counters for graph cardinalities:
  ```rust
  #[cfg(feature = "telemetry")]
  if let Some(a) = &app {
      let l = nc_telemetry::labels::backend(&g.name, "my_backend", Some(&manifest.name));
      let _ = a.counter("graph.populations", g.populations.len() as f64, l.clone());
      let _ = a.counter("graph.connections", g.connections.len() as f64, l.clone());
      let _ = a.counter("graph.probes", g.probes.len() as f64, l);
  }
  ```
- See [docs/metrics/labels.md](docs/metrics/labels.md) and [crates/telemetry/src/lib.rs](crates/telemetry/src/lib.rs) for helpers.

Keep label values low-cardinality (graph, backend, target) and stable.

## 6. CLI and Python integration

CLI:
- The compile subcommand loads the manifest from targets/<name>.toml and routes to the selected backend when its feature is enabled. See [crates/cli/src/main.rs](crates/cli/src/main.rs) for the pattern.

Python:
- Python bindings provide compile_nir_json_py/compile_nir_yaml_py, plus auto-detect compile_nir_str_py. See [crates/py/src/lib.rs](crates/py/src/lib.rs) and [docs/python/usage.md](docs/python/usage.md).
- Build wheels with backend features enabled to expose those backends to Python.

## 7. Testing

Unit tests:
- Add smoke tests under your backend crate (e.g., “compile_smoke”) using a tiny NIR graph.

CLI E2E:
- Extend [crates/cli/tests/e2e.rs](crates/cli/tests/e2e.rs) to invoke “neuro-compiler compile --input … --target my_target”.

Feature-matrix CI:
- Ensure your backend compiles in the matrix (add a matrix case if warranted).

## 8. Error handling and diagnostics

Use anyhow/thiserror for structured diagnostics:
- Validate invariants early; map violations to human-readable errors.
- Prefer specific messages (“MAX_FAN_IN_EXCEEDED for population X”) over generic failures.
- Keep hardware detail in artifacts rather than printing large blobs to stdout.

## 9. Adding a new backend — checklist

1) Create a new crate under crates/backend_my_backend with Cargo metadata and a feature flag in the CLI/Python crates.
2) Implement compile(&Graph, &TargetManifest) and wire telemetry under feature flags.
3) Add a target manifest to [targets/](targets) and ensure it validates.
4) Add unit tests for compile_smoke and (optionally) resource validation behavior.
5) Integrate with CLI: add a match arm in compile subcommand (feature-gated).
6) Integrate with Python: add crate as optional dependency in [crates/py/Cargo.toml](crates/py/Cargo.toml) and ensure wrappers error clearly when feature is disabled.
7) Update docs (this guide + target-specific notes if needed).
8) Add CI coverage in the feature matrix if the backend warrants a dedicated lane.

## 10. Advanced topics

- Multi-chip / partition mapping glue:
  - Use pass outputs to decide chip boundaries and resource allocations; orchestrator crate can coordinate multi-target splits.
- Learned mapping:
  - Integrate cost models via mlopt (latency/energy predictors, search strategies). See [crates/mlopt/src/lib.rs](crates/mlopt/src/lib.rs).
- MLIR interop:
  - Gate the MLIR exporter and parse/emit your backend as an MLIR dialect later (optional).

---

With these guidelines, you can extend the compiler to support new neuromorphic systems while reusing existing NIR/HAL abstractions, passes, and telemetry infrastructure. Keep backend behavior capability-aware and strive for consistent artifacts and diagnostics so CLI and Python consumers have a smooth experience.