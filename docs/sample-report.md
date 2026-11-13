# Rust audit report

| Field | Value |
| --- | --- |
| Root | `tests/fixtures/sample` |
| Profile | service |
| Source engine | ast |
| Files / lines | 2 / 68 |
| Tool | rust-guardrails 1.0.0 |

> Syntactic and compiler signals are leads, not proof. Only compiler errors and benchmark regressions beyond an explicit budget are treated as blockers. Sources and Cargo.lock are never modified (clippy/llvm-lines only write build artifacts to target/).

## What was checked

| Analyzer | Status | Detail |
| --- | --- | --- |
| source | ran | ast engine, 2 files |
| metadata | ran | resolved dependency graph (--frozen) |
| clippy | ran | 3 diagnostic(s), 0 compile error(s), build succeeded in 0.2s; extra lints: 12; lints capped at warn |
| llvm-lines | ran | parsed tests/fixtures/llvm-lines.txt; 4 rows, total 30737 IR lines |
| benchmarks | ran | criterion tests/fixtures/criterion (before → after): 3 compared, 0 unmatched |

## Summary by dimension

| Dimension | Blocker | High | Medium | Info |
| --- | ---: | ---: | ---: | ---: |
| safety | 0 | 1 | 0 | 0 |
| concurrency | 0 | 3 | 1 | 1 |
| ownership | 0 | 0 | 2 | 2 |
| performance | 1 | 0 | 2 | 4 |
| architecture | 0 | 0 | 0 | 1 |
| quality | 0 | 0 | 1 | 1 |

## Benchmark comparison

Source: criterion tests/fixtures/criterion (before → after)

| Benchmark | Verdict | Change | Detail |
| --- | --- | ---: | --- |
| group/encode | inconclusive | +1.7% | mean 300.0 ns → 305.0 ns (+1.7%); 95% CI [280.0 ns, 320.0 ns] vs [285.0 ns, 325.0 ns] |
| parse | improved | -20.0% | mean 1.000 µs → 800.0 ns (-20.0%); 95% CI [990.0 ns, 1.010 µs] vs [795.0 ns, 805.0 ns] |
| render | regressed (over budget) | +20.0% | mean 5.000 µs → 6.000 µs (+20.0%); 95% CI [4.950 µs, 5.050 µs] vs [5.940 µs, 6.060 µs] |

## Prioritized refactoring plan

Proposed only — nothing was changed. Work one item per change set; keep pure moves separate from behavior changes; for items marked *characterize first*, add tests pinning current behavior before editing. Priorities: P0 blocker, P1 ≥10, P2 ≥4, P3 below (score = severity × confidence × hot × log-count).

### R1 · P0 · performance — render: regressed (+20.0%) — exceeds --max-regression budget

- **Where:** project (1 occurrence(s))
- **Rule:** `bench_regression` · severity blocker · confidence measured · score 22.5
- **Why:** Confidence intervals do not overlap and the change exceeds the noise threshold.

| Option | When / what |
| --- | --- |
| Keep as is | The regression is the accepted price of a correctness fix (document it). |
| Minimal fix | Bisect the change set; re-run on a quiet machine to rule out noise. |
| Structural | Profile (perf/flamegraph/samply) before redesigning. |

**Verify:**

- [ ] re-run benchmarks at least twice on the same machine
- [ ] rust-guardrails bench-compare ...

**Done when:** `bench_regression` is no longer reported for the project by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/performance.md`

### R2 · P1 · safety — `unsafe` block without a `SAFETY:` justification (in `read`) [confirmed by clippy::undocumented_unsafe_blocks]

- **Where:** src/lib.rs:43 (1 occurrence(s))
- **Rule:** `unsafe_without_safety_comment` · severity high · confidence compiler · score 10.5
- **Why:** Every unsafe block needs written invariants so reviewers can check them; an undocumented block is unreviewed unsafe.

| Option | When / what |
| --- | --- |
| Keep as is | Never: at minimum document the invariant. |
| Minimal fix | Add a `// SAFETY:` comment stating why each operation is sound; narrow the block to the unsafe operations only. |
| Structural | Encapsulate the unsafe code behind a safe API with its invariants enforced by types; test under Miri where possible. |

