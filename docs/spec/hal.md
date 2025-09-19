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

