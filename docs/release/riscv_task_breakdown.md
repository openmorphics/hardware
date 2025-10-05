
# RISC-V Backend Release Hardening — Task Breakdown Structure (TBS)

1.0 Executive Overview

- Objective: Release-harden the RISC-V backend across supported profiles: rv64gcv/linux_user, rv32imac/bare_metal, rv64gc/control_plane. Ensure deterministic pipelines, CI reliability, coverage, docs alignment, performance baselines, and clear rollback.
- Scope anchors: backend entrypoint [compile()](crates/backend_riscv/src/lib.rs:112) and pipeline [run_pipeline_and_collect_meta()](crates/backend_riscv/src/lib.rs:170); pass entries [RvLowerPass.run()](crates/backend_riscv/src/lib.rs:11), [RvLayoutPass.run()](crates/backend_riscv/src/lib.rs:16), [RvSchedulePass.run()](crates/backend_riscv/src/lib.rs:35), [RvVectorizePass.run()](crates/backend_riscv/src/lib.rs:54), [RvBaremetalTuningPass.run()](crates/backend_riscv/src/lib.rs:74), [RvControlPlanePass.run()](crates/backend_riscv/src/lib.rs:89); HAL [Capabilities](crates/hal/src/lib.rs:36), [builtin_targets()](crates/hal/src/lib.rs:20), [validate_manifest()](crates/hal/src/lib.rs:147); pass framework [ValidatePass.run()](crates/passes/src/lib.rs:30), [PartitionPass.run()](crates/passes/src/lib.rs:74), [PlacementPass.run()](crates/passes/src/lib.rs:138), [RoutingPass.run()](crates/passes/src/lib.rs:243), [TimingPass.run()](crates/passes/src/lib.rs:309); CI workflow [ci.yml](.github/workflows/ci.yml:1).
- Success criteria: CI required jobs are green; runtime smokes emit JSONL with minimal schema; coverage increases for backend_riscv and hal; performance within ±15% of baseline; docs build clean with link integrity; license consistency; defined rollbacks verified.

2.0 Phased WBS (Work Breakdown Structure) with priorities

2.1 Phase 0: Governance & Tracking

2.1.1 Establish RISC-V release governance and tracking
- Priority: P0 blocker
- Risk: Medium — cross-team coordination and missing env agreements
- Effort: S (0.5–1d)
- Ownership: Release Eng
- Dependencies: Existing baseline checklist [docs/release/riscv_release_readiness.md](docs/release/riscv_release_readiness.md:1)
- Acceptance criteria: Board exists with all deliverables, owners, due dates, and status columns; Go/No-Go checklist drafted and stored next to this TBS.
- Verification: Link to board from README of release folder; PR adding checklist reviewed by Maintainer.
- Integration checkpoint: CI-1 metadata: Add job that echoes release metadata JSON as artifact.
- Rollback: Remove board link and metadata job; revert PR.
- Success metrics: Board maintained weekly; zero unassigned P0/P1 items.

2.1.2 Define supported profiles and target matrix
- Priority: P0 blocker
- Risk: Low — definitions exist but may drift
- Effort: S
- Ownership: Backend Owner
- Dependencies: HAL targets [builtin_targets()](crates/hal/src/lib.rs:20), docs [docs/backends/riscv.md](docs/backends/riscv.md:1)
- Acceptance criteria: Matrix enumerates rv64gcv/linux_user, rv32imac/bare_metal, rv64gc/control_plane with ISA/ABI, toolchains, simulators, CI job mapping.
- Verification: Matrix committed in this file; referenced by CI job names in [ci.yml](.github/workflows/ci.yml:1).
- Integration checkpoint: CI-1 — jobs created with matching names.
- Rollback: Revert matrix section.
- Success metrics: Zero ambiguity in PR reviews about profile mapping.

2.1.3 Define gating environment flags for heavy tests
- Priority: P0 blocker
- Risk: Medium — accidental permanent skips
- Effort: S
- Ownership: Release Eng
- Dependencies: CI env injection in [ci.yml](.github/workflows/ci.yml:1)
- Acceptance criteria: Flags defined and documented: NC_RISCV_QEMU_USER=1, NC_RISCV_QEMU_SYSTEM=1, NC_RISCV_RENODE=1, NC_RISCV_SKIP_HEAVY=1; tests skip gracefully if tools missing.
- Verification: CI logs show skip messages when flags off; unit test asserts skip reason strings.
- Integration checkpoint: CI-1
- Rollback: Set flags to 0; mark jobs non-required.
- Success metrics: No hard failures when tools absent; deterministic skipping.

