//! Benchmark comparison for validating optimizations. Reads Criterion output directories
//! (`target/criterion/<bench>/<baseline>/estimates.json`) or two hyperfine `--export-json`
//! files. A change counts only when confidence intervals do not overlap *and* it exceeds
//! the noise threshold; everything else is reported as inconclusive.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::json::{self, Value};
use crate::model::{Confidence, Finding, Severity};
use crate::rules;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// Mean time in nanoseconds.
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Improved,
    Regressed,
    Inconclusive,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Improved => "improved",
            Verdict::Regressed => "regressed",
            Verdict::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub name: String,
    pub base: Estimate,
    pub cand: Estimate,
    pub change_pct: f64,
    pub verdict: Verdict,
    pub over_budget: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BenchResult {
    pub source: String,
    pub comparisons: Vec<Comparison>,
    /// Benchmarks present on only one side.
    pub unmatched: Vec<String>,
}

pub struct Thresholds {
    pub noise_pct: f64,
    pub max_regression_pct: Option<f64>,
}

pub fn criterion(dir: &Path, baseline: &str, candidate: &str, th: &Thresholds) -> Result<BenchResult, String> {
    let base = criterion_estimates(dir, baseline)?;
    let cand = criterion_estimates(dir, candidate)?;
    if base.is_empty() {
        return Err(format!("no `{baseline}/estimates.json` found under {}", dir.display()));
    }
    let mut r = compare(&base, &cand, th);
    if r.comparisons.is_empty() {
        // Typically a misspelled --candidate: nothing compared must not look like "no regressions".
        return Err(format!("no benchmark has both `{baseline}` and `{candidate}` estimates under {}", dir.display()));
    }
    r.source = format!("criterion {} ({baseline} → {candidate})", dir.display());
    Ok(r)
}

pub fn hyperfine(base_file: &Path, cand_file: &Path, th: &Thresholds) -> Result<BenchResult, String> {
    let mut base = hyperfine_estimates(base_file)?;
    let mut cand = hyperfine_estimates(cand_file)?;
    // A single command on each side is paired even if the command lines differ.
    if base.len() == 1 && cand.len() == 1 && base.keys().ne(cand.keys()) {
        let (bk, bv) = base.pop_first().expect("len 1");
        let (ck, cv) = cand.pop_first().expect("len 1");
        let name = format!("{bk} → {ck}");
        base.insert(name.clone(), bv);
        cand.insert(name, cv);
    }
    let mut r = compare(&base, &cand, th);
    if r.comparisons.is_empty() {
        return Err(format!("no hyperfine command appears in both {} and {}", base_file.display(), cand_file.display()));
    }
    r.source = format!("hyperfine {} → {}", base_file.display(), cand_file.display());
    Ok(r)
}

