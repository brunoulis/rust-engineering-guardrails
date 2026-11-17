# Refactor

Improve an identified engineering problem while preserving observable behavior. Follow [refactoring](../references/refactoring.md), using characterization tests first for risky untested code.

Compare keeping the code, a minimal correction and a structural correction. State which defect or maintenance obstacle justifies the chosen change. Preserve errors, ordering, serialization, public APIs and cancellation behavior. Keep mechanical moves separate from semantic or performance changes.

Use [continuous refactoring](../policies/continuous-refactoring.md) to inspect added debt and changed contracts. A lower finding count is not a quality objective. Completion requires evidence that the specific problem improved and relevant behavior remains compatible; legitimate clones, locks and modules can remain unchanged.

Fill the [decision record](../templates/refactor-plan.md) with file-level behavior/consumer/evidence, new state owner/closure and first-to-current revisions. If successive fixes add flags, caches, locks or tasks, compare simplification and a local correction before retaining the current design; establish necessity from invariants and lifecycle, never LOC or revision count. Keep findings introduced by the diff separate from pre-existing debt and unknown attribution, with mechanism, consequence and evidence-backed disposition. Use the [verification matrix](../references/verification.md#verification-matrix) for valid package/feature/target combinations and supported feature-off behavior; unrelated configurations do not prove preservation.
