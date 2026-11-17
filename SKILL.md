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

For a local implementation, inspect the affected API, callers and tests. Use the project's resolved toolchain, features and error conventions. Preserve ordering, ownership and failure behavior. When an operation can partially update state, establish what the caller observes on error and test it. Keep a clone or allocation when it serves a concrete ownership or correctness requirement.

Use the affected crate's tests and applicable format or Clippy checks. Select features and targets that exercise the changed code. Ground performance claims in comparable measurements.

Open additional guidance when the task calls for it.

- For FFI or unsafe code, consult [FFI contracts](references/ffi.md) and establish lifetime, layout, unwind and ownership invariants at the boundary.
- For cancellation or queued work, consult [async reliability](references/async-reliability.md). For crate boundaries, consult [crate decisions](references/crate-boundaries.md).
- For audit, refactor, migration, dependency changes, performance work or a cross-crate change, use the applicable mode and references in the [engineering guide](engineering-guide.md). Audits are read-only unless the user requests edits.

For an implementation, state the observable change, checks actually run and material limitations. For an audit, lead with the finding and its consequence. Answer direct questions at the requested level of detail.
