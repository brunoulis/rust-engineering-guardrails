# Implement

Deliver the requested behavior within the existing project's contracts. Inspect callers, errors, target/features, ownership, resource budgets and version-matched APIs before choosing a design.

For substantive choices compare a minimal implementation with a structural alternative. Prefer existing modules and conventions unless concrete requirements justify a new abstraction or crate. Write tests for meaningful behavior and error cases. Keep unrelated cleanup and dependency upgrades outside the change.

Select verification from the actual risks using [verification intelligence](../references/verification-intelligence.md). Review unsafe/FFI, cancellation, external inputs and public compatibility when touched. Completion requires the requested behavior, applicable checks with actual results, and a clear account of unresolved risks; report performance as unmeasured without representative measurements.
