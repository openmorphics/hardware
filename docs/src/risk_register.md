# Risk Register

- R1: Coverage below threshold
  - Likelihood: Medium; Impact: Medium-High
  - Mitigation: Add tests in low-coverage crates; staged threshold increases (70% → 75% → 80%).

- R2: License conflicts (cargo-deny)
  - Likelihood: Medium; Impact: High
  - Mitigation: Replace/upgrade deps; narrowly scoped allowlist entries with expiry in \`deny.toml\`.

- R3: Artifact size regression > 15%
  - Likelihood: Medium; Impact: Medium
  - Mitigation: Audit generated artifacts, prune debug symbols where safe, adjust pass configurations.

- R4: Renode/QEMU instability
  - Likelihood: Medium; Impact: Medium
  - Mitigation: Retain SKIP semantics with clear logs; retries; compile-only fallback paths.

- R5: External docs link churn
  - Likelihood: Medium; Impact: Low-Medium
  - Mitigation: lychee accepts 429; pin critical references; prefer internal mirrors for critical docs.
