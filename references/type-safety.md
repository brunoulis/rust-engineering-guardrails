# Type-driven correctness and data-model safeguards

Use when a domain has confusing IDs/units, non-trivial state transitions, deterministic arithmetic, unsafe boundaries, or stable entity identity. These techniques are **conditional**, not a mandate to wrap every primitive.

## Newtypes and validation

Use distinct `struct UserId(u64);` and `struct OrderId(u64);` when mixing them causes real bugs. Keep constructors private when values have invariants: `fn try_new(...) -> Result<Self, DomainError>`. Avoid `pub` tuple fields on validated types. Prefer `TryFrom` for fallible conversion and checked conversions (`try_into`) at untrusted boundaries.

Distinguish an *identifier* from a numeric *quantity* with units (bytes, milliseconds, cents, distance). In FFI or serialization boundaries, verify representation intentionally: `#[repr(transparent)]` is appropriate only for a transparent representation contract; newtype alone is enough for pure Rust type separation. Avoid `repr(C)` unless ABI/layout is required. `repr(Rust)` layout must not be presumed stable.

## Typestate or runtime enums?

Use typestate when a transition must be **statically impossible** to skip and transition variants are limited and stable: e.g., `Unverified -> Verified -> Committed`, with consuming methods. Consider complexity of generic types, serialization, persistence and dynamic state transitions. If states must be restored from database, handled in heterogeneous collections, or switch at runtime, a tagged `enum` and checked transition methods may be clearer. Don't create exponential state-type combinatorics.

A valid design should make prohibited actions hard, preserve ergonomic APIs and provide recoverable errors for invalid external input. Typestate cannot replace parsing/validation at I/O boundaries.

## Generational identity

Consider versioned keys/arena when entities are removed/reused and stale raw `usize` indices could alias a newly created entity. Compare `slotmap`/generational arena with plain `Vec`, `HashMap`, and handles given lifetime requirements. Arenas do not automatically make code lock-free, deterministic, leak-free or cache-efficient; confirm allocation and access patterns. Specify what happens on key exhaustion/generation wrapping for very long-lived systems if relevant.

## Deterministic arithmetic

When bit-for-bit parity is a stated requirement, fix input representation, overflow semantics (checked/saturating/wrapping as the domain requires), rounding, order, sorting, sources of randomness, target assumptions and time/clock dependencies. Decimal/fixed-point arithmetic **can** reduce some floating drift but changes representation and is not always equivalent to the prior simulation. Maintain golden or differential tests before changing math types.

Floating-point is not forbidden; document deterministic scope (same architecture/compiler flags vs cross-platform bit-parity). Unordered map iteration can violate replay order even in memory-safe code.

## Errors and sensitive values

Use domain error variants that consumers can handle; adapters attach transport/context errors without leaking secrets. Avoid a global rule against `unwrap`: it can be appropriate for proven invariants in tests/prototypes; for untrusted inputs and production failure paths prefer explicit handling. Keep secrets out of `Debug`/logs and check lifetime/copy semantics; secret wrappers are aids, not guarantees of memory zeroization in every copy or allocator.

## Verification ideas

- Negative compile-time tests for APIs intentionally disallowing invalid states (e.g., `trybuild`, when justified).
- Property tests for validated ranges, conversions, parsing and transition invariants.
- Round-trip serialization and ABI/representation checks where necessary.
- Differential/replay tests for deterministic arithmetic and storage layout changes.
- Mutation testing when it adds value; never treat mutation score as proof of completeness.

Sources: https://doc.rust-lang.org/std/convert/trait.TryFrom.html ; https://doc.rust-lang.org/reference/type-layout.html ; https://docs.rs/slotmap/latest/slotmap/ ; https://doc.rust-lang.org/nomicon/other-reprs.html
