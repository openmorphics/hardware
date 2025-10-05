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
  - Verify tool versions:
    ```
    qemu-riscv64 --version
    riscv64-linux-gnu-gcc --version
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
  - Verify tool versions:
    ```
    qemu-riscv64 --version
    riscv64-linux-gnu-gcc --version   # if you installed a cross GCC; otherwise use clang target above
    ```

- Windows / WSL (Ubuntu recommended):
  - Use WSL2 with an Ubuntu distribution for best compatibility.
  - Install QEMU (user + system) and cross toolchains:
    ```
    sudo apt-get update
    sudo apt-get install -y qemu-user qemu-user-static qemu-system-misc gcc-riscv64-linux-gnu gcc-riscv64-unknown-elf
    ```
  - Verify tool versions:
    ```
    qemu-riscv64 --version
    qemu-system-riscv32 --version
    riscv64-linux-gnu-gcc --version
    riscv64-unknown-elf-gcc --version
    ```
  - Renode (optional, for control-plane sims) can be installed inside WSL:
    ```
    pip install renode-colab
    ```

## RVV codegen feature flag (riscv-v)

The RVV vectorization path for the linux_user profile is gated behind a crate-local feature in the RISC-V backend.

- What it does:
  - Enables emission of RISC-V Vector (RVV) intrinsics in the generated C for the RV64 Linux userspace profile.
  - The emitted C guards the vectorized loop with `#if defined(__riscv_vector)` and provides a scalar fallback in the `#else` block.
  - Build logic attempts to compile with vector ISA flags and gracefully falls back to scalar if the toolchain does not support RVV. Fallback details are written to `WARN.txt`, and `README.txt` notes whether vector flags were attempted.

- How to enable:
  - At the workspace level when testing:
    ```
    cargo test --workspace --features backend-riscv/riscv-v
    ```
  - Or when building the CLI with the backend and RVV enabled:
    ```
    cargo build -p neuro-compiler-cli --features backend-riscv,backend-riscv/riscv-v
    ```

- Expected effect:
  - Telemetry schema is unchanged. Any performance improvements will appear in existing metrics such as `kernel.step_ns`, `cpu.cycle`, and `cpu.instret`.
  - When tools support RVV, the build adds the appropriate flags (GCC: `-march=rv64gcv`, Clang: `--target=riscv64-unknown-linux-gnu -march=rv64gcv`).

## Bare-metal (RV32IMAC) profile

- Target: `riscv32imac_bare` (profile = `bare_metal`)
- Toolchain required:
  - Cross-compiler: `riscv64-unknown-elf-gcc` (multilib supports RV32 with `-march=rv32imac -mabi=ilp32`)
  - System emulator: `qemu-system-riscv32`
- What the backend emits:
  - `crt0.S` (startup, sets SP, clears .bss, calls `main`)
  - `linker.ld` (RAM @ 0x8000_0000, stack at top)
  - `main.c` (polled UART at 0x1000_0000; prints JSONL metrics)
- Build and run (best-effort; writes WARN.txt if tools missing):
  ```
  export NC_RISCV_QEMU_RUN=1                 # optional: run under QEMU after compile
  neuro-compiler compile --input examples/nir/simple.json --target riscv32imac_bare
  ```
  - If tools are present, the backend:
    - Builds `firmware.elf` with `-nostdlib -nostartfiles -Tlinker.ld -march=rv32imac -mabi=ilp32`
    - Runs `qemu-system-riscv32 -nographic -machine virt -bios none -kernel firmware.elf`
    - Captures UART stdout to `$NC_PROFILE_JSONL` or `target/<target>-<graph>/profile.jsonl`
- Telemetry:
  - Same JSONL schema as other backends. Metrics include: `kernel.step_ns`, `events.processed`, `cpu.cycle`, `cpu.instret`.
  - UART is memory-mapped at 0x1000_0000; QEMU writes it to stdout. Firmware signals exit via the SiFive test finisher at 0x0010_0000.

## Control-plane (RV64GC) profile

