# RISC-V Backend Release Readiness Plan

Scope
- This plan governs release readiness for the RISC-V backend across three profiles: rv64gcv Linux (linux_user), rv32imac bare metal (bare_metal), rv64gc control plane (control_plane).
- Applies to core compiler, HAL, passes, backend, CLI, Python bindings, docs, and CI.
- Code references for context:
  - Backend compile dispatcher [compile()](crates/backend_riscv/src/lib.rs:112)
  - HAL RISC-V capabilities and validation [Capabilities](crates/hal/src/lib.rs:36), [validate_manifest()](crates/hal/src/lib.rs:147)
  - Pass framework [Pass](crates/passes/src/lib.rs:18), [ValidatePass.run()](crates/passes/src/lib.rs:30)
  - Backend profile pipeline [run_pipeline_and_collect_meta()](crates/backend_riscv/src/lib.rs:170)

Release objectives and success criteria
- P0: Deterministic compile and optional simulation for all three profiles on Linux CI when tools are available; compile-only paths must succeed without tools.
- P0: CI green across build, test, and RISC-V runtime smoke tests.
- P0: Python examples runnable with CI-built wheel that includes backend-riscv.
- P0: Docs (mdBook + README) consistent, build without errors, and reference the correct CLI invocation.
- P1: Basic performance baseline captured (compile time + kernel step metrics), no >15% regression vs baseline.
- P1: Security and dependency scans clean or with documented allow-list.

Risk taxonomy and prioritization
- Priority levels:
  - P0 blocker: must fix before release.
  - P1 near-term: fix within one iteration post-release or before GA tag.
  - P2 backlog: scheduled improvements with low release risk.

P0 release blockers and resolution strategies
1) CI job duplication and coverage
- Issue: Two jobs share the same name “riscv-renode”, causing one to shadow the other.
- Evidence: [ci.yml](.github/workflows/ci.yml:50), [ci.yml](.github/workflows/ci.yml:75)
- Resolution:
  - Rename jobs to “riscv-qemu-user-linux” and “riscv-renode-control-plane”. Merge any duplicate steps.
  - Add separate “riscv-qemu-system-bare” job for bare-metal runtime.
- Acceptance:
  - All three jobs appear in Actions, execute, and report logs.

2) Missing bare-metal toolchains in CI
- Issue: CI installs qemu-user but not qemu-system-riscv32 or an ELF GCC for bare-metal.
- Evidence: package installs show only qemu-user and riscv64-linux-gnu GCC at [ci.yml](.github/workflows/ci.yml:62)
- Resolution:
  - Install qemu-system-riscv32 and a cross-ELF toolchain (e.g., gcc-riscv64-unknown-elf, or build via xPack/LLVM).
  - Gate tests with env (e.g., CI_RISCV_BARE=1) to keep CI time bounded.
- Acceptance:
  - Bare-metal compile+qemu-system smoke test passes and captures JSONL.

3) Python wheels omit backend-riscv
- Issue: CI-built wheels do not include the RISC-V backend, breaking tutorial parity.
- Evidence: wheel build flags at [ci.yml](.github/workflows/ci.yml:114) lack backend-riscv.
- Resolution:
  - Add a dedicated job “python-wheels-riscv” with flags: -F "python backend-riscv telemetry".
  - Optionally include sims if needed by tutorials, but keep size minimal.
- Acceptance:
  - Import succeeds and list_targets contains riscv64gcv_linux, riscv32imac_bare, riscv64gc_ctrl.

4) CLI naming inconsistency
- Issue: README uses “neuroc” while backend docs prefer “neuro-compiler”.
- Evidence: [README.md](README.md:55) vs [docs/backends/riscv.md](docs/backends/riscv.md:130)
- Resolution:
  - Standardize on the installed binary name. If the binary is “neuro-compiler”, update README examples; if “neuroc” is correct, update backend docs.
  - Add a short alias note to avoid user confusion.
- Acceptance:
  - Grep search for the alternative name shows only one canonical form across docs/tutorials.

5) Runtime tests breadth
- Issue: Current CI only runs Renode control-plane smoke; linux_user and bare_metal runtime paths not exercised.
- Resolution:
  - Add smoke tests that run compile+qemu-user for linux_user when NC_RISCV_QEMU_RUN=1.
  - Add compile+qemu-system for bare_metal when CI_RISCV_BARE=1.
  - Tests should skip gracefully if tools are not in PATH.
