# Deep refactor

Use for requested architectural work or evidence that the present architecture prevents the task. Identify the failure of a smaller correction before increasing scope.

Record invariants, consumers, compatibility and rollback in [refactor plan](../templates/refactor-plan.md). For crate topology changes compare all options in [crate boundaries](../references/crate-boundaries.md) and record the ADR. Stage internal boundaries, mechanical movement and behavior migration separately; use compatibility adapters where they reduce risk.

Verify affected target/features, API/error/serialization contracts, replay parity and resource/lifetime behavior. Measure compile/runtime performance only when claimed. Review each migration boundary and unresolved consumer. Completion requires a coherent verified end state or an explicitly scoped staged deliverable; broad cleanup and extra crates do not establish improvement.
