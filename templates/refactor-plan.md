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

- Base and candidate revisions, dirty diff identity, compared range and existing git changes (preserved):
- Crates touched: exact resolved versions/sources, workspace member, features/target, active toolchain/MSRV and evidence or discrepancies:
- Version-matched official documentation consulted (URLs or local paths), relevant API/default/deprecation facts:
- If updating: current → target, task justification, migration notes, compatibility checks and rollback:
- Current behavior, inputs and invariant cases:
- Relevant tests and commands (actual results):
- Runtime workload and environment when performance matters:
- Data not available (must be marked unknown):

## Findings backed by evidence

| Priority | Path:line | Trigger/mechanism and consequence | Attribution (introduced / pre-existing / unknown) | Evidence/confidence | Disposition and why now? |
| --- | --- | --- | --- | --- | --- |
| P1/P2/etc | | | | | |

Disposition: corrected/verified, legitimate use, accepted in-scope limit, or investigation pending with question/evidence. A reviewer label alone proves neither severity nor attribution.

## Change surface and revision evolution

| Existing/new file | Why changed | Behavior/contract before → after | Affected consumer/path | Verification evidence or gap |
| --- | --- | --- | --- | --- |
| | | | | |

- New/changed mutable state or resource → owner/mutator → invalidation/reset/cancel/join/drop; necessity and failure/shutdown behavior (or justified N/A):
- Affected scenarios and denominator if established; distinguish inferred paths from exercised paths:
- Bug regression failing before/passing after, both identities and evidence, or unavailable/unsafe reason and limit; characterization checks state their different objective:
- Supported feature-off/cfg/other target behavior: evidence, intentional authorized change, or UNVERIFIED:
- First solution → current revisions: added/removed state, dependencies, abstractions and lifecycle rules; motivating review findings and dispositions. Compare simplification/local/structural options and justify what remains. If no prior candidate exists, name the starting proposal; do not invent history:

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

## Verification matrix

| Contract/scenario | Dimension (built / tested / integration / release) | Package/test kind, valid features/cfg, target/profile/runtime and artifact identity | Exact command/job | Selected / executed / passed / failed / skipped / disabled | Result (PASS/FAIL/UNVERIFIED/N/A), reason | Evidence location, date and candidate freshness |
| --- | --- | --- | --- | --- | --- | --- |
| | | | | | | |

Apply [verification rules](../references/verification.md#verification-matrix). Counts on non-test rows are N/A; unknown counts stay unknown. Zero/all-skipped tests and stale evidence leave behavior UNVERIFIED; runner failures remain FAIL. List partial coverage and required outstanding checks.

**Distribution, when applicable:** effective versions/git/path/[patch] sources and reasons; toolchain/features/target/profile, built/linked/loaded artifact identities and consumer exercising them. Connect test and release evidence or mark the gap UNVERIFIED. Otherwise state N/A and why.

## Final report

- What improved and how it was verified:
- Before/after metrics or `UNMEASURED`:
- Real risk / side effects:
- Reviewer decision: GO / GO WITH RISKS / NO-GO
- Remaining debt and suggested future experiments:
