# Rust Engineering Guardrails 1.0.0

Rust engineering guidance plus **`rust-guardrails`**, a read-only Rust auditor. One command produces a report covering **architecture, ownership, concurrency, safety, quality and potential performance**, with a **prioritized, verifiable refactoring plan**. The tool proposes changes; it never edits the audited code. Works as a Claude Code, Codex or Hermes skill.

## Install

Unzip into a project skill directory, for example `.agents/skills/`, so that `SKILL.md` is at `.agents/skills/rust-engineering-guardrails/SKILL.md`. You need a stable Rust toolchain (MSRV 1.75); `clippy` and `cargo-llvm-lines` are optional.

```bash
cargo install --path tools/rust-guardrails          # or: cargo run --release --manifest-path tools/rust-guardrails/Cargo.toml -- ...
cargo test --manifest-path tools/rust-guardrails/Cargo.toml
```

## One audit, one report

```bash
rust-guardrails report /path/to/project --profile service --format markdown --output audit.md
rust-guardrails report . --profile engine --hot 'crates/sim/**' --deep --format json
rust-guardrails report . --with clippy --criterion target/criterion --baseline before --candidate after --max-regression 5
```

The report always says **what was checked and what was skipped**. It contains a summary matrix (dimension × severity), benchmark verdicts, and the plan. Each plan item gives its location, rule, confidence, why it matters, three options (*keep as is*, *minimal fix*, *structural*), whether to *characterize first*, verification commands, and a *done when* criterion. Formats: `text`, `markdown`, `json` (schema 2). `--fail-on high` returns exit code 1 for CI.

## Layered analyzers: light core, opt-in depth

| Layer | Default | Builds the project? | Confidence | What it adds |
| --- | --- | --- | --- | --- |
| Source scan, **syn AST** engine | on (feature `ast`) | no | syntactic | Ignores strings, comments and test code: `#[cfg(test)]` (including `all(test, ..)`, but not `any(test, ..)`), `#![cfg(test)]`, `#[test]`, files loaded by `#[cfg(test)] mod x;`, and `tests/`/`benches/` (`--include-tests` scans them). Knows loop bodies (not the iterator expression or `while` condition), async context (left only through `spawn_blocking`, `block_in_place` and thread spawns), cold error paths (`map_err`, `ok_or_else`, `return`), and `SAFETY:` comments (same line, the comment block above, or above the enclosing statement) as well as `# Safety` docs. Measures functions and types. |
| Source scan, heuristic engine | fallback / `--engine heuristic` | no | heuristic | The original text rules (pre-1.0 v4 tool), now blanking literals and comments first. Used for files `syn` cannot parse; these are reported as `parse_fallback`. |
| `cargo metadata --frozen` | on | no (read-only; never touches the network or `Cargo.lock`) | measured | Workspace members, internal edges, dev-dependency cycles, duplicate crate versions, `features = ["full"]`, wildcard/git deps, build scripts/proc-macros, mixed editions, transitive dependency counts. Falls back to `--no-deps` without a lockfile. |
| `cargo clippy` | `--with clippy` | **yes** | compiler | Type-aware lints and 12 extra audit lints (e.g. `undocumented_unsafe_blocks`, `redundant_clone`, `needless_collect`, `large_futures`), mapped to dimensions. Runs with `--locked` (never writes `Cargo.lock`) and `--cap-lints warn`, so `#![deny(..)]` in the project cannot fail the build or hide other lints. Paths are resolved against the workspace root, so auditing one member works. A diagnostic at the same location as a syntactic finding is merged into it as *confirmed*, capped at high. Only real compile errors (rustc `E…` codes) are blockers. |
| `cargo llvm-lines` | `--with llvm-lines` or `--llvm-lines-file` | **yes** | measured | Functions dominating LLVM IR (monomorphization → compile time, binary size). Also runs `--locked`. |
| Benchmarks | `--criterion DIR --baseline A --candidate B` or `--hyperfine A.json B.json` | no | measured | Per-benchmark verdict: *improved*, *regressed* or *inconclusive*. A change counts only when the 95% CIs do not overlap **and** it exceeds `--noise` (default 2%). `--max-regression` turns a regression into a blocker. Input with no matching baseline/candidate pair (such as a misspelled name) is an error, never an empty pass. |

