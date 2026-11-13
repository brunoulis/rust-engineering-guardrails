//! Source scanning: file discovery, engine selection (syn AST or text heuristic),
//! crate-wide type metrics and profile-based severity adjustment.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::glob;
use crate::model::{Category, Confidence, Finding, Profile, Severity};
use crate::rules;

#[cfg(feature = "ast")]
pub mod ast;
pub mod heuristic;

/// Never Rust sources worth auditing; skipped at any depth.
const ALWAYS_SKIP: &[&str] = &[".git", ".hg", ".svn", "node_modules", ".venv", "venv", ".next", ".idea", ".vscode", ".cache", "__pycache__"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineChoice {
    Auto,
    Ast,
    Heuristic,
}

impl EngineChoice {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(EngineChoice::Auto),
            "ast" => Some(EngineChoice::Ast),
            "heuristic" => Some(EngineChoice::Heuristic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub root: PathBuf,
    pub profile: Profile,
    pub hot: Vec<String>,
    pub exclude: Vec<String>,
    pub max_file_lines: usize,
    #[cfg_attr(not(feature = "ast"), allow(dead_code))]
    pub max_fn_lines: usize,
    pub max_fields: usize,
    pub max_methods: usize,
    pub engine: EngineChoice,
    /// Also scan `tests/`, `benches/` and `#[cfg(test)]` module files.
    pub include_tests: bool,
}

/// Per-type size facts collected by the AST engine and merged crate-wide.
#[derive(Debug, Clone, Default)]
pub struct TypeStat {
    pub file: String,
    pub line: usize,
    pub fields: usize,
    pub inherent_methods: usize,
    pub impl_blocks: usize,
}

#[derive(Debug, Default)]
pub struct FileScan {
    pub findings: Vec<Finding>,
    pub types: Vec<(String, TypeStat)>,
    /// The file carries `#![cfg(test)]`.
    pub test_only: bool,
    /// Names of out-of-line `#[cfg(test)] mod name;` declarations in this file.
    pub test_modules: Vec<String>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub files_scanned: usize,
    pub lines_scanned: usize,
    /// "ast", "heuristic" or "ast+heuristic-fallback".
    pub engine: &'static str,
    pub findings: Vec<Finding>,
    /// Test files left out (`tests/`, `benches/`, `#![cfg(test)]`, `#[cfg(test)] mod x;`).
    pub test_files_skipped: usize,
    /// Directories not descended into (VCS, cargo target dirs, tool caches).
    pub dirs_skipped: Vec<String>,
}

pub fn ast_available() -> bool {
    cfg!(feature = "ast")
}

struct Scanned {
    rel: String,
    hot: bool,
    lines: usize,
    scan: FileScan,
    extra: Vec<Finding>,
}

/// `tests/` and `benches/` directories hold integration tests and benchmarks.
fn is_test_path(rel: &str) -> bool {
    rel.split('/').rev().skip(1).any(|seg| seg == "tests" || seg == "benches")
}

/// Files that `#[cfg(test)] mod name;` in `rel` would load.
fn test_module_files(rel: &str, name: &str) -> [String; 2] {
    let (dir, file) = rel.rsplit_once('/').unwrap_or(("", rel));
    let stem = file.trim_end_matches(".rs");
    let base = if matches!(stem, "lib" | "main" | "mod") {
        dir.to_string()
    } else if dir.is_empty() {
        stem.to_string()
    } else {
        format!("{dir}/{stem}")
    };
    let join = |p: String| if base.is_empty() { p } else { format!("{base}/{p}") };
    [join(format!("{name}.rs")), join(format!("{name}/mod.rs"))]
}

pub fn run(c: &ScanConfig) -> Result<ScanResult, String> {
    let use_ast = match c.engine {
        EngineChoice::Heuristic => false,
        EngineChoice::Auto => ast_available(),
        EngineChoice::Ast if ast_available() => true,
        EngineChoice::Ast => return Err("this build has no AST engine; rebuild with `--features ast` or use `--engine heuristic`".into()),
    };
    let mut files = Vec::new();
    let mut dirs_skipped = Vec::new();
    gather(&c.root, &c.root, &mut files, &mut dirs_skipped)?;
    let mut findings = Vec::new();
    let mut scanned_files = Vec::new();
    let mut test_files_skipped = 0usize;
    let mut fallbacks = 0usize;
    for path in files {
        let rel = path.strip_prefix(&c.root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if glob::any(&c.exclude, &rel) {
            continue;
        }
        if !c.include_tests && is_test_path(&rel) {
            test_files_skipped += 1;
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                findings.push(rules::finding("read_error", Confidence::Heuristic).at(&rel, 0).evidence(e.to_string()));
                continue;
            }
        };
        let mut extra = Vec::new();
        let scan = if use_ast {
            match ast_scan(c, &rel, &content) {
                Ok(s) => s,
                Err(parse_err) => {
                    fallbacks += 1;
                    extra.push(rules::finding("parse_fallback", Confidence::Heuristic).at(&rel, 0).evidence(parse_err));
                    heuristic::scan(&rel, &content)
                }
            }
        } else {
            heuristic::scan(&rel, &content)
        };
        let hot = glob::any(&c.hot, &rel);
        scanned_files.push(Scanned { rel, hot, lines: content.lines().count(), scan, extra });
    }
    // Drop test-only files now that every out-of-line `#[cfg(test)] mod` is known.
    let test_files: std::collections::BTreeSet<String> = scanned_files.iter().flat_map(|s| s.scan.test_modules.iter().flat_map(|m| test_module_files(&s.rel, m))).collect();
    if !c.include_tests {
        let before = scanned_files.len();
        scanned_files.retain(|s| !s.scan.test_only && !test_files.contains(&s.rel));
        test_files_skipped += before - scanned_files.len();
    }
    let mut types: BTreeMap<String, TypeStat> = BTreeMap::new();
    let (mut scanned, mut lines) = (0usize, 0usize);
    for s in scanned_files {
        scanned += 1;
        lines += s.lines;
        if s.lines > c.max_file_lines {
            findings.push(rules::finding("large_file", Confidence::Syntactic).at(&s.rel, 1).hot(s.hot).evidence(format!("{} lines (threshold {})", s.lines, c.max_file_lines)));
        }
        findings.extend(s.extra);
        for mut f in s.scan.findings {
            f.hot = s.hot;
            findings.push(f);
        }
        for (name, t) in s.scan.types {
            let e = types.entry(name).or_default();
            if t.fields > 0 || e.file.is_empty() {
                e.file = t.file;
                e.line = t.line;
            }
            e.fields = e.fields.max(t.fields);
            e.inherent_methods += t.inherent_methods;
            e.impl_blocks += t.impl_blocks;
        }
    }
    for (name, t) in types {
        if t.fields > c.max_fields || t.inherent_methods > c.max_methods {
            let hot = glob::any(&c.hot, &t.file);
            // Many fields with few methods is a data record (options, DTO), not a God object.
            let record = t.inherent_methods <= 5 && t.inherent_methods <= c.max_methods;
            let mut f = rules::finding("large_type", Confidence::Syntactic).at(&t.file, t.line).hot(hot).evidence(format!(
                "{} fields, {} inherent methods across {} impl blocks (thresholds {} fields / {} methods; same-named types in different modules are merged)",
                t.fields, t.inherent_methods, t.impl_blocks, c.max_fields, c.max_methods
            ));
            if record {
                f.severity = Severity::Info;
                f.message = format!("Wide data type `{name}` (many fields, few methods: likely a record)");
            } else {
                f.message = format!("Large type `{name}` (God-object candidate)");
            }
            findings.push(f);
        }
    }
    for f in &mut findings {
        adjust_for_profile(c.profile, f);
    }
    let engine = match (use_ast, fallbacks) {
        (false, _) => "heuristic",
        (true, 0) => "ast",
        (true, _) => "ast+heuristic-fallback",
    };
    Ok(ScanResult { files_scanned: scanned, lines_scanned: lines, engine, findings, test_files_skipped, dirs_skipped })
}

#[cfg(feature = "ast")]
fn ast_scan(c: &ScanConfig, rel: &str, content: &str) -> Result<FileScan, String> {
    ast::scan(rel, content, c.max_fn_lines)
}

#[cfg(not(feature = "ast"))]
fn ast_scan(_: &ScanConfig, _: &str, _: &str) -> Result<FileScan, String> {
    Err("AST engine not compiled in".into())
}

/// Profile-specific emphasis (SKILL.md §0). Never raises anything to `Blocker`.
pub fn adjust_for_profile(p: Profile, f: &mut Finding) {
    let raise = |f: &mut Finding| {
        if f.severity < Severity::High {
            f.severity = match f.severity {
                Severity::Info => Severity::Medium,
                _ => Severity::High,
            };
        }
    };
    // Per-iteration cost only matters on a known hot path (SKILL.md: no blanket bans).
    if f.hot && f.severity == Severity::Info && matches!(f.rule.as_str(), "allocation_in_loop" | "clone_in_loop" | "await_in_loop") {
        f.severity = Severity::Medium;
    }
    match (p, f.rule.as_str()) {
        (Profile::Service, "spawn_in_loop" | "unbounded_channel") => raise(f),
        (Profile::Library, "unwrap_in_non_test" | "panic_in_library" | "process_exit") => raise(f),
        (Profile::Ffi, "unsafe_impl" | "transmute" | "unsafe_keyword") => raise(f),
        (Profile::Prototype, _) if f.category == Category::Quality && f.severity == Severity::Medium => f.severity = Severity::Info,
        _ => {}
    }
}

/// Skips VCS/tool caches anywhere, and cargo build directories: a `target` directory
/// next to a `Cargo.toml`, or any directory cargo tagged with `CACHEDIR.TAG`. Ordinary
/// source directories such as `src/build/` are scanned.
fn skip_dir(dir: &Path, name: &str) -> bool {
    ALWAYS_SKIP.contains(&name) || dir.join("CACHEDIR.TAG").is_file() || (name == "target" && dir.parent().is_some_and(|p| p.join("Cargo.toml").is_file()))
}

fn gather(root: &Path, here: &Path, out: &mut Vec<PathBuf>, skipped: &mut Vec<String>) -> Result<(), String> {
    let mut entries = fs::read_dir(here).map_err(|e| format!("{}: {e}", here.display()))?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let ty = e.file_type().map_err(|err| err.to_string())?;
        if ty.is_symlink() {
            continue;
        }
        let p = e.path();
        if ty.is_dir() {
            if skip_dir(&p, &e.file_name().to_string_lossy()) {
                skipped.push(p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"));
            } else {
                gather(root, &p, out, skipped)?;
            }
        } else if ty.is_file() && p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    Ok(())
}
