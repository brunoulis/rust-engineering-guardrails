//! rust-guardrails: read-only Rust auditor.
//!
//! Layers (each optional beyond the first):
//! 1. source scan — syn AST engine (feature `ast`, default) or zero-dependency heuristic;
//! 2. `cargo metadata` — workspace/dependency/feature analysis (read-only, `--frozen`);
//! 3. `cargo clippy` — type-aware diagnostics (opt-in: builds the project);
//! 4. `cargo llvm-lines` — monomorphization/compile-time hotspots (opt-in);
//! 5. benchmark comparison — criterion or hyperfine results.
//!
//! `report` merges everything into one document with a prioritized, verifiable
//! refactoring plan. Nothing edits the audited code.

mod analyzers;
mod baseline;
mod cli;
mod config;
mod discovery;
mod glob;
mod json;
mod legacy;
mod model;
mod report;
mod rules;
mod sarif;
mod scan;
mod scope;
mod security;
mod verification;

use std::fs;
use std::process;

use analyzers::{bench, clippy, llvm_lines, metadata};
use cli::{Cmd, Opts};
use model::{AnalyzerRun, Severity};

/// syn's parser and the visitor recurse per nesting level; generated code (very long
/// operator chains, deeply nested expressions) overflows the default 1–8 MiB main stack.
/// The memory is reserved virtually, not committed.
const STACK_BYTES: usize = 512 << 20;

fn main() {
    let worker = std::thread::Builder::new().name("rust-guardrails".into()).stack_size(STACK_BYTES).spawn(run);
    let result = match worker {
        Ok(handle) => handle.join().unwrap_or_else(|_| Err("internal error (panic); please report it with the input that caused it".into())),
        Err(e) => Err(format!("cannot start worker thread: {e}")),
    };
    match result {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(2);
        }
    }
}

fn run() -> Result<i32, String> {
    let o = cli::parse(std::env::args().skip(1))?;
    match o.cmd {
        Cmd::Help => println!("{}", cli::USAGE),
        Cmd::Version => println!("rust-guardrails {} (ast engine: {})", env!("CARGO_PKG_VERSION"), if scan::ast_available() { "yes" } else { "no" }),
        Cmd::Explain => explain(o.rule.as_deref())?,
        Cmd::Topology => topology(&o)?,
        Cmd::Audit | Cmd::Triage => {
            let r = scan::run(&scan_config(&o)?)?;
            let out = if o.cmd == Cmd::Audit { legacy::audit(&o, &r) } else { legacy::triage(&o, &r) };
            emit(&o, &out)?;
        }
        Cmd::BenchCompare => return bench_compare(&o),
        Cmd::Report => return full_report(&o),
    }
    Ok(0)
}

fn scan_config(o: &Opts) -> Result<scan::ScanConfig, String> {
    if !o.root.is_dir() {
        return Err(format!("root must be a directory: {}", o.root.display()));
    }
    Ok(scan::ScanConfig {
        root: o.root.clone(),
        profile: o.profile,
        hot: o.hot.clone(),
        exclude: o.exclude.clone(),
        max_file_lines: o.max_file_lines,
        max_fn_lines: o.max_fn_lines,
        max_fields: o.max_fields,
        max_methods: o.max_methods,
        engine: o.engine,
        include_tests: o.include_tests,
        include_ignored: o.include_ignored,
    })
}

