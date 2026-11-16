# Refactor

Improve an identified engineering problem while preserving observable behavior. Follow [refactoring](../references/refactoring.md), using characterization tests first for risky untested code.

Compare keeping the code, a minimal correction and a structural correction. State which defect or maintenance obstacle justifies the chosen change. Preserve errors, ordering, serialization, public APIs and cancellation behavior. Keep mechanical moves separate from semantic or performance changes.

Use [continuous refactoring](../policies/continuous-refactoring.md) to inspect added debt and changed contracts. A lower finding count is not a quality objective. Completion requires evidence that the specific problem improved and relevant behavior remains compatible; legitimate clones, locks and modules can remain unchanged.
