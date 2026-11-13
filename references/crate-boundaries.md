# Adaptive crate architecture — evidence before splitting

**Policy: multiple crates are optional, never an architectural default.** Do not infer the need for a workspace from source lines, the number of `impl` blocks, or how many domain concepts exist. A sound single-crate project with cohesive modules is preferable to a fragmented workspace with unstable or cyclic contracts.

Read this reference when adding a new Rust crate, splitting an existing crate, proposing a workspace, or claiming compilation benefits from a split. Start with the cheapest boundary that solves the observed problem.

## Step 1 — Inventory the current project

Inspect the real code and repository constraints:

- `Cargo.toml`, `Cargo.lock`, toolchain/MSRV, targets, bins, `lib.rs`, modules, features, `build.rs`, optional deps and FFI concerns.
- Relevant owners and dependencies: who creates/owns each state? What calls what? Which types or functions would have to become `pub` if extracted? Are there dependencies pointing in both directions?
- Which other binaries, integration tests or programs genuinely consume the proposed library? Is reuse hypothetical?
- Baseline *clean* and *incremental* compilation only if build speed is a stated concern. Use comparable `cargo check --timings` runs, not one cold build versus one warm build.
- Review `cargo metadata --format-version 1 --no-deps`, `cargo tree --workspace --depth 1`, `cargo tree -e features` and `cargo tree -d` when relevant. These tools expose current packages/features, **not** architecture quality or future performance.
- Review the task's existing test boundary, external I/O, behavior invariants and proposed public API.

Optional read-only helpers: `rust-guardrails report path/to/repo` summarizes the **existing** workspace from `cargo metadata --frozen`: members, internal edges, dev-dependency cycles, duplicate versions, catch-all features, build scripts/proc-macros and transitive dependency counts. `rust-guardrails topology path/to/repo` emits the raw JSON. Neither can infer module coupling or decide whether to split; measure build cost (`cargo build --timings`) before claiming a split is faster.

If an existing single crate has only `src/main.rs`, it may be sufficient to introduce `src/lib.rs` plus a thin binary for testing; no new workspace is required.

## Step 2 — Compare all four options, in order

| Option | What it changes | When it wins | Typical hidden cost |
| --- | --- | --- | --- |
| **A. Keep one crate, refactor modules/state** | Private modules, smaller borrowed types and functions | Most ownership/organization issues; fast iteration | No strong enforced dependency separation |
| **B. One package, `lib.rs` + `main.rs`** | Library API separate from binary adapter | Reusable/testable core within one package | New explicit internal API, but minimal overhead |
| **C. Multiple crates in one workspace** | Compile/dependency boundaries between cohesive components | Meaningful reusable subsystem; enforceable I/O boundary; feature or build isolation | Public API maintenance, more Cargo metadata, dependency/feature coordination |
| **D. Independently versioned crates/repos** | Independent build, packaging and release lifecycles | Actual different consumers/maintainers/release needs | Highest maintenance, compatibility and publish cost |

Compare option A **even for a large source file**. If B solves the problem, don't jump to C. Option D needs independent lifecycle evidence, not simply more code.

## Step 3 — Detailed decision matrix (qualitative, no magic score)

For each candidate boundary record **specific evidence** (paths, dependency edges, build timing, test consumers). Rate each signal as `supports split / neutral / opposes split / unknown`, with confidence and mitigation:

