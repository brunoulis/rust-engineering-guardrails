# Audit

Inspect the authorized scope and report evidence. Do not modify audited source or lockfiles. Writing requested report/baseline artifacts is allowed; baseline acceptance is a separate review decision.

Establish the domain profile, target/features/MSRV, affected contracts and available checks. Begin with source and metadata; build-backed analyzers run only on trusted code. Distinguish suspected patterns, compiler diagnostics, measurements and verified defects.

For each actionable finding provide location, severity, confidence, evidence, consequence, keep/minimal/structural alternatives and a verification that would settle uncertainty. Retain legitimate code and record why. Report analyzer failures and skipped scope. Use [continuous refactoring](../policies/continuous-refactoring.md) when comparing a change and [security](../policies/security-supply-chain.md) for external boundaries or dependency changes.

Completion means the inspected scope, unresolved questions and actual validation are clear; an empty candidate list does not establish quality.
