# Native windows and content fit

Use for Rust-rendered windows, HUDs, overlays and embedded native UI when content, options or fonts can exceed the available surface. These are design and verification guardrails, not a new AST detector: syntax alone cannot establish a visual collision.

## Establish the actual layout contract

- Trace the user's instance through settings, projection, measurement, host placement and painter. Identify its renderer and visual variant; a classic circular widget is not evidence for a separate formula-wheel implementation.
- Classify flow content versus deliberate canvases. For flow panels, calculate intrinsic requirements for enabled content and apply the agreed grow/wrap/scroll policy. Maps, charts with intentional layers, freely positioned dashboards and custom panels need their own constraints. Do not assign automatic growth to every family.
- Define the response when required content exceeds the monitor work area or host maximum: wrapping, scrolling, bounded rows or another explicit overflow policy. Enlarging the window indefinitely is not a complete solution.

## Measure before placing and painting

Prefer shared pure geometry consumed by both measurement and painting. A small per-family correction is often enough; do not introduce a general layout framework without evidence that it is needed.

- Include title, headers, footers, optional rows/columns, gaps, padding and stroke/rotated-widget ink bounds. Scale row advances and reservations with typography, not only glyphs. Keep status/footer bands outside the last data row.
- Measure text with the actual font, weight, units and shaping when exact width matters; reuse valid cached metrics. Average character widths are not reliable for proportional text. Reserve bounded fields conservatively and give long free text an explicit wrap/ellipsis policy.
- Equal-width sibling slots require enough room for **each** slot's widest content: the group needs at least `slot_count * max(child_min_width)` plus gaps/insets. Summing differing child widths can still leave the widest child too narrow.
- Keep units explicit. Measure in logical units and convert once at the host boundary, or document an existing physical-pixel contract. Handle non-finite values, rounding and upper bounds. Check that actual client bounds satisfy the layout before painting a newly enlarged view.

## Separate requested state from effective geometry

Requested/saved bounds represent user intent; content-expanded bounds represent presentation. Automatic host growth must not masquerade as a manual resize, emit a spurious drag event or rewrite a profile. Persist content growth only when the product's explicit editing contract calls for it; preserve larger user sizes and unrelated raw settings.

Moving an expanded window must retain its requested dimensions on unchanged axes. A real resize updates the axes the user changed. Preserve active dragging and profile/instance/epoch ownership fences. Rapid independent option patches must survive cold asynchronous work, while stale work from another profile must be rejected.

Automatic font sizing needs a stable reference. Feeding expanded dimensions back into the font calculation can repeatedly enlarge both text and window. Compute font scale from requested/base geometry, freeze that scale for the current layout pass, and invalidate a retained base on genuine user resizing or a relevant policy change. Keep interactive constraints tied to the original font policy.

Use stable option-driven row counts rather than an unbounded live roster. If provider data reveals additional bounded content later, invalidate measurement before painting it. A per-configuration high-water mark can prevent grow/shrink oscillation during source loss; define its reset keys for options, font/theme metrics, DPI and ownership. A bounds-only move should not erase still-valid requirements. Do not add expensive text measurement or collection rebuilding to every paint without examining caching and invalidation.

## Validate geometry and the visible result

Choose a risk-based matrix, not an exhaustive Cartesian product. Cover the smallest supported/saved surface, the reported options, a worst-case enabled stack, minimum/maximum font sizes, representative DPI, long values/units and empty-to-populated data. Exhaust small boolean sets when cheap; use selected interaction cases for large configuration spaces.

1. Reproduce the intended containment/separation failure. Assert shared geometry invariants for regions that must be disjoint, stable repeated layout, single DPI conversion, movement versus resizing, and profile/reload ownership as applicable. A test that merely repeats constants from the implementation proves little.
2. Render the actual backend and variant with the failing settings. Inspect text/ink bounds as well as allocation rectangles; geometry alone cannot catch font mismatch, clipping, stale bitmap caches or a wrong paint path. Include a native/offscreen regression where practical. Release GPU/text caches in the valid device/apartment lifecycle during test teardown.
3. Build the final host that embeds the Rust code, including its UI/FFI integration. A standalone crate build is an auxiliary check when production uses another host. Bound test runs and avoid concurrent builds fighting over one target directory.
4. Identify the artifact actually executed: path/build identity, loaded resources and whether restart/reload is needed. Distinguish **compiled**, **installed**, **running**, **synthetically rendered** and **visually checked in the user's environment**. Compilation does not update an installed app. Do not claim to have shown or verified a formula variant from a circular-widget screenshot.

Report which variants/settings were rendered, the checks that actually passed, and remaining live QA. If the backend/environment is unavailable, leave visual acceptance pending. This does not prevent delivering a concrete, tested code change; it limits the completion claim.

Behavioral evaluation cases: [native-layout-scenarios.md](../docs/native-layout-scenarios.md).
