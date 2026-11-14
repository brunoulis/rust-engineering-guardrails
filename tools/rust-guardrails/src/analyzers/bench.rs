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
    pub confidence_level: f64,
    pub method: &'static str,
}

impl Estimate {
    fn valid(self) -> bool {
        [self.mean, self.lo, self.hi, self.confidence_level].iter().all(|v| v.is_finite())
            && self.mean > 0.0
            && self.lo >= 0.0
            && self.lo <= self.mean
            && self.mean <= self.hi
            && self.lo < self.hi
            && self.confidence_level > 0.0
            && self.confidence_level < 1.0
    }
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
            out.insert(name, criterion_estimate(&v).ok_or_else(|| format!("{}: missing or invalid mean/confidence interval", est.display()))?);
        } else {
            walk(root, &p, which, out)?;
        }
    }
    Ok(())
}

pub fn criterion_estimate(v: &Value) -> Option<Estimate> {
    let m = v.get("mean")?;
    let mean = m.get("point_estimate")?.as_f64()?;
    let ci = m.get("confidence_interval")?;
    let lo = ci.get("lower_bound")?.as_f64()?;
    let hi = ci.get("upper_bound")?.as_f64()?;
    let confidence_level = ci.get("confidence_level")?.as_f64()?;
    let estimate = Estimate { mean, lo, hi, confidence_level, method: "Criterion supplied interval" };
    estimate.valid().then_some(estimate)
}

