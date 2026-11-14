# Optional Rust project routing

Merge this paragraph into existing project instructions only when persistent routing is wanted. Keep other instructions intact.

For Rust work, use `rust-engineering-guardrails` when available. Inspect the affected crate/workspace, actual toolchain, resolved dependencies, features and targets. Consult matching official documentation before library use, cleanup or upgrades. Preserve public contracts and existing architecture; review ownership, error handling, task/resource lifetimes and measured performance. Choose audit, implement or refactor according to the request. Audit is read-only; compiler-based checks of a target are explicit because builds/proc-macros execute code. Load only relevant references and report actual checks, limitations and rollback. Do not apply this guidance to unrelated languages or upgrade dependencies simply because newer versions exist.
