# Universal Neuromorphic Compiler — Architecture Overview

This document provides a technical overview of the core components, data flows, and extension points of the compiler.

Key code locations
- IR (NIR): [crates/nir/src/lib.rs](crates/nir/src/lib.rs)
- HAL (Target Manifests + Capabilities): [crates/hal/src/lib.rs](crates/hal/src/lib.rs), [targets/](targets)
- Pass framework and pipelines: [crates/passes/src/lib.rs](crates/passes/src/lib.rs)
- Backends: 
  - Loihi: [crates/backend_loihi/src/lib.rs](crates/backend_loihi/src/lib.rs)
  - TrueNorth: [crates/backend_truenorth/src/lib.rs](crates/backend_truenorth/src/lib.rs)
  - Akida: [crates/backend_akida/src/lib.rs](crates/backend_akida/src/lib.rs)
  - SpiNNaker: [crates/backend_spinnaker/src/lib.rs](crates/backend_spinnaker/src/lib.rs)
  - Neurogrid: [crates/backend_neurogrid/src/lib.rs](crates/backend_neurogrid/src/lib.rs)
  - DYNAPs: [crates/backend_dynaps/src/lib.rs](crates/backend_dynaps/src/lib.rs)
  - MemXbar: [crates/backend_memxbar/src/lib.rs](crates/backend_memxbar/src/lib.rs)
  - Custom ASIC: [crates/backend_custom_asic/src/lib.rs](crates/backend_custom_asic/src/lib.rs)
- Simulators: 
  - NEURON: [crates/sim_neuron/src/lib.rs](crates/sim_neuron/src/lib.rs)
  - CoreNEURON: [crates/sim_coreneuron/src/lib.rs](crates/sim_coreneuron/src/lib.rs)
  - Arbor: [crates/sim_arbor/src/lib.rs](crates/sim_arbor/src/lib.rs)
  - HW-specific test adapter: [crates/sim_hw_specific/src/lib.rs](crates/sim_hw_specific/src/lib.rs)
- Telemetry: [crates/telemetry/src/lib.rs](crates/telemetry/src/lib.rs)
- CLI: [crates/cli/src/main.rs](crates/cli/src/main.rs), E2E tests: [crates/cli/tests/e2e.rs](crates/cli/tests/e2e.rs)
- Python bindings: [crates/py/src/lib.rs](crates/py/src/lib.rs), usage doc: [docs/python/usage.md](docs/python/usage.md)
- Orchestrator/Runtime/ML-Opt: [crates/orchestrator/src/lib.rs](crates/orchestrator/src/lib.rs), [crates/runtime/src/lib.rs](crates/runtime/src/lib.rs), [crates/mlopt/src/lib.rs](crates/mlopt/src/lib.rs)
- MLIR bridge (optional): [crates/mlir-bridge/src/lib.rs](crates/mlir-bridge/src/lib.rs)
- Telemetry docs: [docs/metrics/profiling.md](docs/metrics/profiling.md), [docs/metrics/labels.md](docs/metrics/labels.md)
- Error taxonomy (spec): [docs/spec/errors.md](docs/spec/errors.md)

## 1. Core Concepts

### 1.1 NIR (Neuromorphic IR)
- Purpose: Unified, target-independent representation of Spiking Neural Networks (SNNs).
- Encodes: 
  - Graph name, populations (name, size, neuron model, params), connections (pre/post, weight, delay, plasticity), probes.
  - Attributes for cross-cutting metadata (e.g., attachment of HAL manifest path).
- Formats: JSON, YAML (serde-backed); optional binary for high-throughput dumps.
- Validation: structural checks (well-formed population/connection references, etc.)
- See: [crates/nir/src/lib.rs](crates/nir/src/lib.rs)

### 1.2 HAL (Hardware Abstraction Layer)
- Purpose: Normalize target hardware capabilities and constraints via TOML manifests.
- Manifests specify:
  - Memory limits, neurons/synapses per core, fan-in/out limits, interconnect bandwidth, time resolution.
  - Modeling hints (bytes/spike, default spike rate, memory per-neuron/synapse).
- Loader + validation: [crates/hal/src/lib.rs](crates/hal/src/lib.rs), and manifests under [targets/](targets).

## 2. Pipeline and Passes

### 2.1 Pass Framework
- Pass trait with name() and run() over NIR graphs.
- PassManager builds and executes pipelines; optional dumps per-pass (JSON/YAML/Bin).
- Capability-aware passes can read HAL metadata via an attribute (e.g., "hal_manifest_path").

### 2.2 Built-in Passes (initial set)
- validate: NIR validation.
- quantize: uniform symmetric quantization of weights.
- partition: partition graph respecting coarse capacity hints.
- placement: assign partitions to resources, estimate memory usage, check violations.
- routing: estimate interconnect demands and congestion.
- timing: translate delays to ticks by time resolution.
- resource-check: collect and report violations against HAL caps.

See: [crates/passes/src/lib.rs](crates/passes/src/lib.rs)

### 2.3 Partitioning constraints (NIR-aligned)

Goals
- Balance work across partitions, respect capacity/resource hints, minimize inter-partition traffic, and preserve determinism.

Inputs (from NIR and HAL)
- NIR: populations (size, neuron_type/params), projections (connectivity, weights, delays), optional resource_hints (colocate_with, shard_preference, memory_estimate_bytes).
- HAL/targets: capacity limits, memory topology, delay/timing constraints (dt, max delay ticks), fan-in/out limits.

Outputs
- PartitionPlan: number of parts and stable mapping (population/projection → part) emitted to downstream passes.