2.1.4 Assign owners and SLAs
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Maintainer
- Dependencies: Roles defined in Section 4.0
- Acceptance criteria: Each deliverable has a named DRI; SLA for PR review (<2 business days) documented.
- Verification: This doc updated; CODEOWNERS if applicable.
- Integration checkpoint: None
- Rollback: Revert owner assignments
- Success metrics: Review SLA met in >90% of PRs for release items.

2.1.5 Artifact retention policy
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Release Eng
- Dependencies: GitHub retention config in [ci.yml](.github/workflows/ci.yml:1)
- Acceptance criteria: JSONL/CSV artifacts retained 30 days; artifact names standardized.
- Verification: CI artifacts accessible; names match convention.
- Integration checkpoint: CI-1
- Rollback: Revert retention changes
- Success metrics: Artifacts available for last 30 days without manual re-runs.

2.1.6 Go/No-Go criteria finalized
- Priority: P0 blocker
- Risk: Low
- Effort: S
- Ownership: Maintainer
- Dependencies: Success metrics (8.0)
- Acceptance criteria: Go requires all P0 green and ≥80% P1 done; No-Go path documented in Section 7.0.
- Verification: Maintainer signs checklist
- Integration checkpoint: None
- Rollback: N/A
- Success metrics: Binary decision recorded prior to tag.

2.2 Phase 1: CI & Tooling

2.2.1 Normalize CI jobs and split RISC-V runs
- Priority: P0 blocker
- Risk: Medium — job matrix churn and cache invalidations
- Effort: M (1–3d)
- Ownership: Release Eng
- Dependencies: CI workflow [ci.yml](.github/workflows/ci.yml:1)
- Acceptance criteria: Rename duplicated riscv-renode job; create: riscv-qemu-user-linux, riscv-qemu-system-bare, riscv-renode-control-plane; heavy tests gated by flags; missing tools cause graceful skip.
- Verification: CI run shows three jobs; logs show expected gating; artifacts uploaded.
- Integration checkpoint: CI-1
- Rollback: Revert CI job names to prior single job; disable required status checks.
- Success metrics: Required jobs green in 3 consecutive main runs.

2.2.2 Add qemu-system-riscv32 and riscv64-unknown-elf GCC for bare-metal
- Priority: P0 blocker
- Risk: Medium — package availability on CI images
- Effort: M
- Ownership: Release Eng
- Dependencies: OS package installers; tool detection
- Acceptance criteria: qemu-system-riscv32 and riscv64-unknown-elf-{gcc,binutils} installed in CI job; --version printed in logs.
- Verification: CI logs show versions; tool presence check passes.
- Integration checkpoint: CI-1
- Rollback: Guard steps behind NC_RISCV_QEMU_SYSTEM; remove installer
- Success metrics: Bare-metal smoke can boot in emulator.

2.2.3 Tool presence detection and graceful skip
- Priority: P0 blocker
- Risk: Low
- Effort: S
- Ownership: Release Eng
- Dependencies: CI shell steps in [ci.yml](.github/workflows/ci.yml:1)
- Acceptance criteria: If tools absent, job exits 0 with 'SKIPPED: missing <tool>' message and sets outputs to 'skipped'.
- Verification: Simulate by disabling installer; job outcome is neutral/green.
- Integration checkpoint: CI-1
- Rollback: Remove skip logic
- Success metrics: Zero red builds due to tool absence.

2.2.4 Python wheels with backend-riscv
- Priority: P0 blocker
- Risk: Medium — packaging and feature flags
- Effort: M
- Ownership: Python Owner
- Dependencies: Wheel job 'python-wheels-riscv' in [ci.yml](.github/workflows/ci.yml:1), features -F 'python backend-riscv telemetry'
- Acceptance criteria: Wheel build includes backend_riscv; smoke test imports module; nc.list_targets_py() outputs RISC-V targets.
- Verification: CI job log shows import success; artifact wheel exists.
- Integration checkpoint: PY-1
- Rollback: Disable wheel publishing; mark job optional; delete artifacts.
- Success metrics: Wheel usable in downstream environment.

