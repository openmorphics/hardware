# neuro-compiler (skeleton)

A universal neuromorphic compiler scaffold with Rust core and optional Python bindings. This repo currently provides:
- Canonical IR crate (nc-nir) stub
- Hardware Abstraction Layer crate (nc-hal) stub with builtin target list
- Pass manager crate (nc-passes) stub
- Runtime, Telemetry, Orchestrator, and ML optimization stubs
- CLI (neuro-compiler) with 'list-targets' and lowering pipeline controls
- Python bindings crate (feature-gated) and pyproject for maturin
- MLIR bridge crate gated behind 'mlir' feature (off by default)
- Docs skeleton (mdBook) and CI skeleton
- Examples under examples/nir/

Quick start (build workspace):
- cargo build --workspace

Try CLI:
- cargo run -p neuro-compiler-cli -- list-targets

Enable MLIR bridge compile (no-op stub):
- cargo build -p nc-mlir-bridge -F mlir

Python bindings (no-op stub without feature; with feature requires maturin):
- cargo build -p neuro-compiler-py
- With Python module (ABI3), use maturin building with feature:
  maturin build -m pyproject.toml --features python

Docs (mdBook):
- cargo install mdbook
- mdbook build docs
- Start at docs/src/quickstart.md

License: UNLICENSED (see LICENSE).

## Minimal builds via feature flags (per surface)

The CLI supports opt-in features for each frontend, backend, and simulator crate. This minimizes build time and dependency footprint. See [crates/cli/Cargo.toml](crates/cli/Cargo.toml) for the complete feature list and aggregate flags.

Examples:
- Build with only Loihi backend and Arbor sim:
  - cargo run -p neuro-compiler-cli -F backend-loihi -F sim-arbor -- list-targets
- Build with all backends:
  - cargo build -p neuro-compiler-cli -F backends-all
- Build with all surfaces (frontends/backends/sims):
  - cargo build -p neuro-compiler-cli -F all-surfaces

Note: Workspace default-members exclude most surface crates to keep the default `cargo build --workspace` lean. You can still build any crate explicitly by selecting it with `-p` or enabling the CLI features above.

## Structured lowering pipeline and intermediate artifacts

Lowering supports a structured pipeline config and intermediate artifact dumping via the CLI. See [crates/cli/src/main.rs](crates/cli/src/main.rs) and [crates/passes/src/lib.rs](crates/passes/src/lib.rs).

- Run a no-op pipeline and dump artifacts (JSON by default):
  - cargo run -p neuro-compiler-cli -- lower --pipeline noop --dump-dir ./out
- Dump YAML instead (or both, comma separated):
  - cargo run -p neuro-compiler-cli -- lower --pipeline noop --dump-dir ./out --dump-format yaml
  - cargo run -p neuro-compiler-cli -- lower --pipeline noop --dump-dir ./out --dump-format json,yaml

The IR graph dumps are round-trippable via [crates/nir/src/lib.rs](crates/nir/src/lib.rs) JSON/YAML serializers.

## Profiling schema and quick visualization

A JSON Lines schema for profiling is defined in [crates/telemetry/src/lib.rs](crates/telemetry/src/lib.rs) (module `profiling`) and documented in [docs/metrics/profiling.md](docs/metrics/profiling.md). Emit profile records as JSONL using the helper and visualize with Python (Altair/Matplotlib) following the doc’s example.

## Python API versioning policy

- The Python package uses a stable ABI3 wheel (built with pyo3 `abi3-py38`), targeting Python ≥ 3.8.
- Versioning aligns with the Rust workspace version in [Cargo.toml](Cargo.toml) and [pyproject.toml](pyproject.toml).
- Within 0.x, breaking changes may occur between minor versions. We recommend pinning a compatible range:
  - pip install "neuro-compiler>=0.0.1,<0.1.0"
- Once the API stabilizes at 1.0+, semantic versioning will be followed with deprecation windows noted in the docs/book.
- Packaging constraints:
  - Wheels are built via maturin; see [pyproject.toml](pyproject.toml) for classifiers and `requires-python`.
  - The Rust extension uses the stable CPython ABI (no per-Python rebuilds required for supported versions).

