---
name: rust-engineering-guardrails
description: Design, audit and incrementally refactor Rust with evidence-based architecture, type safety and measured performance.
metadata:
  version: 1.3.1
  hermes:
    tags:
      - rust
      - architecture
      - refactoring
      - performance
      - code-review
---

# Rust engineering guardrails

For a small crate, start with one scoped read. Select the manifest, source, nearest tests, named brief and runner together, e.g. `rg -n --max-filesize 48K "^" -g "Cargo.toml" -g "*.rs" -g "TASK.md" -g "run-tests.*" -g "!target/**" .`. Narrow these globs to the affected module in a larger workspace.

Patch the implementation and focused tests together. Reuse fixtures for boundaries, exact errors and state after partial progress. Verify once with the package runner and any tests it omits, collecting exit statuses in the same shell call. Scope rustfmt to edited files. Set `CARGO_TERM_QUIET=true` for PowerShell wrappers that treat Cargo progress as an error.

Use [the engineering guide](engineering-guide.md) for wider refactors, audits, FFI or performance investigations.