- Acceptance:
  - CI logs show both profiles executed or correctly skipped with clear messages.

6) mdBook cross-links for metrics labels
- Issue: Multiple pages reference metrics labels; ensure the page exists and is linked in SUMMARY.
- Current status: labels page exists [labels.md](docs/metrics/labels.md:1). Verify SUMMARY includes a path to it (directly or via an index page).
- Resolution:
  - Add link from docs/src/metrics.md to the labels page and ensure relative paths correct.
- Acceptance:
  - mdBook build yields no broken links; labels and profiling pages are reachable from sidebar.

P1 high-priority improvements (post-blockers)
7) Performance baselines and regression guard
- Add Criterion benches for representative kernels and track:
  - compile time per profile (CLI end-to-end)
  - kernel.step_ns and cpu.cycle from JSONL produced by runtime smoke
- Store artifacts under target/bench/ and publish summary in CI artifacts.
- Acceptance: CI produces a CSV summary; manual thresholding documented (automated gate optional).

8) Security and supply-chain scanning
- Integrate cargo-audit and cargo-deny (licenses + bans).
- Pin GitHub Actions versions; prefer SHA pinning for 3rd-party actions.
- Consider Bandit (if any Python helper scripts are added later).
- Acceptance: CI job “security-scan” passes or has documented allow-lists.

9) Dependency hygiene and conflicts
- Run cargo update -Z minimal-versions in a matrix to catch overly tight constraints.
- Run cargo tree -d to identify duplicates and feature unification opportunities.
- Acceptance: No unresolved duplicate crates with incompatible versions; document any intentional duplicates.

10) Test coverage depth
- Add integration tests for:
  - HAL validation edge-cases (isa/abi mismatches, vector flags, mmio/dma constraints).
  - Backend pipeline metadata assertions (rv_layout, rv_schedule, rv_bare_tuning, rv_ctrl_plane).
  - Python SDK flows for each profile (skip if wheel lacks features).
- Acceptance: Coverage report shows increased lines/branches in backend_riscv and hal.

11) Host compatibility matrix
- Validate on Linux (CI) and provide instructions for macOS (Homebrew QEMU) and Windows (WSL recommended).
- Acceptance: Docs include per-OS setup notes; known limitations called out explicitly.

12) Documentation polish
- Add a “Tooling prerequisites” box to RISC-V docs with package names per OS.
- Ensure consistent sample outputs for telemetry summary to reduce user guesswork.
- Acceptance: Docs PR build renders cleanly; reviewers sign off.

P2 backlog and technical debt
13) Pass realism and fidelity
- Current RISC-V passes are architecture-aware stubs; plan deeper vectorization, scheduling, memory layout realism.
- Track as RFCs with measurable wins (e.g., RVV utilization).

14) Accelerator control-plane fidelity
- Expand Renode device model to cover DMA bursts, error paths, and backpressure; add assertions.

15) Multi-chip / distributed orchestration
- Extend partition/placement/routing to model inter-chip constraints and introduce cluster manifests.

16) ML-based optimizations
- Introduce an opt-in mlopt pass pipeline; curated datasets and reward functions (energy/throughput).

Validation matrix (what we run and where)
- Linux CI (required):
  - Build: cargo build --workspace --all-features
  - Tests: cargo test --workspace
  - RISC-V linux_user: compile + qemu-user smoke (NC_RISCV_QEMU_RUN=1)
  - RISC-V bare_metal: compile + qemu-system smoke (CI_RISCV_BARE=1)
  - RISC-V control_plane: renode smoke
- Local developer (optional):
  - macOS: brew install qemu; clang --target for cross-compiles; run linux_user smoke locally.
  - Windows: WSL Ubuntu recommended; same steps as Linux.

Acceptance criteria (summary)
- All P0 items closed.
- CI shows three RISC-V jobs passing (user/system/renode) and wheel build with backend-riscv.
- Tutorials runnable end to end using CI wheel on Linux host.
- mdBook and README consistent and link-complete.

