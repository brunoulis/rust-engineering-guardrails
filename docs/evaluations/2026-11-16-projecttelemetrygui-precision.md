# Real-project precision regression — 16 November 2026

Skill 1.3.0 / auditor 1.2.0, compared with auditor 1.1.0 on ProyectTelemetryGui revision `b294d6f996a0baf45cac6ccbb5611fb99583a651`. The task was to fix four demonstrated limitations in the auditor, then repeat the audit without changing the application.

## Method

Preserved the old release executable before implementation. Recorded SHA-256 for all 534 Git-selected Rust sources, Cargo manifests and lockfiles; the complete path set, hashes, project revision and Git status remained identical after verification. The pre-existing untracked repaint specification remained untouched.

Ran old/new AST release binaries with profile ffi and no declared hot-path globs. Compared both automatic scope and equal maintained scope using the same explicit exclusions. The new root report discovers Cargo manifests; separate syntactic counts below avoid presenting additional metadata coverage as a precision regression. Both old/new Clippy runs used pf_overlays, all targets, locked dependencies and offline mode.

## Measured results

| Observation | Before | After |
| --- | ---: | ---: |
| Automatically selected source files | 1,238 | 369 |
| Automatic report, all signals | 4,491 | 1,359 |
| Equal maintained scope, source files | 382 | 369 |
| Equal maintained scope, syntactic signals | 1,342 | 1,333 |
| Equal maintained scope, high signals | 288 | 283 |
| Metadata projects checked from application root | 0 | 10 |

The 69.74% reduction in the automatic report primarily removes ignored comparison copies and worktrees. It is **not** a false-positive-rate estimate or an application-quality score. In equal maintained scope, nine findings on test-only helpers disappear and four unsafe-comment findings remain visible as medium-priority unverified review evidence. Thirteen additional test-only source files are excluded. The new automatic Cargo coverage contributes 26 metadata signals, so the total new report is 1,359 rather than the 1,333 syntactic count.

Nine Cargo projects provided complete frozen dependency graphs. Updater lacked a cached dependency and used explicitly reported workspace-only metadata; its full dependency graph remains unverified. No dependencies were downloaded or updated.

## Four fixes

1. **Source scope:** Git selection retains tracked and non-ignored untracked Rust sources/manifests. Ignored build comparisons are omitted; child repositories/worktree stores are pruned. Explicit include-ignored and visible filesystem fallback remain available. Source modules named src/build are retained.
2. **Cargo discovery:** the same inventory snapshot supplies nested manifests. Cargo workspace identity deduplicates invocations, findings retain root-relative locations and per-manifest provenance, and requested build-backed analyzers run serially. Failed checks still fail a configured gate.
3. **Test context:** explicit path attributes, nested inline modules and transitive helpers are resolved. Production sources shared with tests remain in scope. Review found and regression tests corrected genuine library/binary/build-script/example entrypoints reused by tests; arbitrary test helpers named lib.rs are not promoted by basename. Unsupported target hints conservatively retain sources and report uncertainty.
4. **Unsafe evidence:** actual comments are separated from string literals. Unlabeled prose and marker-only comments remain review signals; labeled prose is also unverified. The snapshot default's existing zero-validity argument is retained without presenting marker absence as a high-priority missing argument. No comment proves soundness.

## Verification

New regression fixtures were observed failing before their fixes. The final suites pass:

- Default engine, including real Clippy integrations: **137 passed**, no failures or ignored tests.
- Dependency-free Rust core: **93 passed, 4 ignored**, no failures. AST-context fixtures are compiled only with the AST feature; the shared JSON helper also contributes repeated executions in integration binaries.
- Strict Clippy on all targets, both feature variants, and rustfmt check: passed.
- Independent review's final concrete fixtures: test-only nested lib excluded, real binary retained and real build script retained. No outstanding P1/P2 findings from that review.
- Actual old/new pf_overlays Clippy: **667 diagnostics each, zero compile errors**. These include tests because all-targets was explicitly selected; they are not 667 confirmed application defects.
- Native SARIF output: passed the official OASIS 2.1.0 schema.
- Repeated source-only baseline: **zero new, 1,333 existing**, exit 0 at high threshold. This is an isolated verification snapshot, not acceptance of project debt.
- Negative control in an explicitly included ignored sandbox: truly missing unsafe comment evidence remains high and produces gate exit 1.
- Skill quick_validate, changed local documentation links and workflow YAML parsing: passed.

Reproduce core checks from the skill root:

```bash
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked -- --include-ignored
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked --no-default-features
cargo clippy --manifest-path tools/rust-guardrails/Cargo.toml --locked --all-targets -- -D warnings
cargo clippy --manifest-path tools/rust-guardrails/Cargo.toml --locked --no-default-features --all-targets -- -D warnings
cargo fmt --manifest-path tools/rust-guardrails/Cargo.toml --all -- --check
rust-guardrails report /path/to/ProyectTelemetryGui --profile ffi --format json
```

Full local data, source hashes, before/after reports, schema output and compiler logs are under ignored `tools/rust-guardrails/target/validation/projecttelemetrygui-precision-2026-11-16/`; `comparison.json` separates removed test findings, retained comment reviews and added metadata coverage. These local application reports are not bundled into the skill's source.

## Limits

The scanner remains syntactic. It does not expand macros/cfg_attr into a complete target graph, establish comment validity, prove FFI soundness or measure application performance. Unsupported target declarations are retained conservatively rather than guessed away. Filesystem fallback without Git may include ignored copies. No live GUI/in-game QA, full application test suite, vulnerability scan, Miri, Loom or sanitizer acceptance is claimed. ProyectTelemetryGui was not edited, installed or deployed.
