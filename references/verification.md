# Verification and repo-level guardrails

For specialized verification, first use [verification intelligence](verification-intelligence.md) to connect changed risks to techniques, prerequisites and remaining uncertainty. Use [security and supply-chain policy](../policies/security-supply-chain.md) for audit/deny/vet and untrusted inputs; [continuous refactoring](../policies/continuous-refactoring.md) for baseline gates. Report recommendations as not_run until actually executed.

## Basic gates

Run only commands applicable to the repository, and report which actually ran:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --release --timings
```

Caveats: `--all-features` may be impossible when mutually exclusive features or target constraints exist; choose valid feature combinations. `-D warnings` is strict and may surface existing debt; don't silently disable it or claim success. `cargo build --timings` is optional and for compile-time analysis. Test performance in release mode with representative inputs.

Useful optional tools (do not assume installed): `cargo nextest`, `cargo llvm-cov`, `cargo deny`, `cargo audit`, `cargo machete`, `cargo bloat`, `cargo flamegraph`, `cargo miri`, `cargo fuzz` (libFuzzer), `proptest` (property tests), `cargo-mutants` (mutation testing), `dhat`/`dhat-rs` (heap traces), `turmoil` (simulated async networks), `kani` (bounded model checking), and Loom (concurrency model tests). Miri and some tools need compatible toolchains or dependencies. Pick one that tests the claim, not a kitchen sink by default.

## Clippy policy

- Default CI: `cargo clippy --workspace --all-targets ... -- -D warnings` on the toolchain pinned by the repo.
- Add selected lints (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::clone_on_ref_ptr`, `clippy::large_enum_variant`, etc.) only where their trade-offs match project needs.
- **Never turn on all `clippy::restriction` indiscriminately**; the official docs warn that restrictions can contradict each other or reject reasonable code.
- `clippy.toml` may configure `disallowed-methods` and `disallowed-types`, but indiscriminately disallowing `.clone()`/`Arc`/`mem::take` is unsound policy; target concrete risky APIs and contexts.
- For a deliberate exception use a narrowly scoped `#[allow(...)]` with an explanation where project conventions permit.

## Required regression cases

- Ownership: mutation affects the original state (not a throwaway clone), no hidden invalid temporary state.
- Hot path: representative data sizes and baseline end-to-end throughput, not only unit microbenchmarks.
- Concurrency: bounded workers and queues, cancellations, lock order and shutdown; inspect observable event order.
- Port: golden fixtures and differential verification; check architecture and endian where relevant.
- FFI/unsafe: every unsafe block has `SAFETY` reasoning; dedicated review and appropriate sanitizer/Miri/fuzz checks.
- API: error mapping, public signatures, compatibility, and any schema/protocol changes.
- Native UI, when applicable: small-surface containment, option/font/DPI combinations, requested versus effective bounds, stable auto fonts, actual backend/variant renders and final host artifact identity. Follow [native-layout.md](native-layout.md); deliberate canvases need their own overflow policy.

## CI philosophy

Static candidate scanning is advisory. Gate *actual* build errors, violations of agreed correctness/invariants, and reliable performance regressions only when the environment is controlled. Avoid hard-gating on `clone` counts, file length, or unmeasured metrics. `templates/ci-rust.yml` is an opt-in example.

## Specific gate for code-quality refactors

For an existing-code refactor, do not rely only on green Clippy. Require an explicit quality objective; characterization tests for high-risk flows; a small local change before an architecture rewrite; verification of API/errors/serialization/event ordering; and measured perf only when speed is claimed. See `references/refactoring.md` and `templates/refactor-plan.md`. `cargo fix` or a rust-analyzer assist can be useful but is never a substitute for review and tests.
