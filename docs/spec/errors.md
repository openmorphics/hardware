# Error Taxonomy

This workspace uses thiserror-based error enums within crates and maps high-level CLI outcomes to standardized exit codes.

Core categories
- ImportError: Parsing and format detection failures (JSON/YAML/Frontend adapters)
- LoweringError: Pass pipeline construction and execution failures
- CapabilityError: HAL manifest validation errors and capability violations
- MappingError: Partition/placement/routing/timing resource violations
- RuntimeError: Simulator/backends emission/runtime failures

CLI exit codes
- 0: Success
- 1: Usage/IOError (file not found, unreadable input)
- 2: ParseError (invalid JSON/YAML or frontend import)
- 3: ValidationError (NIR structural issues or HAL manifest invalid)
- 4: CapabilityError (resource/cap violations against HAL)
- 5: CompileError (backend failure)
- 6: SimError (simulator artifact emission failure)
- 7: InternalError (unexpected)

Guidelines
- Library crates define error enums with thiserror and convert to anyhow for ergonomic returns.
- The CLI maps errors to exit codes; errors should include context and hints for remediation.
- Exit code policy is stable; changes require a major version bump and doc updates.

Examples
- CapabilityError: "invalid manifest field: capabilities.max_neurons_per_core (> 0 required)"
- MappingError: "CORE_MEMORY_EXCEEDED part=3 estimate_kib=123.4 cap_kib=64"
