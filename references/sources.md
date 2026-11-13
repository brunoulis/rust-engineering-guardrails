# Technical sources and confidence

## Primary discussion (community experience, not independently verified)

- Reddit thread, posted November 2026: https://www.reddit.com/r/rust/comments/1xryklf/we_designed_rust_to_fix_40_years_of_c_tech_debt/
- Original anecdote: compiler success + 3,091 replay hash checkpoints did not expose ownership-design debt, repeated clones and collections, temporary checked-out fields, repeated OS threads or deep pixel-buffer copies. The quoted numbers and reported speedups are **author claims**; no independent reproduction is provided here.
- u/TheRealCallipygian: newtypes, traits, explicit constraints, channels, reconsider shared mutable state. Interpret `avoid Arc` as a design question, **not an unconditional ban**.
- u/dnew and u/veryusedrname: generational indexing and slotmap to prevent stale index reuse after deletion.
- u/Esgrove: Clippy and Criterion/Hyperfine for perf iteration.
- u/slamb: quantitative success criteria, per-hot-path allocation review, architecture style and skeptical review in a fresh context.
- u/stumblinbear: reviewer distinct from implementer, refresh long-running instructions, improve skills iteratively.
- u/twyx and u/Franks2000inchTV: value clear engineering goals and avoid over-prescriptive architecture; scrutinize hot-path allocations.
- u/BirdTurglere: rewrites into Bevy/ECS demand real architectural adaptation, not class-for-struct substitution.
- Others correctly note classic OOP God-object debt predates AI and that no single architecture is universally superior.

## Original author's separately published skill

The author mentions `pnwcoastie` in a comment. A public repo, [PNWcoastie/rust-zero-drift](https://github.com/PNWcoastie/rust-zero-drift), contains a separate Rust skill, a reference crate, and a Clippy config. It includes stronger/very specific policies (typestate, newtypes, Sans-IO, forbidding unsafe, a prescriptive multi-crate split above a LOC threshold, broad crate recommendations). These are **ideas to evaluate**, not universal rules. The repo uses a restrictive license; this skill independently expresses general concepts without copying source code, examples, prose or configuration. Our alternative decision framework requires observed project benefits before extracting crates.

## Authoritative technical references

- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo metadata: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
- Cargo tree: https://doc.rust-lang.org/cargo/commands/cargo-tree.html
- Cargo resolver/feature unification: https://doc.rust-lang.org/cargo/reference/resolver.html
- Rust layout: https://doc.rust-lang.org/reference/type-layout.html
- Tokio cancellation: https://tokio.rs/tokio/tutorial/select
- Tokio bounded mpsc: https://docs.rs/tokio/latest/tokio/sync/mpsc/
- Rust borrow checker: https://rustc-dev-guide.rust-lang.org/borrow_check.html
- `std::mem::take` (replaces value with `Default`): https://doc.rust-lang.org/std/mem/fn.take.html
- `RefCell` runtime borrow checks: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
- Official Clippy lint groups and caution about `restriction`: https://doc.rust-lang.org/stable/clippy/
- Clippy usage/CI: https://doc.rust-lang.org/clippy/usage.html
- Clippy disallowed-methods config: https://doc.rust-lang.org/clippy/lint_configuration.html
- Cargo compilation timings: https://doc.rust-lang.org/cargo/reference/timings.html
- Slotmap typed generational keys: https://docs.rs/slotmap/latest/slotmap/
- Hyperfine: https://github.com/sharkdp/hyperfine
- Criterion: https://github.com/criterion-rs/criterion.rs
- Hermes skill discovery/project-local `.agents/skills`: https://hermes-agent.nousresearch.com/docs/guides/work-with-skills
- OpenAI skill directory convention: https://developers.openai.com/cookbook/examples/skills_in_api

## What this skill deliberately does NOT claim

- Every `.clone()` is a deep copy or heap allocation.
- `Arc`, `Rc`, `RefCell`, `mem::take`, `collect`, OOP, ECS, or arenas are always bad/good.
- Clippy alone can detect architectural or performance debt.
- Bit-exact replay hashes prove correctness for all unseen cases.
- Split crates automatically improve runtime/compile time.
- An agent's unverified self-assessment is a valid benchmark.

## Additional official refactoring reference material (v3)

- Clippy lint groups, correctness vs style/complexity/perf (and restriction caveat): https://doc.rust-lang.org/stable/clippy/
- `cargo fix` applies `rustc` diagnostic suggestions and mutates source: https://doc.rust-lang.org/cargo/commands/cargo-fix.html
- rust-analyzer semantic refactoring assists (rename, extraction): https://rust-analyzer.github.io/book/assists.html
- LLVM source-based test coverage, optional tool: https://docs.rs/cargo-llvm-cov/latest/cargo_llvm_cov/

These are technical references. The v3 refactor framework, tool, template and prose are independently written.
