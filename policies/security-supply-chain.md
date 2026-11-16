# Security and supply-chain gate

Activate for security review, dependency/lockfile changes, untrusted inputs, credentials or unsafe boundaries. Safe Rust does not establish authorization, resource bounds, parser correctness or the integrity of a build dependency.

## Classify the evidence

| Evidence | Report as | What remains to establish |
| --- | --- | --- |
| Reproducer or reviewed reachable defect under the target configuration | Confirmed vulnerability, with affected contract and evidence | Impact, remediation, regression and deployment exposure |
| cargo-audit/RustSec matches a dependency version | Dependency advisory match | Resolved target/features, advisory conditions, reachability and exploitability; do not claim a working exploit |
| Unmaintained/yanked/unsound informational dependency notice | Dependency warning | Notice category, affected version and relevant operational consequences |
| Missing policy, unrestricted source, stale database, weak limits or unreviewed input | Configuration risk or unknown | Project policy and actual exploit/impact evidence; absence of cargo-vet alone is not a vulnerability |

Do not lower an advisory merely because reachability is unknown. Record the uncertainty and project disposition. Assess severity from impact/evidence; syntactic hints never establish a blocker. Suspected secrets require redacted evidence; do not copy credential values into reports, chat, SARIF or baselines. Rotation/revocation and external reporting require their own authorization.

## Choose the gate

- `cargo audit`: match the actual lockfile against RustSec. Record tool version, lockfile hash, advisory database revision/update time, target filters, ignored advisories and exit status. Capture JSON even on an advisory exit; distinguish that from execution/database failure.
- `cargo deny check`: review configured advisories, licenses, bans and sources. Pin project policy; a license or duplicate-version policy failure is not automatically a security vulnerability.
- `cargo vet`: when the project adopts it, verify trusted audit coverage for the exact dependency versions and criteria. Review imports, exemptions and publisher trust. Do not auto-certify dependencies or expand trust to make CI pass.
- Secret scanning: use the project's established scanner on relevant tracked diff/history. Mask values and distinguish a candidate from a validated live credential. Review logs and generated artifacts as well as source.

These external tools are opt-in and are not run by `rust-guardrails`. Do not install tools, refresh databases or update dependencies merely to inspect. When a needed tool/network is unavailable, report that check as not_run or failed, never as clean. Repository security CI should preserve each tool's exit status; `continue-on-error` or `|| true` must not erase a required gate.

The native importer accepts saved `cargo audit --json` through `report --audit-json audit.json`. It validates required vulnerabilities and warnings containers and count consistency; it rejects unreadable or malformed inputs. Advisory matches are high-severity leads, warnings medium; neither proves exploitation. It does not authenticate the report, run cargo-audit, compare its lockfile to the current project or establish freshness/completeness. Inspect the imported database/settings provenance; filtered or ignored findings can be absent. Preserve the original scan's exit status independently. Empty valid imported data means only that this saved report lists no findings.

## Untrusted data and resource limits

Inspect length/size/depth conversions before allocation, recursion/decompression amplification, parser work limits, CPU/time budgets, cancellation and queue/backlog bounds. Separate syntactic deserialization from domain validation, identity/authentication and authorization. Check default values, unknown fields, duplicate keys, numeric overflow and state transitions according to the protocol. Choose explicit limits based on workloads, with error-path and boundary tests. Avoid logging credentials or full sensitive payloads.

For dependency updates follow the skill's version-aware workflow: current/target APIs, MSRV, feature/target/native compatibility, lockfile review and affected tests. Use [FFI review](../references/ffi.md) for foreign libraries and [verification intelligence](../references/verification-intelligence.md) for fuzz/properties/native checks.

Primary sources: [RustSec](https://rustsec.org/), [cargo-audit JSON model](https://github.com/RustSec/rustsec/blob/main/rustsec/src/report.rs), [cargo-deny](https://embarkstudios.github.io/cargo-deny/), [cargo-vet](https://mozilla.github.io/cargo-vet/). Inspect the installed versions and project configuration when adopting commands.