**Verify:**

- [ ] cargo clippy --workspace --all-targets -- -W clippy::undocumented_unsafe_blocks -W clippy::multiple_unsafe_ops_per_block
- [ ] cargo +nightly miri test (if the code is Miri-compatible)
- [ ] cargo test --workspace (must pass before and after; record both runs)

**Done when:** `unsafe_without_safety_comment` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/anti-patterns.md`

### R3 · P2 · performance — Function dominating generated LLVM IR (monomorphization)

- **Where:** project (2 occurrence(s))
- **Rule:** `llvm_lines_hotspot` · severity medium · confidence measured · score 7.6
- **Why:** A generic instantiated many times inflates compile time and binary size.

| Option | When / what |
| --- | --- |
| Keep as is | Compile time is not a project concern. |
| Minimal fix | Move the non-generic body into an inner non-generic function (outlining) and keep only a thin generic shim. |
| Structural | Use `dyn Trait` at boundaries where dynamic dispatch is cheap relative to the work. |

**Verify:**

- [ ] cargo llvm-lines --release (before/after)
- [ ] cargo build --release --timings
- [ ] cargo test --workspace (must pass before and after; record both runs)

**Done when:** `llvm_lines_hotspot` is no longer reported for the project by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/performance.md`

### R4 · P2 · concurrency — Blocking call inside async code — `std::thread::sleep` (in `slow`)

- **Where:** src/lib.rs:39 (1 occurrence(s))
- **Rule:** `blocking_in_async` · severity high · confidence syntactic · score 7.0
- **Why:** `std::thread::sleep`, `std::fs` or other blocking calls in an async context block the executor thread and stall unrelated tasks.

| Option | When / what |
| --- | --- |
| Keep as is | Never in a multi-task runtime; acceptable only in a dedicated single-purpose runtime with documentation. |
| Minimal fix | Use the async equivalent (`tokio::time::sleep`, `tokio::fs`) or wrap in `spawn_blocking`. |
| Structural | Move blocking I/O behind a dedicated blocking pool or Sans-IO core. |

**Verify:**

- [ ] cargo test --workspace (must pass before and after; record both runs)
- [ ] run under load with tokio-console or timing logs: no executor stalls

**Done when:** `blocking_in_async` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/async-reliability.md`

### R5 · P2 · concurrency — Thread/task spawned per loop iteration — `std::thread::spawn` (in `spawn_all`)

- **Where:** src/lib.rs:34 (1 occurrence(s))
- **Rule:** `spawn_in_loop` · severity high · confidence syntactic · score 7.0
- **Why:** Spawning inside a loop creates one thread/task per item with no upper bound unless something else limits it — oversubscription and memory growth under load.

| Option | When / what |
| --- | --- |
| Keep as is | The iteration count is small and bounded by construction (e.g. one per CPU). |
| Minimal fix | Bound concurrency with a `Semaphore`, `JoinSet` with a cap, or a fixed worker pool. |
| Structural | Use a bounded work queue with a fixed number of workers and an explicit overload policy. |

**Characterize first:** add tests that pin the current observable behavior before changing this code.

**Verify:**

- [ ] cargo test --workspace (must pass before and after; record both runs)
- [ ] load test with N >> expected items; observe thread/task count and memory stay bounded

**Done when:** `spawn_in_loop` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/async-reliability.md`

### R6 · P2 · concurrency — Unbounded channel — `std::sync::mpsc::channel()` is unbounded (in `spawn_all`)

- **Where:** src/lib.rs:31 (1 occurrence(s))
- **Rule:** `unbounded_channel` · severity high · confidence syntactic · score 7.0
- **Why:** If producers outpace consumers, memory grows without limit and overload is invisible.