Determinism
- For identical inputs and a fixed seed, the partition result must be stable (no non-deterministic iteration orders).

References
- Orchestrator API and default planner location: [lib.rs](crates/orchestrator/src/lib.rs:1)
- IR semantics and hints: [nir.md](../spec/nir.md)
- Pass pipeline interactions: [passes.md](../spec/passes.md)

## 3. Backends and Simulators

### 3.1 Backends
- compile(&NIR, &Manifest) → artifact string or structured output.
- Emit standardized telemetry (when enabled) for compile durations and cardinalities.
- Targets include Loihi, TrueNorth, Akida, SpiNNaker, Neurogrid, DYNAPs, memristive crossbars, and custom ASICs (unified compile interface).
- See backend crates listed above.

### 3.2 Simulators
- emit_artifacts(&NIR, out_dir): write runnable artifacts (e.g., RUN.txt, model_summary.json).
- Supported surfaces: NEURON, CoreNEURON, Arbor; a minimal HW-specific test adapter is included.
- Telemetry timers/counters integrated behind features.
- See simulator crates listed above.

## 4. Telemetry and Profiling

- JSONL profiling (Appender) provides timers (duration on drop) and counters; standardized label schema across graph, backend, target, simulator, pass.
- Optional OTLP exporter via feature gate; graceful shutdown hook.
- CLI/Python wrappers can enable/route telemetry based on features and environment.
- Docs: [docs/metrics/profiling.md](docs/metrics/profiling.md), [docs/metrics/labels.md](docs/metrics/labels.md)
- Code: [crates/telemetry/src/lib.rs](crates/telemetry/src/lib.rs)

## 5. CLI and Python bindings

### 5.1 CLI
- Subcommands: list-targets, import, lower, compile, simulate, profile, package, deploy, export-mlir.
- Simulate and lower integrate HAL-aware pipelines and emit artifacts; telemetry optional.
- See: [crates/cli/src/main.rs](crates/cli/src/main.rs)

### 5.2 Python
- PyO3-based bindings expose list_targets, import, compile, simulate, profile summary.
- Auto-detect helpers: compile_nir_str_py(), simulate_nir_str_py() accept either JSON or YAML strings.
- Wheel build matrix in CI across OS and Python versions; optional features select backends, sims, telemetry.
- Docs: [docs/python/usage.md](docs/python/usage.md)
- Code: [crates/py/src/lib.rs](crates/py/src/lib.rs)
- CI: [.github/workflows/ci.yml](.github/workflows/ci.yml)

## 6. Optional MLIR Bridge

- When feature "mlir" is enabled, NIR can be exported/lowered to MLIR for interoperability or future IR evolution.
- Code: [crates/mlir-bridge/src/lib.rs](crates/mlir-bridge/src/lib.rs)

## 7. Orchestrator, Runtime, and ML-Optimizations

- Orchestrator: high-level partitioning coordination across targets/multi-chip (telemetry-labeled timers/counters).
  - [crates/orchestrator/src/lib.rs](crates/orchestrator/src/lib.rs)
- Runtime: deploy/start/stop/status stubs; integration points for on-device or cluster control.
  - [crates/runtime/src/lib.rs](crates/runtime/src/lib.rs)
- ML Optimization (mlopt): place-holder cost models and search strategies with telemetry hooks.
  - [crates/mlopt/src/lib.rs](crates/mlopt/src/lib.rs)

## 8. Error Handling and Diagnostics

- Work in progress: unify errors across crates using consistent thiserror enums and structured diagnostics.
- CLI maps internal errors to stable exit codes; information-rich messages remain on stderr without exposing internals.
- Spec: [docs/spec/errors.md](docs/spec/errors.md)

## 9. Build & Test

- Feature matrix CI covers:
  - Minimal, all-surfaces, and per-backend builds.
  - Wheels across OSes, 3.8–3.12, with smoke-test import.
- Local dev:
  - Lints: `cargo clippy --workspace --all-targets -- -D warnings`
  - Tests: `cargo test --workspace`
- CI workflow: [.github/workflows/ci.yml](.github/workflows/ci.yml)

## 10. Extensibility Guidelines

- Add new backend:
  - Create a backend crate (see existing backends for compile entrypoints).
  - Provide a target manifest under [targets/](targets) and extend HAL schema if needed.
- Add new simulator:
  - Create a sim crate with emit_artifacts and integrate in CLI/Python features.
- Add new pass:
  - Implement Pass trait, register via build_pipeline, wire into CLI lower pipeline as needed.
- Telemetry:
  - Use telemetry::labels helpers for standardized labeling; keep label cardinality small.
- Python:
  - Extend PyO3 module in [crates/py/src/lib.rs](crates/py/src/lib.rs) and update docs and CI feature flags.

## 11. Data Flow Summary

High-level flow:
1) Import/Parse (CLI/Python) → NIR (validate, version tag)
2) Pipeline passes (validate, quantize, partition, placement, routing, timing, resource-check)
3a) Backend compile → artifacts for target hardware
3b) Simulator emit → runnable artifacts for simulators
4) Telemetry (timers/counters) captured throughout
5) Optional MLIR export for interop
6) Runtime/Orchestrator handle deployment and multi-target coordination

Artifacts & dumps:
- Per-pass NIR dumps (JSON/YAML/Bin), simulator run directories (model_summary.json, RUN.txt), backend artifacts.
- Telemetry JSONL (when enabled) and optional OTLP streams.

This architecture emphasizes capability-aware lowering, unified compile/sim interfaces, and consistent instrumentation to support both functional verification and performance analysis across heterogeneous neuromorphic systems.