Detailed remediation tasks (checklist)
- CI
  - Rename duplicate job(s) and split into three: qemu-user, qemu-system, renode.
  - Install toolchains: qemu-system-riscv32, gcc-riscv64-unknown-elf (or equivalent).
  - Add runtime-smoke tests (skip-if-missing), each outputs profile.jsonl and prints summary.
- Python wheels
  - Add job “python-wheels-riscv” that builds wheel with -F "python backend-riscv telemetry".
  - Smoke test: import, list_targets contains RISC-V targets; run a minimal linux_user compile.
- Documentation
  - Standardize CLI command naming across [README.md](README.md:39) and [docs/backends/riscv.md](docs/backends/riscv.md:1).
  - Verify metrics labels links from [docs/src/metrics.md](docs/src/metrics.md:1) to [docs/metrics/labels.md](docs/metrics/labels.md:1).
- Testing
  - Add backend_riscv integration tests per profile; skip when tools absent; assert WARN.txt or JSONL presence.
  - HAL negative tests for rv isa/abi/vector/mmio/dma invariants.
- Security & deps
  - Add cargo-audit and cargo-deny jobs; add minimal-versions matrix.

Rollout and rollback
- Rollout: Tag release after P0 items close and CI passes on main; publish wheels including RISC-V variant.
- Monitoring: Track issue label “riscv-release” for 2 weeks; watch CI for flakes and performance drifts.
- Rollback: If a P0 regression appears post-release, publish a patch release disabling runtime tests by default (docs remain), or temporarily remove RISC-V wheel from artifacts while fixing.

Ownership and approvals
- Workstream “CI”: Release engineer
- Workstream “Backend”: RISC-V backend owner
- Workstream “Docs”: Tech writer
- Workstream “Python”: Python SDK owner
- Sign-off required from all owners plus project maintainer before tagging.

Appendix: commands and hints
- Install tools (Ubuntu):
  - sudo apt-get update && sudo apt-get install -y qemu-user qemu-user-static qemu-system-misc gcc-riscv64-linux-gnu gcc-riscv64-unknown-elf
- macOS:
  - brew install qemu llvm
- Verify RISC-V targets exposed:
  - cargo run -p neuro-compiler-cli -- list-targets | grep riscv
- Validate HAL manifests:
  - cargo run -p neuro-compiler-cli -- list-targets (ensures embed + parsing) and dedicated hal tests.

References
- Backend: [crates/backend_riscv/src/lib.rs](crates/backend_riscv/src/lib.rs:1)
- HAL: [crates/hal/src/lib.rs](crates/hal/src/lib.rs:1)
- Passes: [crates/passes/src/lib.rs](crates/passes/src/lib.rs:1)
- CI workflow: [.github/workflows/ci.yml](.github/workflows/ci.yml:1)
- Backend docs: [docs/backends/riscv.md](docs/backends/riscv.md:1)
- Python usage: [docs/python/usage.md](docs/python/usage.md:1)
- Tutorial: [docs/tutorials/riscv_pysdk_quickstart.md](docs/tutorials/riscv_pysdk_quickstart.md:1)

---

Category-by-category checklist and resolution strategies

A. Technical debt (architecture and code quality)
- What to check:
  - RISC-V pass realism and naming consistency with the global pass catalog. Ensure passes evolve toward real transformations.
  - Remove dead code and consolidate duplicated helpers in backend runners (QEMU, Renode).
  - Ensure every WARNING path writes actionable context into WARN.txt with next steps.
- Actions:
  - Track pass fidelity improvements via RFCs, linking to [passes.ValidatePass.run()](crates/passes/src/lib.rs:30).
  - Add lint gates (clippy + rustfmt on CI) and deny(warnings) for backend_riscv.
  - Add code owners for backend files: [backend_riscv.compile()](crates/backend_riscv/src/lib.rs:112).
- Acceptance:
  - Clippy clean on --all-features; no unreachable or unused warnings in backend_riscv.

B. Integration failures (E2E flows)
- What to check:
  - End-to-end compile-to-run for each profile with optional simulator.
  - HAL manifest attachment/read path correctness across CLI and Python.
- Actions:
  - E2E Smoke tests (skip-if-missing tools):
    - linux_user: compile + qemu-user run, JSONL captured.
    - bare_metal: compile + qemu-system run, UART JSONL captured.
    - control_plane: compile + Renode run, MMIO/DMA exercised (minimal).
  - Verify HAL round-trip from graph attr: hal_manifest_path read in passes; see [hal.validate_manifest()](crates/hal/src/lib.rs:147).
