---
name: rust-engineering-guardrails
description: Design, audit and incrementally refactor Rust with evidence-based architecture, type safety and measured performance.
metadata:
  version: 1.0.0
  hermes:
    tags: [rust, architecture, refactoring, performance, code-review]
---

# Rust Engineering Guardrails

Use for Rust implementation, review, quality-driven refactoring, code ports, async services, simulations, FFI boundaries, and performance work. The objective is **correctness + architectural fit + measurable efficiency + maintainability**, not merely making `cargo check` pass.

## 0. Activation and mode

1. Determine the mode from the actual repository and task. **service**: async/API/DB/queues (latency, throughput, bounded memory); **engine**: simulation/telemetry/game/render (predictable hot paths, determinism when required); **library**: public APIs, compatibility, ergonomics; **ffi**: Rust ↔ C/C++/other language boundaries, clearly scoped unsafe; **prototype**: fast iteration with explicitly recorded debt. More than one mode may apply.
2. Inspect `Cargo.toml`, crate boundaries, dependency features, target toolchain, available tests, and relevant project rules. For existing code that "is not up to standard", choose refactor sub-mode: `audit` (read-only), `refactor` (scoped incremental changes) or `deep-refactor` (justified architecture work). Don't assume a three-crate workspace, ECS, zero allocation, or `no_std` is appropriate.
3. Read the *one* reference needed for the current task; load more only when evidence warrants it:
   - `references/refactoring.md`: **intelligent quality triage and scoped incremental refactoring** (read when improving existing code).
   - `references/architecture.md`: ownership, borrow checker, data model, arenas/newtypes/traits.
   - `references/crate-boundaries.md`: **adaptive crate decision**; compare one crate/modules, lib+bin, workspace, and independently versioned crates with actual evidence.
   - `references/type-safety.md`: newtypes, typestate, validated values, generational keys, deterministic arithmetic and error contracts.
   - `references/performance.md`: hot loops, allocations, profiling, benchmarks, compilation.
   - `references/concurrency.md`: channels, Arc/Mutex, Tokio, backpressure, Sans-IO.
   - `references/async-reliability.md`: cancellation safety, bounded queues/workers, overload, graceful shutdown.
   - `references/parity.md`: determinism, migrations, differential and property tests.
   - `references/verification.md`: Clippy, tests, CI, lint strategy, release gates.
   - `references/anti-patterns.md`: short bad/good code examples and exceptions.
   - `references/agent-workflow.md`: planning, independent reviews, reporting.
   - `references/sources.md`: origin of ideas and primary technical documentation.
   - `references/source-cases.md`: pinned Mini-Redis and ripgrep mechanisms, conditions for retaining them, and linked behavior tests; read before replacing working locks, clones, admission limits or scoped borrows.
4. For nontrivial tasks, establish a *before* baseline or state explicitly why no baseline is possible. Identify invariants and user-visible behavior before editing.

## 1. Non-negotiable engineering behavior

