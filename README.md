# Rust Engineering Guardrails 1.3.0

*Correctness by construction. Guardrails prevent engineering mistakes; they do not impose architecture.*

Rust engineering guidance plus **`rust-guardrails`**, a read-only Rust auditor. One command produces a report covering **architecture, ownership, concurrency, safety, quality and potential performance**, with a **prioritized, verifiable refactoring plan**. The tool proposes changes; it never edits the audited code. Works as a Claude Code, Codex or Hermes skill.

Skill 1.3.0 and auditor 1.2.0 improve [audit precision](policies/source-scope.md): Git-aware source selection, nested Cargo discovery from the same inventory, path-directed test modules and explicit unverified unsafe-comment review. Maintained untracked files stay in scope; `--include-ignored` broadens selection intentionally. Filesystem fallback is reported when Git selection is unavailable. No additional Cargo dependencies are required.

The shared philosophy, [task modes](modes/audit.md), [domain profiles](profiles/README.md), [FFI review](references/ffi.md), [security policy](policies/security-supply-chain.md), [risk-based verification](references/verification-intelligence.md), baselines, SARIF, configuration and benchmark comparison remain. Native-window guidance remains in [native-layout](references/native-layout.md).

## Continuous quality

```bash
rust-guardrails report . --write-baseline rust-baseline.json --format json --output audit.json
rust-guardrails report . --baseline rust-baseline.json --fail-on high --format sarif --output audit.sarif
rust-guardrails report . --audit-json cargo-audit.json --format markdown
```

Review baseline creation separately from CI. Gates compare new/escalated findings, preserve duplicate counts and still fail when requested analyzers fail. Line movement is stable; changed excerpts or file renames may need reconciliation. Criterion retains `--criterion DIR --baseline NAME`; add `--debt-baseline FILE` when using both. See [continuous refactoring](policies/continuous-refactoring.md).

Copy [templates/rust-rails.toml](templates/rust-rails.toml) to `.rust-rails.toml`, or select `--config FILE`. CLI flags take precedence. The native parser supports a documented strict TOML subset and rejects unsupported syntax. Scan settings and CI severity are applied; edition/MSRV, allocation, unsafe and determinism contracts are visible **review inputs, not verified results**. Exceptions require exact rule/path, reason, owner, maximum severity and expiry; reports retain them. See [project profile](policies/project-profile.md).

Security imports validate saved cargo-audit data; they do not run a scan, establish freshness/current lockfile identity or prove exploitability. Separate audit/deny/vet and secret-scanning gates are described in [security policy](policies/security-supply-chain.md). Verification recommendations select techniques by signals/profile, with prerequisites and `not_run` status; no Miri, Loom, properties, mutation testing or sanitizer execution is implied. Inspect the actual change to find risks outside those signals. [Evidence assessment](policies/evidence-assessment.md) keeps unknown dimensions visible and supplies no automatic composite score.

## Install

See [INSTALL.md](INSTALL.md) for Codex, Claude Code and Hermes setup, local/personal scope, the skills CLI and discovery checks. Unzip the complete skill into the selected directory so that `SKILL.md` is directly inside `rust-engineering-guardrails/`.

The instructions do not require Rust to be installed. To build the optional auditor, use a compatible stable Rust toolchain (MSRV 1.96); `clippy` and `cargo-llvm-lines` are optional. From the skill root, the following explicitly installs a binary into Cargo's bin directory (use the local build/run alternative in INSTALL.md if preferred):

```bash
cargo install --path tools/rust-guardrails --locked
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked
```

## One audit, one report

```bash
rust-guardrails report /path/to/project --profile service --format markdown --output audit.md
rust-guardrails report . --profile engine --hot 'crates/sim/**' --deep --format json
rust-guardrails report . --with clippy --criterion target/criterion --baseline before --candidate after --max-regression 5
```

