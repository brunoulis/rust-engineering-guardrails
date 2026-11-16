//! Import saved cargo-audit evidence without running Cargo or refreshing a database.
use std::path::Path;

use crate::json::Value;
use crate::model::{AnalyzerRun, Category, Confidence, Finding, Severity};

pub fn analyze(path: &Path) -> Result<(Vec<Finding>, AnalyzerRun), String> {
    let input = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let report = crate::json::parse(&input).map_err(|e| format!("cargo-audit JSON: {e}"))?;
    parse(&report)
}

fn required<'a>(v: &'a Value, key: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("cargo-audit JSON: missing {key}"))
}

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    required(v, key)?.as_str().filter(|s| !s.trim().is_empty()).ok_or_else(|| format!("cargo-audit JSON: invalid {key}"))
}

fn package(v: &Value) -> Result<String, String> {
    let p = required(v, "package")?;
    Ok(format!("{} {}", text(p, "name")?, text(p, "version")?))
}

fn parse(report: &Value) -> Result<(Vec<Finding>, AnalyzerRun), String> {
    let vulnerabilities = required(report, "vulnerabilities")?;
    let Value::Arr(list) = required(vulnerabilities, "list")? else { return Err("cargo-audit JSON: vulnerabilities.list must be an array".into()) };
    if required(vulnerabilities, "count")?.as_f64() != Some(list.len() as f64) || required(vulnerabilities, "found")?.as_bool() != Some(!list.is_empty()) {
        return Err("cargo-audit JSON: inconsistent vulnerabilities count/found/list".into());
    }
    let Value::Obj(warnings) = required(report, "warnings")? else { return Err("cargo-audit JSON: warnings must be an object".into()) };
    let mut findings = Vec::new();
    for v in list {
        let a = required(v, "advisory")?;
        let id = text(a, "id")?;
        let title = text(a, "title")?;
        let pkg = package(v)?;
        findings.push(
            Finding::new(format!("rustsec::{id}"), Category::Safety, Severity::High, Confidence::Measured)
                .msg(format!("Dependency advisory match for {pkg}: {title}; reachability unverified, exploitability unverified"))
                .evidence(format!(
                    "Imported cargo-audit: {id}; {pkg}; database={}, settings={}",
                    report.get("database").map_or_else(|| "unknown".into(), Value::to_json),
                    report.get("settings").map_or_else(|| "unknown".into(), Value::to_json)
                )),
        );
    }
    for (kind, entries) in warnings {
        let Value::Arr(entries) = entries else { return Err(format!("cargo-audit JSON: warnings.{kind} must be an array")) };
        for entry in entries {
            let pkg = package(entry)?;
            let source = match entry.get("advisory") {
                None | Some(Value::Null) => String::new(),
                Some(advisory) => format!("; {}: {}", text(advisory, "id")?, text(advisory, "title")?),
            };
            findings.push(
                Finding::new(format!("dependency_warning::{kind}"), Category::Safety, Severity::Medium, Confidence::Measured)
                    .msg(format!("Dependency warning ({kind}) for {pkg}{source}; not a confirmed vulnerability"))
                    .evidence(format!("Imported cargo-audit warning: {pkg}; {kind}{source}")),
            );
        }
    }
    let detail = format!(
        "imported {} advisory match(es), {} warning(s); no cargo-audit execution; freshness, lockfile identity, reachability and settings completeness unverified",
        list.len(),
        findings.len() - list.len()
    );
    Ok((findings, AnalyzerRun::ran("cargo-audit-import", detail)))
}
