# Agent orchestration, review, token efficiency and handoff

## Architecture before implementation (but not over-specification)

For substantial work, an architect agent or planning pass must capture: invariants, constraints, hot path budgets, alternative designs and how to prove results. Ask pointed questions of the proposal: Who owns mutable state? Which borrows overlap? What allocates per request/tick? Can queues grow? Where can execution panic? What is measured? Which observable order must be preserved?

Give the implementation agent goals and non-negotiables, but leave room to choose a different architecture with a documented rationale. Don't force ECS, generic `traits`, three crates, or a ban on `Arc` when requirements do not justify them. When a crate extraction is proposed, the architect must compare the four layout options in `references/crate-boundaries.md` and produce `templates/crate-decision.md` with paths, dependency direction, costs and an explicit KEEP/LIB-BIN/WORKSPACE/INDEPENDENT/DEFER recommendation. The auditor independently challenges the evidence for a split.

## Quality-driven refactoring (sub-modes)

Before accepting "clean up Rust" or "make it professional," use `references/refactoring.md` to turn vague quality concerns into specific contracts, risks and testable outcomes. Run `rust-guardrails report` only as an evidence index: its plan ranks places to inspect, it does not decide what to change. The architect selects a small high-confidence improvement, the implementer first characterizes behavior and applies a bounded change, and the auditor checks whether it actually improved the intended quality dimension without increasing complexity elsewhere. Choose `audit`, `refactor` or `deep-refactor` based on evidence; do not silently upgrade scope. A single cohesive crate is often preferable to a gratuitous workspace.

## Suggested roles (parallelism optional)

- **Architect**: compare alternatives and produce a short, falsifiable design. For quality debt, classify confirmed vs suspected findings, define refactor scope and use `templates/refactor-plan.md`. Complexity should be earned.
- **Implementer**: make coherent changes, preserve tests, record evidence. Refuse a superficial borrow-checker workaround without examining alternatives.
- **Auditor**: separate context/agent from implementer; actively seek counterexamples and hidden state, allocations, concurrency and nondeterminism.
- **Performance reviewer**: verify workload and numbers rather than repeating claimed speedups. Can be combined with auditor on small tasks.

Using multiple agents is not a requirement; it costs tokens and coordination overhead. A fresh skeptical second pass is the key independence requirement for high-risk changes.

## Trigger / refresh sequence

Read `SKILL.md` at the start of a Rust change; re-load the relevant reference when: a borrow checker error leads to a new clone/take/interior-mutability choice; an allocation enters a known hot loop; concurrency topology changes; porting a state machine; a benchmark is used to support a claim; or reviewers return findings. At handoff, re-run the checklist rather than relying on the skill being buried in a long context.

## Independent review prompt

```text
You are an adversarial Rust reviewer, independent of the implementation agent.
Inspect the diff and affected call paths. Search for architectural borrow-checker
appeasement, deep clones/allocations in measured hot paths, temporary invalid
state from mem::take, unbounded tasks/threads/queues, unsafe/FFI assumptions,
semantic drift, unjustified crate boundaries, opportunistic reworks, API breakage and false performance claims. For every concern give:
file:line, reachable trigger, mechanism, impact, evidence, severity, introduced /
pre-existing / unknown attribution, disposition, and a concrete alternative.
Distinguish verified problems from hypotheses. Do not assume any API is bad
merely because it is clone/Arc/RefCell/trait/ECS. Identify missing tests.
Return GO / GO WITH RISKS / NO-GO and the exact checks that ran.
```

## Reviewer feedback handling

Triage findings by evidence, not volume. Resolve blockers first; implement corrections; add regression tests; ask an independent agent or fresh pass to verify fixes. Record corrected/verified, legitimate use, accepted in-scope limit, or investigation pending with evidence and unresolved question. Keep pre-existing debt separate from introduced regressions; neither a severe label nor an untouched file establishes impact or authorizes unrelated repair. Compare first and current solutions when fixes accumulate state, caches, tasks or abstractions; retain each addition only with a contract/lifecycle reason, and reconsider simplification. Do not let the implementer silently dismiss unresolved findings. Update project rules only with stable generalizable principles, not one-off incident logs.

## Cargo / filesystem hygiene in multi-agent workflows

A Reddit commenter described many duplicate workspaces and large disk-write volume from unmanaged agent runs; treat the report as an anecdote, not typical behavior. In parallel-agent setups:

- Reuse a tracked workspace and coherent Cargo target-cache strategy; avoid creating repeated duplicate package trees and installing the same dependencies per agent.
- Do **not** run `cargo clean`, delete `target/`, or replace lockfiles as a casual workaround. Obtain permission for disruptive cleanup.
- Avoid simultaneous cargo commands that contend on the same `target` lock when they provide no speed benefit. Consider isolating targets only when contention actually justifies the disk cost.
- Measure incremental `cargo check` and disk consumption when turnaround time is a constraint. Keep build artifacts and skill docs out of the agent context unless needed.

## Standard deliverable

Scale to risk: a small local comment change can use a short explanation, actual diff validation, remaining uncertainty and justified N/A for state, external consumer and release. Applicable dimensions use the following slots; they do not change auditor output formats. Reuse linked evidence rather than repeating it.

- **Mode/scope:** intent/invariants; base/candidate revisions, dirty diff identity and compared range; observed inventory/exclusions versus inferred execution paths.
- **Architecture decision + alternative:** include crate ADR when relevant. First solution → current revisions: added/removed state, dependencies, abstractions and lifecycle rules, motivating review findings and why retained; compare simplification/local/structural options. Name the starting proposal if no prior revision exists.

| Existing/new file | Why changed | Behavior/contract before → after | Affected consumer/path | Verification evidence or gap |
| --- | --- | --- | --- | --- |
| | | | | |

- **State/resources:** new/changed state → owner/mutator → invalidation/reset/cancel/join/drop, necessity and failure/shutdown behavior, or justified N/A. Include effects of changed `Arc`, locks, caches and tasks without treating the syntax as a defect.
- **Regression surface:** affected scenarios and denominator if known, inferred versus exercised paths; bug case failing before/passing after with both identities/evidence or unavailable/unsafe reason and limit. Characterization/new-feature checks state their different objective. Supported feature-off/cfg/other target behavior: evidence, intentional authorized change, or UNVERIFIED.
- **Ownership/allocations/concurrency:** reachable runtime/blocking context, locks/await, pool admission/saturation and cancellation; for changed logs, activation/frequency/sink and observed or unknown cost. See [concurrency](concurrency.md) and [async reliability](async-reliability.md).
- **Validation:** fill the [verification matrix](verification.md#verification-matrix), including counts, evidence freshness and each required outstanding check. Recommendations from the auditor remain not_run until executed.
- **Distribution, when applicable:** effective dependency versions/git/path/[patch] sources and reasons; toolchain/features/target/profile, built/linked/loaded artifact identities and actual consumer, connecting test/release evidence or marking gaps UNVERIFIED. See [supply chain](../policies/security-supply-chain.md) and [FFI](ffi.md); otherwise justified N/A.
- **Baseline/final measured metrics:** representative workload/environment and results, or UNMEASURED.

| Finding/path | Trigger and consequence | Severity/confidence | Attribution (introduced / pre-existing / unknown) | Evidence | Disposition and reason |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

- **Review/remaining risks:** reviewer/source and candidate reviewed, actual checks, unresolved required checks, accepted limits and rollback. Identify self-review/documentary evaluation as such; review count does not prove closure.