- Acceptance:
  - CI logs show compiled artifact path and profile.jsonl summary printed per profile.

C. Performance regressions (baseline + guardrails)
- Metrics to track:
  - Compile time (ms) for backend_riscv compile per profile (timer recorded when telemetry enabled).
  - Binary size: linux_user ELF size; bare_metal firmware.elf size.
  - Runtime kernel metrics: kernel.step_ns, cpu.cycle, cpu.instret (when available).
- Actions:
  - Add Criterion compile-time benches (or timing via CLI + JSONL parsers).
  - Establish baselines on CI; set soft thresholds (e.g., ±15%) with alert logs; no hard gate initially.
- Acceptance:
  - CI artifact “riscv-baseline.csv” with last N runs; manual review on release branch.

D. Documentation gaps
- What to check:
  - CLI name consistency across README and backend docs.
  - Toolchain prerequisites per OS and simulator versions.
  - Metrics labels page linked from Overview -> Metrics.
- Actions:
  - Standardize CLI invocations in [README.md](README.md) and [docs/backends/riscv.md](docs/backends/riscv.md:1).
  - Add “Prerequisites” block for each OS in RISC-V docs.
  - Ensure metrics labels page is referenced from docs/src/metrics.md and present at [docs/metrics/labels.md](docs/metrics/labels.md:1).
- Acceptance:
  - mdBook builds with no broken links; consistent CLI naming in examples.

E. Test coverage deficiencies
- What to add:
  - HAL negative tests (isa/abi mismatch, vector flag/isa, mmio/dma constraints).
  - Backend pipeline attribute assertions (rv_layout, rv_schedule, rv_bare_tuning, rv_ctrl_plane).
  - Runtime smoke tests per profile (skip-if-missing).
  - Python wheel smoke for RISC-V (import + list_targets + minimal compile).
- Actions:
  - Extend hal tests around [Capabilities](crates/hal/src/lib.rs:36).
  - Add backend_riscv integration tests that assert README.txt meta lines exist post-pipeline.
- Acceptance:
  - Coverage trend up for backend_riscv and hal; CI prints explicit “SKIPPED (tool not found)” when skipping runtime.

F. Dependency conflicts
- What to run:
  - cargo tree -d for duplicates.
  - cargo update -Z minimal-versions to catch version floor issues.
  - cargo deny for license and bans.
- Actions:
  - Unify versions where feasible; document intentional splits.
- Acceptance:
  - No unresolved duplicates; deny passes with allow-listed rationale if needed.

G. Security vulnerabilities (supply chain and runtime)
- What to scan:
  - cargo audit (RUSTSEC), cargo deny (vuln + license).
  - GitHub Actions pinning; prefer major@SHA for third-party actions.
  - Python: pinned maturin; any Python deps (e.g., renode-colab) installed with hashes where possible.
- Runtime hardening:
  - For generated C: add -fstack-protector-strong -D_FORTIFY_SOURCE=2 when supported; disable unused syscalls in bare-metal.
  - Validate environment inputs (NC_PROFILE_JSONL path safety) in Rust side when opening files.
- Acceptance:
  - Security CI job green; doc notes any accepted risk with mitigation.

H. Compatibility issues per profile (deep dive)
- linux_user (rv64gcv):
  - Toolchain: riscv64-linux-gnu-gcc or clang --target=riscv64-unknown-linux-gnu; qemu-user (qemu-riscv64).
  - Vector fallback: Generated C guards RVV with __riscv_vector; scalar fallback compiled when V unsupported.
  - ABI: lp64/lp64d per [Capabilities.abi](crates/hal/src/lib.rs:66) rules; ensure validate rejects rv64 + ilp32.
  - Known issues: CSR access may be restricted; cpu.cycle/instret may read zero.
  - Acceptance: Program runs under qemu-user; telemetry JSONL written; WARN.txt notes vector fallback when applicable.
