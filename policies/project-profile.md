# Project profile: .rust-rails.toml

Copy and adapt [the template](../templates/rust-rails.toml) to the audited root. The auditor discovers `ROOT/.rust-rails.toml`; `--config FILE` selects an explicit file relative to the invocation directory. Missing explicit files, unknown keys, invalid values or unsupported syntax are errors. An absent automatic config keeps existing defaults.

The native parser supports a **strict TOML subset**, retaining a dependency-free heuristic core: single-line quoted strings, single-line string arrays, booleans and decimal integers, comments and trailing array commas. Dotted keys, multiline strings/arrays, inline tables, bare dates and floats are unsupported and rejected. Quote dates and version numbers. See the template for supported quoting and fields.

| Section | Fields | Effect |
| --- | --- | --- |
| Root | `schema_version = 1`, `profile` | Versioned schema and service/engine/library/ffi/prototype emphasis |
| `[scan]` | `hot`, `exclude`, `include_tests`, `max_file_lines`, `max_fn_lines`, `max_fields`, `max_methods` | Source selection and candidate thresholds; no type or performance proof |
| `[ci]` | `fail_on` | report severity gate: info/medium/high/blocker |
| `[contracts]` | `edition`, `msrv`, `critical_paths`, `unsafe_policy`, `allocation_budget`, `determinism` | Visible review inputs; **not automatically enforced** |
| `[[exceptions]]` | `rule`, `path`, `max_severity`, `reason`, `owner`, `expires` | Exact rule/path waiver for report gates, retained in output |

CLI flags override corresponding config values. A supplied `--hot` or `--exclude` list replaces that entire configured list; repeated CLI flags accumulate within that list. `include_tests = true` can be selected explicitly or through the chosen config. The legacy audit/triage outputs use scan settings but do not implement report gates, contracts or exceptions.

Contracts express what a human or agent must verify: compare edition/MSRV with actual manifests/toolchains; measure allocations at the named boundary; test deterministic replay; inspect each unsafe invariant. A configured budget or intent cannot count as an achieved result. They appear in report context for that purpose.

Exceptions require a nonempty owner/rationale, exact root-relative file path, exact rule, maximum accepted severity (info/medium/high) and valid quoted YYYY-MM-DD expiry. They remain active through that date inclusive in UTC. No globs, traversal or blocker waivers. Expiration or escalation invalidates the waiver; baseline matching remains a separate acceptance mechanism, so inspect both when reviewing accepted debt. Failed analyzers remain failures regardless of either mechanism.

Configuration never authorizes a dependency update, target/toolchain installation, architectural rewrite or external mutation. Keep the profile specific to actual requirements rather than distributing universal allocation, module-size or architecture limits.