The report always says **what was checked and what was skipped**. It contains a summary matrix (dimension × severity), benchmark verdicts, and the plan. Each plan item gives its location, rule, confidence, why it matters, three options (*keep as is*, *minimal fix*, *structural*), whether to *characterize first*, verification commands, and a *done when* criterion. Formats: `text`, `markdown`, `json` (schema 2, additive context/baseline/verification fields), `sarif` (2.1.0). `--fail-on high` returns exit code 1 for CI; malformed requested inputs return 2.

## Layered analyzers: light core, opt-in depth

| Layer | Default | Builds the project? | Confidence | What it adds |
| --- | --- | --- | --- | --- |
| Source scan, **syn AST** engine | on (feature `ast`) | no | syntactic | Ignores strings, comments and test code: `#[cfg(test)]` (including `all(test, ..)`, but not `any(test, ..)`), `#![cfg(test)]`, `#[test]`, files loaded by `#[cfg(test)] mod x;`, and `tests/`/`benches/` (`--include-tests` scans them). Knows loop bodies (not the iterator expression or `while` condition), async context (left only through `spawn_blocking`, `block_in_place` and thread spawns), cold error paths (`map_err`, `ok_or_else`, `return`), and `SAFETY:` comments (same line, the comment block above, or above the enclosing statement) as well as `# Safety` docs. Measures functions and types. |
| Source scan, heuristic engine | fallback / `--engine heuristic` | no | heuristic | The original text rules (pre-1.0 v4 tool), now blanking literals and comments first. Used for files `syn` cannot parse; these are reported as `parse_fallback`. |
| `cargo metadata --frozen` | on | no (read-only; never touches the network or `Cargo.lock`) | measured | Workspace members, internal edges, dev-dependency cycles, duplicate crate versions, `features = ["full"]`, wildcard/git deps, build scripts/proc-macros, mixed editions, transitive dependency counts. Falls back to `--no-deps` without a lockfile. |
| `cargo clippy` | `--with clippy` | **yes** | compiler | Type-aware lints and 12 extra audit lints (e.g. `undocumented_unsafe_blocks`, `redundant_clone`, `needless_collect`, `large_futures`), mapped to dimensions. Runs with `--locked` (never writes `Cargo.lock`) and `--cap-lints warn`, so `#![deny(..)]` in the project cannot fail the build or hide other lints. Paths are resolved against the workspace root, so auditing one member works. A diagnostic at the same location as a syntactic finding is merged into it as *confirmed*, capped at high. Only real compile errors (rustc `E…` codes) are blockers. |
| `cargo llvm-lines` | `--with llvm-lines` or `--llvm-lines-file` | **yes** | measured | Functions dominating LLVM IR (monomorphization → compile time, binary size). Also runs `--locked`. |
| Benchmarks | `--criterion DIR --baseline A --candidate B` or `--hyperfine A.json B.json` | no | measured | Per-benchmark verdict: *improved*, *regressed* or *inconclusive*. A change counts only when valid CIs at the same confidence level do not overlap **and** it exceeds `--noise` (default 2%). Missing/invalid uncertainty is an error; Criterion retains its supplied level and Hyperfine uses 95% Student-t intervals (a conservative df=30 bound above 31 samples). See `references/performance.md` for assumptions and input validation. `--max-regression` turns a regression into a blocker. Input with no matching baseline/candidate pair (such as a misspelled name) is an error, never an empty pass. |

Build with `--no-default-features` for the zero-dependency Rust core (heuristic engine only). The AST engine's only dependencies are `syn` 2 and `proc-macro2`. Git is optional for ignore-aware inventory; the core runs with a visible filesystem fallback without it. Both engines share source and Cargo manifest selection.

> **Security and side effects:** `clippy` and `llvm-lines` compile the target, so build scripts and proc-macros **execute**. Use them on trusted code only. They write build artifacts to the project's `target/` directory, as any cargo build does. Sources and `Cargo.lock` are never modified.

