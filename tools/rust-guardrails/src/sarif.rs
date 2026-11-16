//! SARIF 2.1.0 export. Source locations use URI references, not filesystem spelling.
use std::collections::BTreeMap;

use crate::baseline;
use crate::json::{obj, Value};
use crate::model::{AnalyzerStatus, Severity};
use crate::report::Report;

fn uri(path: &str) -> String {
    let mut out = String::new();
    for b in path.replace('\\', "/").bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~' | b':') {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

pub fn render(report: &Report) -> String {
    let mut rules = BTreeMap::new();
    for f in &report.findings {
        rules.entry(f.rule.as_str()).or_insert_with(|| {
            obj([("id", f.rule.as_str().into()), ("shortDescription", obj([("text", crate::rules::get(&f.rule).map_or(f.rule.as_str(), |r| r.title).into())]))])
        });
    }
    let results = report
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut fields = vec![
                ("ruleId", f.rule.as_str().into()),
                (
                    "level",
                    match f.severity {
                        Severity::Info => "note",
                        Severity::Medium => "warning",
                        Severity::High | Severity::Blocker => "error",
                    }
                    .into(),
                ),
                ("message", obj([("text", if f.evidence.is_empty() { f.message.clone() } else { format!("{}\nEvidence: {}", f.message, f.evidence) }.into())])),
                ("partialFingerprints", obj([("rustGuardrails/v1", baseline::fingerprint(f).into())])),
                ("properties", obj([("category", f.category.as_str().into()), ("severity", f.severity.as_str().into()), ("confidence", f.confidence.as_str().into())])),
            ];
            if !f.file.is_empty() {
                let mut physical = vec![("artifactLocation", obj([("uri", uri(&f.file).into()), ("uriBaseId", "%SRCROOT%".into())]))];
                if f.line > 0 {
                    physical.push(("region", obj([("startLine", f.line.into())])));
                }
                fields.push(("locations", Value::Arr(vec![obj([("physicalLocation", obj(physical))])])));
            }
            if let Some(c) = &report.baseline {
                fields.push(("baselineState", if c.is_new[i] { "new" } else { "unchanged" }.into()));
            }
            if let Some(Some(reason)) = report.exceptions.get(i) {
                fields.push(("suppressions", Value::Arr(vec![obj([("kind", "external".into()), ("status", "accepted".into()), ("justification", reason.as_str().into())])])));
            }
            obj(fields)
        })
        .collect();
    let failed = report.analyzers.iter().any(|a| matches!(a.status, AnalyzerStatus::Failed { .. }));
    let notifications = report
        .analyzers
        .iter()
        .map(|a| {
            let (status, detail) = a.status_str();
            obj([
                ("descriptor", obj([("id", a.name.into())])),
                ("level", if status == "failed" { "error" } else { "note" }.into()),
                ("message", obj([("text", format!("{status}: {detail}").into())])),
            ])
        })
        .collect();
    let absolute = std::fs::canonicalize(&report.root).unwrap_or_else(|_| std::path::PathBuf::from(&report.root)).display().to_string();
    let absolute = absolute.trim_start_matches("\\\\?\\").replace('\\', "/");
    let base = if absolute.starts_with('/') { format!("file://{}/", uri(absolute.trim_end_matches('/'))) } else { format!("file:///{}/", uri(absolute.trim_end_matches('/'))) };
    obj([
        ("$schema", "https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0-rtm.5.json".into()),
        ("version", "2.1.0".into()),
        (
            "runs",
            Value::Arr(vec![obj([
                (
                    "tool",
                    obj([(
                        "driver",
                        obj([("name", "rust-guardrails".into()), ("version", env!("CARGO_PKG_VERSION").into()), ("rules", Value::Arr(rules.into_values().collect()))]),
                    )]),
                ),
                ("originalUriBaseIds", obj([("%SRCROOT%", obj([("uri", base.into())]))])),
                (
                    "properties",
                    obj([
                        ("project_profile", report.project_profile.clone().unwrap_or(Value::Null)),
                        ("baseline", report.baseline.as_ref().map_or(Value::Null, |b| b.to_json())),
                        ("verification", crate::verification::to_json(&report.findings, report.profile)),
                    ]),
                ),
                ("invocations", Value::Arr(vec![obj([("executionSuccessful", (!failed).into()), ("toolExecutionNotifications", Value::Arr(notifications))])])),
                ("results", Value::Arr(results)),
            ])]),
        ),
    ])
    .to_json()
        + "\n"
}
