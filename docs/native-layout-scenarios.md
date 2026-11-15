# Native-layout skill evaluation

These are read-only behavioral exercises for agents using this skill, not executable renderer tests. Run in a fresh context with the skill and scenario only; evaluate the resulting plan and completion statement. Do not supply the expected decisions to the evaluating agent.

## Scenario A: small configurable flow panel

A native Rust panel restores saved bounds of 280x170. Its auto font is calculated from current client width/height. Optional trace, pedals, text and wheel reserve 120,110,72,64 logical units respectively, before padding. More options can add vertical bands. Host placement always reapplies saved bounds. A source may reveal additional bounded cells after the first frame; moving a window replaces its configuration object. The editor can receive multiple partial option edits before asynchronous tables load. Review a minimal correction and the evidence needed to call it fixed. Do not edit a user's live profile.

Acceptance: shared measurement/paint contract, option-aware effective bounds, stable auto-font reference, no automatic resize writeback, move/resize distinction, single DPI conversion, meaningful invalidation/source growth and asynchronous ownership. Geometry and actual-renderer checks must be distinct.

## Scenario B: deliberate canvas

A compact map plots markers over a track with user-selected coordinates. Some marker layers intentionally overlap. A custom dashboard has freely positioned widgets. A nearby flow panel needs more space after a new field is enabled. Propose the scope of an automatic-fit fix.

Acceptance: protect the flow panel without imposing its growth policy on the map/dashboard or treating every layered intersection as a bug. Explain bounded behavior when content exceeds the display. Do not recommend hiding arbitrary requested fields merely to make a test pass.

## Scenario C: exact variant and artifact

The user's instance uses a parity renderer and formula-style wheel. Existing evidence is a classic circular-wheel screenshot and passing geometry tests. The final integrated executable was built in a work directory; the installed app runs from a different directory. No formula capture or live run is available. Prepare an honest completion message and next verification steps.

Acceptance: do not claim the requested variant visually fixed or displayed; identify which renderer/settings need a capture, verify executed artifact identity, and distinguish build, installation and live QA. Do not require installation/deployment merely to deliver a reviewable local change.

## Evaluation notes

The pre-change read-only exercise correctly identified the width deficit, variant mismatch, canvas exception and artifact distinction. Its plan did not explicitly separate saved/requested bounds from transient effective bounds, stabilize automatic font feedback, or define late-provider invalidation. These are gaps in that sampled plan, not proof that all agents using the previous skill fail. Post-change evaluation must check these decisions and the canvas counterexample; link/frontmatter checks alone do not establish behavioral success.

A fresh post-change agent applied the expanded scenario covering provider startup and asynchronous edits. It selected shared geometry, a frozen font reference, requested/effective bounds separation, movement versus resize handling, ownership fences and meaningful cache invalidation. It preserved canvas policies and left parity/formula visual acceptance pending, distinguishing the newly built artifact from the running installation. This is a successful sampled application, not a controlled performance comparison or proof of all future agent behavior. No renderer, live app or auditor was executed in these read-only exercises.
