# Target Configurations

This project uses target configurations to define compilation profiles for different RISC-V scenarios.

## Target Locations

- **Repo-level targets**: [targets/](../../targets/) - Shared target definitions used by CLI and other tools
- **Python-specific targets**: [crates/py/targets/](../../crates/py/targets/) - Python binding specific overlays

## Current Targets

### RISC-V Profiles
- `riscv32imac_bare` - Bare metal 32-bit with integer, multiply, atomic, compressed
- `riscv64gc_ctrl` - Control plane 64-bit with general + compressed 
- `riscv64gcv_linux` - Linux user 64-bit with general + compressed + vector

## Target Strategy

Python targets in [crates/py/targets/](../../crates/py/targets/) should reference or extend repo-level targets rather than duplicate them. Future work will consolidate these configurations to reduce maintenance overhead.

See [docs/backends/riscv.md](../backends/riscv.md) for backend-specific usage.