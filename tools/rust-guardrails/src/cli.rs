//! Argument parsing (no clap: keeps the core dependency-free).

use std::path::PathBuf;

use crate::model::{Profile, Severity};
use crate::scan::EngineChoice;

pub const USAGE: &str = "\
rust-guardrails — read-only Rust auditor

USAGE:
  rust-guardrails report        [ROOT] [OPTIONS]   unified audit + prioritized refactoring plan
  rust-guardrails audit         [ROOT] [OPTIONS]   legacy candidate list (schema 1)
  rust-guardrails triage        [ROOT] [OPTIONS]   legacy per-file inspection queue
  rust-guardrails bench-compare [OPTIONS]          compare criterion/hyperfine results
  rust-guardrails topology      [ROOT]             raw `cargo metadata --no-deps` JSON
  rust-guardrails explain       [RULE]             playbook for a rule (no RULE: list rules)

SOURCE OPTIONS (report, audit, triage):
  --profile service|engine|library|ffi|prototype   (default service)
  --hot GLOB            mark matching files as known hot paths (repeatable)
  --exclude GLOB        skip matching files (repeatable)
  --include-tests       also scan test files and #[cfg(test)] items/statements
  --include-ignored     include Git-ignored source files and Cargo manifests
  --engine auto|ast|heuristic   (default auto: ast when compiled in)
  --max-file-lines N (1000)  --max-fn-lines N (80)  --max-fields N (15)  --max-methods N (30)
  --limit N             findings shown per section (default 30)
  --format FMT          report: text|markdown|json|sarif; audit/triage/bench-compare: text|json
  --config FILE         project profile (default ROOT/.rust-rails.toml)
  --write-baseline FILE write a debt snapshot (report only)
  --baseline FILE       compare debt in report without --criterion
  --debt-baseline FILE  compare debt, including alongside Criterion
  --audit-json FILE     import a saved cargo-audit JSON report
  --output FILE         write the report to FILE instead of stdout

ANALYZERS (report). Source scan and cargo metadata (--frozen, read-only) run by default:
  --with LIST           add analyzers: clippy,llvm-lines (comma-separated) or `all`
  --deep                same as --with all
  --no-metadata         skip cargo metadata
  --clippy-arg ARG      extra argument for cargo clippy before `--` (repeatable, e.g. --features=x)
  --llvm-lines-package P    package for cargo llvm-lines in a workspace
  --llvm-lines-file F   parse saved `cargo llvm-lines` output instead of running it
  --llvm-top N          top functions reported (default 10)
  --plan-limit N        refactoring plan items (default 20)
  --fail-on SEVERITY    exit 1 for non-exempt findings at info|medium|high|blocker
                       with a debt baseline, only new/escalated findings gate
                       failed requested analyzers always fail a configured gate

BENCHMARKS (report, bench-compare):
  --criterion DIR --baseline NAME [--candidate NAME (default new)]
  --hyperfine BASE.json CAND.json
  --noise PCT           changes below this are inconclusive (default 2)
  --max-regression PCT  a regression above this is a blocker (exit 1 in bench-compare)

`clippy` and `llvm-lines` BUILD the project (build scripts and proc-macros execute):
use them on trusted code only. Nothing ever edits the audited sources.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Report,
    Audit,
    Triage,
    BenchCompare,
    Topology,
    Explain,
    Help,
    Version,
}

#[derive(Debug, Clone)]
pub struct Opts {
    pub cmd: Cmd,
    pub config: Option<PathBuf>,
    pub project_config: Option<crate::config::ProjectConfig>,
    pub explicit: Vec<String>,
    pub write_baseline: Option<PathBuf>,
    pub debt_baseline: Option<PathBuf>,
    pub audit_json: Option<PathBuf>,
    pub root: PathBuf,
    pub rule: Option<String>,
    pub profile: Profile,
    pub hot: Vec<String>,
    pub exclude: Vec<String>,
    pub include_tests: bool,
    pub include_ignored: bool,
    pub engine: EngineChoice,
    pub max_file_lines: usize,
    pub max_fn_lines: usize,
    pub max_fields: usize,
    pub max_methods: usize,
    pub limit: usize,
    pub format: Option<String>,
    pub output: Option<PathBuf>,
    pub metadata: bool,
    pub clippy: bool,
    pub llvm_lines: bool,
    pub clippy_args: Vec<String>,
    pub llvm_lines_package: Option<String>,
    pub llvm_lines_file: Option<PathBuf>,
    pub llvm_top: usize,
    pub plan_limit: usize,
    pub fail_on: Option<Severity>,
    pub criterion: Option<PathBuf>,
    pub baseline: Option<String>,
    pub candidate: String,
    pub hyperfine: Option<(PathBuf, PathBuf)>,
    pub noise: f64,
    pub max_regression: Option<f64>,
}