`--fail-on SEVERITY` exits 1 when a finding reaches that severity **or when a requested analyzer failed**, because a gate must not pass on checks that did not run. With a baseline, findings must be new/escalated and non-exempt. Default source scope uses Git-selected files when available and otherwise reports filesystem fallback; `--include-ignored` requests the broader filesystem view. VCS/tool caches, child repositories, worktree stores and Cargo target/cache directories remain excluded. A module directory such as `src/build/` is scanned when selected. The report lists its selection policy and skipped directories.

At a root without Cargo.toml, metadata and requested build-backed analyzers discover nested projects from the same inventory, deduplicate their Cargo workspace identities and report per-manifest provenance. Path-directed test modules and their transitive helpers are excluded unless production also uses them. Adjacent unsafe prose without a SAFETY label remains a medium review signal; labeled comment evidence is also explicitly unverified. Neither category establishes soundness. See [source scope and precision](policies/source-scope.md).

## Other commands

```bash
rust-guardrails bench-compare --criterion target/criterion --baseline main --candidate new [--max-regression 5] [--format json]
rust-guardrails explain                  # list every rule
rust-guardrails explain spawn_in_loop    # playbook for one rule
rust-guardrails audit  . --format json   # legacy candidate list (pre-1.0 schema 1 + "engine")
rust-guardrails triage .                 # legacy per-file inspection queue
rust-guardrails topology .               # raw cargo metadata --no-deps
```

## Severity policy (from SKILL.md §3)

- Pattern matches never become **blocker**. Only compiler errors and benchmark regressions beyond an explicit `--max-regression` budget do.
- Per-iteration clone/allocation candidates stay at **info** unless the file is a declared hot path (`--hot`). No blanket ban on `clone`, `collect` or `Vec` in cold code.
- Profiles shift emphasis: spawn-in-loop and unbounded-channel signals stay medium in `service` until admission/backlog bounds are reviewed; `library` raises `unwrap`/`panic`/`process::exit`; `ffi` raises `unsafe impl`/`transmute`; `prototype` lowers quality debt.
- Thresholds (`--max-file-lines` 1000, `--max-fn-lines` 80, `--max-fields` 15, `--max-methods` 30) are project choices. A wide type with few methods is reported as a data record (info), not a God object.

## Validation performed for 1.3.0 / auditor 1.2.0

The [real-project regression](docs/evaluations/2026-11-16-projecttelemetrygui-precision.md) compares old/new binaries on 534 unchanged application source/manifest/lock hashes. Automatic report noise falls from 4,491 to 1,359 signals, chiefly by excluding ignored copies. Equal maintained scope removes nine test-only findings (1,342 → 1,333 syntactic signals), retains four comment findings as unverified review evidence and discovers ten Cargo projects. Nine dependency graphs resolve frozen; one is explicitly workspace-only offline. Additional metadata contributes 26 separate signals.

Final local checks: **137 default tests**, **93 core tests** (4 intentionally ignored), both strict Clippy variants, rustfmt, SARIF schema, baseline/negative controls and skill validation pass. The independent final review verifies production bin/build-script retention as well as test-only exclusion. These are precision and verification results, not an application soundness/performance certificate.

## Historical validation for 1.2.0 / auditor 1.1.0

Validated locally on Windows with Rust 1.96.0. Default tests including opt-in Clippy integrations: **95 passed** (54 unit, 11 baseline, 16 legacy/report CLI, 5 config, 9 security/verification). Dependency-free build: **70 passed, 4 ignored**; specialized AST/build-backed cases were checked in the default run. Counts include the shared JSON helper's tests in integration test binaries.

Both feature variants pass strict Clippy and formatting. The two executable ownership/typestate examples pass. `quick_validate.py`, local Markdown reference checks and workflow/template YAML parsing pass. Baseline and configured reports pass the official OASIS SARIF 2.1.0 JSON schema using PowerShell `Test-Json`. [Validation details](docs/continuous-quality-validation.md) record commands, review fixes and limits; [behavioral evaluation](docs/evaluations/2026-11-16-continuous-quality.md) is explicitly a non-blind tabletop application.

