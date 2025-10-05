# HW-specific simulator quickstart

This tutorial shows how to run the hardware-specific simulator adapter behind a feature gate and verify emitted artifacts.

## Prerequisites
- Rust and Cargo installed
- This repository checked out

## Enabling the feature
The simulate subcommand for the "hw" simulator is feature-gated. You must enable the sim-hw-specific feature on the CLI crate.

Run:
```bash
cargo run -p neuro-compiler-cli --features sim-hw-specific -- simulate --simulator hw --input examples/nir/simple.json --out-dir target/sim-hw-out
```
Replace the input path as needed for your graph.

## Expected output
On success, the command prints:
simulate artifacts written to "target/sim-hw-out"

and writes files:
- target/sim-hw-out/RUN.txt
- target/sim-hw-out/model_summary.txt

The provided example graph [examples/nir/simple.json](examples/nir/simple.json:1) will produce content like:
- RUN.txt: "hw simulate run for example-json"
- model_summary.txt: "graph=example-json, populations=2, connections=1"

## Behavior when the feature is disabled
If you forget the feature flag, the CLI prints a clear message: see [crates/cli/src/main.rs](crates/cli/src/main.rs:553).
To enable the adapter via feature mapping, see [crates/cli/Cargo.toml](crates/cli/Cargo.toml:67).

## Implementation notes
The CLI dispatch that invokes the adapter is in [crates/cli/src/main.rs](crates/cli/src/main.rs:544). The adapter emits plain-text artifacts into the selected out directory.

## Repro steps
1. Ensure the repository builds: `cargo build`
2. Run with features enabled (see command above)
3. Inspect artifacts under target/sim-hw-out

## Troubleshooting
- If target/sim-hw-out is empty, confirm you passed `--features sim-hw-specific` and did not run a cached binary.
- If the input file is missing, use the sample [examples/nir/simple.json](examples/nir/simple.json:1).
- If the CLI prints a disabled message, enable the feature as shown above.

## References
- CLI feature mapping: [crates/cli/Cargo.toml](crates/cli/Cargo.toml:67)
- Disabled message: [crates/cli/src/main.rs](crates/cli/src/main.rs:553)
- Example input: [examples/nir/simple.json](examples/nir/simple.json:1)