- bare_metal (rv32imac):
  - Toolchain: riscv64-unknown-elf-gcc (multilib) with -march=rv32imac -mabi=ilp32; qemu-system-riscv32.
  - Board: “virt”; UART at 0x1000_0000; finisher at 0x0010_0000; linker.ld places RAM at 0x8000_0000.
  - Constraints: no FPU; compressed C optional; size optimization enabled in [RvBaremetalTuningPass](crates/backend_riscv/src/lib.rs:73).
  - Acceptance: firmware.elf runs; UART JSONL captured; WARN.txt if tools missing.
- control_plane (rv64gc):
  - Toolchain: riscv64-linux-gnu-gcc or clang; Renode installed.
  - MMIO/DMA: validate mmio_width_bits ∈ {32,64}, dma_alignment power-of-two; see [validate_manifest()](crates/hal/src/lib.rs:237).
  - Simulation: generated accelerator.repl/accelerator.py/run.resc; run_renode captures telemetry.
  - Acceptance: Renode runs and exits; MMIO operations count ≥ 1 in telemetry.

Release test matrix (profiles × env × actions)
- linux_user:
  - Build-only: cargo test -p nc-backend-riscv (no tools needed).
  - Runtime smoke: set NC_RISCV_QEMU_RUN=1 and NC_PROFILE_JSONL=run.jsonl; compile; verify JSONL lines exist.
- bare_metal:
  - Build-only: compile to emit crt0.S/linker.ld/main.c; create README.txt.
  - Runtime smoke: qemu-system-riscv32 -nographic runs; JSONL captured.
- control_plane:
  - Build-only: emit main.c + renode artifacts.
  - Runtime smoke: renode script runs; telemetry log extracted.

Go/No-Go checklist (final gate)
- CI:
  - Three RISC-V jobs present (user/system/renode) and green. See [.github/workflows/ci.yml](.github/workflows/ci.yml:1).
  - Wheel job “python-wheels-riscv” green; wheel smoke passes.
  - Security scan jobs green (audit/deny).
- Artifacts:
  - profile.jsonl produced in all runtime smokes with non-empty metrics.
  - WARN.txt absent for tool-available runs (or contains only informational messages).
- Docs:
  - mdBook build clean; README/Backend docs consistent on CLI name; labels page linked.
- Owners sign-off:
  - Backend, HAL, CLI, Docs, Python.

Ownership and timelines
- CI and Security scans: Release Eng (Day 1–2)
- Backend runtime tests (QEMU/ Renode): Backend owner (Day 1–3)
- Python wheel RISC-V variant + smoke: Python owner (Day 2–3)
- Docs alignment and link audit: Tech writer (Day 1–2)
- Go/No-Go review: Maintainer + owners (Day 4)

Concrete commands for smokes (reference)
- linux_user:
  - export NC_RISCV_QEMU_RUN=1; export NC_PROFILE_JSONL=run.jsonl
  - cargo run -p neuro-compiler-cli -- compile --input examples/nir/simple.json --target riscv64gcv_linux
- bare_metal:
  - export NC_RISCV_QEMU_RUN=1; export NC_PROFILE_JSONL=run.jsonl
  - cargo run -p neuro-compiler-cli -- compile --input examples/nir/simple.json --target riscv32imac_bare
- control_plane:
  - export NC_RISCV_QEMU_RUN=1; export NC_PROFILE_JSONL=run.jsonl
  - cargo run -p neuro-compiler-cli -- compile --input examples/nir/simple.json --target riscv64gc_ctrl

References (quick links)
- Backend entrypoint: [compile()](crates/backend_riscv/src/lib.rs:112)
- Backend pipeline: [run_pipeline_and_collect_meta()](crates/backend_riscv/src/lib.rs:170)
- HAL schema and validation: [Capabilities](crates/hal/src/lib.rs:36), [validate_manifest()](crates/hal/src/lib.rs:147)
- Pass framework: [Pass](crates/passes/src/lib.rs:18), [ValidatePass](crates/passes/src/lib.rs:29)
- CI workflow: [ci.yml](.github/workflows/ci.yml:1)
- RISC-V docs: [docs/backends/riscv.md](docs/backends/riscv.md:1)
- Python usage: [docs/python/usage.md](docs/python/usage.md:1)
- Tutorial: [docs/tutorials/riscv_pysdk_quickstart.md](docs/tutorials/riscv_pysdk_quickstart.md:1)