2.2.5 Security and dependency hygiene
- Priority: P1 near-term
- Risk: Medium — false positives and dependency conflicts
- Effort: M
- Ownership: Release Eng
- Dependencies: cargo-audit, cargo-deny, cargo tree -d, minimal-versions
- Acceptance criteria: CI jobs pass without high severity advisories; minimal-versions builds; no duplicate dependencies.
- Verification: CI logs attached; artifacts of audit reports uploaded.
- Integration checkpoint: CI-1
- Rollback: Temporarily allowlist advisories with expiration
- Success metrics: Zero high severity advisories; reduced duplicates.

2.2.6 Pin third-party GitHub Actions
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Release Eng
- Dependencies: [ci.yml](.github/workflows/ci.yml:1)
- Acceptance criteria: All actions pinned by commit SHA or version tag.
- Verification: Review PR; actionlint job passes if configured.
- Integration checkpoint: CI-1
- Rollback: Revert pins causing breakage
- Success metrics: No supply-chain warnings.

2.2.7 CI performance and caching
- Priority: P2 backlog
- Risk: Low
- Effort: M
- Ownership: Release Eng
- Dependencies: cache keys on Cargo.lock and pyproject
- Acceptance criteria: Cache hit ratio ≥70%; CI time reduced by ≥20% vs baseline.
- Verification: CI timing comparison; cache logs.
- Integration checkpoint: CI-1
- Rollback: Remove caching steps
- Success metrics: Sustained CI time improvements.

2.3 Phase 2: Testing & Coverage

2.3.1 Runtime smoke: linux_user (rv64gcv)
- Priority: P0 blocker
- Risk: Medium — emulator nondeterminism
- Effort: M
- Ownership: Backend Owner
- Dependencies: [compile()](crates/backend_riscv/src/lib.rs:112), [run_pipeline_and_collect_meta()](crates/backend_riscv/src/lib.rs:170), qemu-user
- Acceptance criteria: With NC_RISCV_QEMU_USER=1, compile minimal linux_user and run under qemu-user; produce non-empty JSONL; summarize in CI log.
- Verification: Artifact JSONL uploaded; CI log includes summary line 'linux_user smoke: OK'.
- Integration checkpoint: RT-1
- Rollback: Set NC_RISCV_QEMU_USER=0; run compile-only.
- Success metrics: 3 consecutive green runs with JSONL present.

2.3.2 Runtime smoke: bare_metal (rv32imac)
- Priority: P0 blocker
- Risk: Medium — UART capture and qemu-system availability
- Effort: M
- Ownership: Backend Owner
- Dependencies: qemu-system-riscv32; riscv64-unknown-elf toolchain; bare-metal target template in crates/backend_riscv/target
- Acceptance criteria: With NC_RISCV_QEMU_SYSTEM=1, build and boot sample; capture UART JSONL; artifact uploaded.
- Verification: CI artifact contains UART JSONL; CI log prints byte count.
- Integration checkpoint: RT-2
- Rollback: Disable job via flag; compile-only fallback.
- Success metrics: Boot completes within 60s; JSONL contains at least one event.

2.3.3 Runtime smoke: control_plane (rv64gc via Renode)
- Priority: P0 blocker
- Risk: High — Renode flakiness and licensing issues
- Effort: M
- Ownership: Backend Owner
- Dependencies: Renode; target assets under crates/backend_riscv/target/riscv64gc_ctrl-ctrl
- Acceptance criteria: With NC_RISCV_RENODE=1, run Renode script; observe MMIO ops; capture JSONL.
- Verification: CI artifact contains JSONL; CI log shows 'MMIO ops observed: N>0'.
- Integration checkpoint: RT-3
- Rollback: Disable job; accept compile-only until Renode stabilized.
- Success metrics: Flake rate <5% over 20 runs.

2.3.4 HAL negative tests
- Priority: P0 blocker
- Risk: Medium — strictness may break existing manifests
- Effort: M
- Ownership: Backend Owner
- Dependencies: [Capabilities](crates/hal/src/lib.rs:36), [validate_manifest()](crates/hal/src/lib.rs:147)
- Acceptance criteria: Add tests for isa/abi mismatch; vector flag vs ISA; mmio/dma invariants (width, alignment); page/cacheline power-of-two; tests fail with clear errors.
- Verification: cargo test passes locally and in CI; snapshots of error messages stored.
- Integration checkpoint: CI-1
- Rollback: Feature-flag stricter checks; allowlist legacy targets.
- Success metrics: Coverage for hal::validate increases; regressions blocked.