- **Borrow checker as design feedback.** Before adding `.clone()`, `.collect()`, `Rc<RefCell<_>>`, `Arc<Mutex<_>>` or `mem::take` *solely to compile*, examine smaller borrows, split state, helper functions receiving only needed fields, ownership transfer, IDs/arenas, and message passing. A clone may still be correct—explain its size, frequency, and purpose.
- **No unexamined hot-path allocation.** Identify hot paths via application knowledge and profiling; inspect allocation candidates there. No blanket ban on `Vec`, `collect`, clone, or hashmap in non-hot code. Preferring reusable scratch space is conditional on evidence and complexity.
- **Avoid accidental hidden state.** An extracted field via `mem::take` is replaced with `Default` during downstream calls; check nested access, reentrancy, early return, and panic/unwind invariants. `mem::take` is not inherently bad.
- **Avoid God objects and mechanical OOP ports.** Select decomposition by responsibility, not by arbitrary file-length targets. Use `newtype` identifiers, explicit state transitions, appropriate `traits`, and well-scoped modules where they pay for themselves.
- **Refactoring must address concrete quality debt.** Do not equate fewer lines, new abstractions, additional crates or disappearance of `clone` with improvement. For a refactor, preserve the observed contract, characterize untested behavior first, prioritize by demonstrated risk/benefit, and stage small reversible changes. Follow `references/refactoring.md` and `templates/refactor-plan.md`; no repo-wide rewriting by default.
- **Crate boundaries must be earned.** Do not split on LOC thresholds, force a fixed number of crates, or assume a workspace is faster. Before any crate extraction, use `references/crate-boundaries.md` to compare **single crate/modules → lib+bin → workspace → independent crates**. Record an ADR and the evidence: cohesion, coupling, public API, dependency DAG, ownership, test isolation, feature resolution, measured build cost and migration risk. If the benefit is not established, keep modules or defer.
- **Make invalid states hard, not APIs unusable.** Consider validated newtypes, typestate, runtime enums and generational keys where the domain benefits; verify runtime deserialization and long-lived ID behavior. Check arithmetic overflow, rounding, ordering and serialization parity before changes. See `references/type-safety.md`.
- **Concurrency is budgeted.** No unbounded thread/task creation, buffers, queues, or accidental blocking within async runtimes. Investigate data ownership and channels before shared mutable state where sensible. `Arc` itself is a legitimate tool, not an error. Analyze cancellation-safety, overload policy, bounded capacity and shutdown behavior; see `references/async-reliability.md`.
- **Safety matters.** Do not introduce `unsafe` to bypass compiler errors. If FFI or a demonstrated need requires unsafe, document invariants, isolate it behind a safe API, and review separately. Never weaken tests or lints to force a pass.
- **Correctness is a gate.** Preserve specified results, observable ordering, error behavior, and compatibility. For deterministic systems, test replay parity; for FFI, test ABI and lifetime contracts.
- **Performance claims require evidence.** Record before/after, workload, release profile, machine/environment, repeated runs, variance, and caveats. Do not claim an improvement based on fewer `.clone()` tokens or prettier code.

## 2. Workflow (required when changing Rust)

**A. Investigate → B. Design → C. Implement → D. Verify → E. Independent review**. When refactoring, use the six-phase quality process and autonomous scope limits in `references/refactoring.md`.

**A. Investigate**: inspect existing code and test coverage; locate allocations/locks/ownership conflicts; identify which code is truly hot. Optionally run the read-only auditor; one `report` covers architecture, ownership, concurrency, safety, quality and potential performance, plus a prioritized refactoring plan with verification steps:

```bash
cargo run --quiet --release --manifest-path path/to/rust-engineering-guardrails/tools/rust-guardrails/Cargo.toml -- report . --profile service --format markdown
cargo run --quiet --release --manifest-path path/to/rust-engineering-guardrails/tools/rust-guardrails/Cargo.toml -- report . --profile engine --hot 'crates/sim/**' --with clippy --format json
```

Layers are opt-in by cost. The default runs the `syn` AST scan and `cargo metadata --frozen`, both read-only. `--with clippy` adds type-aware lints and `--with llvm-lines` adds monomorphization hotspots; **both build the project, so use them on trusted code only.** `--criterion DIR --baseline A --candidate B` (or `--hyperfine A.json B.json`) validates an optimization: it reports *improved/regressed* only when confidence intervals separate beyond the noise threshold. `bench-compare` runs that comparison alone. `explain RULE` prints a rule's playbook. `audit`/`triage` keep the legacy (pre-1.0) output.

The report states each finding's confidence (`heuristic` < `syntactic` < `compiler`/`measured`) and which analyzers were skipped. The AST engine has **no type information**: a finding is an investigation lead, never proof of an allocation, vulnerability or bad architecture. Before acting on a plan item, read the code at its location and pick an option (keep / minimal / structural). Record one disposition with evidence: fixed and verified, legitimate use retained, or investigation deferred with the unresolved question. Valid code need not change or acquire repeated comments to silence a signal. The tool never edits code.

**B. Design**: for substantive changes, compare at least two options, including a minimal correction. For existing debt, compare keep-as-is / minimal fix / structural alternative using evidence; do not perform an opportunistic repo-wide rewrite. State trade-offs in readability, ownership, allocations, latency, compile time, API and invariants. Don't cargo-cult the Reddit case study. **If proposing additional crates**, analyze all four layout options via `references/crate-boundaries.md`, record `templates/crate-decision.md`, and default to the least complex design that meets observed needs. Never silently add crates during an unrelated refactor.

