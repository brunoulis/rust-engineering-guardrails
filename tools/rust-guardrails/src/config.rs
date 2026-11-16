//! Project profiles with a deliberately small, strict TOML subset.
//! Single-line strings, string arrays, booleans and positive integers only;
//! unsupported syntax/keys fail closed. No new runtime dependency in the core.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{Cmd, Opts};
use crate::json::{obj, Value};
use crate::model::{Finding, Profile, Severity};

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    path: PathBuf,
    values: BTreeMap<String, Value>,
    exceptions: Vec<Exception>,
}

#[derive(Debug, Clone)]
struct Exception {
    rule: String,
    path: String,
    max_severity: Severity,
    reason: String,
    owner: String,
    expires: String,
    expiry_day: i64,
}

pub fn apply(o: &mut Opts) -> Result<(), String> {
    if !matches!(o.cmd, Cmd::Report | Cmd::Audit | Cmd::Triage) {
        if o.config.is_some() {
            return Err("--config is supported by report, audit and triage only".into());
        }
        return Ok(());
    }
    let path = o.config.clone().unwrap_or_else(|| o.root.join(".rust-rails.toml"));
    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && o.config.is_none() => return Ok(()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let c = parse(&source, path)?;
    let explicit = o.explicit.clone();
    let absent = |flag: &str| !explicit.iter().any(|f| f == flag);
    if absent("--profile") {
        if let Some(v) = c.values.get("profile") {
            o.profile = Profile::parse(v.as_str().unwrap()).unwrap();
        }
    }
    for (key, flag, dest) in [("scan.hot", "--hot", &mut o.hot), ("scan.exclude", "--exclude", &mut o.exclude)] {
        if absent(flag) {
            if let Some(v) = c.values.get(key) {
                *dest = v.as_array().iter().map(|v| v.as_str().unwrap().to_string()).collect();
            }
        }
    }
    if absent("--include-tests") {
        if let Some(v) = c.values.get("scan.include_tests") {
            o.include_tests = v.as_bool().unwrap();
        }
    }
    if absent("--include-ignored") {
        if let Some(v) = c.values.get("scan.include_ignored") {
            o.include_ignored = v.as_bool().unwrap();
        }
    }
    for (key, flag, dest) in [
        ("scan.max_file_lines", "--max-file-lines", &mut o.max_file_lines),
        ("scan.max_fn_lines", "--max-fn-lines", &mut o.max_fn_lines),
        ("scan.max_fields", "--max-fields", &mut o.max_fields),
        ("scan.max_methods", "--max-methods", &mut o.max_methods),
    ] {
        if absent(flag) {
            if let Some(v) = c.values.get(key) {
                *dest = v.as_f64().unwrap() as usize;
            }
        }
    }
    if absent("--fail-on") {
        if let Some(v) = c.values.get("ci.fail_on") {
            o.fail_on = Severity::parse(v.as_str().unwrap());
        }
    }
    o.project_config = Some(c);
    Ok(())
}

pub fn review_context(o: &Opts) -> Option<Value> {
    let c = o.project_config.as_ref()?;
    let contracts = c.values.iter().filter_map(|(k, v)| k.strip_prefix("contracts.").map(|key| (key.to_string(), v.clone())));
    let exceptions = c
        .exceptions
        .iter()
        .map(|e| {
            obj([
                ("rule", e.rule.as_str().into()),
                ("path", e.path.as_str().into()),
                ("max_severity", e.max_severity.as_str().into()),
                ("reason", e.reason.as_str().into()),
                ("owner", e.owner.as_str().into()),
                ("expires", e.expires.as_str().into()),
                ("active", (e.expiry_day >= today()).into()),
            ])
        })
        .collect();
    Some(obj([
        ("path", c.path.to_string_lossy().into_owned().into()),
        ("contract_status", "review inputs; not automatically enforced".into()),
        ("contracts", obj(contracts)),
        ("exceptions", Value::Arr(exceptions)),
    ]))
}

/// Exact rule+relative-path exceptions retain findings in reports. Escalation past
/// the reviewed severity and expired waivers never exempt a finding. Blockers
/// cannot be exempted by this configuration.
pub fn exception_indices(o: &Opts, findings: &[Finding]) -> Vec<Option<String>> {
    let now = today();
    findings
        .iter()
        .map(|f| {
            let c = o.project_config.as_ref()?;
            if f.severity == Severity::Blocker {
                return None;
            }
            c.exceptions
                .iter()
                .find(|e| e.rule == f.rule && e.path == f.file && f.severity <= e.max_severity && e.expiry_day >= now)
                .map(|e| format!("{} (owner: {}; expires: {}; max severity: {})", e.reason, e.owner, e.expires, e.max_severity.as_str()))
        })
        .collect()
}

fn today() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| (d.as_secs() / 86_400) as i64).unwrap_or(i64::MAX)
}

