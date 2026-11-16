//! `cargo llvm-lines` integration: which (generic) functions dominate generated LLVM IR,
//! a proxy for monomorphization-driven compile time and binary size. Measured data,
//! but about compile time, not runtime speed.

use std::fs;
use std::path::Path;

use super::{cargo, first_line, run_timed};
use crate::model::{AnalyzerRun, Confidence, Finding, Severity};
use crate::rules;

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub lines: u64,
    pub lines_pct: f64,
    pub copies: u64,
    pub name: String,
}

pub fn available(root: &Path) -> bool {
    cargo(root).args(["llvm-lines", "--version"]).output().is_ok_and(|o| o.status.success())
}

pub enum Source<'a> {
    Run { package: Option<&'a str> },
    File(&'a Path),
}

pub fn analyze(root: &Path, src: Source, top: usize) -> (Vec<Finding>, AnalyzerRun) {
    let (text, detail) = match src {
        Source::File(p) => match fs::read_to_string(p) {
            Ok(t) => (t, format!("parsed {}", p.display())),
            Err(e) => return (vec![], AnalyzerRun::failed("llvm-lines", format!("{}: {e}", p.display()))),
        },
        Source::Run { package } => {
            if !available(root) {
                return (vec![], AnalyzerRun::failed("llvm-lines", "requested cargo-llvm-lines unavailable; install it separately if appropriate"));
            }
            let mut c = cargo(root);
            // --locked: never create or rewrite Cargo.lock in the audited project.
            c.args(["llvm-lines", "--release", "--locked"]);
            if let Some(p) = package {
                c.args(["-p", p]);
            }
            match run_timed(c) {
                Ok(t) if t.output.status.success() => (String::from_utf8_lossy(&t.output.stdout).into_owned(), format!("cargo llvm-lines --release in {:.1}s", t.secs)),
                Ok(t) => return (vec![], AnalyzerRun::failed("llvm-lines", first_line(&String::from_utf8_lossy(&t.output.stderr)))),
                Err(e) => return (vec![], AnalyzerRun::failed("llvm-lines", e.to_string())),
            }
        }
    };
    let rows = parse(&text);
    if rows.is_empty() {
        return (vec![], AnalyzerRun::failed("llvm-lines", format!("{detail}; no parseable rows; expected cargo llvm-lines output")));
    }
    let total = rows.iter().find(|r| r.name == "(TOTAL)").map_or(0, |r| r.lines);
    let mut findings = Vec::new();
    for r in rows.iter().filter(|r| r.name != "(TOTAL)" && r.lines_pct >= 1.0).take(top) {
        let mut f = rules::finding("llvm_lines_hotspot", Confidence::Measured)
            .msg(format!("`{}` generates {:.1}% of LLVM IR", r.name, r.lines_pct))
            .evidence(format!("{} lines, {} monomorphized copies (total {total} lines)", r.lines, r.copies));
        if r.lines_pct >= 5.0 {
            f.severity = Severity::Medium;
        }
        findings.push(f);
    }
    (findings, AnalyzerRun::ran("llvm-lines", format!("{detail}; {} rows, total {total} IR lines", rows.len())))
}

/// Parses the table printed by cargo-llvm-lines:
/// `  1395 (4.5%,  5.0%)     83 (7.5%)  core::ptr::drop_in_place<T>` (the cumulative
/// percentage is optional; function names may contain spaces).
pub fn parse(text: &str) -> Vec<Row> {
    text.lines().filter_map(parse_row).collect()
}

/// `N [(pct%[, cum%])] M [(pct%[, cum%])] name` — percentage groups are optional.
fn parse_row(line: &str) -> Option<Row> {
    let (lines, rest) = leading_number(line.trim())?;
    let (pct, rest) = optional_pct_group(rest);
    let (copies, rest) = leading_number(rest)?;
    let (_, rest) = optional_pct_group(rest);
    let name = rest.trim();
    (!name.is_empty()).then(|| Row { lines, lines_pct: pct.unwrap_or(0.0), copies, name: name.to_string() })
}

fn leading_number(s: &str) -> Option<(u64, &str)> {
    let s = s.trim_start();
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let n = s[..end].parse().ok()?;
    // A number must be followed by whitespace (rejects names that start with digits).
    s[end..].starts_with(char::is_whitespace).then_some((n, &s[end..]))
}

fn optional_pct_group(s: &str) -> (Option<f64>, &str) {
    let t = s.trim_start();
    if !t.starts_with('(') || t.starts_with("(TOTAL)") {
        return (None, s);
    }
    match t.find(')') {
        Some(close) => (t[1..close].split([',', '%']).next().and_then(|p| p.trim().parse().ok()), &t[close + 1..]),
        None => (None, s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_table_with_and_without_cumulative_column() {
        let text = "  Lines                 Copies               Function name\n  -----                 ------               -------------\n  30737                 1107                 (TOTAL)\n  30737 (100%)          1107 (100%)          (TOTAL)\n   1395 (4.5%,  4.5%)     83 (7.5%,  7.5%)  core::ptr::drop_in_place<T>\n    760 (2.5%)            2 (0.2%)          <alloc::vec::Vec<T> as core::ops::Drop>::drop\n";
        let rows = parse(text);
        assert_eq!(rows.len(), 4, "{rows:?}");
        assert_eq!(rows[0], Row { lines: 30737, lines_pct: 0.0, copies: 1107, name: "(TOTAL)".into() });
        let rows = &rows[1..];
        assert_eq!(rows[0].name, "(TOTAL)");
        assert_eq!(rows[1], Row { lines: 1395, lines_pct: 4.5, copies: 83, name: "core::ptr::drop_in_place<T>".into() });
        assert_eq!(rows[2].name, "<alloc::vec::Vec<T> as core::ops::Drop>::drop");
    }
}