| Option | When / what |
| --- | --- |
| Keep as is | Producer rate is bounded by construction (e.g. one message per user action). |
| Minimal fix | Switch to a bounded channel and decide the policy when full (await, drop, reject). |
| Structural | Make backpressure part of the API contract. |

**Characterize first:** add tests that pin the current observable behavior before changing this code.

**Verify:**

- [ ] cargo test --workspace (must pass before and after; record both runs)
- [ ] overload test: memory stays bounded when the consumer is paused

**Done when:** `unbounded_channel` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/async-reliability.md`

### R7 · P2 · performance — Probable allocation per loop iteration — `.collect()` (in `step`)

- **Where:** src/hot.rs:6 — **hot path** (1 occurrence(s))
- **Rule:** `allocation_in_loop` · severity medium · confidence syntactic · score 6.0
- **Why:** `collect`, `to_vec`, `to_owned`, `to_string`, `format!`, `vec!`, `with_capacity` or `Box::new` inside a loop body usually allocate every iteration. It matters only if the loop is hot: info by default, medium in files marked --hot.

| Option | When / what |
| --- | --- |
| Keep as is | The loop is not on a measured hot path, or each iteration must produce an owned value. |
| Minimal fix | Reuse a scratch buffer declared outside the loop (`clear()` + `extend`), or write into an existing `String` with `write!`. |
| Structural | Restructure the pipeline to stream (iterators) instead of materializing intermediate collections. |

**Verify:**

- [ ] cargo test --workspace (must pass before and after; record both runs)
- [ ] cargo clippy --workspace --all-targets -- -W clippy::needless_collect
- [ ] only if speed is claimed: cargo bench -- --save-baseline before, apply, cargo bench -- --save-baseline after, then rust-guardrails bench-compare --criterion target/criterion --baseline before --candidate after

**Done when:** `allocation_in_loop` is no longer reported for src/hot.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/performance.md`

### R8 · P2 · concurrency — `clippy::unused_async`: unused `async` for function with no await statements

- **Where:** src/lib.rs:38 (1 occurrence(s))
- **Rule:** `clippy::unused_async` · severity medium · confidence compiler · score 4.5
- **Why:** Reported by the compiler with type information (1 occurrence(s)). Docs: https://rust-lang.github.io/rust-clippy/master/index.html#unused_async

| Option | When / what |
| --- | --- |
| Keep as is | The lint's trade-off does not apply here: allow it narrowly with `#[allow(lint)]` and a comment. |
| Minimal fix | Apply the compiler/clippy suggestion after reading it (`cargo clippy --fix` only on a clean tree; review the diff). |
| Structural | Rework ownership of the shared state (owner task + bounded channel). |

**Characterize first:** add tests that pin the current observable behavior before changing this code.

**Verify:**

- [ ] cargo clippy --workspace --all-targets -- -W clippy::unused_async (no `clippy::unused_async` at src/lib.rs)
- [ ] cargo test --workspace (must pass before and after)

**Done when:** `clippy::unused_async` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/concurrency.md`

### R9 · P2 · ownership — `clippy::redundant_clone`: redundant clone

- **Where:** src/lib.rs:55 (1 occurrence(s))
- **Rule:** `clippy::redundant_clone` · severity medium · confidence compiler · score 4.5
- **Why:** Reported by the compiler with type information (1 occurrence(s)). Docs: https://rust-lang.github.io/rust-clippy/master/index.html#redundant_clone

| Option | When / what |
| --- | --- |
| Keep as is | The lint's trade-off does not apply here: allow it narrowly with `#[allow(lint)]` and a comment. |
| Minimal fix | Apply the compiler/clippy suggestion after reading it (`cargo clippy --fix` only on a clean tree; review the diff). |
| Structural | Re-examine the data model: split state, IDs, or ownership transfer. |

**Characterize first:** add tests that pin the current observable behavior before changing this code.

