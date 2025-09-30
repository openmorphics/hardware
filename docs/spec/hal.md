# HAL (Hardware Abstraction Layer)

This document specifies the Target Manifest schema and the capabilities used by the compiler to reason about hardware constraints and optimizations.

Manifest file format: TOML
- Location: targets/<name>.toml
- Parsed by: nc_hal::parse_target_manifest_path()
- Validated by: nc_hal::validate_manifest()

Core fields:
- name: string (non-empty)
- vendor: string (non-empty)
- family: string (free-form family/group)
- version: string (hardware generation)
- notes: optional string
- [capabilities]: optional table describing hardware properties

Capabilities (all optional; if present, validated for >0 where numeric):
- on_chip_learning: bool
- weight_precisions: [u32] — allowed weight bitwidths (must be >0)
- max_neurons_per_core: u32 — maximum neurons per core (>0)
- max_synapses_per_core: u32 — maximum synapses per core (>0)
- time_resolution_ns: u64 — tick resolution in nanoseconds (>0)
- supports_sparse: bool — whether sparse synaptic matrices are natively supported
- neuron_models: [string] — supported neuron model identifiers (e.g., "LIF", "Izhikevich")
- max_fan_in: u32 — per-neuron maximum incoming synapses (>0)
- max_fan_out: u32 — per-neuron maximum outgoing synapses (>0)
- core_memory_kib: u32 — approximate per-core memory in KiB (>0)
- interconnect_bandwidth_mbps: u32 — on-chip/off-chip bandwidth in Mbps (>0)
- analog: bool — analog (true) vs digital (false) signaling/compute emphasis
- on_chip_plasticity_rules: [string] — supported on-chip learning rules (e.g., "STDP")
- neuron_mem_kib_per: f64 — approximate memory footprint per neuron in KiB (>0.0)
- syn_mem_kib_per: f64 — approximate memory footprint per synapse in KiB (>0.0)
- bytes_per_event: u32 — size in bytes per spike/event transferred over interconnect (>0)
- default_spike_rate_hz: f64 — default spike rate used for coarse bandwidth estimates (>0.0)

Built-in targets (see files under targets/):
- loihi2, truenorth, akida, spinnaker2, neurogrid, dynaps, memxbar, custom_asic

Compiler usage:
- The HAL validator enforces basic consistency checks for numerical fields and non-empty identifiers.
- Backends and passes consume capabilities to drive quantization, partitioning, routing, and legality checks.
- Unknown or omitted fields are treated as “unspecified.” Passes should adopt conservative fallbacks.

Change policy:
- Adding new optional capability fields is a non-breaking change.
- Tightening validation or changing semantics requires a version bump and migration notes.

Modeling guidance:
- neuron_mem_kib_per and syn_mem_kib_per are used by placement/resource passes to estimate per-part memory usage; choose values that reflect implementation detail (e.g., ~10–20B/neuron and ~2–4B/synapse equivalent).
- bytes_per_event and default_spike_rate_hz drive coarse interconnect bandwidth estimation in the routing pass; prefer conservative over-estimates for safety.
- Omitted fields fall back to conservative defaults; prefer specifying all modeling fields for accurate diagnostics on real hardware.

## CPU/RISC-V capabilities (optional)

These fields are additive and backward-compatible. They allow targets to describe RISC‑V specific execution environments. All fields are optional.

- ISA/ABI/Extensions
  - isa: string — e.g., "rv64gcv", "rv32imac", "rv64gc"
  - abi: string — e.g., "lp64d", "ilp32"
  - has_a, has_c, has_f, has_d, has_b, has_p: bool — standard extension flags (atomics, compressed, FP, double FP, bitmanip, DSP)
  - has_vector: bool — whether RVV is available
  - vlen_bits_max: u32 — maximum vector length in bits (>0 when has_vector = true)
  - zvl_bits_min: u32 — minimum legal vector length in bits (must be ≤ vlen_bits_max; both multiples of 8 when both present)
  - vlen_is_dynamic: bool — true if VLEN can vary at runtime
  - has_zicntr, has_zihpm: bool — standard counter/PMU extensions
  - extensions: [string] — free-form extension strings, e.g., ["zba","zbb","zbs","v"]
- Memory model / layout
  - endianness: "little" | "big"
  - cacheline_bytes: u32 — power-of-two (>0)
  - icache_kib, dcache_kib, l2_kib: u32 — sizes in KiB
  - page_size_bytes: u32 — power-of-two (>0), OS profiles
  - code_model: "medlow" | "medany" | "small"
- MMIO/DMA
  - mmio_supported: bool
  - mmio_base_addr: u64 — >0 when mmio_supported = true
  - mmio_width_bits: u32 — 32 or 64 when mmio_supported = true
  - dma_supported: bool
  - dma_alignment: u32 — power-of-two (>0) when dma_supported = true
- Profile
  - profile: string — e.g., "linux_user", "bare_metal", "control_plane" (free-form)

Validation rules (best-effort; enforced only when fields are present):
- Vector constraints:
  - If has_vector = true ⇒ vlen_bits_max must be > 0
  - If both zvl_bits_min and vlen_bits_max are present ⇒ zvl_bits_min ≤ vlen_bits_max and both are multiples of 8
  - If isa present and has_vector = true ⇒ isa contains 'v'
- ISA/ABI width coherence for RISC-V manifests (family contains "risc" or isa present):
  - If isa starts with "rv32" and abi is present ⇒ abi starts with "ilp32"
  - If isa starts with "rv64" and abi is present ⇒ abi starts with "lp64"
- MMIO/DMA constraints:
  - If mmio_supported = true ⇒ mmio_base_addr > 0 and mmio_width_bits ∈ {32, 64}
  - If dma_supported = true ⇒ dma_alignment > 0 and is power-of-two
- Memory/layout:
  - If endianness present ⇒ must be "little" or "big"
  - If cacheline_bytes present ⇒ power-of-two (>0)
  - If page_size_bytes present ⇒ power-of-two (>0)
  - If code_model present ⇒ one of {"medlow","medany","small"}
