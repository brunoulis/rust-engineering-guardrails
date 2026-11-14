# Source cases: when to keep working code

Read when a review proposes replacing shared state, clones, spawned tasks, or explicit borrow boundaries. These pinned examples demonstrate specific mechanisms, not whole-project quality or measured performance. The upstream suites below were inspected, not executed here.

## Mini-Redis: short lock and shared bytes

[Db::get](https://github.com/tokio-rs/mini-redis/blob/3d93b42bc363220f85af4fc9e1bebd35b588a4a3/src/db.rs#L144) locks in a synchronous method, reads the entry and clones a `Bytes` handle. The guard ends before returning; the clone shares the payload. Keep this design when the section is short, contains no await and contention is acceptable. Replacing it with an actor or async mutex needs a concrete ownership or contention problem. [Tokio's shared-state guidance](https://tokio.rs/tokio/tutorial/shared-state) explains these conditions.

[Protocol tests](https://github.com/tokio-rs/mini-redis/blob/3d93b42bc363220f85af4fc9e1bebd35b588a4a3/tests/server.rs#L12) cover get/set; `key_value_timeout` uses paused time and explicit advancement to check expiry. They preserve behavior, not a claim of low contention. Mini-Redis is educational; larger critical sections and different workloads need their own evidence.

## Mini-Redis: a bounded infinite spawn loop

[Listener::run](https://github.com/tokio-rs/mini-redis/blob/3d93b42bc363220f85af4fc9e1bebd35b588a4a3/src/server.rs#L215) acquires a semaphore permit before accepting and spawning, moves it into the task and drops it after the handler finishes. This limits concurrent handlers even though the loop is infinite. Keep the existing mechanism after checking permit lifetime and error/cancellation paths. A permit acquired inside each spawned task would leave waiting tasks unbounded.

The example's 250-connection limit is not a universal budget and does not bound every queue or message size. [Server tests](https://github.com/tokio-rs/mini-redis/blob/3d93b42bc363220f85af4fc9e1bebd35b588a4a3/tests/server.rs) cover protocol/lifecycle behavior; no saturation-specific test was identified in that file. The local scanner regression tests the admission pattern's severity, not runtime semaphore correctness. A remaining `spawn_in_loop` signal can close as reviewed/retained without editing this code.

## ripgrep: borrows limited to the operation

[search_path and search_reader](https://github.com/BurntSushi/ripgrep/blob/178cc027e283fd965cb72d0a7395b219eada170c/crates/core/search.rs#L380) receive a matcher, mutable searcher and mutable printer explicitly. Each printer branch creates a scoped sink and returns the relevant result. Use this as an ownership-boundary example when an oversized mutable receiver causes conflicting borrows. The local [split-state example](../examples/split_state.rs) provides an executable smaller exercise.

Keeping the explicit printer branches is a review judgment: similar lines alone do not justify a new abstraction. [CLI regressions](https://github.com/BurntSushi/ripgrep/blob/178cc027e283fd965cb72d0a7395b219eada170c/tests/regression.rs#L4), including `r16` and `r25`, pin exit behavior, paths and output. They do not prove that this particular decomposition is faster or optimal. Preserve those observable contracts when refactoring.

Only links and independent summaries are included here; upstream source/license files remain in the external audit corpus.
