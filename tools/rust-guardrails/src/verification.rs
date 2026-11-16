//! Select investigation techniques; recommendations never count as executed checks.
use crate::json::{obj, Value};
use crate::model::{Category, Finding, Profile};

struct Technique {
    name: &'static str,
    reason: &'static str,
    prerequisite: &'static str,
    limit: &'static str,
}

fn select(findings: &[Finding], profile: Profile) -> Vec<Technique> {
    let has = |category| findings.iter().any(|f| f.category == category);
    let memory = profile == Profile::Ffi
        || findings
            .iter()
            .any(|f| matches!(f.rule.as_str(), "unsafe_keyword" | "unsafe_block" | "unsafe_without_safety_comment" | "unsafe_fn_without_safety_doc" | "unsafe_impl" | "transmute"));
    let mut plan = vec![Technique {
        name: "contract-tests",
        reason: "Preserve observable behavior, errors, serialization and relevant features/targets",
        prerequisite: "Identify the changed contract and applicable repository test commands",
        limit: "Passing selected cases does not prove all program behavior",
    }];
    if memory {
        plan.push(Technique {
            name: "miri",
            reason: "Unsafe or FFI profile warrants memory-invariant investigation",
            prerequisite: "Compatible nightly with Miri; focused Rust tests; isolate unsupported foreign calls",
            limit: "Checks executed paths; most native FFI is unsupported; no soundness proof",
        });
        plan.push(Technique {
            name: "sanitizers",
            reason: "Exercise real native allocation, lifetime and FFI boundaries",
            prerequisite: "Supported toolchain/target and compatible instrumentation across native components",
            limit: "Executed paths only; target-specific; ordinary stable Windows builds do not establish support",
        });
    }
    if has(Category::Concurrency) {
        plan.push(Technique {
            name: "loom",
            reason: "Concurrency signals warrant checking interleavings and synchronization invariants",
            prerequisite: "A bounded model using Loom synchronization types; review model coverage",
            limit: "Lexical signal does not prove a race; unmodelled operations and bounded schedules remain unverified",
        });
    }
    if profile == Profile::Engine || profile == Profile::Library || has(Category::Ownership) {
        plan.push(Technique {
            name: "proptest",
            reason: "Check state, ownership, replay or public-value invariants over generated inputs",
            prerequisite: "Define an independent invariant/oracle and bounded generators; retain shrunk regressions",
            limit: "Generated samples do not exhaust the domain or prove determinism",
        });
    }
    if has(Category::Quality) || has(Category::Architecture) {
        plan.push(Technique {
            name: "cargo-mutants",
            reason: "Assess whether tests detect meaningful changes in affected logic",
            prerequisite: "Passing deterministic tests, scoped files and an explicit runtime budget",
            limit: "Survivors require review; equivalent mutants and timeouts are not confirmed defects",
        });
    }
    if has(Category::Performance) {
        plan.push(Technique {
            name: "benchmarks",
            reason: "Potential performance signals require representative measurements",
            prerequisite: "Comparable release workloads and machines, baseline, repetitions and uncertainty",
            limit: "Unmeasured candidates remain unknown; fewer clones or allocations is not proof",
        });
    }
    plan
}

pub fn to_json(findings: &[Finding], profile: Profile) -> Value {
    Value::Arr(
        select(findings, profile)
            .into_iter()
            .map(|t| {
                obj([
                    ("technique", t.name.into()),
                    ("status", "not_run".into()),
                    ("reason", t.reason.into()),
                    ("prerequisite", t.prerequisite.into()),
                    ("limitation", t.limit.into()),
                ])
            })
            .collect(),
    )
}

pub fn to_text(findings: &[Finding], profile: Profile) -> String {
    let mut out = String::from("\nVerification recommendations (not executed; inspect the actual change):\n");
    for t in select(findings, profile) {
        out.push_str(&format!("- {} [not_run]: {}. Requires: {}. Limit: {}.\n", t.name, t.reason, t.prerequisite, t.limit));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Severity};

    #[test]
    fn dependency_advisory_does_not_select_memory_or_concurrency_tools() {
        let f = Finding::new("rustsec::TEST", Category::Safety, Severity::High, Confidence::Measured);
        let selected = select(&[f], Profile::Service);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "contract-tests");
    }
}