fn parse(source: &str, path: PathBuf) -> Result<ProjectConfig, String> {
    let mut values = BTreeMap::new();
    let mut raw_exceptions: Vec<BTreeMap<String, Value>> = vec![];
    let mut section = String::new();
    let mut tables = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line = uncomment(line)?.trim();
        if line.is_empty() {
            continue;
        }
        let err = |s: &str| format!("{}:{}: {s} (supported TOML subset: single-line strings, string arrays, booleans and integers)", path.display(), index + 1);
        if line == "[[exceptions]]" {
            section = "exceptions".into();
            raw_exceptions.push(BTreeMap::new());
            continue;
        }
        if line.starts_with('[') {
            let name = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')).ok_or_else(|| err("invalid table"))?.trim();
            if !["scan", "ci", "contracts"].contains(&name) {
                return Err(err("unknown table"));
            }
            if !tables.insert(name.to_string()) {
                return Err(err("duplicate table"));
            }
            section = name.into();
            continue;
        }
        let (key, raw) = line.split_once('=').ok_or_else(|| err("expected key = value"))?;
        let key = key.trim();
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(err("unsupported key syntax"));
        }
        let value = parse_value(raw.trim()).map_err(|e| err(&e))?;
        if section == "exceptions" {
            let e = raw_exceptions.last_mut().unwrap();
            if e.insert(key.into(), value).is_some() {
                return Err(err("duplicate exception key"));
            }
        } else {
            let full = if section.is_empty() { key.into() } else { format!("{section}.{key}") };
            validate_value(&full, &value).map_err(|e| err(&e))?;
            if values.insert(full, value).is_some() {
                return Err(err("duplicate key"));
            }
        }
    }
    if values.get("schema_version").and_then(Value::as_f64) != Some(1.0) {
        return Err(format!("{}: schema_version = 1 is required", path.display()));
    }
    let exceptions = raw_exceptions.into_iter().map(parse_exception).collect::<Result<Vec<_>, _>>().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(ProjectConfig { path, values, exceptions })
}

fn validate_value(key: &str, v: &Value) -> Result<(), String> {
    let string = || v.as_str().filter(|s| !s.trim().is_empty()).ok_or_else(|| format!("{key} requires a nonempty string"));
    match key {
        "schema_version" if v.as_f64() == Some(1.0) => Ok(()),
        "profile" => Profile::parse(string()?).map(|_| ()).ok_or("invalid profile".into()),
        "ci.fail_on" => Severity::parse(string()?).map(|_| ()).ok_or("invalid ci.fail_on severity".into()),
        "scan.hot" | "scan.exclude" | "contracts.critical_paths" => {
            if matches!(v, Value::Arr(a) if a.iter().all(|x| x.as_str().is_some_and(|s| !s.trim().is_empty()))) {
                Ok(())
            } else {
                Err(format!("{key} requires a string array"))
            }
        }
        "scan.include_tests" | "scan.include_ignored" if v.as_bool().is_some() => Ok(()),
        "scan.max_file_lines" | "scan.max_fn_lines" | "scan.max_fields" | "scan.max_methods" => {
            if v.as_f64().is_some_and(|n| n > 0.0 && n.fract() == 0.0 && n <= u32::MAX as f64) {
                Ok(())
            } else {
                Err(format!("{key} requires a positive integer <= {}", u32::MAX))
            }
        }
        "contracts.edition" => {
            if ["2015", "2018", "2021", "2024"].contains(&string()?) {
                Ok(())
            } else {
                Err("unsupported edition".into())
            }
        }
        "contracts.msrv" => {
            let s = string()?;
            let parts: Vec<_> = s.split('.').collect();
            if (2..=3).contains(&parts.len()) && parts[0] == "1" && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) {
                Ok(())
            } else {
                Err("msrv requires a Rust version such as '1.85' or '1.85.0'".into())
            }
        }
        "contracts.unsafe_policy" | "contracts.allocation_budget" | "contracts.determinism" => string().map(|_| ()),
        _ => Err(format!("unknown or invalid profile key: {key}")),
    }
}

fn parse_exception(raw: BTreeMap<String, Value>) -> Result<Exception, String> {
    let get = |key: &str| raw.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string).ok_or_else(|| format!("exception requires nonempty {key}"));
    if raw.keys().any(|key| !["rule", "path", "max_severity", "reason", "owner", "expires"].contains(&key.as_str())) {
        return Err("unknown exception key".into());
    }
    let max_severity =
        Severity::parse(&get("max_severity")?).filter(|s| *s != Severity::Blocker).ok_or("exception max_severity must be info, medium or high; blockers cannot be waived")?;
    let rule = get("rule")?;
    if rule.contains(['*', '?']) {
        return Err("exception rule must be exact".into());
    }
    let path = get("path")?.replace('\\', "/");
    if path.starts_with('/') || path.contains(':') || path.split('/').any(|p| p.is_empty() || p == ".." || p == "." || p.contains(['*', '?'])) {
        return Err("exception path must be an exact relative file path without glob or traversal".into());
    }
    let expires = get("expires")?;
    let expiry_day = date_day(&expires)?;
    Ok(Exception { rule, path, max_severity, reason: get("reason")?, owner: get("owner")?, expires, expiry_day })
}

