//! Versioned debt snapshots. Matching is a multiset: one old observation cannot
//! excuse two new observations. Full keys are compared; hashes are display only.
use std::collections::BTreeMap;
use std::path::Path;

use crate::json::{self, obj, Value};
use crate::model::{Finding, Severity};

pub struct Snapshot {
    entries: BTreeMap<String, Vec<Severity>>,
}

pub struct Comparison {
    pub is_new: Vec<bool>,
    pub resolved_count: usize,
}

fn key(f: &Finding) -> String {
    // Never normalize whitespace inside Rust literals: it can change semantics.
    // Advisory provenance (database date/settings) changes independently of the
    // advisory/package identity carried by the message.
    let advisory = f.rule.starts_with("rustsec::") || f.rule.starts_with("dependency_warning::");
    obj([
        ("rule", f.rule.as_str().into()),
        ("file", f.file.replace('\\', "/").into()),
        ("evidence", if advisory { "" } else { f.evidence.trim() }.into()),
        ("message", if advisory || f.evidence.is_empty() { f.message.as_str() } else { "" }.into()),
    ])
    .to_json()
}

pub fn fingerprint(f: &Finding) -> String {
    let hash = key(f).bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    format!("v1-{hash:016x}")
}

impl Snapshot {
    pub fn read(path: &Path) -> Result<Self, String> {
        let read = || -> Result<Self, String> {
            let v = json::parse(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)?;
            if v.str_of("tool") != "rust-guardrails-debt" || v.get("schema_version").and_then(Value::as_f64) != Some(1.0) {
                return Err("expected rust-guardrails-debt schema_version 1".into());
            }
            let Some(Value::Arr(items)) = v.get("entries") else {
                return Err("entries must be an array".into());
            };
            let mut entries: BTreeMap<String, Vec<Severity>> = BTreeMap::new();
            for item in items {
                let k = item.get("key").and_then(Value::as_str).ok_or("entry key must be a string")?;
                // Reject unrecognizable keys rather than quietly accepting unrelated JSON.
                let fields = json::parse(k)?;
                if !["rule", "file", "evidence", "message"].iter().all(|field| fields.get(field).and_then(Value::as_str).is_some()) {
                    return Err("invalid entry identity".into());
                }
                let severity = Severity::parse(item.str_of("severity")).ok_or("invalid entry severity")?;
                entries.entry(k.to_owned()).or_default().push(severity);
            }
            Ok(Self { entries })
        };
        read().map_err(|e| format!("debt baseline {}: {e}", path.display()))
    }

    pub fn write(path: &Path, findings: &[Finding]) -> Result<(), String> {
        let entries = findings.iter().map(|f| obj([("key", key(f).into()), ("severity", f.severity.as_str().into())])).collect();
        let snapshot = obj([("tool", "rust-guardrails-debt".into()), ("schema_version", 1usize.into()), ("entries", Value::Arr(entries))]);
        std::fs::write(path, snapshot.to_json() + "\n").map_err(|e| format!("write debt baseline {}: {e}", path.display()))
    }

    pub fn compare(mut self, findings: &[Finding]) -> Comparison {
        let mut is_new = vec![true; findings.len()];
        // Allocate stronger existing evidence first, so input traversal order cannot
        // hide a new high-severity occurrence behind an old low-severity one.
        let mut order: Vec<usize> = (0..findings.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(findings[i].severity));
        for i in order {
            let f = &findings[i];
            if let Some(old) = self.entries.get_mut(&key(f)) {
                if let Some((j, _)) = old.iter().enumerate().filter(|(_, s)| **s >= f.severity).min_by_key(|(_, s)| **s) {
                    old.swap_remove(j);
                    is_new[i] = false;
                }
            }
        }
        Comparison { is_new, resolved_count: self.entries.values().map(Vec::len).sum() }
    }
}

impl Comparison {
    pub fn to_json(&self) -> Value {
        obj([
            ("new_count", self.is_new.iter().filter(|b| **b).count().into()),
            ("existing_count", self.is_new.iter().filter(|b| !**b).count().into()),
            ("resolved_count", self.resolved_count.into()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Confidence};
    fn f(line: usize, severity: Severity) -> Finding {
        Finding::new("rule", Category::Safety, severity, Confidence::Syntactic).at("src/a.rs", line).evidence("unsafe { act() }")
    }
    #[test]
    fn fingerprint_ignores_line_but_not_evidence_or_file() {
        assert_eq!(fingerprint(&f(1, Severity::Medium)), fingerprint(&f(100, Severity::Medium)));
        assert_ne!(fingerprint(&f(1, Severity::Medium)), fingerprint(&f(1, Severity::Medium).at("src/b.rs", 1)));
    }
    #[test]
    fn duplicate_count_and_severity_increase_cannot_be_hidden() {
        let old = f(1, Severity::Medium);
        let mut entries = BTreeMap::new();
        entries.insert(key(&old), vec![Severity::Medium]);
        let c = Snapshot { entries }.compare(&[f(5, Severity::High), f(7, Severity::Medium), f(8, Severity::Medium)]);
        assert_eq!(c.is_new, [true, false, true]);
    }
}
