# Source scope and audit precision

Establish scope before interpreting finding counts or accepting debt. Generated comparisons and other checkouts are evidence inputs, not automatically part of the maintained application.

## Shared inventory

The auditor selects Rust sources and Cargo manifests once for each report. Scanning and nested Cargo discovery consume this same inventory. In a Git checkout, optional read-only `git ls-files` selection includes tracked files (including tracked files matching ignore patterns) and non-ignored untracked files. It honors the repository's Git ignore rules without adding a Rust dependency. Git's filesystem monitor is disabled for this inventory command.

`--include-ignored` or `[scan] include_ignored = true` explicitly selects filesystem discovery, including ignored sources. It does not enter VCS internals, caches, Cargo target directories, tool worktree stores or child Git repositories. A separately selected repository root remains auditable. Ordinary source modules such as `src/build/` remain in scope. `--exclude` applies to sources and discovered manifests and prunes directories when its pattern excludes all descendants.

Without usable Git selection, filesystem discovery remains available and the source analyzer states that ignored files may be included. Do not assume this fallback honors `.gitignore`. Use explicit exclusions for comparison/build copies in a non-Git source tree. The tool reports the actual selection policy, scanned files, skipped directories and skipped tests.

## Nested Cargo projects

If the selected root contains Cargo.toml, that manifest remains the selected Cargo project, preserving existing workspace/member behavior. Otherwise, report discovers manifests from the retained inventory. Cargo's own workspace identity deduplicates member manifests; directory depth or crate size does not decide architecture. Findings and analyzer details identify each selected manifest relative to the audit root.

Metadata remains read-only and frozen. Clippy and llvm-lines run serially across discovered projects only when requested; they build trusted project code. Failed requested analyzers still fail a configured gate. An inherited workspace outside the selected manifest scope is reported as failed instead of building beyond the selected boundary. Saved llvm-lines input is parsed once rather than attributed to every discovered crate.

Do not create a workspace merely to run the auditor. An excluded manifest is not automatically checked through discovery; verify scope and feature/target assumptions before interpreting a complete-looking report.

## Test modules and unsafe evidence

The AST scanner resolves explicit `#[path]` test module locations, nested inline modules and transitive module children. A source reachable from production remains in scope even when tests also use it. Default Cargo library/binary/build-script/example roots and plain explicit target paths protect real targets reused by tests; an arbitrary test helper named lib.rs does not become a production root by its basename. Disabled automatic target discovery is respected. These target hints are not a complete TOML parser: unsupported target syntax conservatively retains package sources and reports that target identity is unverified. `--include-tests` restores test scanning, including inline test functions and file-level test configuration. Macro-expanded and cfg_attr-generated module paths remain syntactic limitations; the scanner does not invoke rustc to obtain a complete target module graph. The dependency-free heuristic engine cannot provide this AST context.

Unsafe comment signals describe evidence availability, not soundness. Actual adjacent comments are inspected separately from string literals. `unsafe_safety_comment_review` retains unlabeled prose or a marker without a substantive argument as a medium-priority review signal; `unsafe_block` reports labeled comment evidence as unverified information. Missing adjacent evidence remains an investigation lead. Review the argument, caller preconditions, ABI, lifetimes and applicable tests before deciding whether code needs to change. Clippy corroboration can establish a documentation-convention diagnostic, not prove an unsafe operation sound or unsound.

## Comparing revisions of the auditor

Use the same project source hashes, profile, hot-path declarations, exclusions and analyzer settings. Compare default-scope noise separately from changes within an equal maintained scope. Separate removed duplicate/test findings, reclassified documentation signals and new compiler/metadata evidence. Lower counts alone do not demonstrate application quality or scanner recall. Retain negative fixtures proving that production hazards, missing unsafe evidence and failed analyzers remain visible. Do not refresh an accepted baseline automatically after an auditor upgrade.
