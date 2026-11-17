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

## Verification matrix

Attribute each result to the contract and actual selection, using this documentary matrix in the [handoff](agent-workflow.md#standard-deliverable) or [refactor record](../templates/refactor-plan.md). It requires no new auditor fields.

| Contract/scenario | Dimension (built / tested / integration / release) | Package/test kind, valid features/cfg, target/profile/runtime and artifact identity | Exact command/job | Selected / executed / passed / failed / skipped / disabled | Result (PASS/FAIL/UNVERIFIED/N/A), reason | Evidence location, date and candidate freshness |
| --- | --- | --- | --- | --- | --- | --- |
| | | | | | | |

- Record workspace member/package, unit/integration/doc/ignored test selection, default/no-default/explicit feature set, relevant cfg, target triple, toolchain/MSRV, profile, actual execution host/runtime and artifact identity as applicable. Exercise valid supported combinations; `--all-features` does not replace incompatible combinations or supported feature-off paths. Cross-compiling proves a build only, never execution on that target.
- Inspect runner summaries as well as exit status. Zero selected/executed, all ignored/skipped/disabled, unavailable tests, or stale logs leave the behavioral contract **UNVERIFIED**, even with exit zero. Preserve an actual runner failure as **FAIL** and report its separate verification gap. Counts unavailable from the runner stay unknown; non-test rows use N/A. Filtered-out tests are not executed, and listing tests alone is inventory evidence.
- Tie logs to base/candidate revision or dirty diff identity, command/job, configuration, artifact and date. Reuse evidence only when the relevant code and selection remain unchanged and that relationship is established. Prior green output alone is not candidate evidence. A current nonempty passing selection verifies only the contracts it actually exercises; record partial coverage and outstanding required checks.
- For bugfixes, identify the same regression failing before and passing after, with both identities; if unsafe/unavailable, record why and the limit. Characterization and new-feature checks state their own objective. An unchanged signature does not verify semantics, encoding, serialization, generated code or real consumer compatibility.
- Keep built, tested, integration and release claims separate. Debug tests do not establish a different release artifact's behavior; release remains UNVERIFIED until applicable release checks actually run and its identity/configuration match. An inaccessible consumer remains UNVERIFIED; use justified N/A only when no such contract applies. Auditor recommendations marked not_run are plans, never executed results.

Example assessment: `cargo test -p service missing_cancel_case` exits zero and reports zero executed; cancellation is UNVERIFIED. A default-feature test pass cannot verify a changed `#[cfg(feature = "retry")]` branch when `retry` is off, nor prove supported `--no-default-features` behavior. Obtain the relevant nonempty selection and runtime evidence rather than widening claims from the command exit.

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
