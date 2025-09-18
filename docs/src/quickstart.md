# Quickstart

- Build: `cargo build --workspace`
- List targets: `cargo run -p neuro-compiler-cli -- list-targets`
- NIR round-trip in Rust: see nc-nir tests and examples/nir/*
- Python bindings: `maturin build -m pyproject.toml --features python` (produces wheel)