## Historical validation performed for 1.0.0

Run on Windows 11 with Rust 1.96.0 and clippy 0.1.96:

- `cargo test -- --include-ignored`: 39 unit + 14 CLI tests pass. These include two real `cargo clippy` runs: the fixture project, and a lockfile-less `#![deny(warnings)]` project that checks no `Cargo.lock` is created and no blocker is reported. `cargo test --no-default-features`: 25 unit + 9 CLI tests pass; 3 AST-only CLI tests are ignored there by design.
- `cargo clippy --all-targets -- -D warnings` is clean with default features and with `--no-default-features`. `cargo fmt --check` is clean.
- An independent review (fresh context) found 12 classes of defect. All are fixed and each has a regression test: a policy breach (deny-lint → blocker), `Cargo.lock` creation, wrong workspace-member paths, stack overflow on generated code, silent benchmark typos, blockers truncated from the plan, and several AST context false positives and negatives.
- No parse fallbacks on tokio 1.52 (335 source files plus 212 test files skipped, 0.5 s warm), syn 3.0, hashbrown 0.17 or serde 1.0. Long expression chains (3,000 terms) are handled because the scan runs on a 512 MiB-reserved stack.
- False-positive check: on this tool's own source, the pre-1.0 v4 scanner reported 225 candidates, including 5 `unsafe_keyword` hits that were string literals. 1.0.0 reports no safety findings (the crate has no `unsafe`).
- `cargo-llvm-lines` was **not installed** here. Its parser is tested against saved output only.

## Limitations

- Git ignore selection is optional; fallback without Git can include ignored source copies. `--include-ignored` intentionally broadens scope. Tracked sources remain selected even when an ignore pattern matches them. AST test-module discovery does not expand macros or cfg_attr paths; unsafe comment classification is lexical evidence, not semantic verification.
- No type information in the AST engine: `.clone()` on an `Arc` and on a `Vec` look the same. Use `--with clippy` for type-aware confirmation.
- Known filesystem I/O is matched by explicit `std::fs` operation spelling; pure values such as `Permissions::clone` are not I/O. Aliases, re-exports and method receiver types are unresolved, so calls can be missed or shadowed.
- Context is lexical: calls through helper functions, trait dispatch and macro-generated code are not followed. Only well-known macro bodies (`format!`, `println!`, `vec!`, `assert*!`, …) are inspected.
- `sync_lock_in_async` fires only for `.lock()/.read()/.write()` followed by `.unwrap()`/`.expect()` (std locks). A bare parking_lot `.lock()` cannot be told apart from an async lock future without types; `--with clippy` (`await_holding_lock`) covers it.
- `large_type` merges same-named types from different modules.
- `--with clippy` lints all targets, test code included. So clippy findings can point into `#[cfg(test)]` code that the syntactic scan skips (see `docs/sample-report.md`, R9).
- llvm-lines measures compile-time IR volume, not runtime speed. Benchmark verdicts are only as good as the workload and machine noise.
- `syn` 2 is pinned for API stability. Files with syntax it cannot parse fall back to the heuristic engine, and the report says so.

## References

Read `SKILL.md`, then only the reference you need: `references/refactoring.md` for code quality, `references/crate-boundaries.md` for crate decisions, `references/type-safety.md` for typestate/newtypes, `references/async-reliability.md` for cancellation and backpressure, `references/performance.md` for benchmarks. Record decisions with `templates/refactor-plan.md` and `templates/crate-decision.md`; `rust-guardrails report --format markdown` gives you the evidence and verification steps to fill them.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for reproducible problems, valid counterexamples, positive/negative tests, version-matched documentation and safety requirements. [CI](.github/workflows/skill-ci.yml) covers the default AST engine, dependency-free core, Clippy integrations and executable examples on Linux, macOS and Windows.

## License

MIT. See [LICENSE](LICENSE).