impl Opts {
    fn new(cmd: Cmd) -> Self {
        Opts {
            cmd,
            config: None,
            project_config: None,
            explicit: vec![],
            write_baseline: None,
            debt_baseline: None,
            audit_json: None,
            root: PathBuf::from("."),
            rule: None,
            profile: Profile::Service,
            hot: vec![],
            exclude: vec![],
            include_tests: false,
            include_ignored: false,
            engine: EngineChoice::Auto,
            max_file_lines: 1000,
            max_fn_lines: 80,
            max_fields: 15,
            max_methods: 30,
            limit: 30,
            format: None,
            output: None,
            metadata: true,
            clippy: false,
            llvm_lines: false,
            clippy_args: vec![],
            llvm_lines_package: None,
            llvm_lines_file: None,
            llvm_top: 10,
            plan_limit: 20,
            fail_on: None,
            criterion: None,
            baseline: None,
            candidate: "new".into(),
            hyperfine: None,
            noise: 2.0,
            max_regression: None,
        }
    }

    pub fn format(&self) -> &str {
        self.format.as_deref().unwrap_or("text")
    }
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Opts, String> {
    let mut args = args.into_iter();
    let cmd = match args.next().as_deref() {
        None | Some("help" | "--help" | "-h") => return Ok(Opts::new(Cmd::Help)),
        Some("--version" | "-V" | "version") => return Ok(Opts::new(Cmd::Version)),
        Some("report") => Cmd::Report,
        Some("audit") => Cmd::Audit,
        Some("triage") => Cmd::Triage,
        Some("bench-compare") => Cmd::BenchCompare,
        Some("topology") => Cmd::Topology,
        Some("explain" | "rules") => Cmd::Explain,
        Some(other) => return Err(format!("unknown command: {other} (see --help)")),
    };
    let mut o = Opts::new(cmd);
    let mut positional_set = false;
    let num = |v: String, flag: &str| v.parse::<usize>().map_err(|_| format!("invalid number for {flag}: {v}"));
    let pct = |v: String, flag: &str| v.parse::<f64>().ok().filter(|p| p.is_finite() && *p >= 0.0).ok_or(format!("invalid percentage for {flag}: {v}"));
    while let Some(a) = args.next() {
        if a.starts_with('-') {
            o.explicit.push(a.clone());
        }
        match a.as_str() {
            "--config" => o.config = Some(PathBuf::from(next(&mut args, &a)?)),
            "--write-baseline" => o.write_baseline = Some(PathBuf::from(next(&mut args, &a)?)),
            "--debt-baseline" => o.debt_baseline = Some(PathBuf::from(next(&mut args, &a)?)),
            "--audit-json" => o.audit_json = Some(PathBuf::from(next(&mut args, &a)?)),
            "--help" | "-h" => return Ok(Opts::new(Cmd::Help)),
            "--profile" => {
                let v = next(&mut args, &a)?;
                o.profile = Profile::parse(&v).ok_or(format!("invalid --profile: {v}"))?;
            }
            "--hot" => o.hot.push(next(&mut args, &a)?),
            "--exclude" => o.exclude.push(next(&mut args, &a)?),
            "--include-tests" => o.include_tests = true,
            "--include-ignored" => o.include_ignored = true,
            "--engine" => {
                let v = next(&mut args, &a)?;
                o.engine = EngineChoice::parse(&v).ok_or(format!("invalid --engine: {v}"))?;
            }
            "--max-file-lines" => o.max_file_lines = num(next(&mut args, &a)?, &a)?,
            "--max-fn-lines" => o.max_fn_lines = num(next(&mut args, &a)?, &a)?,
            "--max-fields" => o.max_fields = num(next(&mut args, &a)?, &a)?,
            "--max-methods" => o.max_methods = num(next(&mut args, &a)?, &a)?,
            "--limit" => o.limit = num(next(&mut args, &a)?, &a)?,
            "--plan-limit" => o.plan_limit = num(next(&mut args, &a)?, &a)?,
            "--llvm-top" => o.llvm_top = num(next(&mut args, &a)?, &a)?,
            "--format" => o.format = Some(next(&mut args, &a)?),
            "--output" | "-o" => o.output = Some(PathBuf::from(next(&mut args, &a)?)),
            "--no-metadata" => o.metadata = false,
            "--deep" => {
                o.clippy = true;
                o.llvm_lines = true;
            }
            "--with" => {
                for w in next(&mut args, &a)?.split(',').map(str::trim).filter(|w| !w.is_empty()) {
                    match w {
                        "clippy" => o.clippy = true,
                        "llvm-lines" | "llvm_lines" => o.llvm_lines = true,
                        "metadata" => o.metadata = true,
                        "all" => {
                            o.clippy = true;
                            o.llvm_lines = true;
                        }
                        other => return Err(format!("unknown analyzer in --with: {other} (clippy, llvm-lines, all)")),
                    }
                }
            }
            "--clippy-arg" => o.clippy_args.push(next(&mut args, &a)?),
            "--llvm-lines-package" => o.llvm_lines_package = Some(next(&mut args, &a)?),
            "--llvm-lines-file" => {
                o.llvm_lines_file = Some(PathBuf::from(next(&mut args, &a)?));
                o.llvm_lines = true;
            }
            "--fail-on" => {
                let v = next(&mut args, &a)?;
                o.fail_on = Some(Severity::parse(&v).ok_or(format!("invalid --fail-on: {v}"))?);
            }
            "--criterion" => o.criterion = Some(PathBuf::from(next(&mut args, &a)?)),
            "--baseline" => o.baseline = Some(next(&mut args, &a)?),
            "--candidate" => o.candidate = next(&mut args, &a)?,
            "--hyperfine" => {
                let b = next(&mut args, &a)?;
                let c = next(&mut args, "--hyperfine (CAND.json)")?;
                o.hyperfine = Some((PathBuf::from(b), PathBuf::from(c)));
            }
            "--noise" => o.noise = pct(next(&mut args, &a)?, &a)?,
            "--max-regression" => o.max_regression = Some(pct(next(&mut args, &a)?, &a)?),
            _ if !a.starts_with('-') && !positional_set => {
                positional_set = true;
                if o.cmd == Cmd::Explain {
                    o.rule = Some(a);
                } else {
                    o.root = PathBuf::from(a);
                }
            }
            _ => return Err(format!("unrecognized argument: {a}")),
        }
    }
    crate::config::apply(&mut o)?;
    validate(&o)?;
    Ok(o)
}

fn validate(o: &Opts) -> Result<(), String> {
    let formats: &[&str] = match o.cmd {
        Cmd::Report => &["text", "markdown", "md", "json", "sarif"],
        Cmd::Audit | Cmd::Triage | Cmd::BenchCompare => &["text", "json"],
        _ => &["text"],
    };
    if !formats.contains(&o.format()) {
        return Err(format!("invalid --format for this command: {} (allowed: {})", o.format(), formats.join(", ")));
    }
    if o.max_file_lines == 0 || o.max_fn_lines == 0 {
        return Err("--max-file-lines and --max-fn-lines must be positive".into());
    }
    if o.criterion.is_some() && o.baseline.is_none() {
        return Err("--criterion and --baseline must be used together".into());
    }
    if o.cmd != Cmd::Report && (o.write_baseline.is_some() || o.debt_baseline.is_some() || o.audit_json.is_some() || (o.baseline.is_some() && o.criterion.is_none())) {
        return Err("debt baselines and --audit-json are only supported by report".into());
    }
    if o.debt_baseline.is_some() && o.baseline.is_some() && o.criterion.is_none() {
        return Err("use --baseline or --debt-baseline for debt, not both".into());
    }
    if o.criterion.is_some() && o.hyperfine.is_some() {
        return Err("use either --criterion or --hyperfine, not both".into());
    }
    if o.cmd == Cmd::BenchCompare && o.criterion.is_none() && o.hyperfine.is_none() {
        return Err("bench-compare needs --criterion DIR --baseline NAME or --hyperfine BASE CAND".into());
    }
    Ok(())
}

fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("missing value after {flag}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Opts, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_report_options() {
        let o = p("report proj --profile engine --hot src/sim/* --with clippy --format markdown --fail-on high --criterion target/criterion --baseline main").unwrap();
        assert_eq!(o.cmd, Cmd::Report);
        assert_eq!(o.root, PathBuf::from("proj"));
        assert_eq!(o.profile, Profile::Engine);
        assert!(o.clippy && !o.llvm_lines && o.metadata);
        assert_eq!(o.fail_on, Some(Severity::High));
        assert_eq!(o.candidate, "new");
    }

    #[test]
    fn rejects_invalid_combinations() {
        assert!(p("audit . --format markdown").is_err());
        assert!(p("report . --criterion dir").is_err());
        assert!(p("bench-compare").is_err());
        assert!(p("report . --with magic").is_err());
        assert!(p("report a b").is_err());
        assert!(p("frobnicate").is_err());
    }

    #[test]
    fn explain_takes_rule() {
        let o = p("explain clone_in_loop").unwrap();
        assert_eq!((o.cmd, o.rule.as_deref()), (Cmd::Explain, Some("clone_in_loop")));
    }
}
