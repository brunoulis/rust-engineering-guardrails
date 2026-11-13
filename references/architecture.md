# Architecture, ownership and data structures

## Resolve borrow conflicts without local appeasement debt

1. Identify the exact mutable and immutable borrows, their lifetimes, and whether the data is logically disjoint. Don't fix borrow errors by blindly copying data.
2. Shrink borrow scope; split fields where legal: `let (map, units) = (&self.map, &mut self.units);`.
3. Move computation into a function taking `&Map, &mut Units, &mut Scratch` instead of accepting `&mut Game`.
4. If objects must refer to each other dynamically, compare: stable/generational IDs, owned command queues, ECS, arena/slotmap, actor/message passing, or interior mutability. Benchmark important access patterns.
5. Use `clone` when actually expressing required value semantics/snapshot isolation, when cheap enough, or to reduce complexity where measured impact is negligible. Record why.

`#[derive(Clone)]` does not require cloning. `Arc::clone` increments a refcount; `Arc::new(frame.clone())` copies the frame before wrapping. `Vec::clone` generally duplicates elements/allocation; `Vec::clear` retains capacity. `collect` *may* allocate, but not every `collect` necessarily incurs heap allocation. Examine the destination.

## Structure by responsibility, not arbitrary crate count

Prefer narrow types: `WorldState`, `Map`, `EntityStorage`, `TraversalScratch`, `Renderer`, `IOAdapter`, `Simulation`. Split modules for comprehensibility; split crates only when an evidence-backed comparison shows meaningful benefits over modules or a single `lib.rs`/`main.rs` package. A 3-crate workspace is an example, **not** a mandatory target. Splitting blindly can increase build graph complexity, public API surface, and feature-resolution interactions. **Required for crate changes:** `references/crate-boundaries.md` and `templates/crate-decision.md`.

Keep business rules separate from sockets, filesystem, graphics, clocks or platform APIs when that enables reproducible tests (Sans-IO). Prefer `&[T]`, `&str`, `&mut [T]` and purposeful custom views over passing a giant `&mut Self` through every helper. Avoid gratuitous abstraction: `trait` only when real substitutability, multiple implementations, testing seams or generic algorithms benefit.

## Newtypes and states

Newtypes prevent mixing semantically different scalar identifiers:

```rust
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct PlayerId(u32);
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct SessionId(u32);
```

Use enums for explicit transitions and state; typestate is useful for critical compile-time invariants but not obligatory for every workflow. Consider exhaustive matching, invalid-state prevention, API visibility, and error semantics.

## Storage choices

- `Vec`: locality, contiguous traversal, stable *indices only if deletion/reordering policy allows*.
- Slotmap/generational arena: identity survives reallocation and deletion does not silently redirect old IDs to a new entity; versioned-key overhead, holes, cache locality, and different iteration costs still matter. Prefer typed keys per entity kind.
- `HashMap`/`HashSet`: flexible lookup, but compare memory/cache costs with dense slots only if IDs are bounded/dense.
- ECS: useful for many entities with heterogeneous components/queries, not a free speed win or universal requirement. Porting OOP engines to Bevy requires semantic redesign, not transliteration.
- SoA vs AoS: choose based on dominant operations and measured cache behavior; avoid switching only for fashion.

## `std::mem::take` and temporary invalid states

`mem::take(&mut self.field)` swaps the value for `Default` and returns the original. Before using it, answer:
- Can any nested call observe `self.field` while empty?
- Can control flow return early? Can code panic before restoration? What invariant exists under unwind?
- Does the temporary state violate externally observable behavior?
- Is `std::mem::replace`, field splitting, `Option::take`, extracting a transaction struct, or owned commands clearer?

If an operation relies on restoring borrowed state, structure the API so restoration is enforced and tested, or avoid checkout. RAII may help, but borrowing `&mut self` while guard owns a field can still block helper calls.

## Unsafe/FFI decision

Use `#![forbid(unsafe_code)]` for pure safe-Rust crates when feasible. For unavoidable unsafe/FFI, contain code in a dedicated module/crate and document `// SAFETY:` invariants, aliasing, lifetime, allocation/deallocation ownership, thread-safety, panic across boundaries, alignment, endian, layout and ABI. The skill never asks the agent to bypass the borrow checker with `unsafe`.
