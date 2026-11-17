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

Follow the affected crate's toolchain, features, callers and error conventions. Preserve ownership, ordering and state after partial failure. Exercise the affected paths and applicable fmt/Clippy checks. For scoped edits, compare protected files before and after tools: rustfmt may traverse `mod` from a test or crate root. Format only owned files and preserve existing edits.

Read related source together and expand test output on failure. Consult the relevant section for [unsafe/FFI invariants](references/ffi.md), [async cancellation](references/async-reliability.md) or [crate boundaries](references/crate-boundaries.md). For audits, migrations, performance or wider refactors, select the workflow in the [engineering guide](engineering-guide.md). Audit read-only unless edits are requested. Support performance claims with comparable measurements. Report behavior, checks and material limits.
