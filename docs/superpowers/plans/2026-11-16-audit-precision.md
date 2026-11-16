# Rust auditor precision implementation plan

**Goal:** Correct the four reproduced limitations and verify reduced false positives on unchanged ProyectTelemetryGui sources.

**Spec:** User request and the preceding real-project evaluation: scope pollution, nested Cargo manifests, cfg(test) modules with path attributes, and safety-comment classification.

**Approach:** Preserve the existing Rust scanner and evidence model. Use one shared, read-only source inventory for scanning and manifest discovery; honor Git ignore selection when Git is available, with an explicit include-ignored override and visible filesystem fallback. Resolve test module paths from syntax rather than filename guesses. Distinguish comment convention from verified soundness; retain unsafe review signals rather than silently suppressing them.

**Constraints:** No new Cargo dependencies, no changes to ProyectTelemetryGui, no automatic acceptance of debt, no tool/dependency upgrades, no publication. Preserve Criterion/baseline/SARIF and standalone crate behavior. Do not infer soundness from prose or from a SAFETY label.

## Tasks

- [x] Scope: failing tests for ignored comparison copies, nested worktrees, untracked maintained files, tracked ignored files, explicit override, filesystem fallback, and exclusions. Add `scope.rs` returning `Inventory { files, dirs_skipped, note }`; inventory includes Rust sources and Cargo manifests.
- [x] Cargo integration: failing tests for multiple standalone manifests, root-relative findings, workspace deduplication, excluded manifests and unavailable requested analyzers. Discover analysis roots from the shared inventory when the root has no manifest; preserve build-backed opt-in behavior.
- [x] AST context: failing tests for path-directed test modules, nested inline modules, transitive test helpers and retained production paths. Add syntax-aware resolution and keep include-tests opt-in working.
- [x] Safety evidence: failing tests for the real zero-initialization explanation, missing comments, misleading literals, marker-only comments and comments belonging to adjacent statements. Keep comment evidence unverified; differentiate documentation review from established defects.
- [x] Integration: complete default/core suites, Clippy and formatting, independent review, SARIF/schema/baseline checks. Repeat old/new binaries on the same source hash set and settings; compare both default scope and equal maintained scope, separately counting removed duplicate/test findings and reclassified safety signals.
- [x] Update skill/reference/CLI documentation and publish the local comparison artifact with verification limits.

## Shared interfaces

`scope::discover(root: &Path, exclude: &[String], include_ignored: bool) -> Result<Inventory, String>` returns discovered paths anchored like the supplied root, relative skipped directories and a human-readable scope note. `Inventory.files` contains only `.rs` and `Cargo.toml` files. Paths must remain inside the selected root; symlink directories and child Git repositories are not followed. The explicitly selected root itself is permitted.

`ScanConfig.include_ignored: bool` selects the override. `ScanResult.scope_note: String` reports the actual policy and `cargo_manifests: Vec<PathBuf>` retains manifests from that inventory snapshot. CLI/config integration supplies the new boolean; the scanning integration filters the shared inventory to Rust sources. Cargo discovery consumes retained manifests and rebases findings to the audited root.

## Evidence record

- Clean skill checkout at f1c4044; working branch fix/rust-audit-precision.
- Preserved pre-change release executable under ignored target/validation/projecttelemetrygui-precision-2026-11-16.
- ProyectTelemetryGui has 514 Git-selected Rust files including tests; its build comparison copies and nested worktree are Git-ignored. Initial project revision b294d6f9; existing untracked repaint specification is outside this task.
- Final: 534 source/manifest/lock hashes and path set unchanged; project status unchanged. Default report 4491 → 1359; equal maintained syntactic scope 1342 → 1333, nine test signals removed and four comment signals reclassified. Ten metadata projects, nine full frozen graphs and one offline workspace-only result.
- Independent review caught production-root ambiguity; added RED→GREEN tests for nested test lib.rs, real bins, custom targets, build scripts and examples reused by tests. Unsupported target hints retain source conservatively and report uncertainty. Final review confirmed fixes.
- Final validation: 137 default and 93 core tests pass, both strict Clippy variants and fmt pass; native SARIF schema passes; baseline repeats with zero new signals; a synthetic missing-unsafe-evidence control still fails its high gate. Details in docs/evaluations/2026-11-16-projecttelemetrygui-precision.md.