fn date_day(s: &str) -> Result<i64, String> {
    if s.len() != 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' || !s.bytes().enumerate().all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit()) {
        return Err("expires must be a quoted YYYY-MM-DD date".into());
    }
    let year: i64 = s[..4].parse().unwrap();
    let month: i64 = s[5..7].parse().unwrap();
    let day: i64 = s[8..].parse().unwrap();
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    if year == 0 || day < 1 || day > max_day {
        return Err("invalid expires date".into());
    }
    // Proleptic Gregorian days relative to 1970-01-01.
    let y = year - i64::from(month <= 2);
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    Ok(era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468)
}

fn uncomment(s: &str) -> Result<&str, String> {
    let mut quote = None;
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && ch == '\\' {
            escaped = true;
            continue;
        }
        if Some(ch) == quote {
            quote = None;
        } else if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if quote.is_none() && ch == '#' {
            return Ok(&s[..i]);
        }
    }
    if quote.is_some() {
        return Err("unterminated string in .rust-rails.toml".into());
    }
    Ok(s)
}

fn parse_value(s: &str) -> Result<Value, String> {
    if let Some(inner) = s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        if inner.contains('\'') || inner.chars().any(|c| c.is_control()) {
            return Err("invalid single-line literal string".into());
        }
        return Ok(inner.into());
    }
    if s.starts_with('"') {
        let inner = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).ok_or("unterminated basic string")?;
        let mut chars = inner.chars();
        let mut value = String::new();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                let esc = chars.next().ok_or("unterminated escape")?;
                value.push(match esc {
                    'b' => '\u{8}',
                    't' => '\t',
                    'n' => '\n',
                    'f' => '\u{c}',
                    'r' => '\r',
                    '"' => '"',
                    '\\' => '\\',
                    'u' | 'U' => {
                        let mut cp = 0;
                        for _ in 0..if esc == 'u' { 4 } else { 8 } {
                            let digit = chars.next().and_then(|c| c.to_digit(16)).ok_or("invalid Unicode escape")?;
                            cp = cp * 16 + digit;
                        }
                        char::from_u32(cp).ok_or("Unicode escape is not a scalar value")?
                    }
                    _ => return Err("unsupported basic-string escape".into()),
                });
            } else if ch == '"' || ch.is_control() {
                return Err("invalid single-line basic string".into());
            } else {
                value.push(ch);
            }
        }
        return Ok(value.into());
    }
    if let Some(inner) = s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let mut values = vec![];
        let mut start = 0;
        let mut quote = None;
        let mut escaped = false;
        for (i, ch) in inner.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if quote == Some('"') && ch == '\\' {
                escaped = true;
                continue;
            }
            if Some(ch) == quote {
                quote = None;
            } else if quote.is_none() && matches!(ch, '\'' | '"') {
                quote = Some(ch);
            } else if quote.is_none() && ch == ',' {
                let item = parse_array_string(inner[start..i].trim())?;
                values.push(item);
                start = i + 1;
            }
        }
        if !inner[start..].trim().is_empty() {
            let item = parse_array_string(inner[start..].trim())?;
            values.push(item);
        }
        return Ok(Value::Arr(values));
    }
    match s {
        "true" => Ok(true.into()),
        "false" => Ok(false.into()),
        _ if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s.len() == 1 || !s.starts_with('0')) => {
            s.parse::<u32>().map(|n| Value::from(n as usize)).map_err(|_| "integer out of range".into())
        }
        _ => Err("unsupported value syntax".into()),
    }
}

fn parse_array_string(s: &str) -> Result<Value, String> {
    if !s.starts_with(['\'', '"']) {
        return Err("arrays must contain strings".into());
    }
    parse_value(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_are_validated_and_compared_in_utc_days() {
        assert_eq!(date_day("1970-01-01").unwrap(), 0);
        assert_eq!(date_day("2000-02-29").unwrap(), 11_016);
        assert!(date_day("2100-02-29").is_err());
        assert!(date_day("2026-13-01").is_err());
        assert!(date_day("2026-11-31").is_err());
        assert!(date_day("2026-11-01").unwrap() < date_day("2026-11-16").unwrap());
    }

    #[test]
    fn strings_follow_toml_escapes_without_accepting_json_only_syntax() {
        assert_eq!(parse_value(r#""C:\\src\t\u00e9""#).unwrap().as_str(), Some("C:\\src\té"));
        assert!(parse_value(r#""a\/b""#).is_err());
        assert!(parse_value(r#""\ud800\udc00""#).is_err());
    }
}
