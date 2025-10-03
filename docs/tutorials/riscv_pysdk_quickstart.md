# RISC-V Python SDK Quickstart

## Introduction

This tutorial demonstrates how to compile neuromorphic network models for all three RISC-V target profiles using the Python SDK. You'll learn to use the neuromorphic compiler's Python bindings to generate optimized code for different RISC-V deployment scenarios, from high-performance Linux applications to resource-constrained embedded systems and neuromorphic accelerator control planes.

## Prerequisites

Before proceeding with this tutorial, ensure you have the required RISC-V toolchains and simulation environments installed. For detailed setup instructions, refer to the [RISC-V Backend Documentation](../backends/riscv.md).

Required tools:
- RISC-V GNU toolchain (for cross-compilation)
- QEMU with RISC-V support (for Linux user and bare-metal profiles)
- Renode (for control-plane profile simulation)
- Python neuromorphic compiler with RISC-V backend support

## Example 1: Linux User Profile (`riscv64gcv_linux`)

The Linux user profile targets RV64GCV systems running in user mode on Linux, with vector extensions and performance counters enabled. This profile is ideal for high-performance neuromorphic applications.

```python
#!/usr/bin/env python3
"""
RISC-V Linux User Profile Compilation Example

This script demonstrates compiling a NIR model for the riscv64gcv_linux target,
which generates optimized code for 64-bit RISC-V processors with vector extensions
running in Linux userspace.
"""

import os
import neuro_compiler as nc

def main():
    print("=== RISC-V Linux User Profile (riscv64gcv_linux) ===")
    
    # Load the NIR model from the examples directory
    with open("examples/nir/simple.json", "r") as f:
        nir_content = f.read()
    
    # Set environment variables for RISC-V simulation and telemetry capture
    os.environ["NC_RISCV_QEMU_RUN"] = "1"
    os.environ["NC_PROFILE_JSONL"] = "linux_user_profile.jsonl"
    
    try:
        # Compile for the Linux user profile target
        artifact_path = nc.compile_nir_str_py("riscv64gcv_linux", nir_content)
        print(f"✓ Compilation successful!")
        print(f"  Artifacts written to: {artifact_path}")
        print(f"  Generated executable optimized for RV64GCV with vector extensions")
        
        # Wait for simulation to complete and analyze telemetry
        print("\n📊 Telemetry Summary:")
        summary = nc.profile_summary_py("linux_user_profile.jsonl")
        print(summary)
        
    except Exception as e:
        print(f"✗ Compilation failed: {e}")

if __name__ == "__main__":
    main()
```

This profile generates a Linux executable that leverages RISC-V vector extensions for optimized neuromorphic computation. The simulation runs under `qemu-riscv64` in user mode, and telemetry data captures performance metrics including vector instruction utilization.

## Example 2: Bare-Metal Profile (`riscv32imac_bare`)

The bare-metal profile targets resource-constrained embedded systems without operating system support. This profile is designed for real-time applications where deterministic behavior and minimal resource usage are critical.

```python
#!/usr/bin/env python3
"""
RISC-V Bare-Metal Profile Compilation Example

This script demonstrates compiling a NIR model for the riscv32imac_bare target,
which generates firmware for 32-bit RISC-V microcontrollers without vector
extensions, suitable for embedded and real-time applications.
"""

import os
import neuro_compiler as nc

def main():
    print("=== RISC-V Bare-Metal Profile (riscv32imac_bare) ===")
    
    # Load the NIR model from the examples directory
    with open("examples/nir/simple.json", "r") as f:
        nir_content = f.read()
    
    # Set environment variables for RISC-V simulation and telemetry capture
    os.environ["NC_RISCV_QEMU_RUN"] = "1"
    os.environ["NC_PROFILE_JSONL"] = "bare_metal_profile.jsonl"
    
    try:
        # Compile for the bare-metal target
        artifact_path = nc.compile_nir_str_py("riscv32imac_bare", nir_content)
        print(f"✓ Compilation successful!")
        print(f"  Artifacts written to: {artifact_path}")
        print(f"  Generated firmware.elf for RV32IMAC bare-metal execution")
        print(f"  Optimized for low memory footprint and deterministic timing")
        
        # Analyze telemetry from simulated UART output
        print("\n📊 Telemetry Summary:")
        summary = nc.profile_summary_py("bare_metal_profile.jsonl")
        print(summary)
        print("  Telemetry captured from qemu-system-riscv32 UART simulation")
        
    except Exception as e:
        print(f"✗ Compilation failed: {e}")

if __name__ == "__main__":
    main()
```