**Verify:**

- [ ] cargo clippy --workspace --all-targets -- -W clippy::redundant_clone (no `clippy::redundant_clone` at src/lib.rs)
- [ ] cargo test --workspace (must pass before and after)

**Done when:** `clippy::redundant_clone` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/architecture.md`

### R10 · P3 · ownership — `Rc<RefCell<_>>` shared mutable state

- **Where:** src/lib.rs:11 (1 occurrence(s))
- **Rule:** `rc_refcell` · severity medium · confidence syntactic · score 3.0
- **Why:** Moves borrow checking to runtime (possible `BorrowMutError` panics) and often hides an ownership graph that could be expressed with IDs, arenas or message passing.

| Option | When / what |
| --- | --- |
| Keep as is | Genuinely shared graph-like data with short, non-reentrant borrows. |
| Minimal fix | Keep borrows short and never hold a `borrow_mut()` across calls that may re-borrow. |
| Structural | Replace with an arena + typed IDs, or a single owner that receives commands. |

**Characterize first:** add tests that pin the current observable behavior before changing this code.

**Verify:**

- [ ] cargo test --workspace (must pass before and after; record both runs)
- [ ] cargo clippy --workspace --all-targets -- -W clippy::await_holding_refcell_ref

**Done when:** `rc_refcell` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/architecture.md`

### R11 · P3 · quality — `todo!()` / `unimplemented!()` reachable in non-test code — `todo!` (in `later`)

- **Where:** src/lib.rs:47 (1 occurrence(s))
- **Rule:** `unfinished_code` · severity medium · confidence syntactic · score 3.0
- **Why:** Panics at runtime if reached.

| Option | When / what |
| --- | --- |
| Keep as is | Prototype profile with the debt recorded. |
| Minimal fix | Implement or return an explicit error. |
| Structural | Remove the unfinished path from the public surface. |

**Verify:**

- [ ] cargo test --workspace (must pass before and after; record both runs)

**Done when:** `unfinished_code` is no longer reported for src/lib.rs by `rust-guardrails report` (or each remaining occurrence carries a justification comment / narrow `#[allow]`), and every verification step passed with its output recorded.

Reference: `references/refactoring.md`

## Findings by dimension

### safety (1)

| Sev | Conf | Location | Rule | Message | Evidence |
| --- | --- | --- | --- | --- | --- |
| high | compiler | src/lib.rs:43 | `unsafe_without_safety_comment` | `unsafe` block without a `SAFETY:` justification (in `read`) [confirmed by clippy::undocumented_unsafe_blocks] | `unsafe { *p }` |

### concurrency (5)

| Sev | Conf | Location | Rule | Message | Evidence |
| --- | --- | --- | --- | --- | --- |
| high | syntactic | src/lib.rs:31 | `unbounded_channel` | Unbounded channel — `std::sync::mpsc::channel()` is unbounded (in `spawn_all`) | `let (tx, _rx) = std::sync::mpsc::channel::<usize>();` |
| high | syntactic | src/lib.rs:34 | `spawn_in_loop` | Thread/task spawned per loop iteration — `std::thread::spawn` (in `spawn_all`) | `std::thread::spawn(move \|\| tx.send(i).unwrap());` |
| high | syntactic | src/lib.rs:39 | `blocking_in_async` | Blocking call inside async code — `std::thread::sleep` (in `slow`) | `std::thread::sleep(std::time::Duration::from_millis(1));` |
| medium | compiler | src/lib.rs:38 | `clippy::unused_async` | unused `async` for function with no await statements | `pub async fn slow() {` |
| info | syntactic | src/lib.rs:12 | `arc_mutex` | `Arc<Mutex<_>>` / `Arc<RwLock<_>>` shared state — `Arc<Mutex<_>>` | `pub counter: Arc<Mutex<u64>>,` |

### ownership (4)

