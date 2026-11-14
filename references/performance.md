# Performance engineering: evidence before changes

## Classification

Mark actual hot paths by workload: simulation ticks, iRacing telemetry samples, server request path, video frame conversion, packet parsing, or DB row transforms. Don't infer hotness from line count. Collect baseline runtime distributions and profile traces if feasible.

For each candidate operation, determine **frequency × actual cost × data size**:
- `.clone()`, `.to_vec()`, `.to_owned()`, `.collect()`, `format!`, `String::from`, `Vec::new`, `Box::new`, growth of `HashMap` / `Vec`, serialization, `Arc::new(data.clone())`.
- `Arc::clone` is a cheap atomic refcount operation (not a deep copy); contention can matter. `String`/`Vec` cloning often allocates. `collect` may use stack destinations or reuse capacities depending on type; confirm, don't assume.
- Reuse `Vec` via `clear`, `reserve`, `with_capacity`; pre-allocate scratch buffers when size limits are understood. Avoid retaining unbounded buffers or building huge pools.
- Never treat fewer source-level clones as measured throughput improvement. Test CPU, RSS, allocator pressure, tail latency, cache misses when relevant.

## Buffer and thread pools

A fixed-size bounded pool can reduce per-frame allocations **if** buffer sizes and concurrency are stable; maintain backpressure and handle pool exhaustion explicitly. A long-lived worker pool prevents repeated OS-thread startup. But choosing more workers than available cores can worsen contention, cache pressure and latency. Measure single-thread vs pooled vs parallel, including end-to-end pipeline throughput.

## Benchmark protocol

1. Identify workload, input size, build flags (`--release`), version, OS, CPU, power/thermal state, concurrent loads, memory and disk characteristics.
2. Run baseline on a clean build of the *same scenario*. Use warmups and repetitions; preserve raw results and median/variance, not a single stopwatch measurement.
3. Implement one bounded change, compare again with same input and environment; rerun correctness gates.
4. Record p50/p95/p99 for services; throughput, cycles/tick, allocations/tick and bytes/frame where appropriate; peak RSS and compile time separately.
5. Note noise and confounders; never claim speedups based only on microbenchmarks when throughput bottlenecks lie elsewhere.

Suggested tools, **if installed**:

```bash
cargo bench                  # Project-defined benchmarks (Criterion if configured)
cargo build --release --timings
hyperfine --warmup 3 --runs 10 './target/release/my-program --fixture sample.json'
# Platform-specific when available: cargo flamegraph (Linux), Instruments (macOS), perf, heaptrack, dhat, samply
```

Do not assume `criterion`, `hyperfine`, `cargo-flamegraph`, or allocator instrumentation is installed. Do not add benchmarking dependencies without project approval. `cargo build --timings` concerns compiler performance; runtime numbers are separate.

## Validating compiler and memory observations

`bench-compare` requires complete, finite, positive timing estimates with nondegenerate intervals. Criterion must include bounds and its actual confidence level; comparisons with different levels are inconclusive. Hyperfine must include at least two valid times and positive sample standard deviation, with mean/stddev matching the samples within relative tolerance `1e-6`. If `exit_codes` is present, every sample must have numeric exit code zero; failed, unknown or mismatched statuses are rejected. Legacy input without `exit_codes` is accepted with command success unverified. Missing, zero-variance, inconsistent or invalid uncertainty is rejected rather than converted into certainty. Preserve full export precision.

Hyperfine intervals use two-sided 95% Student-t values for 1–30 degrees of freedom, conservatively using the df=30 value for larger samples. This avoids applying the narrower normal `1.96` approximation to small samples. The method still assumes independent, approximately normal observations; sample count alone cannot verify those assumptions. Interval separation plus the noise threshold is a screening rule, not Criterion's hypothesis test or proof of comparable workloads. See [NIST mean confidence limits](https://www.itl.nist.gov/div898/handbook/eda/section3/eda352.htm), [t critical values](https://www.itl.nist.gov/div898/handbook/eda/section3/eda3672.htm) and [Criterion confidence/significance settings](https://docs.rs/criterion/latest/criterion/struct.Criterion.html#method.confidence_level).

- Split compilation boundaries only when `--timings` shows a bottleneck or maintainability justifies it. A small single crate can be preferable.
- Detect repeated dependency versions/features and unnecessary proc macros when build time matters.
- Use an allocation-counting benchmark harness or heap profiler to prove allocation frequency. Static grep only produces **candidates**.
- For performance reports, say `UNMEASURED` when profiling or baseline isn't available; don't invent percentages.
