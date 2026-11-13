# Review patterns: evidence-driven before/after recipes

These are **review questions**, not blanket bans or automatic code rewrites. Prefer the minimal tested change over an elaborate pattern replacement.

## 1. Clone created only to drop a borrow

```rust
// WARNING when called every tick on a large vector:
let obstacles = self.map.obstacles.clone();
for unit in &mut self.units { move_unit(unit, &obstacles); }
```

Prefer distinct borrows through narrow functions when no snapshot semantics are needed:

```rust
let (obstacles, units) = (&self.map.obstacles, &mut self.units);
for unit in units { move_unit(unit, obstacles); }
```

If snapshot semantics are actually required (world frozen during iteration), cloning can be intentional. Verify mutating the actual state is correct. If both values are elements of the **same** Vec, consider `split_at_mut` and preserve index validity; don't construct aliased mutable references.

## 2. Checkout of live field through `mem::take`

```rust
let mut events = std::mem::take(&mut self.pending);
for event in events.drain(..) { self.handle(event); }
self.pending = events;
```

Problem to investigate: `handle()` sees `self.pending` empty; it may enqueue into that field and the final assignment can discard those new events. Early return/panic can leave the field replaced with `Default`. Alternatives: separate `process(events, &mut world)`, an explicit local `Vec`/queue that is the owner of processing work, or a well-defined two-queue state machine; write a regression test for nested enqueue. Not every `take` creates this bug.

## 3. Frame transfer with a full data clone

```rust
let copied = std::sync::Arc::new(frame.clone()); // Vec<u8>::clone copies pixels
send_to_encoder(copied);
```

Evaluate sending **owned** frames across a bounded channel, returning their buffers to a pool when encoding finishes; or sharing immutable data without copying. Confirm single-producer/multi-consumer and lifetime needs. A finite pool must be sized and backpressured, or it can deadlock/block the pipeline.

## 4. Repeated OS thread scopes

```rust
for frame in frames {
    std::thread::scope(|s| {
        s.spawn(|| process_part_a(&frame));
        s.spawn(|| process_part_b(&frame));
    });
}
```

This may spawn OS threads once per frame. Benchmark a fixed set of persistent workers, Rayon, or sequential processing; the optimal design depends on frame size, cores, synchronization cost and bandwidth. `thread::scope` is a useful API, not inherently a bug.

## 5. Mechanical OOP-to-Rust ports

```rust
// Suspicious shape when every handler receives the entire &mut Game:
fn attack(game: &mut Game, id: usize) { /* touches unrelated state */ }
fn heal(game: &mut Game, id: usize) { /* ... */ }
```

First map actual read/write sets; make dependency subsets explicit (`&Map`, `&mut Units`, `&mut Scratch`) and use typed IDs/commands as needed. When the behavior fundamentally fits ECS, design systems/queries and schedule phases rather than reproducing original object graph. Multiple `impl Game` blocks are not inherently an anti-pattern.

## 6. Global synchronization as a universal shortcut

```rust
// Smell when dozens of unrelated async services share one global mutable state:
Arc<tokio::sync::Mutex<Everything>>
```

Ask whether one owner with bounded channels, independent locks, immutable snapshots, or isolated services better expresses invariants. Consider blocking vs async locks, `await` boundaries and fault behavior. Use actual contention and p95/p99 evidence rather than assuming a particular primitive is faster.

## 7. Benchmark that proves the wrong thing

Bad: "We removed 15 clones, so it is 2× faster." Better: demonstrate the same workload on comparable release binaries, report repeated measurements, allocations/frame, peak RSS, and end-to-end throughput. Verify identical outputs. A fast inner function can still slow the full pipeline.