| Sev | Conf | Location | Rule | Message | Evidence |
| --- | --- | --- | --- | --- | --- |
| medium | compiler | src/lib.rs:55 | `clippy::redundant_clone` | redundant clone | `let _ = v.clone();` |
| medium | syntactic | src/lib.rs:11 | `rc_refcell` | `Rc<RefCell<_>>` shared mutable state | `pub graph: Rc<RefCell<Vec<u32>>>,` |
| info | syntactic | src/lib.rs:24 | `clone_in_loop` | `.clone()` executed per loop iteration (in `per_item`) | `let owned = n.clone();` |
| info | syntactic | src/lib.rs:33 | `clone_in_loop` | `.clone()` executed per loop iteration (in `spawn_all`) | `let tx = tx.clone();` |

### performance (7)

| Sev | Conf | Location | Rule | Message | Evidence |
| --- | --- | --- | --- | --- | --- |
| blocker | measured | project | `bench_regression` | render: regressed (+20.0%) — exceeds --max-regression budget | `mean 5.000 µs → 6.000 µs (+20.0%); 95% CI [4.950 µs, 5.050 µs] vs [5.940 µs, 6.060 µs]` |
| medium | syntactic | src/hot.rs:6 🔥 | `allocation_in_loop` | Probable allocation per loop iteration — `.collect()` (in `step`) | `let copy: Vec<f32> = f.iter().copied().collect();` |
| medium | measured | project | `llvm_lines_hotspot` | `core::ptr::drop_in_place<T>` generates 9.4% of LLVM IR | `2900 lines, 83 monomorphized copies (total 30737 lines)` |
| info | measured | project | `llvm_lines_hotspot` | `<alloc::vec::Vec<T> as core::ops::Drop>::drop` generates 2.5% of LLVM IR | `760 lines, 2 monomorphized copies (total 30737 lines)` |
| info | measured | project | `bench_inconclusive` | group/encode: inconclusive (+1.7%) | `mean 300.0 ns → 305.0 ns (+1.7%); 95% CI [280.0 ns, 320.0 ns] vs [285.0 ns, 325.0 ns]` |
| info | measured | project | `bench_improvement` | parse: improved (-20.0%) | `mean 1.000 µs → 800.0 ns (-20.0%); 95% CI [990.0 ns, 1.010 µs] vs [795.0 ns, 805.0 ns]` |
| info | syntactic | src/lib.rs:25 | `allocation_in_loop` | Probable allocation per loop iteration — `format!` (in `per_item`) | `out.push(format!("{owned}!"));` |

### architecture (1)

| Sev | Conf | Location | Rule | Message | Evidence |
| --- | --- | --- | --- | --- | --- |
| info | measured | project | `workspace_summary` | Workspace and dependency summary | `1 workspace member(s) [sample]; 0 internal edge(s) (0 dev); 1 packages in graph; transitive non-dev deps per member: sample=0` |

### quality (2)

| Sev | Conf | Location | Rule | Message | Evidence |
| --- | --- | --- | --- | --- | --- |
| medium | syntactic | src/lib.rs:47 | `unfinished_code` | `todo!()` / `unimplemented!()` reachable in non-test code — `todo!` (in `later`) | `todo!()` |
| info | syntactic | src/lib.rs:34 | `unwrap_in_non_test` | `unwrap()`/`expect()` in non-test code — `.unwrap()` (in `spawn_all`) | `std::thread::spawn(move \|\| tx.send(i).unwrap());` |

## Limitations

- The syntactic engine has no type information: a `.clone()` of an `Arc` and of a `Vec` look the same. Run with `--with clippy` for type-aware lints.
- Loop and async context are lexical; calls through helper functions are not followed.
- `large_type` merges same-named types from different modules.
- llvm-lines measures compile-time IR volume, not runtime speed. Benchmarks are only as representative as their workload.
- Thresholds (`--max-file-lines`, `--max-fn-lines`, `--max-fields`, `--max-methods`) are project choices, not universal rules.
