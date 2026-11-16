# Continuous quality validation — 16 November 2026

Scope: skill 1.2.0, native auditor 1.1.0. Existing source scanner, metadata, Clippy, llvm-lines, benchmark and legacy interfaces retained. No new Cargo dependency or dependency upgrade; Cargo.lock changes only the auditor's package version. Work is local to the Rust repository on `feat/rust-continuous-quality`; installed skills and the other language repositories were not modified.

## Actual validation

Windows, rustc/cargo 1.96.0. Commands run from the skill root:

```bash
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked -- --include-ignored
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked --no-default-features
cargo clippy --manifest-path tools/rust-guardrails/Cargo.toml --locked --all-targets -- -D warnings
cargo clippy --manifest-path tools/rust-guardrails/Cargo.toml --locked --no-default-features --all-targets -- -D warnings
cargo fmt --manifest-path tools/rust-guardrails/Cargo.toml --all -- --check
```

- Default: 54 unit + 11 baseline + 16 report/legacy CLI + 5 config + 9 security/verification = **95 passed**, none ignored or failed. This includes the real failing-Clippy baseline regression and two existing real Clippy integrations.
- Core: 36 unit + 10 baseline + 10 report/legacy CLI + 5 config + 9 security/verification = **70 passed**, 4 ignored AST/build-backed tests, none failed. Shared JSON helper tests also run in integration binaries; these counts are executions, not unique test bodies.
- Both strict Clippy variants and rustfmt passed.
- `rustc --edition 2021 --test` plus execution for `examples/split_state.rs` and `examples/typestate.rs`: one passing test each.
- Skill Creator's `quick_validate.py`: passed, using an isolated validation venv in ignored `target/`. Changed local Markdown links resolved and the two workflow/template YAML files parsed.
- Actual CLI examples: sample baseline creation and comparison gate passed; a configured sample report was emitted. Both SARIF outputs passed `Test-Json` against [the official OASIS schema](https://docs.oasis-open.org/sarif/sarif/v2.1.0/cos02/schemas/sarif-schema-2.1.0.json).
- `git diff --check`: passed. Logs and generated validation artifacts are under ignored `tools/rust-guardrails/target/validation/`.

## Regression evidence and independent review

New behavior tests failed first on absent baseline/SARIF/config flags, missing security import/verification output and the failure paths below, then passed after implementation. Independent review used actual CLI reproducers and no source edits.

| Reproduced problem | Corrected behavior |
| --- | --- |
| Failed Clippy build recorded as ran; baseline hid an existing compile blocker | Failed builds retain diagnostics, block the gate even against an exact legacy baseline, prevent snapshot creation and set SARIF executionSuccessful false |
| Missing requested Cargo/Clippy/llvm-lines or absent required manifest | Failed analyzer; a configured gate cannot pass on missing evidence |
| Invalid saved llvm-lines text parsed as an empty clean result | No parseable rows is failed; a valid zero TOTAL remains a successful empty measurement |
| Dependency warning lost optional advisory ID/title | Advisory identity retained; a new notice for the same package/version is new debt; advisory-free warnings remain accepted |
| Heuristic unsafe_keyword omitted memory recommendations | Both scanner engines recommend applicable memory investigation, still not_run |
| Imported dependency evidence received generic compiler/clippy plan text | Dedicated dependency applicability, remediation and audit/deny/vet review guidance |

Baseline tests additionally cover line movement, added duplicate occurrences, severity escalation, invalid/missing snapshots, writing a snapshot without resetting the current gate, and SARIF URI encoding. Configuration tests cover discovery, explicit selection, CLI precedence, exclusions, visible non-enforced contracts, malformed syntax and dated waivers. Existing benchmark tests preserve Criterion's baseline semantics and uncertainty requirements.

## Limits

No Linux/macOS execution was performed locally; the existing CI matrix remains available. No live cargo-audit/deny/vet, Miri, Loom, mutation, property or sanitizer run is claimed. Security parsing uses controlled saved inputs; imports do not authenticate freshness/current lockfile or establish exploitability. Advanced verification recommendations remain not_run. Configuration is a documented strict TOML subset and descriptive contracts require separate verification. Fingerprints tolerate line motion and surrounding whitespace, not arbitrary semantic moves or file renames. Baselines and dated exceptions are separate acceptance mechanisms. No runtime-performance improvement is claimed.

Behavioral guidance was independently applied to three scenarios before and after the changes; this was non-blind tabletop evaluation, not a statistical compliance experiment. See [the evaluation record](evaluations/2026-11-16-continuous-quality.md).
