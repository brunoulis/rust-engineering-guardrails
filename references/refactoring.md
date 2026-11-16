# Intelligent Refactoring: quality without speculative rewrites

When reviewing a change continuously, apply [the baseline and exception policy](../policies/continuous-refactoring.md). Examine changed APIs, invariants, locks/copies and benchmarks independently of alert counts. Use [the optional evidence assessment](../policies/evidence-assessment.md) when dimensions need separate conclusions.

## Purpose and activation

Use this guide when a Rust repository **compiles but remains hard to understand, test, maintain, or operate**, or when an agent is asked to improve existing code. The aim is to improve the engineering outcome while preserving the agreed behavior. A refactor is not automatically a speedup, and a redesign that changes interfaces or runtime behavior is a migration, not a purely mechanical refactor.

**Modes** (select one based on the request, existing tests and risk):

- `audit`: read-only inventory, evidence-backed proposals, no code changes. Default when behavior or ownership is poorly understood.
- `refactor`: scoped, incremental changes with tests before/after. Prefer this for day-to-day debt removal.
- `deep-refactor`: substantiated boundary or data-model changes; perform an architecture decision (including a smaller alternative) and an explicit compatibility/migration plan. Not a synonym for "big cleanup".

Quality is multidimensional: correctness, safety, data invariants, architecture/ownership, maintainability, testability, runtime cost, reliability, developer experience and API stability. **Never collapse these into an unexplained 0–100 quality score**. Define the target based on the task and profile (`service`, `engine`, `library`, `ffi`, `prototype`).

## Investigate: smell -> mechanism -> impact -> evidence

For each suspected issue record **path:line, mechanism, consequence, evidence, uncertainty, scope**. A smell is not proof of a defect.

| Signal | Questions to investigate | Conditional refactor | Proof before/after |
| --- | --- | --- | --- |
| Wide mutable `Game`/`Context`, borrow conflicts, repeated clone/take to compile | Which fields are borrowed? Does the clone affect the original? Can a nested call observe an empty field? | Split ownership into coherent fields, narrow function arguments, identifiers/arenas if justified | Invariant tests, golden replay, profiling for hot calls |
| Long function / very large module | Are unrelated responsibilities mixed? Are tests or changes blocked by coupling? | Extract cohesive functions/modules; keep in one crate unless extraction evidence exists | Equivalence tests; dependency review |
| Deep nesting, duplicate logic, massive generic parameterization | Is the abstraction clearer to consumers? Is duplication intentional for performance? | Early-return, flatten state transitions, focused helpers, simplify excessive traits | Table-driven tests, compile-time and API review |
| `unwrap`, `expect`, panic paths in fallible production flow | Is it an actual invariant or external input? What does failure look like? | Typed errors and explicit boundary mapping (`Result`), narrowly scoped validation | Error-path regression and compatibility tests |
| Async tasks/locks/queues grow without limits | What blocks, how is work cancelled, who owns shutdown? | Bounded channels/workers, structured cancellation, isolate blocking work | Overload, cancellation, shutdown, leak tests |
| `.clone()`, `.collect()`, formatting, conversions in potential hot paths | Which type? How often? Does it allocate, and how much? | Borrow/stream/preallocate/reuse buffers when justified | Allocation trace and representative benchmark |
| Hard-to-test code due to I/O, globals or clocks | Can domain decisions run with deterministic inputs? | Sans-IO core; dependency injection at boundary; bounded interfaces | Unit tests with fake adapters; integration tests retained |
| Unsafe/FFI block with unclear invariants | What guarantees lifetime, aliasing, alignment and ABI? | Isolate unsafe behind reviewed safe API, document SAFETY | Miri/sanitizers where applicable, FFI integration |
| Changes trigger unrelated rebuilds | Which compile unit and features cause churn? | First reorganize modules or lib/bin; assess crates only after evidence | Controlled cold/incremental build timings, API/feature checks |
| Too much type machinery / many small crates | Are abstractions used? Do they add consumers, APIs or churn? | **Simplify**, inline unnecessary wrappers, merge cohesive modules/crates where safe | Compile/test matrix and dependency changes |

Anti-pattern detection must work both ways: **unnecessary indirection is as much debt as a God object**. Never equate newtypes/traits/crates/async/ECS with quality without context.

## Workflow: six constrained phases

### 1) Inventory and baseline (read-only)

- For crate-dependent cleanup or upgrades, apply **SKILL.md — Version-aware crate and toolchain work**: verify the selected version/source, features and target, then consult matching documentation. A newer API example does not justify changing dependencies. For an authorized update, compare current/target docs and migration notes and verify MSRV, feature/target and public-contract compatibility separately from unrelated cleanup.
- Read project requirements, public interfaces, hot-path budgets and intended semantics; check worktree and do not overwrite unrelated edits.
- Inspect Cargo workspace/modules, upstream callers, tests, feature flags, target triples, error contracts, serialization, persistence and performance baselines.
- Run applicable *non-mutating* checks, when tools exist (`cargo fmt --all -- --check`, `cargo check`, `cargo test`, `cargo clippy`). `tools/rust-guardrails/src/main.rs` summarizes **inspection signals**, not quality measurements.
- For absent tests around a risky path, create characterization tests/golden fixtures *before* restructuring. Coverage is a clue, not a pass/fail percentage by itself.

