//! Legacy `audit` and `triage` output (pre-1.0 schema_version 1, plus an `engine` field).
//! Rule names differ when the AST engine is active (e.g. `clone_in_loop`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::cli::Opts;
use crate::json::escape as json;
use crate::model::{Category, Finding};
use crate::scan::ScanResult;

const DISCLAIMER: &str = "Static signals only; not proof of allocation, unsoundness, architectural defects, or regressions.";

fn priority(f: &Finding) -> &'static str {
    if f.hot {
        "hot-review"
    } else {
        "review"
    }
}

fn map_json<K: AsRef<str>>(map: &BTreeMap<K, usize>) -> String {
    let entries: Vec<String> = map.iter().map(|(k, v)| format!("{}:{v}", json(k.as_ref()))).collect();
    format!("{{{}}}", entries.join(","))
}

pub fn audit(o: &Opts, r: &ScanResult) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_file_rule: BTreeMap<String, usize> = BTreeMap::new();
    for f in &r.findings {
        *counts.entry(f.rule.as_str()).or_default() += 1;
        *by_file_rule.entry(format!("{}::{}", f.file, f.rule)).or_default() += 1;
    }
    let mut out = String::new();
    if o.format() == "json" {
        let items: Vec<String> = r
            .findings
            .iter()
            .map(|f| format!("{{\"rule\":{},\"file\":{},\"line\":{},\"priority\":{},\"evidence\":{}}}", json(&f.rule), json(&f.file), f.line, json(priority(f)), json(&f.evidence)))
            .collect();
        let _ = writeln!(
            out,
            "{{\"schema_version\":1,\"disclaimer\":{},\"profile\":{},\"engine\":{},\"files_scanned\":{},\"lines_scanned\":{},\"findings_total\":{},\"hot_file_findings\":{},\"by_rule\":{},\"by_file_rule\":{},\"findings\":[{}]}}",
            json(DISCLAIMER),
            json(o.profile.as_str()),
            json(r.engine),
            r.files_scanned,
            r.lines_scanned,
            r.findings.len(),
            r.findings.iter().filter(|f| f.hot).count(),
            map_json(&counts),
            map_json(&by_file_rule),
            items.join(",")
        );
    } else {
        let _ = writeln!(out, "Rust audit [{}] engine={}: {} files, {} lines, {} candidates", o.profile, r.engine, r.files_scanned, r.lines_scanned, r.findings.len());
        let _ = writeln!(out, "WARNING: {DISCLAIMER}");
        for (k, n) in &counts {
            let _ = writeln!(out, "  {k}: {n}");
        }
        for f in r.findings.iter().take(o.limit) {
            let _ = writeln!(out, "  {} {}:{} [{}] {}", priority(f), f.file, f.line, f.rule, f.evidence);
        }
    }
    out
}

fn theme(c: Category) -> &'static str {
    match c {
        Category::Ownership => "ownership_and_state",
        Category::Performance => "potential_allocation",
        Category::Concurrency => "concurrency",
        Category::Safety => "unsafe_and_ffi",
        Category::Architecture => "module_cohesion",
        Category::Quality => "quality",
    }
}

fn queue(hits: &[&Finding]) -> &'static str {
    if hits.iter().any(|f| f.category == Category::Safety) {
        "review-safety-boundary"
    } else if hits.iter().any(|f| f.hot) {
        "review-known-hot-path"
    } else {
        "review-if-relevant"
    }
}

pub fn triage(o: &Opts, r: &ScanResult) -> String {
    let mut grouped: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in r.findings.iter().filter(|f| f.rule != "read_error" && !f.file.is_empty()) {
        grouped.entry(f.file.as_str()).or_default().push(f);
    }
    let mut rows: Vec<(&str, Vec<&Finding>)> = grouped.into_iter().collect();
    rows.sort_by_key(|(name, items)| {
        let q = match queue(items) {
            "review-safety-boundary" => 0,
            "review-known-hot-path" => 1,
            _ => 2,
        };
        (q, *name)
    });
    let themes = |items: &[&Finding]| items.iter().map(|f| theme(f.category)).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let mut out = String::new();
    if o.format() == "json" {
        let entries: Vec<String> = rows
            .iter()
            .map(|(name, items)| {
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for f in items {
                    *counts.entry(f.rule.as_str()).or_default() += 1;
                }
                let locations: BTreeSet<usize> = items.iter().map(|f| f.line).filter(|n| *n > 0).collect();
                format!(
                    "{{\"file\":{},\"inspection_queue\":{},\"explicitly_hot\":{},\"signals_total\":{},\"themes\":[{}],\"by_rule\":{},\"sample_locations\":[{}]}}",
                    json(name),
                    json(queue(items)),
                    items.iter().any(|f| f.hot),
                    items.len(),
                    themes(items).iter().map(|s| json(s)).collect::<Vec<_>>().join(","),
                    map_json(&counts),
                    locations.iter().take(12).map(usize::to_string).collect::<Vec<_>>().join(",")
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "{{\"schema_version\":1,\"disclaimer\":{},\"profile\":{},\"engine\":{},\"files_scanned\":{},\"files_with_signals\":{},\"inspection_candidates\":[{}]}}",
            json(DISCLAIMER),
            json(o.profile.as_str()),
            json(r.engine),
            r.files_scanned,
            rows.len(),
            entries.join(",")
        );
    } else {
        let _ = writeln!(out, "Rust refactor triage [{}] engine={}: {} files with signals", o.profile, r.engine, rows.len());
        let _ = writeln!(out, "WARNING: {DISCLAIMER}");
        for (file, hits) in rows.iter().take(o.limit) {
            let _ = writeln!(out, "  {} {file}: {} signal(s); review {}", queue(hits), hits.len(), themes(hits).join(", "));
        }
        let _ = writeln!(out, "Tip: `rust-guardrails report` gives a prioritized plan with verification steps.");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Severity};

    #[test]
    fn triage_prioritizes_safety() {
        let f = Finding::new("unsafe_keyword", Category::Safety, Severity::Medium, Confidence::Heuristic).at("src/lib.rs", 1);
        assert_eq!(queue(&[&f]), "review-safety-boundary");
    }
}
