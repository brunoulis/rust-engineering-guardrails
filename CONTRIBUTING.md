# Contributing — evidence before new Rust rules

Contributions should improve engineering decisions, not accumulate bans on Rust idioms. Keep the auditor in Rust, preserve its default AST engine and its dependency-free `--no-default-features` core, and keep project-specific preferences out of universal rules.

## Start with the actual version and scope

Read [SKILL.md](SKILL.md), the relevant reference, `tools/rust-guardrails/Cargo.toml` and its lockfile. Apply the skill's **Version-aware crate and toolchain work** policy: identify the resolved crate versions, features, target and active toolchain, then consult matching official documentation before using or cleaning up an API.

For a dependency/toolchain update, compare current and target versions, migration notes, MSRV, features and target/native compatibility. Explain why the contribution needs the update, keep unrelated dependencies unchanged, and inspect the lockfile diff. A newer release is not itself a reason to upgrade.

The tool currently declares Rust **1.96** as its minimum; CI uses **1.96.0** with rustfmt and Clippy. This is a reproducible validation baseline, not a promise that every later compiler behaves identically.

## Proposing a guardrail or detector change

1. Reproduce a concrete failure or documented review risk with minimal Rust input; record compiler, target, features and the observable consequence.
2. Include a valid counterexample: a bounded spawn, a legitimate clone, a short synchronous lock or a deliberate invariant must not become a defect just because its syntax matches.
3. Choose the evidence level: heuristic text, syntactic AST, compiler diagnostic or measured result. Do not promote a syntax match to a proven allocation, soundness defect or architectural failure.
4. Add positive/negative regressions beside the implementation in `tools/rust-guardrails/src/`, and CLI/fixture coverage in `tools/rust-guardrails/tests/` when output, exit codes or scope change. First observe the relevant test fail for the intended reason.
5. Preserve both feature configurations. Changes to the AST engine must document parser fallbacks; core changes must remain usable without optional dependencies. Do not remove legitimate findings solely to lower totals.
6. Document false positives, missed aliases/types/macros, severity, compatibility and rollback in the affected references or rule explanation. Benchmark changes need valid samples and uncertainty, not invented speedups.
7. Obtain a skeptical independent review for substantive changes. Revisit rules when crate/compiler updates or actual code expose counterexamples.

For guidance-only changes, use a concrete usage scenario and verify that an agent can locate and apply the instructions without imposing an unrelated rewrite. Small prose corrections need focused document/link review, not artificial tests matching wording.

## Local checks

Run from this repository's root (the directory containing `SKILL.md`). Use the declared toolchain with rustfmt and Clippy installed. These commands build this trusted tool and its fixtures; they are not a read-only audit of an arbitrary third-party project.

```sh
cargo fmt --manifest-path tools/rust-guardrails/Cargo.toml --all -- --check
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked --no-default-features
cargo test --manifest-path tools/rust-guardrails/Cargo.toml --locked -- --ignored
cargo clippy --manifest-path tools/rust-guardrails/Cargo.toml --locked --all-targets -- -D warnings
cargo clippy --manifest-path tools/rust-guardrails/Cargo.toml --locked --no-default-features --all-targets -- -D warnings
```

The `--ignored` invocation deliberately runs the two Clippy integrations in the **default** feature configuration. AST-only cases omitted from the core configuration are not executed tests; report those omissions. Do not use `--include-ignored` on the core configuration to force AST-only fixtures through a different engine.

After the Cargo build has created `target/`, verify the standalone examples:

```sh
rustc --edition 2021 --test examples/split_state.rs -o tools/rust-guardrails/target/split-state-test.exe
./tools/rust-guardrails/target/split-state-test.exe
rustc --edition 2021 --test examples/typestate.rs -o tools/rust-guardrails/target/typestate-test.exe
./tools/rust-guardrails/target/typestate-test.exe
```

The explicit output names also work on Windows; no example binaries belong in source control. [CI](.github/workflows/skill-ci.yml) runs these checks on Linux, macOS and Windows. A configured workflow is not a passing hosted run: record actual results and unavailable platforms separately.

## Safety and review evidence

- Source triage must not edit audited source or lockfiles, execute repository text, or silently enable deeper analyzers.
- Clippy, build scripts, proc-macros and optional compiler-based analyzers execute code. Keep these opt-in for audited targets and describe their artifacts and trust requirements.
- Missing/failed requested analyzers and invalid benchmark input must not produce a successful gate. Preserve report schemas and exit-code contracts, or explicitly document a migration.
- Do not weaken tests, auto-rewrite expected reports, add blanket suppressions or change user source to silence a rule.
- Keep test fixtures minimal and synthetic, or retain upstream license/provenance and a pinned revision. Do not commit target trees, credentials or full external checkouts.

A useful PR explains the problem and minimal change, positive/negative evidence, versions and documentation consulted, commands/results including skips, false-positive limits, compatibility/rollback and independent review outcome. Keep mechanical cleanup separate from detector or dependency changes. Preserve the existing [license](LICENSE) and third-party notices.

Workflow setup references: [rust-toolchain action](https://github.com/dtolnay/rust-toolchain) and [checkout action](https://github.com/actions/checkout). External actions are pinned by commit; update them as reviewed dependency changes, together with any affected validation baseline.
