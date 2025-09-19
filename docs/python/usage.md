# Python bindings: usage, features, and examples

This page documents the Python bindings exposed by the Rust universal neuromorphic compiler. It covers installation, feature flags and wheels, core APIs for compile/simulate/profile, telemetry usage, and quickstart examples.

Repository sources
- Core file: [crates/py/src/lib.rs](crates/py/src/lib.rs)
- CI wheels matrix and smoke test: [.github/workflows/ci.yml](.github/workflows/ci.yml)
- Targets (HAL manifests): [targets/](targets)

Installation

Option A: install a prebuilt wheel artifact
1. Download the wheel from CI artifacts for your OS and Python version.
2. Install with pip:
   ```bash
   pip install target/wheels/neuro_compiler-*.whl
   ```

Option B: build locally with maturin
- Prerequisites: Rust toolchain + Python 3.8–3.12
- Build a local wheel and install it:
  ```bash
  pip install maturin
  maturin build -m pyproject.toml -F python -F backend-truenorth -F backend-dynaps -F sim-neuron -F sim-coreneuron -F sim-arbor -F telemetry
  pip install target/wheels/*.whl
  ```
- For a developer workflow (editable build):
  ```bash
  maturin develop -m pyproject.toml -F python -F backend-truenorth -F backend-dynaps -F sim-neuron -F sim-coreneuron -F sim-arbor -F telemetry
  ```

Feature flags (Python crate)
- backends:
  - backend-truenorth
  - backend-dynaps
- simulators:
  - sim-neuron
  - sim-coreneuron
  - sim-arbor
- telemetry for Python simulate helpers:
  - telemetry
- core Python extension:
  - python

CI build matrix (wheels)
- OS: ubuntu-latest, macos-latest, windows-latest
- Python: 3.8, 3.9, 3.10, 3.11, 3.12
- See the workflow step “Build Python wheel (bindings + backends + sims + telemetry)” in [.github/workflows/ci.yml](.github/workflows/ci.yml)

Python module import
```python
import neuro_compiler as nc
```

API reference (high-level)
Note: Some functions are feature-gated. If a requested backend/simulator feature is not enabled at build time, the function will raise a runtime error with a helpful message describing the missing feature.

- Version and targets
  - nc.version_py() → str
  - nc.list_targets_py() → list[str]

- Import helpers (demonstration of parsing only)
  - nc.import_json_py(s: str) → str
  - nc.import_yaml_py(s: str) → str
  These return the graph name after parsing.

- Compile helpers (feature-gated per backend)
  - nc.compile_nir_json_py(target: str, json: str) → str
  - nc.compile_nir_yaml_py(target: str, yaml: str) → str
  - nc.compile_nir_str_py(target: str, s: str) → str
    Auto-detects JSON or YAML by first non-whitespace character.

  Targets (examples): "truenorth", "dynaps"
  - Build with -F backend-truenorth and/or -F backend-dynaps

- Simulate helpers (feature-gated per simulator)
  - nc.simulate_nir_json_py(simulator: str, json: str, out_dir: Optional[str]) → str
  - nc.simulate_nir_yaml_py(simulator: str, yaml: str, out_dir: Optional[str]) → str
  - nc.simulate_nir_str_py(simulator: str, s: str, out_dir: Optional[str]) → str
    Auto-detects JSON or YAML.

  Simulators (examples): "neuron", "coreneuron", "arbor"
  - Build with -F sim-neuron / sim-coreneuron / sim-arbor

- Profiling summaries (JSONL)
  - nc.profile_summary_py(path: str) → str
    Returns CSV: metric,count,avg,min,max

Telemetry (JSONL) from Python simulate helpers
- Enable the “telemetry” feature in the Python crate build (-F telemetry).
- Set the environment variable NC_PROFILE_JSONL to a writable path before calling simulate helpers.
- The bindings will append metrics and latency counters to the JSONL file.
- See also the label schema and profiling docs:
  - [docs/metrics/labels.md](docs/metrics/labels.md)
  - [docs/metrics/profiling.md](docs/metrics/profiling.md)

Quickstart: compile and simulate (JSON string)
```python
import os
import neuro_compiler as nc

print("version:", nc.version_py())
print("targets:", nc.list_targets_py())

nir_json = """
{
  "nir_version":"0.1",
  "name":"py-quickstart",
  "populations":[
    {"name":"A","size":2,"model":"LIF","params":{}},
    {"name":"B","size":2,"model":"LIF","params":{}}
  ],
  "connections":[
    {"pre":"A","post":"B","weight":0.5,"delay_ms":0.0}
  ],
  "probes":[]
}
""".strip()

# Compile for a backend (requires matching backend feature in the wheel)
try:
  art = nc.compile_nir_str_py("truenorth", nir_json)
  print("compile artifact:", art)
except Exception as e:
  print("compile failed or backend not enabled:", e)

# Optional telemetry: record JSONL metrics during simulate
os.environ["NC_PROFILE_JSONL"] = "py-sim-profile.jsonl"

# Simulate with a simulator (requires matching simulator feature in the wheel)
try:
  out_dir = nc.simulate_nir_str_py("neuron", nir_json, "py-sim-out")
  print("simulate artifacts written to:", out_dir)
except Exception as e:
  print("simulate failed or simulator not enabled:", e)

# Summarize profiling JSONL (if telemetry feature was enabled during build)
try:
  summary_csv = nc.profile_summary_py("py-sim-profile.jsonl")
  print(summary_csv)
except Exception as e:
  print("profiling summary skipped:", e)
```

Quickstart: YAML input and auto-detect
```python
nir_yaml = """
nir_version: "0.1"
name: "py-quickstart-yaml"
populations:
  - {name: A, size: 1, model: LIF, params: {}}
  - {name: B, size: 1, model: LIF, params: {}}
connections:
  - {pre: A, post: B, weight: 0.25, delay_ms: 0.0}
probes: []
""".strip()

# Auto-detects YAML by leading character and compiles accordingly
try:
  art = nc.compile_nir_str_py("dynaps", nir_yaml)
  print("compile artifact:", art)
except Exception as e:
  print("compile failed or backend not enabled:", e)

# Simulate with Arbor (if enabled)
try:
  out_dir = nc.simulate_nir_str_py("arbor", nir_yaml, None)
  print("simulate artifacts written to:", out_dir)
except Exception as e:
  print("simulate failed or simulator not enabled:", e)
```

Recommended build feature sets for common tasks
- Sim + telemetry (no hardware backends):
  ```bash
  maturin develop -m pyproject.toml -F python -F sim-neuron -F sim-coreneuron -F sim-arbor -F telemetry
  ```
- Truenorth + basic sims:
  ```bash
  maturin develop -m pyproject.toml -F python -F backend-truenorth -F sim-neuron -F telemetry
  ```
- Dynaps only:
  ```bash
  maturin develop -m pyproject.toml -F python -F backend-dynaps
  ```

Troubleshooting
- “backend not enabled” or “simulator not enabled”: Rebuild the wheel with the appropriate feature flags (see above).
- “Module import” errors on Windows/macOS: Ensure your Python and wheel architecture match (x86_64 vs ARM64). On macOS, prefer matching universal2 wheels when available.
- “No JSONL produced”: Ensure NC_PROFILE_JSONL is set and telemetry feature was enabled at build time; also verify write permissions to the destination path.

Source references (for maintainers)
- Python Rust sources: [crates/py/src/lib.rs](crates/py/src/lib.rs)
- CI wheels: [.github/workflows/ci.yml](.github/workflows/ci.yml)
- HAL manifests: [targets](targets)