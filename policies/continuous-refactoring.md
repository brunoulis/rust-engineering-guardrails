# Continuous refactoring gate

Review the change against a trusted base, not only a whole-tree alert count. Read the diff and affected callers; baseline comparison cannot discover semantic changes by itself.

1. Establish the same root, profile, analyzer/tool versions, features/target, exclusions and inputs before/after. Review failures and skipped scope before comparing.
2. Inspect added debt, altered invariants, new locks/copies/allocations, cancellation, public APIs, error/serialization contracts and resource budgets. A legitimate clone/lock may be retained with evidence. Re-run affected contracts and representative benchmarks only for relevant claims.
3. Disposition each concern: fixed and verified, legitimate retained use, or deferred with owner, reason, next verification and review date. Compare keep/minimal/structural options for meaningful debt.
4. Gate new or escalated findings at the project's chosen severity; preserve known debt in the report. Failed requested analyzers must still fail a configured gate. Baseline acceptance never establishes correctness or safety.

## Native baseline

```bash
rust-guardrails report . --write-baseline rust-baseline.json --format json --output rust-audit.json
rust-guardrails report . --baseline rust-baseline.json --fail-on high --format sarif --output rust-audit.sarif
rust-guardrails report . --criterion target/criterion --baseline before --candidate after --debt-baseline rust-baseline.json --fail-on high
```

Review and version the baseline intentionally. Keep baseline generation out of the gate path. Writing a baseline does not reset the gate for that run. Do not omit tests, weaken severity, change exclusions or regenerate debt to make a failure disappear.

Fingerprints use rule, relative path and trimmed evidence without line numbers; occurrence counts prevent one known instance from hiding added duplicates. Advisory identity uses its stable message rather than volatile database provenance. Severity increases are new debt. Line movement and surrounding whitespace changes are tolerated; internal whitespace/evidence changes, renames, analyzer changes or identical repeated statements can require manual reconciliation. This is conservative syntactic identity, not semantic tracking. Reports retain all findings; baseline state supplements severity and confidence.

## Exceptions

Use [project profile exceptions](project-profile.md) for a narrowly scoped rule/path with rationale, accountable owner, maximum accepted severity and expiry date. Review the underlying issue before granting one. Expired, escalated or blocker findings cannot be waived by an exception; baseline acceptance is separate, so inspect both mechanisms. An exception records acceptance of a known lead; it cannot prove a sound unsafe boundary, repair an ABI contract, excuse a failed check or authorize skipping a required test.

Keep exception reasons and expiry visible. Renewal requires current evidence; never auto-extend expired waivers. Stable legitimate code can have one recorded disposition rather than repeated suppression comments. For a real production vulnerability or observed correctness failure, follow the project's remediation process regardless of existing baseline status.

## Completion

Report changed contracts, baseline/new debt, retained code, actual checks, benchmark evidence or UNMEASURED, exceptions, analyzer gaps and rollback where relevant. Use [evidence assessment](evidence-assessment.md) only when useful; no universal clone count, file-size threshold or composite score defines success.