This profile produces a `firmware.elf` file suitable for deployment on RISC-V microcontrollers. The compilation optimizes for minimal memory usage and real-time constraints, making it ideal for edge AI applications where power efficiency is paramount.

## Example 3: Control-Plane Profile (`riscv64gc_ctrl`)

The control-plane profile is designed for scenarios where a RISC-V processor acts as a host controller for specialized neuromorphic accelerator hardware. It includes MMIO and DMA support for high-bandwidth communication with accelerators.

```python
#!/usr/bin/env python3
"""
RISC-V Control-Plane Profile Compilation Example

This script demonstrates compiling a NIR model for the riscv64gc_ctrl target,
which generates control software for RISC-V processors managing neuromorphic
accelerators via MMIO and DMA interfaces.
"""

import os
import neuro_compiler as nc

def main():
    print("=== RISC-V Control-Plane Profile (riscv64gc_ctrl) ===")
    
    # Load the NIR model from the examples directory
    with open("examples/nir/simple.json", "r") as f:
        nir_content = f.read()
    
    # Set environment variables for RISC-V simulation and telemetry capture
    os.environ["NC_RISCV_QEMU_RUN"] = "1"
    os.environ["NC_PROFILE_JSONL"] = "control_plane_profile.jsonl"
    
    try:
        # Compile for the control-plane target
        artifact_path = nc.compile_nir_str_py("riscv64gc_ctrl", nir_content)
        print(f"✓ Compilation successful!")
        print(f"  Artifacts written to: {artifact_path}")
        print(f"  Generated control software for RV64GC with MMIO/DMA support")
        print(f"  Includes accelerator.repl and run.resc for Renode simulation")
        
        # Analyze telemetry from Renode simulation
        print("\n📊 Telemetry Summary:")
        summary = nc.profile_summary_py("control_plane_profile.jsonl")
        print(summary)
        print("  Telemetry reflects RISC-V↔accelerator communication patterns")
        print("  Simulation includes virtual neuromorphic accelerator hardware")
        
    except Exception as e:
        print(f"✗ Compilation failed: {e}")

if __name__ == "__main__":
    main()
```

This profile generates control plane software that manages neuromorphic accelerator hardware through memory-mapped I/O and DMA transfers. The Renode simulation environment provides a virtual accelerator for testing the host-accelerator interaction patterns.

## Running the Examples

To execute these examples, save each script to a file and run with Python:

```bash
# Linux User Profile
python3 riscv_linux_example.py

# Bare-Metal Profile  
python3 riscv_bare_metal_example.py

# Control-Plane Profile
python3 riscv_control_plane_example.py
```

Each script will:
1. Load the example NIR model (`examples/nir/simple.json`)
2. Set the required environment variables for simulation
3. Invoke the neuromorphic compiler for the target RISC-V profile
4. Display the compilation results and artifact locations
5. Present a summary of captured performance telemetry

## Understanding the Output

### Compilation Artifacts

Each profile generates different types of artifacts in the `target/` directory:

- **Linux User**: Executable binary optimized for user-mode execution
- **Bare-Metal**: `firmware.elf` with startup code and linker scripts
- **Control-Plane**: Host software plus Renode simulation files (`.repl`, `.resc`)

### Telemetry Data

The captured telemetry provides insights into:
- Compilation pass execution times
- Memory usage patterns
- Instruction scheduling efficiency
- Vector utilization (Linux profile)
- MMIO/DMA transaction patterns (control-plane profile)

## Conclusion

You have successfully learned how to compile neuromorphic models for all three RISC-V target profiles using the Python SDK. Each profile serves different deployment scenarios:

- **Linux User Profile**: High-performance applications with vector acceleration
- **Bare-Metal Profile**: Embedded systems with strict resource constraints
- **Control-Plane Profile**: Host processors managing specialized accelerators

### Next Steps

1. **Experiment with your own NIR models**: Replace `examples/nir/simple.json` with your custom network definitions
2. **Optimize compilation settings**: Explore target-specific configuration options in the `.toml` manifest files
3. **Deploy to real hardware**: Use the generated artifacts for actual RISC-V system deployment
4. **Performance analysis**: Leverage the telemetry data to identify optimization opportunities

For advanced usage and configuration options, consult the [RISC-V Backend Documentation](../backends/riscv.md) and [Python API Reference](../python/usage.md).