# Mapping Pipeline Tutorial

This tutorial shows how to run the end-to-end lowering/mapping pipeline from the CLI and Python, including capability-aware passes, HAL manifest attachment, simulator emission, and telemetry profiling.

Core references
- CLI entrypoint: [cli.main.rs](crates/cli/src/main.rs:1)
- Pass framework and pipeline: [passes.lib.rs](crates/passes/src/lib.rs:1)
- HAL + target manifests: [hal.lib.rs](crates/hal/src/lib.rs:1), [targets/](targets)
- Telemetry (timers/counters/labels): [telemetry.lib.rs](crates/telemetry/src/lib.rs:1), [docs/metrics/labels.md](docs/metrics/labels.md), [docs/metrics/profiling.md](docs/metrics/profiling.md)
- Python usage: [docs/python/usage.md](docs/python/usage.md)

Prerequisites
- Rust toolchain installed
- From the repo root, the following commands will work against included examples

Example NIR model
- Small example: [examples/nir/simple.json](examples/nir/simple.json:1)

1) Validate and inspect NIR
```bash
# Validate and print basic stats using CLI import
cargo run -p neuro-compiler-cli -- import --input examples/nir/simple.json
```
Under the hood, the CLI uses [nir.Graph](crates/nir/src/lib.rs:1) parsing and [nir.Graph::validate()](crates/nir/src/lib.rs:1), returning a summary.

2) Run a mapping pipeline (no HAL)
```bash
# Run a basic pipeline over a minimal in-memory graph constructed by the CLI and dump intermediate artifacts in JSON
cargo run -p neuro-compiler-cli -- lower \
  --pipeline validate,partition,placement,routing,timing,resource-check \
  --dump-dir target/tutorial-dumps \
  --dump-format json
```
This compiles a trivial graph in CLI for demonstration and writes per-pass dumps (e.g., 00_validate.json, 01_partition.json, …). See dump code in [passes.dump_graph()](crates/passes/src/lib.rs:519).

3) Run a mapping pipeline with HAL manifest
Capability-aware passes can enforce target constraints (e.g., memory, fan-in/out, interconnect bandwidth).

```bash
# Use a builtin target manifest (loihi2) to make passes capability-aware
cargo run -p neuro-compiler-cli -- lower \
  --pipeline validate,partition,placement,routing,timing,resource-check \
  --dump-dir target/tutorial-dumps-loihi2 \
  --dump-format json \
  --target loihi2
```

Alternatively, provide a manifest file directly:
```bash
cargo run -p neuro-compiler-cli -- lower \
  --pipeline validate,partition,placement,routing,timing,resource-check \
  --dump-dir target/tutorial-dumps-custom \
  --dump-format json \
  --manifest targets/loihi2.toml
```
The CLI attaches the manifest path to the NIR attributes (key: "hal_manifest_path"), which passes read via [passes.extract_caps_from_graph()](crates/passes/src/lib.rs:64).

4) Enable telemetry profiling (JSONL)
When built with the telemetry feature, the pipeline can emit JSONL profiling records (timers and counters) using labels standardized in [docs/metrics/labels.md](docs/metrics/labels.md).

```bash
# Build with telemetry and run a pipeline producing JSONL
# (You can also use `cargo run --features telemetry -p neuro-compiler-cli -- ...` if features are opt-in)
NC_PROFILE_JSONL=target/lower-prof.jsonl \
cargo run -p neuro-compiler-cli -- lower \
  --pipeline validate,partition,placement \
  --dump-dir target/tutorial-telemetry \
  --dump-format json
```

To summarize the JSONL (when telemetry is enabled in the build):
```bash
cargo run -p neuro-compiler-cli -- profile --input target/lower-prof.jsonl
```

5) Simulate the model (NEURON/CoreNEURON/Arbor)
Simulators emit runnable artifacts (or stubs, depending on features) using the same NIR input.

```bash
# Simulate with NEURON artifacts
cargo run -p neuro-compiler-cli -- simulate \
  --simulator neuron \
  --input examples/nir/simple.json \
  --out-dir target/sim-neuron-out
```
Other simulators:
```bash
cargo run -p neuro-compiler-cli -- simulate --simulator coreneuron --input examples/nir/simple.json --out-dir target/sim-coreneuron-out
cargo run -p neuro-compiler-cli -- simulate --simulator arbor      --input examples/nir/simple.json --out-dir target/sim-arbor-out
```
See simulator emitters in:
- [sim_neuron.lib.rs](crates/sim_neuron/src/lib.rs:1)
- [sim_coreneuron.lib.rs](crates/sim_coreneuron/src/lib.rs:1)
- [sim_arbor.lib.rs](crates/sim_arbor/src/lib.rs:1)

6) Python quickstart (auto-detect JSON vs YAML)
See the complete guide in [docs/python/usage.md](docs/python/usage.md). The minimal path, using auto-detect helpers exposed in [py.lib.rs](crates/py/src/lib.rs:1):

```python
import os
import neuro_compiler as nc

nir = '{"nir_version":"0.1","name":"py-tutorial","populations":[{"name":"A","size":1,"model":"LIF","params":{}},{"name":"B","size":1,"model":"LIF","params":{}}],"connections":[{"pre":"A","post":"B","weight":0.5,"delay_ms":0.0}],"probes":[]}'

# Compile (requires matching backend feature in built wheel)
try:
    art = nc.compile_nir_str_py("truenorth", nir)
    print("compile artifact:", art)
except Exception as e:
    print("compile disabled:", e)

# Simulate with telemetry JSONL
os.environ["NC_PROFILE_JSONL"] = "py-tutorial-prof.jsonl"
try:
    out_dir = nc.simulate_nir_str_py("neuron", nir, "py-tutorial-sim-out")
    print("simulate artifacts:", out_dir)
except Exception as e:
    print("simulate disabled:", e)
```

7) Understanding pass outputs
- Partition: parts count, population assignments, and violations (e.g., population exceeding max neurons/core).
- Placement: estimated memory per part vs. core_memory_kib; fan-in/out checks.
- Routing: cross-part edge counts, bandwidth estimate (bytes_per_event × default_spike_rate_hz) vs. interconnect bandwidth.
- Timing: per-connection ticks computed from time_resolution_ns.

All of these are recorded into the graph’s attributes by each pass (see [passes.lib.rs](crates/passes/src/lib.rs:1)) and are included in the dump artifacts when enabled.

8) Troubleshooting
- “manifest invalid”: Ensure the TOML schema fields are legal (non-zero caps, positive memory/rates). See [hal.validate_manifest()](crates/hal/src/lib.rs:95).
- “simulate unsupported”: The chosen simulator feature may not be enabled; rebuild CLI with the required feature flag.
- Empty JSONL: Ensure the telemetry feature is enabled in the build and NC_PROFILE_JSONL points to a writable path.

9) Next steps
- Backend compilation: Use the CLI compile command to generate backend-specific artifacts (when compiled with that backend). See [cli.main.rs](crates/cli/src/main.rs:1).
- Label schema and analysis: Explore the labels in JSONL via [docs/metrics/labels.md](docs/metrics/labels.md) and summarize with the CLI or a Python notebook.
- Extend the pipeline: Add or reorder passes by name using [passes.build_pipeline()](crates/passes/src/lib.rs:545).
