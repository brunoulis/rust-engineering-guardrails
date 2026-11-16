# Rust documentation severity and test-code filtering

**Goal:** Keep missing unsafe documentation visible as medium review work and exclude explicitly test-only AST subtrees by default, including statements inside production files.

**Approach:** Keep existing rule IDs, evidence and baseline fingerprints. Change the two documentation rules from high to medium and describe their confidence accurately, including Clippy corroboration. Filter attributes before traversing AST nodes; retain uncertain/production-capable cfg branches and restore test code with `--include-tests`. Keep the heuristic engine's limitations explicit. No application fixes or dependency upgrades.

**Alternatives:** Suppressing documentation findings would hide useful review work. Text deletion of cfg blocks would lose nested syntax and production branches. Both are rejected in favor of syntactic filtering and retained medium signals.

- [x] Run the unchanged auditor tests, then add regressions reproducing documentary high gates and cfg(test) statements/items; observe failure.
- [x] Update rule severities/corroboration and AST filtering; verify production hazards and `--include-tests` counterfactuals.
- [x] Run AST/core suites, strict Clippy and formatting checks. Update skill/auditor patch versions and concise scope guidance.
- [x] Compare old installed auditor against corrected auditor on unchanged ProyectTelemetryGui, with matched settings and source hashes; retain occurrence-level deltas and a durable evaluation.
- [x] Obtain independent review and validate the skill package. Leave the changes reviewable on the fix branch.

**Review focus:** locals, expression/unsafe blocks, macros and match arms; cfg-gated static/const/trait/impl items; negative cfg and mixed production/test branches; native errors remain blockers; reduced high counts do not fix application defects.

**Completion evidence:** 146 AST/default/opt-in tests, 94 core tests (four ignored), both strict Clippy variants, format/diff checks and skill validation pass. Independent review's parameter and quoted-cfg counterexamples were reproduced red, fixed and covered by the passing suite. The Clippy cap-level wording was clarified. No dependency upgrade or application changes. Fresh matched reports retain 282 documentation signals at medium and remove exactly the test-only worker statement; source hashes and Git status are unchanged. See `docs/evaluations/2026-11-16-projecttelemetrygui-documentation-cfg.md`.