2.3.5 Backend pipeline attribute assertions
- Priority: P0 blocker
- Risk: Low
- Effort: S
- Ownership: Backend Owner
- Dependencies: Pass stubs [RvLayoutPass.run()](crates/backend_riscv/src/lib.rs:16), [RvSchedulePass.run()](crates/backend_riscv/src/lib.rs:35), [RvBaremetalTuningPass.run()](crates/backend_riscv/src/lib.rs:74), [RvControlPlanePass.run()](crates/backend_riscv/src/lib.rs:89); pass framework [ValidatePass.run()](crates/passes/src/lib.rs:30)
- Acceptance criteria: After pipeline, metadata includes rv_layout, rv_schedule, rv_bare_tuning, rv_ctrl_plane keys; unit test asserts presence for each profile.
- Verification: Test outputs include dump from [run_pipeline_and_collect_meta()](crates/backend_riscv/src/lib.rs:170).
- Integration checkpoint: CI-1
- Rollback: Scope assertions per profile; temporarily mark as P1.
- Success metrics: Assertions stable across 10 runs.

2.3.6 Entry dispatch tests
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Backend Owner
- Dependencies: [compile()](crates/backend_riscv/src/lib.rs:112)
- Acceptance criteria: Unit tests ensure profile dispatcher selects correct pipeline for each target.
- Verification: cargo test outputs
- Integration checkpoint: CI-1
- Rollback: Remove failing target until fixed
- Success metrics: All dispatch tests green.

2.3.7 Coverage reporting
- Priority: P1 near-term
- Risk: Medium — coverage noise
- Effort: M
- Ownership: Release Eng
- Dependencies: grcov/llvm-cov integration
- Acceptance criteria: Coverage for backend_riscv and hal reported; baseline captured; delta computed per PR.
- Verification: CI artifacts include LCOV/HTML; PR comment with delta.
- Integration checkpoint: CI-1
- Rollback: Disable PR comments; keep artifact only
- Success metrics: +X% coverage over baseline (target set in 8.0).

2.3.8 Flake detection
- Priority: P1 near-term
- Risk: Medium — time increase
- Effort: S
- Ownership: Release Eng
- Dependencies: CI job to retry flaked tests
- Acceptance criteria: Renode job retried up to 2x on known flake signatures.
- Verification: CI logs show retry and eventual pass/fail.
- Integration checkpoint: CI-1
- Rollback: Remove retry; mark job optional
- Success metrics: Reduced red builds due to flakes.

2.4 Phase 3: Documentation & Developer Experience

2.4.1 CLI naming and docs alignment
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Docs Writer
- Dependencies: [README.md](README.md:39), [docs/backends/riscv.md](docs/backends/riscv.md:1), [docs/python/usage.md](docs/python/usage.md:1)
- Acceptance criteria: CLI name and examples consistent; backreferences updated.
- Verification: mdBook build; manual spot checks
- Integration checkpoint: DOC-1
- Rollback: Revert doc changes
- Success metrics: No doc PR comments about naming.

2.4.2 OS prerequisites blocks
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Docs Writer
- Dependencies: Tooling choices from Phase 1
- Acceptance criteria: Linux/macOS/Windows/WSL prerequisites documented for toolchains and simulators.
- Verification: mdBook build; links valid
- Integration checkpoint: DOC-1
- Rollback: Remove blocks
- Success metrics: Fewer user setup issues reported.

2.4.3 Metrics pages and link hygiene
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Docs Writer
- Dependencies: [docs/metrics/labels.md](docs/metrics/labels.md:1), [docs/src/metrics.md](docs/src/metrics.md:1)
- Acceptance criteria: Metrics labels linked from metrics index; mdBook link-check passes; zero broken links.
- Verification: mdBook build logs; link-check output
- Integration checkpoint: DOC-1
- Rollback: Revert links
- Success metrics: 0 warnings/errors.

2.4.4 License alignment and changelog
- Priority: P1 near-term
- Risk: Low
- Effort: S
- Ownership: Docs Writer
- Dependencies: [LICENSE](LICENSE:1), project changelog
- Acceptance criteria: README license text consistent with LICENSE; changelog entry added for RISC-V backend.
- Verification: Review PR; mdBook if changelog included
- Integration checkpoint: DOC-1
- Rollback: Revert mismatched lines
- Success metrics: License checks pass.