fn criterion_estimates(dir: &Path, which: &str) -> Result<BTreeMap<String, Estimate>, String> {
    let mut out = BTreeMap::new();
    walk(dir, dir, which, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, here: &Path, which: &str, out: &mut BTreeMap<String, Estimate>) -> Result<(), String> {
    let entries = fs::read_dir(here).map_err(|e| format!("{}: {e}", here.display()))?;
    for e in entries.flatten() {
        let p = e.path();
        // Criterion's HTML report lives in `<root>/report` (and `<bench>/report`, which
        // has no estimates.json); a benchmark *group* may also be named "report".
        if !p.is_dir() || (here == root && e.file_name() == "report" && !p.join(which).is_dir()) {
            continue;
        }
        let est = p.join("estimates.json");
        if e.file_name() == which && est.is_file() {
            let text = fs::read_to_string(&est).map_err(|err| format!("{}: {err}", est.display()))?;
            let v = json::parse(&text).map_err(|err| format!("{}: {err}", est.display()))?;
            let name = here.strip_prefix(root).unwrap_or(here).to_string_lossy().replace('\\', "/");
            out.insert(name, criterion_estimate(&v).ok_or_else(|| format!("{}: missing mean estimate", est.display()))?);
        } else {
            walk(root, &p, which, out)?;
        }
    }
    Ok(())
}

pub fn criterion_estimate(v: &Value) -> Option<Estimate> {
    let m = v.get("mean")?;
    let mean = m.get("point_estimate")?.as_f64()?;
    let ci = m.get("confidence_interval");
    let lo = ci.and_then(|c| c.get("lower_bound")).and_then(Value::as_f64).unwrap_or(mean);
    let hi = ci.and_then(|c| c.get("upper_bound")).and_then(Value::as_f64).unwrap_or(mean);
    Some(Estimate { mean, lo, hi })
}

fn hyperfine_estimates(file: &Path) -> Result<BTreeMap<String, Estimate>, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let v = json::parse(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut out = BTreeMap::new();
    for r in v.get("results").map(Value::as_array).unwrap_or_default() {
        let name = r.str_of("command").to_string();
        let mean = r.get("mean").and_then(Value::as_f64).ok_or_else(|| format!("{}: result without mean", file.display()))?;
        let sd = r.get("stddev").and_then(Value::as_f64).unwrap_or(0.0);
        let n = r.get("times").map(|t| t.as_array().len()).unwrap_or(0).max(1) as f64;
        // 95% CI of the mean (normal approximation); seconds → ns.
        let half = if n >= 2.0 { 1.96 * sd / n.sqrt() } else { sd };
        out.insert(name, Estimate { mean: mean * 1e9, lo: (mean - half) * 1e9, hi: (mean + half) * 1e9 });
    }
    if out.is_empty() {
        return Err(format!("{}: no hyperfine results", file.display()));
    }
    Ok(out)
}

pub fn compare(base: &BTreeMap<String, Estimate>, cand: &BTreeMap<String, Estimate>, th: &Thresholds) -> BenchResult {
    let mut r = BenchResult::default();
    for (name, b) in base {
        let Some(c) = cand.get(name) else {
            r.unmatched.push(format!("{name} (baseline only)"));
            continue;
        };
        let change_pct = if b.mean > 0.0 { (c.mean - b.mean) / b.mean * 100.0 } else { 0.0 };
        let overlap = c.lo <= b.hi && b.lo <= c.hi;
        let verdict = if overlap || change_pct.abs() < th.noise_pct {
            Verdict::Inconclusive
        } else if change_pct > 0.0 {
            Verdict::Regressed
        } else {
            Verdict::Improved
        };
        let over_budget = verdict == Verdict::Regressed && th.max_regression_pct.is_some_and(|m| change_pct > m);
        r.comparisons.push(Comparison { name: name.clone(), base: *b, cand: *c, change_pct, verdict, over_budget });
    }
    r.unmatched.extend(cand.keys().filter(|k| !base.contains_key(*k)).map(|k| format!("{k} (candidate only)")));
    r
}

pub fn findings(r: &BenchResult) -> Vec<Finding> {
    r.comparisons
        .iter()
        .map(|c| {
            let rule = match c.verdict {
                Verdict::Regressed => "bench_regression",
                Verdict::Improved => "bench_improvement",
                Verdict::Inconclusive => "bench_inconclusive",
            };
            let mut f = rules::finding(rule, Confidence::Measured).msg(format!("{}: {} ({:+.1}%)", c.name, c.verdict.as_str(), c.change_pct)).evidence(describe(c));
            if c.over_budget {
                f.severity = Severity::Blocker;
                f.message.push_str(" — exceeds --max-regression budget");
            }
            f
        })
        .collect()
}

pub fn describe(c: &Comparison) -> String {
    format!(
        "mean {} → {} ({:+.1}%); 95% CI [{}, {}] vs [{}, {}]",
        fmt_ns(c.base.mean),
        fmt_ns(c.cand.mean),
        c.change_pct,
        fmt_ns(c.base.lo),
        fmt_ns(c.base.hi),
        fmt_ns(c.cand.lo),
        fmt_ns(c.cand.hi)
    )
}

pub fn fmt_ns(ns: f64) -> String {
    let a = ns.abs();
    if a >= 1e9 {
        format!("{:.3} s", ns / 1e9)
    } else if a >= 1e6 {
        format!("{:.3} ms", ns / 1e6)
    } else if a >= 1e3 {
        format!("{:.3} µs", ns / 1e3)
    } else {
        format!("{ns:.1} ns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(mean: f64, half: f64) -> Estimate {
        Estimate { mean, lo: mean - half, hi: mean + half }
    }

    #[test]
    fn classifies_changes() {
        let base =
            BTreeMap::from([("a".to_string(), est(100.0, 2.0)), ("b".to_string(), est(100.0, 2.0)), ("c".to_string(), est(100.0, 20.0)), ("gone".to_string(), est(1.0, 0.1))]);
        let cand = BTreeMap::from([("a".to_string(), est(80.0, 2.0)), ("b".to_string(), est(130.0, 2.0)), ("c".to_string(), est(110.0, 20.0)), ("new".to_string(), est(1.0, 0.1))]);
        let r = compare(&base, &cand, &Thresholds { noise_pct: 2.0, max_regression_pct: Some(10.0) });
        let v: Vec<_> = r.comparisons.iter().map(|c| (c.name.as_str(), c.verdict, c.over_budget)).collect();
        assert_eq!(v, vec![("a", Verdict::Improved, false), ("b", Verdict::Regressed, true), ("c", Verdict::Inconclusive, false)]);
        assert_eq!(r.unmatched, vec!["gone (baseline only)", "new (candidate only)"]);
        let f = findings(&r);
        assert_eq!(f[1].severity, Severity::Blocker);
    }

    #[test]
    fn small_non_overlapping_change_is_noise() {
        let base = BTreeMap::from([("a".to_string(), est(100.0, 0.1))]);
        let cand = BTreeMap::from([("a".to_string(), est(101.0, 0.1))]);
        let r = compare(&base, &cand, &Thresholds { noise_pct: 2.0, max_regression_pct: None });
        assert_eq!(r.comparisons[0].verdict, Verdict::Inconclusive);
    }

    #[test]
    fn reads_criterion_estimate() {
        let v =
            json::parse(r#"{"mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":9.5,"upper_bound":10.5},"point_estimate":10.0,"standard_error":0.2}}"#).unwrap();
        assert_eq!(criterion_estimate(&v), Some(Estimate { mean: 10.0, lo: 9.5, hi: 10.5 }));
    }
}