- Target: `riscv64gc_ctrl` (profile = `control_plane`)
- Purpose: Emits Linux user-space programs that control a simulated neuromorphic accelerator via MMIO/DMA operations in Renode
- Toolchain required:
  - Cross-compiler: `riscv64-linux-gnu-gcc` or `clang --target=riscv64-unknown-linux-gnu`
  - Simulator: `renode` (can be installed via `pip install renode-colab`)
- What the backend emits:
  - `main.c` (Linux user-space program using `mmap` on `/dev/mem` for MMIO access)
  - `accelerator.repl` (Renode platform description defining RV64 system + custom peripheral)
  - `accelerator.py` (Python model implementing the SNN_Accelerator peripheral registers)
  - `run.resc` (Renode script to load platform, ELF binary, and start simulation)
- Build and run (best-effort; writes WARN.txt if tools missing):
  ```
  export NC_RISCV_QEMU_RUN=1                 # optional: run Renode simulation after compile
  neuro-compiler compile --input examples/nir/simple.json --target riscv64gc_ctrl
  ```
  - If tools are present, the backend:
    - Builds a Linux RV64 binary targeting the control-plane workflow
    - Runs Renode with the generated platform and script
    - Captures simulation output to `renode.log` and extracts JSONL telemetry
- MMIO operations:
  - The generated `main.c` performs typical accelerator control sequences:
    - Reset accelerator via control register
    - Configure DMA transfer (if `dma_supported = true` in manifest)
    - Start operation and poll status register for completion
    - Read results and generate telemetry
  - MMIO base address is configurable via `mmio_base_addr` in the target manifest (default: 0x40000000)
- Telemetry:
  - Same JSONL schema as other backends, with `simulator=renode`
  - Additional metrics: `mmio.operations` (count of MMIO register accesses)
  - Renode peripheral model logs operations for debugging and verification

## Usage

- List targets:
  ```
  neuro-compiler list-targets
  ```

- Compile to RISC-V (RV64GCV):
  ```
  neuro-compiler compile --input examples/nir/simple.json --target riscv64gcv_linux
  ```

- Compile to RISC-V control-plane:
  ```
  neuro-compiler compile --input examples/nir/simple.json --target riscv64gc_ctrl
  ```

- Lowering via compile (recommended):
  ```
  neuro-compiler compile --input examples/nir/simple.json --target riscv64gcv_linux
  ```
  Note: Backend-specific passes (e.g., rv-lower/layout/schedule) are owned and wired by the RISC-V backend crate. They may not be directly invocable via the generic lower subcommand unless explicitly exposed. Use compile with a RISC-V target to run the correct pass pipeline.

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
  Gating environment variables:
  - NC_RISCV_QEMU_RUN=1 runs linux_user and bare_metal runtime smokes (qemu); NC_RISCV_QEMU_RUN=0 skips run and compiles only.
  - RUN_RENODE_TESTS=1 runs control_plane runtime smokes in Renode; RUN_RENODE_TESTS=0 skips and compiles only.
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

The emitted Linux userspace binary also attempts to report hardware counters when CSR access is available (Zicntr/Zihpm):
- metrics: `cpu.cycle`, `cpu.instret`
- These may be zero in some environments (e.g., when user-mode CSR access is disabled via `counteren`, or in certain simulators).

Example additional JSONL lines:
```
{"metric":"cpu.cycle","value":123456,"labels":{"graph":"...","backend":"riscv","isa":"rv64gcv","simulator":"qemu"}}
{"metric":"cpu.instret","value":7890,"labels":{"graph":"...","backend":"riscv","isa":"rv64gcv","simulator":"qemu"}}
```
## CI notes (optional)

- Add steps to install qemu-user and riscv64 cross toolchain for Linux jobs.
- Gate runtime tests behind a feature or environment flag to keep CI fast:
  - Example: only run QEMU tests when `CI_RISCV=1`.

## Notes

- The backend emits a scalar fallback by default. With the `riscv-v` feature enabled, it additionally emits RVV intrinsics guarded by `__riscv_vector`, preserving full backward compatibility with older toolchains.
- Control-plane profile emits Renode simulation artifacts including MMIO/DMA device control code, platform descriptions, and peripheral models for end-to-end testing.
