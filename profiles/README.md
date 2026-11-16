# Domain profiles

Profiles are independent of audit/implement/refactor/deep-refactor task modes. Choose the primary CLI profile from actual requirements; inspect additional domain risks when profiles overlap.

| Profile | Primary questions | Relevant guidance |
| --- | --- | --- |
| service | Are admission, queues, memory, timeouts and shutdown bounded? Is blocking isolated? | [Async reliability](../references/async-reliability.md), [security](../policies/security-supply-chain.md) |
| engine | Which paths are hot? What allocation/latency budgets apply? Is replay deterministic? | [Performance](../references/performance.md), [parity](../references/parity.md) |
| library | Which APIs, feature/target/MSRV and error/serialization contracts do consumers need? | [Type safety](../references/type-safety.md), [verification](../references/verification-intelligence.md) |
| ffi | Who owns each allocation, pointer, callback and handle? Which ABI/unwind guarantees apply? | [FFI](../references/ffi.md) |
| prototype | What debt is explicitly accepted, and when will it be reviewed? | [Continuous refactoring](../policies/continuous-refactoring.md) |

No profile universally requires zero allocations, async, extra crates or a specific architecture. Prototype status does not relax memory soundness or correctness of promised behavior. Configure actual project contracts with [project profiles](../policies/project-profile.md).
