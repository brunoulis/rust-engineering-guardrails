# Deterministic parity, migration and differential testing

## Porting from TypeScript, Java, C++ or existing Rust

Do **not** translate the original inheritance/object graph literally into one mutable Rust God object. First extract behavioral contracts, state transitions, data dependencies, error behavior and observable ordering. Preserve behavior while designing Rust ownership intentionally.

1. Freeze a baseline fixture corpus: smallest case, edge cases, pathological sizes, realistic workloads, deterministic RNG seeds.
2. Produce versioned golden results and, for state machines, periodic snapshot hashes. Treat hashes as **regression detectors**, not cryptographic proofs of equality; diagnose mismatches by comparing structured state.
3. Run differential tests against the reference implementation on the same inputs. Compare full outputs and relevant internal state when possible. Include error paths and events, not just successful outputs.
4. For floating point, define target architecture, rounding, serialization, tolerance and whether *bitwise* equality is required. Floating-point reassociation or different libraries may alter results.
5. For hash tables, preserve iteration order where it is observable; `HashMap` traversal order is not a deterministic contract. Verify the chosen ordering instead of assuming it.
6. For ECS/arenas, define entity insertion/deletion and ID reuse semantics before replacing pointers. Use generational keys if stale IDs could be reused.
7. On optimized paths, rerun exact parity and performance checks. Never delete failing golden cases to make benchmarks green.

## Additional checks by area

- Rust/TS: `number` precision vs Rust integer widths, overflow/wrapping, sorted keys, JSON number/NaN behavior, timestamps, timezone, `null` vs absent.
- Rust/C++: integer conversions, object layouts, padding, alignment, ownership, allocation/deallocation pairs, destructor order, exceptions vs panics, ABI and threading contracts.
- Protocols/FFI: byte-for-byte wire fixtures, big/little endian, fuzz boundary inputs, error mapping.
- Rust game simulations: event order, RNG calls, scheduler phase ordering, replay hashes, and update-in-place semantics.

## When to stop

If parity fails, report the smallest divergent input and tick/event, then investigate the first divergent state field. No speed claim may supersede required correctness. Hash parity alone also cannot prove absence of latent bugs, memory leaks or performance debt.
