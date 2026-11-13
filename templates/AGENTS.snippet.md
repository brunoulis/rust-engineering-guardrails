# Rust engineering policy (paste into repo-level AGENTS.md / CLAUDE.md)

For any Rust change, load the `rust-engineering-guardrails` skill before modifying code. Use project-specific requirements as primary constraints. Treat borrow-checker failures as potential design feedback rather than automatically adding clones, `RefCell`, or `mem::take` checkouts. Explain costs for newly introduced hot-path allocations and concurrency topology changes.

For nontrivial changes: define invariants, compare architectural options, implement, run the relevant tests/Clippy, gather reproducible benchmark data when making perf claims, and obtain an independent skeptical review. Label unmeasured performance claims explicitly. Refer to `references/agent-workflow.md` for handoff format. Do not enforce absolute bans on standard Rust APIs unless the project has a documented policy.

When asked to refactor low-quality existing Rust, load `references/refactoring.md` and select audit/refactor/deep-refactor. Demand evidence of an actual maintenance/testability/safety/performance obstacle. Prefer characterization tests and small independent commits. Do not automatically move code into crates or claim that fewer clones/lines makes it faster. Document the decision in `templates/refactor-plan.md`.
