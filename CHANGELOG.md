# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.0] - 2025-10-03

### Added
- RISC-V backend with profiles: rv64gcv linux_user, rv32imac bare_metal, rv64gc control_plane. See [docs/backends/riscv.md](docs/backends/riscv.md:1).
- Python bindings feature wiring for RISC-V (compile-only without external tools).
- CI: RISC-V runtime jobs (qemu-user, qemu-system, Renode), Python RISC-V wheel job, security audit (cargo-audit), minimal-versions build, dependency tree duplicates, mdBook docs build, coverage via cargo-llvm-cov, performance baseline artifacts (JSONL+CSV). See [.github/workflows/ci.yml](.github/workflows/ci.yml:1).

### Documentation
- RISC-V Python SDK quickstart, backend docs updates, CLI naming normalization to "neuro-compiler". See [docs/tutorials/riscv_pysdk_quickstart.md](docs/tutorials/riscv_pysdk_quickstart.md:1), [docs/backends/riscv.md](docs/backends/riscv.md:1), and [README.md](README.md).