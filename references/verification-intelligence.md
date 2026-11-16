# Verification intelligence

Choose a technique that could falsify the changed claim. First inspect the diff, contracts, tests and resolved toolchain/dependencies; syntactic report signals supplement that inspection and cannot find every risk. The auditor emits recommendations with `status: not_run`; it never installs or runs the techniques below.

| Changed risk | Technique | Preparation and limits |
| --- | --- | --- |
| Unsafe pointer, aliasing, initialization, lifetime | Miri | Compatible nightly and Miri component; focused Rust tests. Unsupported foreign calls may require a Rust model. A clean run covers executed paths, not all unsafe code. |
| Atomics, custom synchronization, publication or lock ordering | Loom | Build a small bounded model using Loom types; ordinary std/Tokio tests do not become model checked by running a command. Record model/preemption bounds and unmodelled operations. |
| State, parsing, round-trip, arithmetic, replay invariants | proptest or differential tests | Independent oracle and bounded generators; retain seeds/shrunk failures. A round-trip alone may preserve the same wrong encoding twice; include known external fixtures. |
| Tests might miss wrong branches or error handling | cargo-mutants | Start from passing deterministic tests; scope files and runtime. Review survivors and equivalent mutants. Timeouts/unsupported changes are not proof of weak tests. |
| Real FFI allocations, data races or native lifetime paths | Supported sanitizers | Check exact toolchain, target and native instrumentation/linker compatibility. Test the real boundary; Rust-only Miri does not replace native integration. |
| Untrusted parsing, decompression, lengths, callbacks | Fuzz/property tests plus explicit resource bounds | Include huge lengths, deep nesting, malformed data, cancellation and allocation failures as appropriate. Use time/memory budgets; a finite fuzz run is not security proof. |
| Allocation/latency/throughput claim | Representative release benchmark and profiling | Same machine, workload and feature/target setup; repetitions, uncertainty, baseline and regressions. Unmeasured remains unknown. |

For every selected check record: claim → technique → test/model scope → prerequisites → actual command/configuration → outcome → remaining uncertainty. Status is passed, failed, not_run or unsupported; none of the last three means passed. If the needed test/model does not exist, implement it within the authorized scope before calling a tool run meaningful.

Examples after checking project-specific support:

```bash
cargo +nightly miri test -p affected-package --test memory_contract
cargo test -p affected-package --test property_contracts
cargo mutants --file src/changed_logic.rs
```

These names are examples, not commands the auditor executes. Pin a compatible nightly when a repository adopts a specialized gate; do not install one merely to inspect code. For Loom use the project's model-test configuration, not an assumed universal `--features loom` flag. For sanitizers follow the target-specific rustc documentation rather than copying Linux flags onto Windows.

Primary references: [Miri](https://github.com/rust-lang/miri), [Loom 0.7.2](https://docs.rs/loom/0.7.2/loom/), [proptest](https://proptest-rs.github.io/proptest/), [cargo-mutants](https://mutants.rs/), [rustc sanitizers](https://doc.rust-lang.org/unstable-book/compiler-flags/sanitizer.html). Verify the version actually used by the repository before changing its test APIs.
