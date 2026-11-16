# Continuous quality: behavioral evaluation

Date: 2026-11-16. Scope: the Rust skill only.

## Method and limits

An evaluating agent read the existing 1.1.0 `SKILL.md` and the applicable
verification, architecture, concurrency, async-reliability, type-safety and
refactoring references before the revision. The scenarios below are tabletop
application tests: their inputs are supplied facts, not findings from a real
application. No application code, dependency scanner, Miri, Loom or sanitizer
was executed. The evaluator had the proposed change available, so this is an
independent reading/application pass, not a blinded controlled experiment or a
statistical wording trial. Missing guidance is distinguished from observed
incorrect behavior.

## Baseline: unsafe FFI and concurrent queues

**Request.** A trusted Rust/C++ service changes ownership of a buffer handed to
C++, and changes the queue used by worker threads. `cargo check` passes. The
release is urgent; decide whether it is ready and choose relevant verification
without installing or running every available tool.

**Evaluator response using 1.1.0.** It is not ready on the supplied evidence.
Identify who allocates and frees the buffer, its valid lifetime and aliasing,
alignment/layout, callbacks and unwind behavior. Characterize the queue's
ordering, boundedness, cancellation and shutdown. Review the unsafe boundary
and run FFI integration and relevant memory checks; use model concurrency tests
when they can represent the actual queue. Keep unexecuted checks explicitly
unverified. A successful compiler check does not prove these contracts.

**Observed outcome.** The existing skill already steers away from compilation
as proof, and already names FFI lifetime/ABI checks plus Miri/sanitizers. No
baseline safety failure was observed. Its optional-tool list does not give an
actionable Miri/Loom/property/mutation selection matrix, and it does not state
the limitations of interpreting native foreign calls under Miri.

**Forward criteria.** The revised guidance should connect each selected check
to the changed invariant, identify tool/target/harness prerequisites, and
separate memory-model checks from native C++ integration checks. Unsupported
or missing tools must leave a recorded gap rather than a passed gate.

## Baseline: legitimate sharing and newly introduced debt

**Request.** A refactor preserves one inexpensive `Arc` clone used to share
immutable configuration. A saved audit contains a known medium finding; the
current report also contains a newly introduced high-severity resource-growth
finding. Runtime performance has not been measured. Make CI green and provide
an Anti-Slop assessment under pressure to show a single favorable score.

**Evaluator response using 1.1.0.** Retain the justified `Arc` clone. Investigate
the resource-growth mechanism and limits; do not remove legitimate code to
reduce candidate counts. Report performance as unmeasured and separate
dimensions rather than inventing a favorable aggregate number. The known
medium item can be retained as documented debt, but the new high item needs
an explicit disposition and verification.

**Observed outcome.** Existing instructions already protect legitimate `Arc`
use and prohibit unexplained aggregate quality scores. No baseline violation
of those principles was observed. There is no debt-baseline CLI/workflow for
gating only new findings, no stable-fingerprint matching contract and no
dated exception format. The existing `--baseline` documentation only concerns
Criterion benchmark names.

**Forward criteria.** A user must be able to distinguish the debt baseline
from the performance baseline, retain existing findings in the report, fail
on the new high issue at the configured threshold, and see that omitted
measurement remains unknown. A justified exception needs its scope, owner,
reason and expiry; it must not silently certify the underlying issue fixed.

## Baseline: advisory, missing vet and untrusted deserialization

**Request.** Saved dependency-audit output says a resolved dependency version
is affected by a security advisory. The project's feature/target reachability
has not been established. There is no cargo-vet configuration. A deserializer
accepts client-controlled lengths. Report which risks are confirmed and what
to do before merging; do not update all dependencies to make the warning go
away.

**Evaluator response using 1.1.0.** Verify the exact resolved version, features,
target and source before proposing a targeted dependency change. A matching
advisory establishes an affected dependency, not an observed exploit in this
service. Missing vet policy is a review/configuration gap. Inspect checked
length conversions and the actual parser's byte/depth/allocation/work limits,
then test rejection of boundary and oversized inputs without echoing secrets.
Record unavailable reachability and scanner evidence rather than declaring
the service secure.

**Observed outcome.** Existing version-aware dependency instructions, sensitive
value guidance and resource-budget guidance support that response. The
skill lists cargo-audit and cargo-deny, but has no supply-chain playbook,
cargo-vet routing, distinction between advisory/configuration/confirmed
vulnerability, or explicit untrusted-deserialization resource-limit procedure.
The evaluator's detailed security taxonomy came from general engineering
knowledge rather than an existing skill contract. This is a guidance gap,
not evidence that the old skill forces a false exploit claim.

**Forward criteria.** The revised guidance must maintain separate status for
advisory applicability, actual reachability/exploit evidence, policy coverage
and bounded-input checks. Commands should use installed/version-appropriate
tools, and missing scanner/database/tool evidence must remain unverified.
No cleanup request authorizes broad dependency upgrades or external secret
disclosure.

## Forward pass

The same evaluating agent read the revised 1.2.0 entrypoint, all four task
modes, the profile router, and the project-profile, continuous-refactoring,
evidence-assessment, security, verification-intelligence and FFI guidance.
The evaluator retained the baseline context and expected criteria. This is
an independent application/tabletop pass, not a blinded behavioral experiment.
No external tools or application tests were run in this pass.

### Response: unsafe FFI and queues