### 2) Triage by impact and confidence

Group findings into **P0** correctness/soundness/production failures; **P1** boundedness, reliability and measured performance problems; **P2** maintainability and testability bottlenecks with concrete evidence; **P3** purely cosmetic or speculative ideas. The severity of an issue is established from impact and evidence, not by an analyzer hit or LOC count.

For each candidate estimate: impact, confidence, change radius, reversibility, verification ability, migration/compatibility cost and operational risk. Prefer a **small high-confidence change** over broad unmeasured cleanup. If uncertainty is high, design a characterization test or profiler experiment first.

### 3) Design with alternatives

Document at least **minimal correction** and **structural correction**, including an explicit **leave-as-is** option. Compare behavior, ownership, complexity, allocation, concurrency, API stability, compile cost, tests and cost of change. Apply `references/crate-boundaries.md` only if cross-crate boundaries are relevant; never extract crates just because a file is long.

**Do not bundle structural refactoring, async topology change and perf optimization into one opaque diff.** If all are needed, stage them so each can be verified independently.

### 4) Implement in small, reviewable slices

- First secure characterization tests and boundary contracts.
- Extract functions/narrow borrows; preserve ordering, errors, public signatures, serialization and event timing unless explicitly changing them.
- Limit each slice to a coherent responsibility, ideally separate mechanical move from semantic change.
- Avoid blind `cargo fix`/`cargo clippy --fix` over the full repo. These modify code; inspect a clean diff and opt in for scoped changes. `cargo fmt --check` is safe read-only.
- Avoid weakening lints, removing tests, changing expected outputs or stubbing behavior to force CI green.

### 5) Verify twice

**Behavior gate**: tests, error cases, golden outputs, protocol/API contract and feature/target matrix as relevant. If behavior intentionally changes, name it and supply a migration/rollout/rollback plan.

**Engineering gate**: Clippy, formatting, tests for new edge cases, measured hot-path CPU/allocations/RAM when perf is claimed, bounded queues/shutdown when concurrency changes, and compile timing only if promised. Compare like with like and report **UNMEASURED** if no baseline or profiling was run.

Avoid claiming that fewer lines, fewer clones or more crates proves better quality or speed. Cover behavior before and after; use property-based / differential tests where risk warrants.

### 6) Independent skeptical review and handoff

Reviewer challenges the refactor's necessity, alternatives, tests, semantic drift, undue generic complexity and migration cost. Report exact commands that actually ran and failures. Include a **debt register** of deferred improvements rather than touching every area in one PR.

## Autonomous agent authorization boundaries

- `audit`: no changes to project files.
- `refactor`: apply **scoped, reversible** changes supported by agreed requirements and tests; do not silently expand to unrelated modules, public contract changes, dependencies or crate topology.
- `deep-refactor`: require the user's requested scope or concrete evidence that the existing architecture blocks the task. Write an ADR and stage migrations with compatibility gates. If scope or backward compatibility cannot be verified, produce a plan rather than a speculative wholesale rewrite.
- Never remove user work; inspect existing changes before applying source fixes.

## Minimal useful handoff

Use `templates/refactor-plan.md` and report: selected mode; target quality dimension; path:line evidence; accepted/deferred findings with reasons; invariant/behavior tests; options and chosen minimal boundary; changed files; commands/results; before/after metrics or UNMEASURED; independent reviewer outcome; rollback and residual risks.

## Automation: read-only first

```bash
cargo run --release --manifest-path .agents/skills/rust-engineering-guardrails/tools/rust-guardrails/Cargo.toml -- report . --profile engine --hot 'crates/sim/**' --format markdown --output audit.md
cargo run --release --manifest-path .agents/skills/rust-engineering-guardrails/tools/rust-guardrails/Cargo.toml -- report . --with clippy --format json > refactor-candidates.json
```

`report` parses Rust with `syn`, so strings, comments and test code no longer produce findings. It still does **not** know types, follow call graphs, estimate coverage or measure runtime performance; `--with clippy` adds type-aware confirmation. Each plan item has a priority (P0–P3), keep/minimal/structural options, a *characterize first* flag, verification commands and a *done when* criterion. Treat the plan as a ranked inspection queue: read each location before proposing a fix, and take one item per change set. The tool never declares a file "substandard" and never edits code.

## Tooling notes

- **Clippy**: use `clippy::correctness`, `suspicious`, `complexity`, `style`, `perf` as evidence; select pedantic/restriction lints individually. Never blanket-enable `restriction`.
- **rust-analyzer**: semantic Rename/Extract Function/assists can help keep mechanical changes scoped; check their diff and compiler output.
- **Cargo fix**: automatically applies rustc-suggested edits; only run on deliberately scoped work and inspect resulting diff.
- **Tests**: `cargo llvm-cov` measures coverage; `cargo-mutants` can reveal weak tests; neither guarantees correctness or authorizes a refactor by itself.
- **Performance**: Criterion/Hyperfine/flamegraph/DHAT are optional; only claim improvements from representative repeated measurements.

See `references/verification.md` and `references/agent-workflow.md` for general verification and agent orchestration.