**C. Implement**: make the smallest coherent architectural change; for risky legacy paths, write characterization tests first and keep mechanical moves separate from behavior-changing optimizations; avoid repeated token-level borrow-checker appeasement. Keep public APIs stable unless deliberately changed. Make behavior easy to test without I/O where appropriate (Sans-IO). For justified crate splits, first establish internal module boundaries and separate pure code movement from behavioral changes.

**D. Verify**: run applicable checks from `references/verification.md`. A refactor must verify observable behavior, API/error/serialization contracts and relevant feature/target combinations. For perf-sensitive changes, measure release-mode representative workloads. If unavailable, mark results **UNMEASURED**. On migration, require golden outputs / differential comparisons from `references/parity.md`. For crate moves, validate relevant feature/target combinations, dependency directions, exposed API and comparable cold/incremental build timings **only if performance is claimed**.

**E. Review**: request independent reviewer in a fresh context when available; otherwise do a deliberate skeptical pass. The reviewer must examine architectural debt, concurrency assumptions, false speed claims, tests, and compatibility, not just `cargo clippy`. Re-read this skill at handoff/review in long agent sessions.

## 3. Severity and decision policy

- **BLOCKER**: failing correctness tests, unsafe soundness risk, violated deterministic/ABI contract, known deadlock, lost state, or proven unacceptable regression.
- **HIGH**: unbounded resource growth, verified harmful hot-path allocations, panic-sensitive temporary invalid state, concurrency oversubscription, unreviewed FFI.
- **MEDIUM**: plausible but unmeasured hotspot, God-object trend, excessive coupled borrows, unnecessary work with evidence incomplete.
- **INFO**: style issue, isolated justified clone, optional performance idea.

A refactor with unverified changes to public behavior, serialization, state invariants or critical error handling is a **BLOCKER**, not merely maintainability work. A crate split without demonstrated benefit is **not automatically a blocker**; it is an architecture-review question. A new dependency cycle, broken public API or lost runtime invariant can become a blocker based on evidence.

Never escalate `clone`, `Arc`, `mem::take`, raw line count, or a single `impl` count to BLOCKER solely from a regex match. No universal numeric thresholds: project-specific budgets belong in repo docs/CI.

## 4. Required handoff format

Scale the handoff to the change: for a small correction, a short explanation plus actual validation and remaining uncertainty is sufficient. Use the fuller breakdown below when its dimensions affect the decision.

1. **Intent and scope** (for existing-code refactors: audit/refactor/deep-refactor, quality dimension and inspected evidence); **behavior/invariants** retained or changed, with tests.
2. **Architectural decision** (option chosen, alternative rejected, trade-offs). If crate boundaries are relevant: report **KEEP / LIB-BIN / WORKSPACE / INDEPENDENT / DEFER**, rationale and ADR.
3. **Hot-path and concurrency audit** (evidence, or `NOT MEASURED`).
4. **Validation** (actual commands, pass/fail; don't claim unrun checks).
5. **Measured results** (baseline ↔ after, same representative workload, or `UNMEASURED`).
6. **Residual risks** and independent-review findings.

## 5. Additional reading, only when relevant

For a quality-driven refactor read `references/refactoring.md`, use `rust-guardrails report` for read-only inspection, and record `templates/refactor-plan.md` (the report's plan items map onto its findings, options and slices). For a crate/workspace proposal read `references/crate-boundaries.md` and fill `templates/crate-decision.md` before adding packages. The report's metadata section (members, internal edges, cycles, duplicate versions, transitive counts) and the raw `topology` JSON are inputs to that decision; they never decide architecture. For types, state transitions and deterministic values read `references/type-safety.md`; for cancellation and queue budgets read `references/async-reliability.md`. For a concrete bad/good rewrite read `references/anti-patterns.md`. For a borrow checker failure read `references/architecture.md`; for `Arc`, Tokio, channels or thread-pools read `references/concurrency.md`; for porting JS/TS/C++ to Rust read `references/parity.md` before changing order or data representations; for benchmark claims read `references/performance.md` and `references/verification.md`. Example ownership split: `examples/split_state.rs`. Check templates only if the user asks to add repository-wide rules or CI.
