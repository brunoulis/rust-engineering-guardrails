# Optional evidence assessment (Anti-Slop)

Use when the user requests a quality assessment or a substantial design/refactor needs a concise view of debt. It is a decision aid, not a certificate or a competition to remove patterns. The auditor does not compute an automated Anti-Slop score.

Report dimensions independently:

| Dimension | Useful evidence |
| --- | --- |
| Correctness | Requirements, regressions, error/boundary cases, differential fixtures |
| Design | Actual cohesion/coupling, consumer contracts, keep/minimal/structural comparison |
| Ownership | Real lifetimes, invalid temporary states, allocation ownership and transfer |
| Security | Reviewed boundaries, advisory reachability, authorization and bounded resources |
| Concurrency | Publication, cancellation, bounded backlog, lock ordering and shutdown tests |
| Verifiability | Independent oracles, representative tests, model scope, mutation review |
| Performance | Representative release measurements, allocation/latency traces and uncertainty |

For each dimension record **status** (`supported`, `concern`, `unknown`, `not_applicable`), evidence/location, confidence, inspected scope, concrete consequence and next action. Supported describes the inspected claim only. Not applicable requires a reason. Unknown is never scored as zero debt or a pass. Do not average away a correctness/soundness failure with good style scores.

If a project explicitly requires numeric scoring, it must define its rubric, weighting, evidence requirements and coverage first. Report unknown dimensions separately and omit an aggregate when it would imply unverified coverage. Counting clone calls, lines, crates, traits or files does not measure engineering quality. Performance without measurement stays unknown even when the implementation looks efficient.

Keep the report proportional: a small correction may only need the affected claim, evidence, actual check and remaining uncertainty. Baseline debt and justified retained code remain visible; do not alter working code just to improve a score.
