# Concurrency, async ownership and bounded resource use

## Decision order

1. Can the data be owned by one task/thread and commands/results passed through bounded channels?
2. Is immutable shared state sufficient (`Arc<T>`, no mutex)?
3. If mutable sharing is required, choose `Mutex`, `RwLock`, atomics or actor state based on critical section and consistency needs. Compare measured contention, complexity, and failure modes.
4. Are CPU-bound or blocking tasks being run in async tasks? Use appropriate blocking/CPU work pools and avoid oversubscribing cores.

`Arc` is not a code smell by itself. `Rc` and `RefCell` are single-thread tools; `RefCell` checks borrows at runtime and can panic. `Arc<Mutex<T>>` is appropriate for some jobs, but widespread global shared mutable state should trigger a design review.

Keep a short synchronous mutex section without await when contention is acceptable; async code does not by itself require an async mutex. Before changing a spawn loop, check whether admission is bounded before spawning and permits survive until completion. A bounded producer rate does not bound backlog when a consumer stalls: account for outstanding messages/bytes, producer lifetime and overload policy. See the pinned [Mini-Redis cases](source-cases.md) for mechanisms and their limits.

## Review checklist

- Thread creation frequency (`thread::spawn`, `thread::scope` inside per-frame/tick loops) and max active threads.
- Task creation (`tokio::spawn`) and max in-flight work; tasks are cheaper than OS threads but still consume memory and scheduler time.
- Backpressure: explicit bounded `mpsc` channels or semaphores; what happens on overload, cancellation, shutdown and partial failure?
- `await` while holding a blocking `MutexGuard` (avoid). Consider if async mutex is *actually* needed; an async mutex is not automatically faster.
- Lock ordering/poisoning, try-lock loops, priority inversion, long critical sections, fairness, and deadlock risk.
- Cloning large messages across channels vs sending an owned allocation or a reused buffer.
- Whether a worker pool starves the latency-sensitive simulation/core thread on a small machine.
- Shared cache access and deterministic order: concurrency may change event sequence even if all operations are data-race safe.

## Sans-IO pattern

Separate deterministic core (`fn on_event(state: &mut State, event: Event) -> Vec<Action>` or an output buffer) from transport adapters (HTTP, sockets, file). Returning a `Vec<Action>` may allocate; if it is on a measured hot path, consider passing a reusable output buffer. This permits isolated testing, replay, fuzzing and deterministic fixtures.

## Tail latency

Optimize measured p95/p99 and memory footprint for services; optimizing average service time while adding lock contention can hurt users. For telemetry pipelines prioritize bounded queues, dropping policy, timestamps, backpressure and lifecycle.
