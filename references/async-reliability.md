# Async systems — cancellation, boundedness and shutdown

Use for Tokio, async services, request queues, streaming, background jobs, and FFI workers. The goal is bounded resource usage and explicit cancellation behavior, **not** banning standard synchronization primitives.

## 1. Capacity plan before concurrency

For each queue, task group, thread pool and buffer, identify: maximum in-flight tasks, max queue length, per-item bytes, producer/consumer rate, overload policy (block, reject, drop newest/oldest, spill), and shutdown drain budget. A bounded `tokio::sync::mpsc::channel(cap)` provides backpressure when `send` waits for capacity; unbounded channels are not automatically wrong but demand a defensible upper bound elsewhere or an explicit risk exception.

For CPU-bound work use a bounded worker/concurrency design. `available_parallelism()` is a useful starting hint, not a universal ideal worker count: profile reserved cores, memory bandwidth, CPU quotas, and latency-sensitive tasks. Avoid spawning an OS thread per tick/frame and avoid assuming `tokio::spawn` is free.

## 2. Cancellation and partial side effects

`tokio::select!` cancels losing branches by dropping their futures. Examine cancellation safety at each `.await`: could dropping this future lose a message, abandon a transaction mid-flight, retain a lock/resource, partially write a frame, or leave external side effects with ambiguous completion? Distinguish cancellation of a future from termination of separately spawned tasks.

Use a cancellation/shutdown mechanism that matches lifecycle (e.g. token or shared signal) and track spawned tasks when graceful drain matters. During shutdown: reject new work, signal consumers/producers, flush durable state within time budget, join/trap failures, and report partial completion. Avoid fire-and-forget where correctness depends on finishing.

## 3. Locking and safety

Holding a synchronous blocking mutex guard across `.await` risks blocking runtime worker threads and can cause deadlocks. Use a short non-await critical section or an async-aware mutex when it is actually needed. `std::sync::Mutex` remains appropriate for short synchronous critical sections in async applications; `parking_lot` is **not** a universal replacement.

Check lock order, fairness, data ownership, starvation, orphan workers and failure recovery. Prefer message ownership over `Arc<Mutex<_>>` when it simplifies design, not merely to comply with style.

## 4. Agent acceptance criteria

A concurrency change is incomplete without: overload scenario, cancellation scenario, shutdown/restart scenario, representative latency & peak memory observations (or `UNMEASURED`), and an explanation of what the system does with rejected/dropped work.

Sources: https://tokio.rs/tokio/tutorial/select ; https://docs.rs/tokio/latest/tokio/sync/mpsc/ ; https://docs.rs/tokio/latest/tokio/task/index.html
