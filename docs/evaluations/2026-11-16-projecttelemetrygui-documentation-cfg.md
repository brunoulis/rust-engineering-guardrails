# Documentation severity and inline test-code precision

Skill 1.3.1 / auditor 1.2.1, compared with the installed skill 1.3.0 / auditor 1.2.0. Read-only application audit on 2026-11-16, ProyectTelemetryGui commit `b294d6f996a0baf45cac6ccbb5611fb99583a651`.

## Reproducible comparison

Both binaries ran fresh reports on the same application with `--profile ffi --format json`, default Git source selection and frozen Cargo metadata. The old executable was previously built directly from the installed 1.3.0 source; the new executable was built offline and locked from this branch. Both identify the AST engine. All 534 source/manifest/lockfile SHA-256 values match the preceding triage and remain unchanged after these runs; application Git status also remains unchanged.

| Metric | Before | After |
|---|---:|---:|
| Scanned files | 369 | 369 |
| High signals | 283 | 0 |
| Medium signals | 217 | 499 |
| Info signals | 859 | 859 |
| Total signals, including metadata | 1,359 | 1,358 |
| Syntactic signals | 1,333 | 1,332 |

The occurrence-level delta is exactly **282 high → medium documentation signals**, **one removed test-only signal**, **zero added signals**. This is severity calibration and a scope correction, not removal of 283 findings or a measured false-positive rate.

The removed signal is `unsafe_without_safety_comment` at `flutter_desktop/windows/rust/pf_overlays/src/preview/worker.rs:295`. Its enclosing `OBSERVED_PRIORITY.store` statement is `#[cfg(test)]`. A counterfactual run with `--include-tests --no-metadata` restores that occurrence at medium severity; it scans 514 files and reports 3,538 signals (580 medium, 2,958 info). This run has deliberately broader source scope and no metadata, so its total is not a default-report comparison.

Nested discovery still finds ten Cargo projects. Nine dependency graphs resolve with `--frozen`; updater remains explicitly workspace-only offline because `arbitrary v1.4.2` is not cached. Metadata contributes the same 26 signals in both default reports.

Raw reports, source hashes and every delta are retained locally under `tools/rust-guardrails/target/validation/projecttelemetrygui-doc-cfg-2026-11-16/` (`old.json`, `new.json`, `new-with-tests.json`, `comparison.json`, `compare.py`). These machine-local artifacts are ignored build evidence; this evaluation is the durable summary.

## Implementation and regression evidence

Missing adjacent unsafe explanations and missing `# Safety` sections retain stable rule IDs and evidence, at medium severity. Matching Clippy warnings confirm documentation conventions without promoting them to high. Existing blocker diagnostics are not folded into lower-severity documentation signals, and failed requested analyzers retain their configured gate behavior. Audit Clippy still caps lint levels; it is not a replacement for the project's strict lint CI.

One shared AST attribute boundary now drives finding filtering and inherited module context: items, associated/foreign items, expressions, locals, statement macros, match arms, fields, variants and function/generic parameters. `--include-tests` restores these nodes. Production-capable `not(test)`, mixed `any(test, feature=...)` and unresolved `cfg_attr` branches remain visible. Physical line metrics still count source lines. Macro expansion and arbitrary conditional compilation remain outside this syntactic engine; the heuristic fallback cannot promise equivalent filtering.

New regression tests were observed failing before their fixes: inline statements/unsafe/macro/match bodies, static/const/trait/impl items, mixed production cfg retention and documentation-only high gates. An independent review identified two additional concrete gaps: test-only parameter types and quoted cfg values containing `,test,`. Both counterexamples were reproduced as failing tests and fixed. Parsing cfg predicates as syn metadata preserves string literals instead of splitting text at commas. Test module children inherit the same context; shared production files stay in scope.

Validation on Windows with Rust 1.96.0:

- Default suite: 143 passed, three intentionally opt-in tests excluded from that invocation; those three build-backed Clippy/baseline cases also passed when explicitly run. **146 passed in total.**
- Dependency-free core: **94 passed**, four intentionally ignored specialized cases.
- Strict Clippy for both feature variants, formatting and diff whitespace checks pass.
- The CLI regression proves documentation remains visible without failing `--fail-on high`, while adding a production `static mut` still fails that gate. Existing compiler-error and requested-analyzer failure regressions pass.

## Limits and actual application defects

The previous manual review identified concrete audio-buffer lifetime, shutdown handle, renderer handle, COM lifecycle, image-cache and font-fallback defects. **They have not been repaired by changing the auditor.** In particular, zero syntactic high signals is not evidence that the application's known high audio/resource risks are gone. Manual defect severity comes from actual ownership and resource contracts, not presence or absence of a comment.

No application source/lockfile/profile changes, Flutter build, live GUI/game QA, native failure injection, Miri, sanitizers or performance measurements were performed for this skill correction. No security acceptance, soundness certificate or performance improvement is claimed.
