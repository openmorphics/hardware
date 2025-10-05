# Metrics Label Schema

This document defines the canonical key/value labels used across profiling and telemetry in the universal neuromorphic compiler. Labels attach dimensional context to every metric and trace to enable robust filtering, grouping, and correlation across components (passes, simulators, backends, runtime).

Source of truth (helper constructors)
- [telemetry::labels::graph()](crates/telemetry/src/lib.rs:192)
- [telemetry::labels::target()](crates/telemetry/src/lib.rs:199)
- [telemetry::labels::backend()](crates/telemetry/src/lib.rs:207)
- [telemetry::labels::simulator()](crates/telemetry/src/lib.rs:218)
- [telemetry::labels::pass()](crates/telemetry/src/lib.rs:226)
- [telemetry::labels::merge()](crates/telemetry/src/lib.rs:233)
- [telemetry::labels::with()](crates/telemetry/src/lib.rs:241)

Standard label keys
- graph: Logical model/graph name (string; stable across a pipeline run)
- target: HAL manifest target (e.g., "loihi2", "truenorth")
- backend: Backend identifier (e.g., "loihi", "truenorth", "dynaps")
- simulator: Simulator identifier (e.g., "neuron", "coreneuron", "arbor", "hw")
- pass: Compiler pass name (e.g., "validate", "partition", "placement", "routing", "timing", "resource-check")

Additional labels are allowed and encouraged when they add clear, low-cardinality context (e.g., "kind" = "latency"|"energy", "phase" = "compile"|"simulate"). Avoid high-cardinality or unbounded values in labels (timestamps, file paths, random IDs).

Design guidelines
- Keys are snake_case; values are short strings, stable across runs for the same dimension.
- Prefer standard keys above; extend using telemetry::labels::with() to add one-off dimensions.
- Keep label sets minimal; every unique label combination increases metric series cardinality and storage cost.
- Use the helper constructors to ensure consistency and ordering (BTreeMap ensures deterministic serialization).

Example: building labels

Rust (passes)
```rust
use nc_telemetry::labels;
let mut l = labels::pass("my-graph", "placement");
l = labels::with(l, "phase", "compile");
// result: {"graph":"my-graph","pass":"placement","phase":"compile"}
```

Rust (backends)
```rust
use nc_telemetry::labels;
let l = labels::backend("my-graph", "loihi", Some("loihi2"));
// result: {"graph":"my-graph","backend":"loihi","target":"loihi2"}
```

Rust (simulators)
```rust
use nc_telemetry::labels;
let l = labels::simulator("my-graph", "neuron");
// result: {"graph":"my-graph","simulator":"neuron"}
```

Label merging and overrides
```rust
use nc_telemetry::labels;
let base = labels::graph("my-graph");
let sim = labels::simulator("my-graph", "arbor");
let merged = labels::merge(base, sim);
let merged = labels::with(merged, "phase", "simulate");
// {"graph":"my-graph","simulator":"arbor","phase":"simulate"}
```

Where labels are used
- JSONL profiling: every record written via Appender carries labels alongside metric and value.
- Timers: start_timer(...) captures duration with labels when the guard drops.
- Counters: counter(...) attaches instantaneous counts (e.g., graph cardinalities) with labels.

Example JSONL records
```json
{"ts_ms": 1736966400000, "metric": "passes.pass_ms", "value": 3.71, "labels": {"graph":"cli-lower-demo","pass":"placement"}}
{"ts_ms": 1736966400050, "metric": "sim.emit_ms", "value": 12.40, "labels": {"graph":"cli-sim-demo","simulator":"neuron"}}
{"ts_ms": 1736966400100, "metric": "backend.compile_ms", "value": 45.2, "labels": {"graph":"example","backend":"loihi","target":"loihi2"}}
```

Telemetry enablement (build-time)
- Enable the "telemetry" feature on crates that should emit JSONL (CLI, passes, simulators, backends).

Runtime configuration
- Emit JSONL: set NC_PROFILE_JSONL=/path/to/profile.jsonl (CLI or Python wrappers will append)
- OTLP export: build the CLI with feature "telemetry-otlp" and set NC_OTLP_ENDPOINT (e.g., http://localhost:4317)

Life-of-run consistency
- Use a single graph name per pipeline run to enable easy joins across pass/backends/simulator metrics.
- Propagate target and backend labels from compile paths into downstream metrics where applicable.

Do and don't
- Do prefer graph/target/backend/simulator/pass for primary dimensions
- Do add "kind" for multiple semantic forms of the same metric (e.g., latency vs energy)
- Do keep values normalized (lowercase ASCII, dashed or snake_case)
- Don't include PII, absolute paths, or secrets
- Don't include timestamps or highly variable strings in labels

Python usage
- The Python bindings forward labels for simulate_* wrappers when "telemetry" is enabled in the Python crate build and NC_PROFILE_JSONL is set.

Related references
- [telemetry.profiling.ProfileRecord](crates/telemetry/src/lib.rs:68)
- [telemetry.profiling.emit_profile_jsonl()](crates/telemetry/src/lib.rs:77)
- [docs/metrics/profiling.md](docs/metrics/profiling.md)