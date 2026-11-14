# Rust refactoring decision record

> One focused slice per record. Do not use a numeric "quality score" to justify changes.

- **Date / commit / owner:**
- **Profile:** service / engine / library / ffi / prototype
- **Mode:** audit / refactor / deep-refactor
- **In-scope modules and dependencies:**
- **Explicitly out of scope:**
- **Quality objective:** correctness / soundness / testability / maintainability / reliability / performance / API
- **User-visible contract:**

## Baseline and reproducibility

- Existing git changes (preserved):
- Crates touched: exact resolved versions/sources, workspace member, features/target, active toolchain/MSRV and evidence or discrepancies:
- Version-matched official documentation consulted (URLs or local paths), relevant API/default/deprecation facts:
- If updating: current → target, task justification, migration notes, compatibility checks and rollback:
- Current behavior, inputs and invariant cases:
- Relevant tests and commands (actual results):
- Runtime workload and environment when performance matters:
- Data not available (must be marked unknown):

## Findings backed by evidence

| Priority | Path:line | Mechanism, not just pattern | Impact | Evidence/confidence | Why now? |
| --- | --- | --- | --- | --- | --- |
| P1/P2/etc | | | | | |

## Competing solutions (including no change)

| Option | Correctness/compatibility | Ownership & complexity | Runtime/build impact | Migration/risk |
| --- | --- | --- | --- | --- |
| Keep as-is | | | | |
| Small, local correction | | | | |
| Larger structural option | | | | |

**Decision and why:**

**Crate decision if relevant:** KEEP / LIB-BIN / WORKSPACE / INDEPENDENT / DEFER. Only after `references/crate-boundaries.md`.

## Scoped implementation plan

| Slice | Files | Characterization test first | Code change | Verification command | Revert path |
| --- | --- | --- | --- | --- | --- |
| 1 | | | | | |

## Acceptance checks

- [ ] Behavioral invariant tests passed (or changes explicitly approved with migration plan)
- [ ] Public API, serialization, error and ordering contracts checked
- [ ] `cargo fmt`, `check`, `clippy`, `test` run where applicable (actual output)
- [ ] Relevant feature/target combinations tested
- [ ] Performance compared with a reproducible baseline **if** performance is claimed
- [ ] Async overload/cancel/shutdown or FFI soundness tested if changed
- [ ] Independent reviewer challenged necessity, tests and compatibility
- [ ] Deferred debt documented; no unrelated rewrite

## Final report

- What improved and how it was verified:
- Before/after metrics or `UNMEASURED`:
- Real risk / side effects:
- Reviewer decision: GO / GO WITH RISKS / NO-GO
- Remaining debt and suggested future experiments:
