# Backends

Status: v0.1-draft
Intent: Declare per-backend capability constraints and mapping expectations that compiler passes must respect.

Capability categories (declare per backend)
- Supported dtypes (weights/activations/params)
- Max fan-in/fan-out; connectivity limits (dense/sparse)
- Delay range (ticks) and dt constraints
- Plasticity support (static, STDP variants, on/off-chip learning)
- Memory topology (on-chip buffers vs. host memory) and placement constraints
- Execution model (event-driven vs. time-stepped), scheduling granularity, parallelism
- Quantization recommendations (bitwidths, symmetric/asymmetric)
- Unsupported features with actionable fallbacks (e.g., clamp, dequantize)

Backend declarations (illustrative, not normative)
- Loihi2
  - dtypes: weights f16/i8, activations spike/rate f16
  - delays: 0..D_max (device-specific)
  - plasticity: STDP variants with constraints
  - notes: prefer on-chip weights; sparse CSR recommended
- TrueNorth
  - dtypes: binary/low-bit weights
  - plasticity: static only
  - notes: strict fan-in limits; recommend partition-first strategies
- Akida
  - dtypes: low-bit quant; rate-based pipelines supported
  - delays: limited; recommend zero/low delay
- SpiNNaker
  - execution: time-stepped event simulation
  - placement: core/memory constraints per chip
- Neurogrid
  - analog constraints; prefer rate/analog transmissions
- DYNAPs
  - event-driven; sparse connectivity preferred; plasticity limited
- MemXbar
  - analog crossbar; quantization and drift constraints
- Custom ASIC
  - declare explicit constraints; treat as contract between compiler and silicon team

Feature flags and selection
- Cargo features/backends config keys should follow: backend_{name} (e.g., backend_loihi2)
- A backend selection document should map features to pass pipeline variants

Cross-links
- IR requirements: see [nir.md](nir.md)
- Pass pipeline: see [passes.md](passes.md)

Compliance
- Each backend must provide a minimal suite of conformance tests (fixtures + goldens) verifying that:
  - Delay and dtype constraints are enforced
  - Plasticity unsupported paths produce actionable diagnostics
  - Layout/quant decisions obey memory topologies
