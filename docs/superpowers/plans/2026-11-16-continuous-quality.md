# Rust continuous quality implementation plan

**Goal:** Evolve the existing Rust skill and native auditor into a continuous quality workflow, preserving evidence levels and legitimate code.

**Spec:** User-supplied review, 16 November 2026. Rust scope only; retain existing AST, metadata, Clippy, llvm-lines and benchmark behavior.

**Architecture:** Add focused native Rust modules for baseline comparison, SARIF, project configuration, saved security evidence and risk-based verification recommendations. Keep the heuristic build dependency-free. Separate task modes from domain profiles and route detailed policies progressively.

**Choices:** Extend the existing auditor rather than rewrite it or introduce a common runtime. Preserve Criterion's `--baseline NAME`; debt baseline applies without Criterion and has an explicit alias when both are needed. Verification is a recommendation plan, never an automatic execution or a passing result. Security consumes saved cargo-audit JSON and documents separate cargo-deny/vet gates.

## Constraints and review focus

- No changes to the other three skills, installed skills, backups, publication or dependency upgrades unrelated to the implementation.
- Baselines tolerate line movement, preserve duplicate counts, and cannot suppress severity increases or failed analyzers.
- Configuration errors and unreadable security inputs must fail; descriptive contracts must be labelled as unverified.
- SARIF retains every finding and execution status. Paths require URI encoding.
- Missing verification, advisory reachability and unmeasured performance remain unknown.

## Tasks

- [x] Baseline and SARIF: CLI compatibility tests first; implement fingerprints, multiset comparison, gate integration and complete report output. Test line changes, new duplicates, escalation, malformed inputs, analyzer failure, encoding and Criterion compatibility.
- [x] Project profile: CLI override and malformed-config tests first; implement auto-discovery, explicit config and documented schema, with justified dated exceptions and visible contracts.
- [x] Security and verification: saved-input and risk-selection tests first; import advisory matches separately from dependency warnings; reject malformed reports. Recommend Miri, Loom, properties, mutation testing and sanitizers only for relevant risks, with prerequisites and limitations.
- [x] Skill guidance: preserve strong existing behaviors; add philosophy, four modes, domain profiles, security, FFI, evidence assessment and continuous refactoring policies. Behavioral baseline evaluation before edits; forward-test changed guidance afterwards.
- [x] Integration: run default/core tests, opt-in existing integrations, fmt, Clippy, executable examples, configuration and CLI examples; review output and limitations independently.

## Progress and rulings

- Repository initially clean on main; working branch `feat/rust-continuous-quality`. Work remains in the selected local checkout; no publication or installed-skill changes.
- Baseline behavior evaluation confirms existing correct handling of legitimate Arc clones, compiler limits and unknown performance; gaps are concrete tool selection, continuous debt and supply-chain taxonomy.
- Existing 49 unit tests pass. First new baseline tests fail on missing flags/SARIF as expected (TDD red); these are implementation tests, not pre-existing failures.
- Implemented native CLI changes without new Cargo dependencies; skill version 1.2.0, auditor 1.1.0. Project configuration intentionally accepts a documented strict TOML subset; unsupported syntax fails instead of being approximated.
- Independent behavior evaluation is a non-blind tabletop application, not an empirical proof of agent compliance; see `docs/evaluations/2026-11-16-continuous-quality.md`.
- Independent review reproduced failed Clippy baseline acceptance, lost warning-advisory identity and omitted heuristic unsafe verification recommendations. Regression tests failed first, then passed after correction. Security plan generation now uses dependency-specific evidence and checks rather than generic compiler claims.
- Explicit analyzers with missing manifests/executables fail; specialized verification remains advisory and all suggested techniques retain not_run status. No Miri/Loom/sanitizer execution or live supply-chain scan is claimed.
- SARIF sample passed the official OASIS 2.1.0 schema through PowerShell Test-Json; quick_validate.py passed using an isolated validation venv under ignored target/, with YAML and local-link checks also passing.
- Final: 95 passing default tests including integrations; 70 passing core tests (4 ignored AST/build-backed cases); both strict Clippy variants, fmt, examples and two SARIF schema checks passed. Independent review's remaining metadata/llvm input failure paths were reproduced and fixed. See `docs/continuous-quality-validation.md` for exact scope and limits.
