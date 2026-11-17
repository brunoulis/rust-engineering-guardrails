# Implement

Deliver the requested behavior within the existing project's contracts. Inspect callers, errors, target/features, ownership, resource budgets and version-matched APIs before choosing a design.

For substantive choices compare a minimal implementation with a structural alternative. Prefer existing modules and conventions unless concrete requirements justify a new abstraction or crate. Write tests for meaningful behavior and error cases. Keep unrelated cleanup and dependency upgrades outside the change.

Select verification from the actual risks using [verification intelligence](../references/verification-intelligence.md). Review unsafe/FFI, cancellation, external inputs and public compatibility when touched. Completion requires the requested behavior, applicable checks with actual results, and a clear account of unresolved risks; report performance as unmeasured without representative measurements.

Use the [standard deliverable](../references/agent-workflow.md#standard-deliverable) to record base/candidate/diff, changed files with behavior/consumer/evidence, state owners/closure and first-to-current solution evolution. For a bugfix, identify the regression failing before and passing after, or the concrete reason that evidence is unavailable; new-feature and characterization checks have different objectives. Attribute results to actual package/test kind/features/cfg/target/profile/runtime using the [verification matrix](../references/verification.md#verification-matrix). Record supported feature-off behavior and intentional authorized contract changes separately; inaccessible consumers or unexercised behavior remain UNVERIFIED. Build success does not establish those contracts.