1. **Cohesion:** Is the candidate a real capability owning behavior and state, not just a `types/` dumping ground?
2. **Coupling:** Is its interface narrow and stable? Would extraction require widespread `pub`, `pub(crate)` workarounds, shared mutable globals or constant cross-crate edits?
3. **Dependency direction:** Can imports be a DAG, with core/domain below adapters? Does the proposed `core` depend on transport/UI/database implementation? Detect cycles before change.
4. **Ownership & data flow:** Does extraction clarify ownership and borrow scopes? Would it introduce serialization, clones, locks, dynamic dispatch or cross-component circular borrow hacks?
5. **Independent testing:** Can the component be tested in isolation with meaningful fixtures, without needing full runtime setup? Could equivalent tests stay in-module?
6. **Reuse & API stability:** Are there multiple *real* consumers? Is a shared API worth maintaining? Public API exposure is a cost, not a free improvement.
7. **Compilation & features:** Are bottlenecks proven with representative cold/warm timings? Can expensive dependencies/features be isolated? A split can **increase** compile and link work; Cargo may unify features across workspace builds.
8. **Operational lifecycle:** Must the component target `no_std`, another platform, a different release cadence, separate safety policy or independently deployed binary?
9. **Risk & migration cost:** Test coverage, serialization/ABI stability, data migration, repository conventions, CI and team familiarity.

A new crate needs **at least one material observed benefit** plus an implementable, acyclic boundary with acceptable ongoing maintenance. No fixed LOC threshold, point score or automatic three-crate recommendation. Unknowns trigger measurements or a reversible spike, not confident claims.

## Decision gates

- **KEEP SINGLE CRATE** when modules plus field/ownership decomposition solve the issue, behavior is tightly coupled, or the only argument is file size.
- **USE LIB/BIN SPLIT** when core logic is testable/useful separately but further package boundaries add no independent value.
- **SPLIT INTO WORKSPACE** when strong evidence exists for a stable capability boundary: independent consumers, safety/feature/I/O isolation, or measured compile/testing benefit. The API must be expressible without circular dependencies.
- **DEFER** when evidence is missing: instrument builds/tests or attempt a low-risk module extraction first. Do not perform a speculative workspace rewrite during an unrelated fix.
- **INDEPENDENT CRATE** only when independent publication/versioning/distribution is actually needed.

## If a split is justified: safe rollout

1. Capture baseline tests, feature sets, relevant public API and deterministic or binary golden results.
2. Refactor into **private modules** inside the current crate first to stabilize boundaries and state ownership, while preserving behavior.
3. Define the prospective dependency DAG and interface ownership; identify required public items and disallowing reverse edges. Avoid a `common` crate with no domain owner.
4. Move one cohesive responsibility and its tests into a single crate; avoid mixing the move with behavior changes, algorithm rewrites, or performance optimization.
5. Validate the exact requested targets/feature combinations (`cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, targeted `cargo test -p ...`; CI targets as needed).
6. Verify binary/API behavior and benchmark only claims supported by before/after reproducible measurements.
7. Audit cycles, new `pub` surface, duplicate dependency versions, feature unification, dependency leakage, generated code/build scripts, and rollback cost. If improvement is unproven and complexity increased, revert or keep the module-only design.

Note: `cargo check --workspace` alone does not guarantee all feature/target combinations. Select relevant CI matrices; don't blindly enable `--all-features` where features are intentionally incompatible.

## Required agent output — Architecture Decision Record (ADR)

Use `templates/crate-decision.md` and return:

- Current layout and evidence (module/dependency/ownership map, test consumers, build timings if relevant).
- Side-by-side option comparison A/B/C/D, confidence and maintenance cost.
- Decision: **KEEP / LIB-BIN / WORKSPACE / INDEPENDENT / DEFER**, with justification.
- Proposed dependency direction and public interface (if splitting).
- Migration stages, tests, rollback criteria and unresolved questions.

**Never** say that crate splitting inherently reduces CPU/RAM at runtime; crate boundaries are an architecture and Cargo-build decision. Runtime wins require separate data-layout/algorithm/profiling evidence.

## Sources

- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo resolver/features: https://doc.rust-lang.org/cargo/reference/resolver.html
- Cargo metadata: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
- Cargo tree: https://doc.rust-lang.org/cargo/commands/cargo-tree.html
- Cargo build timings: https://doc.rust-lang.org/cargo/reference/timings.html
