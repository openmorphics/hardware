# RISC-V Backend

This backend targets RISC-V CPUs in three profiles:
- RV64GCV Linux userspace (vector-enabled)
- RV32IMAC bare-metal or RTOS (size-optimized, no vector)
- RV64GC Linux control-plane with MMIO/DMA to a neuromorphic accelerator

## Targets

- riscv64gcv_linux — RV64GCV with V 1.0 under Linux userspace
- riscv32imac_bare — RV32IMAC bare-metal/RTOS without vector
- riscv64gc_ctrl — RV64GC Linux control-plane offloading via MMIO/DMA

## Build (enable backend)

Enable the RISC-V backend feature in the CLI:
```
cargo build -p neuro-compiler-cli --features backend-riscv
```

## Toolchain install (host)

- Linux (Debian/Ubuntu):
  - QEMU user-mode and cross GCC:
    ```
    sudo apt-get update
    sudo apt-get install -y qemu-user qemu-user-static gcc-riscv64-linux-gnu
    ```
  - Optional Clang cross:
    ```
    sudo apt-get install -y clang lld
    ```
- macOS (Homebrew):
  - QEMU:
    ```
    brew install qemu
    ```
  - Cross compilers:
    - Prefer using a container for cross GCC, or install LLVM and use:
      ```
      clang --target=riscv64-unknown-linux-gnu ...
      ```

## Usage

- List targets:
  ```
  neuro-compiler list-targets
  ```

- Compile to RISC-V (RV64GCV):
  ```
  neuro-compiler compile --input examples/nir/simple.json --target riscv64gcv_linux
  ```

- Lower with RISC-V-oriented passes:
  ```
  neuro-compiler lower --pipeline "validate,rv-lower,rv-layout,rv-schedule"
  ```

### Run under QEMU (optional M1 runner)

The backend attempts best-effort code emission and build. To run the generated binary under qemu-user and capture telemetry:

- Set a JSONL output path (recommended):
  ```
  export NC_PROFILE_JSONL=target/riscv-profile.jsonl
  ```
- Ask the backend to run qemu after build:
  ```
  export NC_RISCV_QEMU_RUN=1
  neuro-compiler compile --input examples/nir/simple.json --target riscv64gcv_linux
  ```
- If tools are present (`qemu-riscv64` and either `riscv64-linux-gnu-gcc` or `clang --target=riscv64-unknown-linux-gnu`), the backend will:
  - Emit `main.c` and build an RV64 binary
  - Run it under qemu-user
  - Write JSONL to `$NC_PROFILE_JSONL` or `target/<target>-<graph>/profile.jsonl`

Summarize the profile:
```
neuro-compiler profile --input $NC_PROFILE_JSONL
```

## Telemetry

Records are JSONL with labels aligned to the compiler’s standard schema:
- labels: graph, backend=riscv, isa=rv64gcv, simulator=qemu
- metrics: `kernel.step_ns`, `events.processed`, etc.

## CI notes (optional)

- Add steps to install qemu-user and riscv64 cross toolchain for Linux jobs.
- Gate runtime tests behind a feature or environment flag to keep CI fast:
  - Example: only run QEMU tests when `CI_RISCV=1`.

## Notes

- M1 uses scalar C codegen; RVV vectorization and richer runtime will follow.
- Control-plane profile will emit device control code once the MMIO/DMA generator is implemented.
