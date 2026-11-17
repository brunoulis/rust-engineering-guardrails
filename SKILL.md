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

For a local refactor, compare returned errors and resulting state with the old implementation, including a failure after partial progress. Measure the affected workload. Read the implementation, caller and closest tests together; keep successful output to counts and retain failure details. Consult the relevant section for [unsafe/FFI invariants](references/ffi.md), [async cancellation](references/async-reliability.md) or [crate boundaries](references/crate-boundaries.md). Use [refactor mode](modes/refactor.md) for changes across ownership boundaries, [performance](references/performance.md) for benchmark design and the [engineering guide](engineering-guide.md) for audits or migrations. Audit read-only unless edits are requested. Support performance claims with comparable measurements. Report behavior, checks and material limits.