2.5 Phase 4: Performance & Benchmarks

2.5.1 Compile-time and binary-size baselines
- Priority: P1 near-term
- Risk: Low
- Effort: M
- Ownership: Release Eng
- Dependencies: Timing capture around [compile()](crates/backend_riscv/src/lib.rs:112)
- Acceptance criteria: CI captures compile wall time and binary sizes per profile; CSV artifact uploaded.
- Verification: Artifact presence; CSV schema documented
- Integration checkpoint: CI-1
- Rollback: Remove timing wrappers; keep unit tests only
- Success metrics: Baselines established and tracked over PRs.

2.5.2 Runtime perf counters
- Priority: P1 near-term
- Risk: Medium — counter availability differences
- Effort: M
- Ownership: Backend Owner
- Dependencies: qemu/renode support for cycle/instret; kernel.step_ns metrics
- Acceptance criteria: When available, collect cpu.cycle/instret and kernel.step_ns; upload CSV.
- Verification: CI artifact present; logs show counts
- Integration checkpoint: RT-1/RT-2/RT-3
- Rollback: Disable collection on unsupported hosts
- Success metrics: Perf variability within ±15% across week.

2.5.3 Perf regression guard
- Priority: P2 backlog
- Risk: Medium — noisy metric gating PRs
- Effort: M
- Ownership: Release Eng
- Dependencies: Baselines from 2.5.1/2.5.2
- Acceptance criteria: Soft gate that warns when deltas exceed thresholds; notifies DRI.
- Verification: PR comment shows delta and status
- Integration checkpoint: CI-1
- Rollback: Disable warnings
- Success metrics: Reduced regressions post-merge.

2.6 Phase 5: Compatibility, Release, and Rollback

2.6.1 Compatibility audit across profiles
- Priority: P1 near-term
- Risk: Medium — drifting target semantics
- Effort: M
- Ownership: Backend Owner
- Dependencies: Target TOMLs under targets/, HAL [builtin_targets()](crates/hal/src/lib.rs:20)
- Acceptance criteria: Profiles compile and (where gated) run smokes; semantics documented.
- Verification: CI job matrix green; docs updated
- Integration checkpoint: RT-1/RT-2/RT-3
- Rollback: Mark profile experimental; disable runtime
- Success metrics: Consistent behavior across profiles.

2.6.2 Release packaging and sign-off
- Priority: P1 near-term
- Risk: Medium — packaging failures late
- Effort: M
- Ownership: Maintainer
- Dependencies: Wheel artifacts from 2.2.4; docs from Phase 3
- Acceptance criteria: Tag RC; run full CI; Maintainer approves Go; publish wheels and docs.
- Verification: Release page assets; PyPI/Artifacts present
- Integration checkpoint: PY-1, DOC-1
- Rollback: Yank release; unlist wheels; create hotfix branch
- Success metrics: Successful installs reported; zero critical issues in 72h.

2.6.3 Rollback readiness
- Priority: P0 blocker
- Risk: Low
- Effort: S
- Ownership: Release Eng
- Dependencies: Section 7.0
- Acceptance criteria: Documented steps exist for CI, tests, wheels, and docs; time-to-rollback ≤30 minutes.
- Verification: Dry-run rollback in staging branch
- Integration checkpoint: CI-1
- Rollback: N/A (this defines rollback)
- Success metrics: Dry run completed within SLA.

3.0 Detailed Deliverables

3.1 CI job normalization and coverage
- Rename duplicated 'riscv-renode'; split into: riscv-qemu-user-linux, riscv-qemu-system-bare, riscv-renode-control-plane.
- Add qemu-system-riscv32 and riscv64-unknown-elf GCC; print versions.
- Gate heavy runtime tests with NC_RISCV_* flags; skip gracefully if missing tools.
- Acceptance criteria: See 2.2.1–2.2.3; Integration: CI-1; Rollback: revert CI steps; Metrics: required jobs green, artifacts present.

3.2 Python wheels with backend-riscv
- Add 'python-wheels-riscv' CI job with -F 'python backend-riscv telemetry'.
- Smoke: import module; nc.list_targets_py() shows RISC