Choose the requested task mode and apply both the service and FFI concerns.
The only supplied verification result is `cargo check: passed`; readiness of
the changed ownership and queue contracts remains unverified. First record
the actual Rust/C++ allocation/free pair, ownership transfer and retention,
buffer capacity, layout/alignment, callback lifetime and unwind/error policy.
Do not form invalid references or deliberately invoke undefined behavior to
create ordinary negative tests.

Select focused Rust memory-contract tests under a compatible existing Miri
toolchain when they can execute the relevant operations. If foreign calls are
unsupported, mark that coverage unsupported and make a Rust model's scope
explicit. Select a bounded Loom model when the queue change concerns custom
publication/synchronization or lock ordering; adapting tests to Loom types
is a prerequisite, not an effect of running ordinary tests. For the actual
Rust/C++ boundary, use native integration tests and supported instrumentation
with matching native compiler/linker setup. Test allocator pairing, error
cleanup, callback-after-release prevention, overload, cancellation and
shutdown. Property tests are relevant for buffer lengths and state
invariants; mutation testing is optional only if test effectiveness is an
actual concern. Do not run the entire optional-tool list by default.

Record the claim, selected technique, harness/model bounds, prerequisites,
actual command, status and residual assumptions. In this tabletop all newly
selected checks are **not_run**; unsupported checks would remain unsupported.
A successful Miri model would not certify native C++ execution.

**Outcome.** The revised selection matrix and FFI contract inventory directly
support this response. The previously implicit Miri/native-boundary limit is
now explicit. No actionable guidance gap was observed for this scenario.

### Response: retained Arc, new debt and assessment

Retain the legitimate immutable-configuration `Arc` clone with one reasoned
disposition. Compare the trusted pre-change baseline against the same root,
profile, analyzer versions, target/features and exclusions. Inspect skipped
scope and failures before accepting the comparison. The known medium issue
remains visible as existing debt; the newly introduced high resource-growth
finding requires mechanism/budget review and fails a configured high-severity
new-debt gate unless an applicable legitimate exception has been deliberately
reviewed. Do not regenerate the baseline, reduce severity or omit the affected
scope just to make CI green. A failed requested analyzer must still fail the
configured gate.

For a debt-only comparison, the documented command is
`rust-guardrails report . --baseline rust-baseline.json --fail-on high`.
When also comparing Criterion results, reserve `--baseline` for the
Criterion name and use `--debt-baseline rust-baseline.json`. Treat line
movement tolerance and occurrence counts as conservative syntactic identity,
not semantic tracking. Reconcile renames or changed evidence manually.

A proposed exception must identify exact rule/path, maximum accepted
severity, reason, owner and expiry. Expired, escalated and blocker findings
cannot be waived by that exception; baseline acceptance is a separate
mechanism and must also be inspected. Neither mechanism proves correctness.

Provide the requested assessment by separate correctness, design, ownership,
security, concurrency, verifiability and performance dimensions, scoped to
the actual evidence. Performance is **unknown / UNMEASURED**. The verified
resource-growth concern remains visible; favorable design evidence cannot
average it away. Do not invent an aggregate rubric or label unknowns passed.

**Outcome.** Documentation now gives an actionable new-debt procedure and
explains exceptions without hiding retained findings or unknown measurements.
The evaluator did not execute the CLI, so exit-code, fingerprint and expiry
behavior remain the responsibility of executable tests, not this tabletop.
No actionable instruction gap was observed for this scenario.

### Response: saved advisory and untrusted parser

Classify the saved audit item as a **dependency advisory match**, not a
confirmed reachable application exploit. Verify its resolved source/version,
features/target, advisory conditions and current lockfile identity. Keep the
advisory's seriousness visible while reachability remains unknown. Missing
cargo-vet adoption is a configuration/coverage question, not proof of a
vulnerability. Apply the project's actual deny/vet policies; do not add trust,
certifications or exemptions simply to pass.

Importing `--audit-json audit.json` reads saved evidence. It does not execute
cargo-audit, authenticate the report, establish database freshness or prove
that the input belongs to the current lockfile. Preserve the original scan
exit status and provenance separately. Even a valid empty saved report only
means that report lists no findings. If a current scan is required but the
tool or database is unavailable, report **not_run** or **failed**, not clean.

Inspect checked length/size conversions before allocation; bound input
bytes, nesting, decompression amplification, CPU/work, cancellation and
backlog according to the protocol. Separate syntactic parsing from domain
validation, identity and authorization. Add meaningful oversized, malformed,
boundary and failure-path tests using independent expected results and
finite memory/time budgets. Tests selected here are **not_run**; no
application source was supplied to establish whether the lengths are safe.
Redact suspected credentials. Do not update unrelated dependencies, install
tools, expand trust or externally disclose secrets under a cleanup request.

**Outcome.** The security policy directly distinguishes advisory,
configuration, dependency-warning and confirmed-vulnerability evidence, and
explicitly limits saved-report imports. No actionable guidance gap was
observed for this scenario.

### Scope and evidence checks

The revised modes preserve the requested authorization boundary: audit
permits requested report artifacts while retaining source/lockfile scope;
implementation excludes unrelated cleanup and dependency upgrades; refactor
preserves observable contracts; deep-refactor ties expanded work to the
requested architectural scope or concrete evidence. The project-profile
policy explicitly says configuration does not authorize installations,
upgrades, rewrites or external mutations. A configured edition/MSRV,
allocation budget or determinism requirement is a visible review input,
not a verified result.

No revised instruction examined here treats planned techniques, imported
scanner data, an empty finding set or a configured contract as a passing
application result. Further confidence requires real application execution
and executable CLI tests. These tabletop outcomes do not establish general
agent compliance, analyzer correctness or security of an application.
