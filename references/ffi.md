# FFI boundary audit

Start with a boundary inventory: exported/imported functions, generated headers, representations, callbacks, handles and allocation/free pairs. Inspect Rust and the actual foreign callers; a Rust-only compile cannot establish compatibility.

An unchanged signature can still change ownership, encoding, serialized data, generated bindings, error semantics or cancellation order. Identify the actual consuming call path and exercise it against the selected artifact; unavailable foreign consumers remain UNVERIFIED. Include schema/version, byte order, lengths, nullability and round-trip/error fixtures when serialized data crosses the boundary. `repr(C)` or generated code alone does not prove those contracts.

| Contract | Evidence and checks |
| --- | --- |
| Ownership | For every pointer/handle record owner, borrow duration, transfer, retention and destruction. Pair allocation/deallocation in the same allocator/API; never free a Rust Vec with C free or reconstruct it with invented capacity. Test single release, error cleanup, and cancellation. |
| Lifetime and aliasing | Establish validity, initialization, provenance, alignment and exclusivity for each dereference. Check null/zero-length behavior and bounds before forming slices/references. Foreign mutation and retained references must obey Rust aliasing rules. |
| ABI and layout | Match calling convention, target, architecture, sizes/alignments/offsets, integer widths and signedness. Use suitable explicit representations; `repr(C)` is not a recursive serialization protocol. Rust Vec/String/trait objects and Rust-layout enums are not interchangeable with C++ standard-library types. Test against compiled foreign headers/callers. |
| Unwind and errors | Define panic/error translation, valid unwind ABI and foreign exception policy. `catch_unwind` does not catch aborts or make arbitrary foreign exceptions safe. Prevent unwinding across a boundary that disallows it. Ensure cleanup and output initialization on each failure path. |
| Callbacks and concurrency | Record callback thread, reentrancy, registration lifetime, cancellation and shutdown ordering. Prove no callback after release; review Send/Sync and foreign thread-affinity assumptions separately. |
| Resources and inputs | Bound lengths and allocation conversions, check overflow and encoding, and define out-pointer/buffer capacities and error ownership. Handles need invalid/stale use behavior; do not promise to safely dereference arbitrary invalid foreign pointers. |

For each unsafe block or impl, connect `SAFETY` reasoning to the invariant established by its caller or safe wrapper. A comment is an argument to review, not a proof. Include negative/error paths that can be exercised without invoking UB intentionally in an ordinary test process.

Use Miri for supported Rust paths/models, sanitizers for supported real native paths and native integration for ABI/allocator/callback behavior; see [verification intelligence](verification-intelligence.md). Report unsupported coverage and remaining assumptions. Never claim that successful Miri tests validate foreign code Miri did not execute.

For distributed native artifacts connect the effective Cargo versions/git revisions/path sources/[patch] overrides and their reasons to the foreign toolchain/ABI, allocator and unwind settings, features/cfg/target/profile and built/linked/runtime-loaded identities. The consumer check must exercise that artifact's allocation/free, error/unwind and callback/cancellation/shutdown contracts as applicable. Debug tests against another library do not validate the release library; record the mismatch UNVERIFIED in the [matrix](verification.md#verification-matrix). Use [supply-chain provenance](../policies/security-supply-chain.md#effective-selection-and-distribution) for selection and release evidence. Local changes with no foreign/distribution boundary use justified N/A.

Sources: [Rust Reference: type layout](https://doc.rust-lang.org/reference/type-layout.html), [external blocks](https://doc.rust-lang.org/reference/items/external-blocks.html), [Rustonomicon FFI](https://doc.rust-lang.org/nomicon/ffi.html). Check against the actual toolchain, target and foreign compiler.