Build with `--no-default-features` for the zero-dependency core (heuristic engine only). The AST engine's only dependencies are `syn` 2 and `proc-macro2`.

> **Security and side effects:** `clippy` and `llvm-lines` compile the target, so build scripts and proc-macros **execute**. Use them on trusted code only. They write build artifacts to the project's `target/` directory, as any cargo build does. Sources and `Cargo.lock` are never modified.

`--fail-on SEVERITY` exits 1 when a finding reaches that severity **or when a requested analyzer failed**, because a gate must not pass on checks that did not run. The source scan skips VCS and tool caches, and only cargo's own build directories (`target/` next to a `Cargo.toml`, or anything tagged `CACHEDIR.TAG`). A module directory such as `src/build/` is scanned. The report lists what it skipped.

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
- Profiles shift emphasis: `service` raises spawn-in-loop and unbounded channels; `library` raises `unwrap`/`panic`/`process::exit`; `ffi` raises `unsafe impl`/`transmute`; `prototype` lowers quality debt.
- Thresholds (`--max-file-lines` 1000, `--max-fn-lines` 80, `--max-fields` 15, `--max-methods` 30) are project choices. A wide type with few methods is reported as a data record (info), not a God object.

## Validation performed for 1.0.0

Run on Windows 11 with Rust 1.96.0 and clippy 0.1.96:

- `cargo test -- --include-ignored`: 39 unit + 14 CLI tests pass. These include two real `cargo clippy` runs: the fixture project, and a lockfile-less `#![deny(warnings)]` project that checks no `Cargo.lock` is created and no blocker is reported. `cargo test --no-default-features`: 25 unit + 9 CLI tests pass; 3 AST-only CLI tests are ignored there by design.
- `cargo clippy --all-targets -- -D warnings` is clean with default features and with `--no-default-features`. `cargo fmt --check` is clean.
- An independent review (fresh context) found 12 classes of defect. All are fixed and each has a regression test: a policy breach (deny-lint → blocker), `Cargo.lock` creation, wrong workspace-member paths, stack overflow on generated code, silent benchmark typos, blockers truncated from the plan, and several AST context false positives and negatives.
- No parse fallbacks on tokio 1.52 (335 source files plus 212 test files skipped, 0.5 s warm), syn 3.0, hashbrown 0.17 or serde 1.0. Long expression chains (3,000 terms) are handled because the scan runs on a 512 MiB-reserved stack.
- False-positive check: on this tool's own source, the pre-1.0 v4 scanner reported 225 candidates, including 5 `unsafe_keyword` hits that were string literals. 1.0.0 reports no safety findings (the crate has no `unsafe`).
- `cargo-llvm-lines` was **not installed** here. Its parser is tested against saved output only.

## Limitations

- No type information in the AST engine: `.clone()` on an `Arc` and on a `Vec` look the same. Use `--with clippy` for type-aware confirmation.
- Context is lexical: calls through helper functions, trait dispatch and macro-generated code are not followed. Only well-known macro bodies (`format!`, `println!`, `vec!`, `assert*!`, …) are inspected.
- `sync_lock_in_async` fires only for `.lock()/.read()/.write()` followed by `.unwrap()`/`.expect()` (std locks). A bare parking_lot `.lock()` cannot be told apart from an async lock future without types; `--with clippy` (`await_holding_lock`) covers it.
- `large_type` merges same-named types from different modules.
- `--with clippy` lints all targets, test code included. So clippy findings can point into `#[cfg(test)]` code that the syntactic scan skips (see `docs/sample-report.md`, R9).
- llvm-lines measures compile-time IR volume, not runtime speed. Benchmark verdicts are only as good as the workload and machine noise.
- `syn` 2 is pinned for API stability. Files with syntax it cannot parse fall back to the heuristic engine, and the report says so.

## References

Read `SKILL.md`, then only the reference you need: `references/refactoring.md` for code quality, `references/crate-boundaries.md` for crate decisions, `references/type-safety.md` for typestate/newtypes, `references/async-reliability.md` for cancellation and backpressure, `references/performance.md` for benchmarks. Record decisions with `templates/refactor-plan.md` and `templates/crate-decision.md`; `rust-guardrails report --format markdown` gives you the evidence and verification steps to fill them.

## License

MIT. See [LICENSE](LICENSE).