fn hyperfine_estimates(file: &Path) -> Result<BTreeMap<String, Estimate>, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let v = json::parse(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut out = BTreeMap::new();
    for r in v.get("results").map(Value::as_array).unwrap_or_default() {
        let name = r.str_of("command").to_string();
        let mean = r.get("mean").and_then(Value::as_f64).ok_or_else(|| format!("{}: result without mean", file.display()))?;
        let invalid = || {
            format!(
                "{}: `{name}` requires a positive finite mean/stddev and at least two finite nonnegative times; zero-width or invalid intervals are unsupported",
                file.display()
            )
        };
        let sd = r.get("stddev").and_then(Value::as_f64).ok_or_else(invalid)?;
        let times = r.get("times").map(Value::as_array).unwrap_or_default();
        if !sd.is_finite() || sd <= 0.0 || times.len() < 2 || times.iter().any(|t| !t.as_f64().is_some_and(|v| v.is_finite() && v >= 0.0)) {
            return Err(invalid());
        }
        if let Some(exits) = r.get("exit_codes") {
            let exits = exits.as_array();
            if exits.len() != times.len() || exits.iter().any(|code| code.as_f64() != Some(0.0)) {
                return Err(format!("{}: `{name}` exit_codes must contain one successful numeric zero per timing sample", file.display()));
            }
        }
        let sample_mean = times.iter().map(|t| t.as_f64().unwrap() / times.len() as f64).sum::<f64>();
        let sample_sd = (times.iter().map(|t| (t.as_f64().unwrap() - sample_mean).powi(2)).sum::<f64>() / (times.len() - 1) as f64).sqrt();
        let agrees = |reported: f64, sampled: f64| sampled.is_finite() && sampled > 0.0 && (reported - sampled).abs() <= sampled * 1e-6;
        if !agrees(mean, sample_mean) || !agrees(sd, sample_sd) {
            return Err(format!("{}: `{name}` mean/stddev disagree with times (relative tolerance 1e-6), or sample variance is zero/invalid", file.display()));
        }
        // Small samples need Student-t, not a normal 1.96 approximation. Values
        // rounded upward at 0.001 (NIST t table, two-sided 95%). For df > 30,
        // t30 is a conservative upper bound, avoiding an abrupt narrowing at n=32.
        const T95: [f64; 30] = [
            12.707, 4.303, 3.183, 2.777, 2.571, 2.447, 2.365, 2.307, 2.263, 2.229, 2.201, 2.179, 2.161, 2.145, 2.132, 2.120, 2.110, 2.101, 2.094, 2.086, 2.080, 2.074, 2.069,
            2.064, 2.060, 2.056, 2.052, 2.049, 2.046, 2.043,
        ];
        let (critical, method) = if times.len() <= 31 {
            (T95[times.len() - 2], "Student-t; assumes independent approximately normal observations")
        } else {
            (T95[29], "conservative Student-t bound using df=30; assumes independent approximately normal observations")
        };
        let half = critical * sd / (times.len() as f64).sqrt();
        let estimate = Estimate { mean: mean * 1e9, lo: (mean - half) * 1e9, hi: (mean + half) * 1e9, confidence_level: 0.95, method };
        if !estimate.valid() {
            return Err(invalid());
        }
        out.insert(name, estimate);
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
        let verdict = if !b.valid() || !c.valid() || b.confidence_level != c.confidence_level || !change_pct.is_finite() || overlap || change_pct.abs() < th.noise_pct {
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
        "mean {} → {} ({:+.1}%); {}% CI [{}, {}] ({}) vs {}% CI [{}, {}] ({}); interval separation is a comparison heuristic, not proof of experiment comparability",
        fmt_ns(c.base.mean),
        fmt_ns(c.cand.mean),
        c.change_pct,
        c.base.confidence_level * 100.0,
        fmt_ns(c.base.lo),
        fmt_ns(c.base.hi),
        c.base.method,
        c.cand.confidence_level * 100.0,
        fmt_ns(c.cand.lo),
        fmt_ns(c.cand.hi),
        c.cand.method
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

    #[test]
    fn criterion_rejects_missing_or_invalid_uncertainty() {
        for text in [
            r#"{"mean":{"point_estimate":2}}"#,
            r#"{"mean":{"point_estimate":2,"confidence_interval":{"lower_bound":1,"upper_bound":3}}}"#,
            r#"{"mean":{"point_estimate":2,"confidence_interval":{"confidence_level":0.95,"lower_bound":3,"upper_bound":1}}}"#,
            r#"{"mean":{"point_estimate":2,"confidence_interval":{"confidence_level":1,"lower_bound":1,"upper_bound":3}}}"#,
            r#"{"mean":{"point_estimate":-2,"confidence_interval":{"confidence_level":0.95,"lower_bound":-3,"upper_bound":-1}}}"#,
            r#"{"mean":{"point_estimate":2,"confidence_interval":{"confidence_level":0.95,"lower_bound":2,"upper_bound":2}}}"#,
        ] {
            assert!(criterion_estimate(&json::parse(text).unwrap()).is_none(), "accepted {text}");
        }
    }

    #[test]
    fn criterion_preserves_non95_confidence_in_description() {
        let make = |mean, lo, hi| {
            criterion_estimate(
                &json::parse(&format!(r#"{{"mean":{{"point_estimate":{mean},"confidence_interval":{{"confidence_level":0.99,"lower_bound":{lo},"upper_bound":{hi}}}}}}}"#))
                    .unwrap(),
            )
            .unwrap()
        };
        let r = compare(
            &BTreeMap::from([("work".into(), make(20, 19, 21))]),
            &BTreeMap::from([("work".into(), make(10, 9, 11))]),
            &Thresholds { noise_pct: 2.0, max_regression_pct: None },
        );
        assert!(describe(&r.comparisons[0]).contains("99%"));
    }

    #[test]
    fn hyperfine_rejects_inadequate_samples_and_numeric_values() {
        let file = std::env::temp_dir().join(format!("rust-guardrails-invalid-bench-{}.json", std::process::id()));
        for result in [
            r#"{"command":"work","mean":2}"#,
            r#"{"command":"work","mean":2,"stddev":0.1}"#,
            r#"{"command":"work","mean":2,"times":[1.9,2.1]}"#,
            r#"{"command":"work","mean":2,"stddev":0.1,"times":[2]}"#,
            r#"{"command":"work","mean":2,"stddev":-1,"times":[1.9,2.1]}"#,
            r#"{"command":"work","mean":2,"stddev":0,"times":[2,2]}"#,
            r#"{"command":"work","mean":2,"stddev":0.1,"times":[2,"bad"]}"#,
            r#"{"command":"work","mean":0,"stddev":0.1,"times":[-1,1]}"#,
            r#"{"command":"work","mean":2,"stddev":0.1,"times":[2,2]}"#,
            r#"{"command":"work","mean":2,"stddev":0.1,"times":[20,21]}"#,
        ] {
            fs::write(&file, format!(r#"{{"results":[{result}]}}"#)).unwrap();
            assert!(hyperfine_estimates(&file).is_err(), "accepted {result}");
        }
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn different_confidence_levels_are_inconclusive() {
        let mut candidate = est(50.0, 1.0);
        candidate.confidence_level = 0.99;
        let r = compare(
            &BTreeMap::from([("work".into(), est(100.0, 1.0))]),
            &BTreeMap::from([("work".into(), candidate)]),
            &Thresholds { noise_pct: 2.0, max_regression_pct: Some(5.0) },
        );
        assert_eq!(r.comparisons[0].verdict, Verdict::Inconclusive);
        assert!(!r.comparisons[0].over_budget);
    }

    #[test]
    fn hyperfine_rejects_failed_or_invalid_exit_statuses() {
        let file = std::env::temp_dir().join(format!("rust-guardrails-exits-{}.json", std::process::id()));
        for exits in ["[1,1]", "[0,1]", "[0,null]", "[0]", "[0,0,0]", "null", "0", "[0,\"0\"]", "[0,false]"] {
            fs::write(&file, format!(r#"{{"results":[{{"command":"work","mean":1,"stddev":0.01414213562373095,"times":[0.99,1.01],"exit_codes":{exits}}}]}}"#)).unwrap();
            assert!(hyperfine_estimates(&file).is_err(), "accepted exit_codes {exits}");
        }
        fs::write(&file, r#"{"results":[{"command":"work","mean":1,"stddev":0.01414213562373095,"times":[0.99,1.01],"exit_codes":[0,0]}]}"#).unwrap();
        assert!(hyperfine_estimates(&file).is_ok());
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn hyperfine_small_samples_use_t_and_large_samples_keep_conservative_bound() {
        let file = std::env::temp_dir().join(format!("rust-guardrails-t-bench-{}.json", std::process::id()));
        for n in [2usize, 10, 31, 32, 100] {
            let times: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 / 100.0).collect();
            let mean = times.iter().sum::<f64>() / n as f64;
            let sd = (times.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt();
            fs::write(&file, format!(r#"{{"results":[{{"command":"work","mean":{mean},"stddev":{sd},"times":{times:?}}}]}}"#)).unwrap();
            let e = hyperfine_estimates(&file).unwrap()["work"];
            assert!((e.hi - e.mean) > 1.96 * sd / (n as f64).sqrt() * 1e9);
            assert!(e.method.contains("Student-t"));
            if n == 2 {
                assert!((e.hi - e.mean) > 12.7 * sd / (n as f64).sqrt() * 1e9);
            }
            if n > 31 {
                assert!(e.method.contains("conservative"));
            }
        }
        fs::remove_file(file).unwrap();
    }

    fn est(mean: f64, half: f64) -> Estimate {
        Estimate { mean, lo: mean - half, hi: mean + half, confidence_level: 0.95, method: "test interval" }
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
        assert_eq!(criterion_estimate(&v), Some(Estimate { mean: 10.0, lo: 9.5, hi: 10.5, confidence_level: 0.95, method: "Criterion supplied interval" }));
    }
}