fn emit(o: &Opts, text: &str) -> Result<(), String> {
    match &o.output {
        Some(p) => {
            fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))?;
            eprintln!("wrote {}", p.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}

fn thresholds(o: &Opts) -> bench::Thresholds {
    bench::Thresholds { noise_pct: o.noise, max_regression_pct: o.max_regression }
}

fn load_bench(o: &Opts) -> Option<Result<bench::BenchResult, String>> {
    if let (Some(dir), Some(base)) = (&o.criterion, &o.baseline) {
        Some(bench::criterion(dir, base, &o.candidate, &thresholds(o)))
    } else {
        o.hyperfine.as_ref().map(|(b, c)| bench::hyperfine(b, c, &thresholds(o)))
    }
}

fn full_report(o: &Opts) -> Result<i32, String> {
    let debt_path = o.debt_baseline.clone().or_else(|| if o.criterion.is_none() { o.baseline.as_ref().map(std::path::PathBuf::from) } else { None });
    // Read comparison input before any explicit writes; refreshing a snapshot must
    // never change the exit status for the scan that generated it.
    let debt = debt_path.as_deref().map(baseline::Snapshot::read).transpose()?;
    // Requested benchmark input that cannot be read or matched is an error, not a skipped
    // section: a gate must not pass because of a typo in a baseline name.
    let bench_input = load_bench(o).transpose()?;
    let scan_res = scan::run(&scan_config(o)?)?;
    let mut detail = format!("{} engine, {} files", scan_res.engine, scan_res.files_scanned);
    detail.push_str(&format!("; {}", scan_res.scope_note));
    if scan_res.test_files_skipped > 0 {
        detail.push_str(&format!("; {} test file(s) skipped (--include-tests to scan)", scan_res.test_files_skipped));
    }
    if !scan_res.dirs_skipped.is_empty() {
        let shown: Vec<&str> = scan_res.dirs_skipped.iter().take(5).map(String::as_str).collect();
        detail.push_str(&format!("; skipped dirs: {}{}", shown.join(", "), if scan_res.dirs_skipped.len() > 5 { ", …" } else { "" }));
    }
    if o.hot.is_empty() {
        detail.push_str("; no --hot globs, so per-iteration clone/allocation candidates stay at info");
    }
    let mut analyzers = vec![AnalyzerRun::ran("source", detail)];
    let mut findings = scan_res.findings;
    let mut extend = |(f, run): (Vec<model::Finding>, AnalyzerRun)| {
        findings.extend(f);
        analyzers.push(run);
    };
    let projects = if o.metadata || o.clippy || (o.llvm_lines && o.llvm_lines_file.is_none()) { discovery::projects(&o.root, &scan_res.cargo_manifests)? } else { vec![] };
    let no_manifest = || "no Cargo.toml at root or in selected scope".to_string();

    if !o.metadata {
        extend((vec![], AnalyzerRun::skipped("metadata", "--no-metadata")));
    } else if projects.is_empty() {
        extend((vec![], AnalyzerRun::skipped("metadata", no_manifest())));
    } else {
        for project in &projects {
            let (f, run, base) =
                if let Some(reason) = &project.scope_error { (vec![], AnalyzerRun::failed("metadata", reason), project.root.clone()) } else { metadata::analyze(&project.root) };
            extend(project.rebase(&o.root, &base, (f, run)));
        }
    }

    if !o.clippy {
        extend((vec![], AnalyzerRun::skipped("clippy", "not requested (--with clippy); builds the project")));
    } else if projects.is_empty() {
        extend((vec![], AnalyzerRun::failed("clippy", no_manifest())));
    } else {
        for project in &projects {
            let result = if let Some(reason) = &project.scope_error {
                (vec![], AnalyzerRun::failed("clippy", reason))
            } else {
                eprintln!("running cargo clippy for {} (builds the project)…", project.manifest);
                clippy::analyze(&project.root, &o.clippy_args)
            };
            extend(project.rebase(&o.root, &project.root, result));
        }
    }

    if !o.llvm_lines {
        extend((vec![], AnalyzerRun::skipped("llvm-lines", "not requested (--with llvm-lines)")));
    } else if let Some(file) = &o.llvm_lines_file {
        extend(llvm_lines::analyze(&o.root, llvm_lines::Source::File(file), o.llvm_top));
    } else if projects.is_empty() {
        extend((vec![], AnalyzerRun::failed("llvm-lines", no_manifest())));
    } else {
        for project in &projects {
            let result = if let Some(reason) = &project.scope_error {
                (vec![], AnalyzerRun::failed("llvm-lines", reason))
            } else {
                eprintln!("running cargo llvm-lines --release for {} (builds the project)…", project.manifest);
                llvm_lines::analyze(&project.root, llvm_lines::Source::Run { package: o.llvm_lines_package.as_deref() }, o.llvm_top)
            };
            extend(project.rebase(&o.root, &project.root, result));
        }
    }

    let bench_res = match bench_input {
        None => {
            extend((vec![], AnalyzerRun::skipped("benchmarks", "no --criterion/--hyperfine input")));
            None
        }
        Some(b) => {
            let detail = format!("{}: {} compared, {} unmatched", b.source, b.comparisons.len(), b.unmatched.len());
            extend((bench::findings(&b), AnalyzerRun::ran("benchmarks", detail)));
            Some(b)
        }
    };

    if let Some(path) = &o.audit_json {
        let (security_findings, run) = security::analyze(path)?;
        findings.extend(security_findings);
        analyzers.push(run);
    } else {
        analyzers.push(AnalyzerRun::skipped("cargo-audit", "no saved --audit-json input; supply-chain status is unknown"));
    }
    report::corroborate(&mut findings);
    let comparison = debt.map(|d| d.compare(&findings));
    let exceptions = config::exception_indices(o, &findings);
    let root = o.root.display().to_string();
    let (plan, plan_omitted) = report::build_plan(&findings, &root, o.plan_limit);
    let rep = report::Report {
        root,
        profile: o.profile,
        engine: scan_res.engine,
        files_scanned: scan_res.files_scanned,
        lines_scanned: scan_res.lines_scanned,
        analyzers,
        findings,
        baseline: comparison,
        exceptions,
        project_profile: config::review_context(o),
        bench: bench_res,
        plan,
        plan_omitted,
    };
    let out = match o.format() {
        "sarif" => sarif::render(&rep),
        "json" => rep.to_json() + "\n",
        "markdown" | "md" => rep.to_markdown(o.limit),
        _ => rep.to_text(o.limit),
    };
    emit(o, &out)?;
    // With a gate, a requested analyzer that failed counts as a failure: the gate cannot
    // pass on checks that did not run.
    let analyzer_failed = rep.analyzers.iter().any(|a| matches!(a.status, model::AnalyzerStatus::Failed { .. }));
    if let Some(path) = &o.write_baseline {
        if analyzer_failed {
            return Err("cannot write a debt baseline while an analyzer failed".into());
        }
        baseline::Snapshot::write(path, &rep.findings)?;
    }
    if o.fail_on.is_some() && analyzer_failed {
        eprintln!("--fail-on: an analyzer failed; see the report");
        return Ok(1);
    }
    Ok(fail_code(o.fail_on, rep.max_severity()))
}

fn fail_code(fail_on: Option<Severity>, max: Option<Severity>) -> i32 {
    match (fail_on, max) {
        (Some(t), Some(m)) if m >= t => 1,
        _ => 0,
    }
}

fn bench_compare(o: &Opts) -> Result<i32, String> {
    let b = load_bench(o).ok_or("no benchmark input")??;
    let out = if o.format() == "json" {
        let findings = bench::findings(&b);
        let rows: Vec<json::Value> = b
            .comparisons
            .iter()
            .zip(&findings)
            .map(|(c, f)| {
                json::obj([
                    ("name", c.name.as_str().into()),
                    ("verdict", c.verdict.as_str().into()),
                    ("change_pct", ((c.change_pct * 100.0).round() / 100.0).into()),
                    ("over_budget", c.over_budget.into()),
                    ("severity", f.severity.as_str().into()),
                    ("detail", bench::describe(c).into()),
                ])
            })
            .collect();
        json::obj([
            ("schema_version", 2usize.into()),
            ("source", b.source.as_str().into()),
            ("noise_pct", o.noise.into()),
            ("max_regression_pct", o.max_regression.map_or(json::Value::Null, json::Value::from)),
            ("comparisons", json::Value::Arr(rows)),
            ("unmatched", json::Value::Arr(b.unmatched.iter().map(|u| u.as_str().into()).collect())),
        ])
        .to_json()
            + "\n"
    } else {
        let mut s = format!("Benchmark comparison: {} (noise {}%{})\n", b.source, o.noise, o.max_regression.map(|m| format!(", budget {m}%")).unwrap_or_default());
        for c in &b.comparisons {
            s.push_str(&format!("  {:<12}{} {}  {}\n", c.verdict.as_str(), if c.over_budget { " OVER-BUDGET" } else { "" }, c.name, bench::describe(c)));
        }
        for u in &b.unmatched {
            s.push_str(&format!("  unmatched    {u}\n"));
        }
        s
    };
    emit(o, &out)?;
    Ok(i32::from(b.comparisons.iter().any(|c| c.over_budget)))
}

fn topology(o: &Opts) -> Result<(), String> {
    let output = analyzers::cargo(&o.root).args(["metadata", "--no-deps", "--format-version", "1"]).output().map_err(|e| format!("cargo metadata unavailable: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&output.stderr)));
    }
    eprintln!("NOTE: raw Cargo metadata, not an architectural recommendation. `report` analyzes it.");
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn explain(rule: Option<&str>) -> Result<(), String> {
    let Some(id) = rule else {
        println!("Rules (id — category/default severity — title):");
        for r in rules::RULES {
            println!("  {:<32} {:<12} {:<7} {}", r.id, r.category.as_str(), r.severity.as_str(), r.title);
        }
        println!("\nClippy/rustc diagnostics keep their own codes (e.g. clippy::redundant_clone).");
        return Ok(());
    };
    let r = rules::get(id).ok_or_else(|| format!("unknown rule: {id} (run `rust-guardrails explain` for the list)"))?;
    println!("{} — {}\n", r.id, r.title);
    println!("category: {}   default severity: {}   characterize first: {}", r.category.as_str(), r.severity.as_str(), r.characterize_first);
    println!("\nwhy:        {}\nkeep:       {}\nminimal:    {}\nstructural: {}\n\nverify:", r.why, r.keep, r.minimal, r.structural);
    for v in r.verify {
        println!("  - {v}");
    }
    println!("\nreference:  {}", r.reference);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fail_code_thresholds() {
        assert_eq!(fail_code(None, Some(Severity::Blocker)), 0);
        assert_eq!(fail_code(Some(Severity::High), Some(Severity::Medium)), 0);
        assert_eq!(fail_code(Some(Severity::High), Some(Severity::Blocker)), 1);
        assert_eq!(fail_code(Some(Severity::Info), None), 0);
    }
}
