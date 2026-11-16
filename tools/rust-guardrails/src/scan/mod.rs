//! Source scanning: file discovery, engine selection (syn AST or text heuristic),
//! crate-wide type metrics and profile-based severity adjustment.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::glob;
use crate::model::{Category, Confidence, Finding, Profile, Severity};
use crate::rules;

#[cfg(feature = "ast")]
pub mod ast;
pub mod heuristic;

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
    pub include_ignored: bool,
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
    /// Out-of-line module source candidates, resolved relative to the scan root.
    pub modules: Vec<ModuleRef>,
}

#[derive(Debug)]
pub struct ModuleRef {
    pub candidates: Vec<String>,
    pub test_only: bool,
    /// A path-directed source has a different default child directory in Rust.
    pub path_attr: bool,
    pub in_path_file: bool,
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
    pub scope_note: String,
    /// Cargo analysis consumes the same selected inventory as the source scan.
    pub cargo_manifests: Vec<PathBuf>,
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

/// Only remove a test-reachable file when no production root also reaches it.
/// Unreferenced sources stay in scope: this is module selection, not dead-code analysis.
fn test_module_files(files: &[Scanned], target_roots: &BTreeSet<String>) -> BTreeSet<String> {
    let by_path: BTreeMap<_, _> = files.iter().enumerate().map(|(i, s)| (s.rel.as_str(), i)).collect();
    // A source can be loaded both normally and with #[path]. Track both module
    // directory contexts rather than guessing the child directory from its stem.
    let mut edges = vec![Vec::new(); files.len() * 2];
    let mut incoming = BTreeSet::new();
    for (i, s) in files.iter().enumerate() {
        for m in &s.scan.modules {
            for candidate in &m.candidates {
                if let Some(&j) = by_path.get(candidate.as_str()) {
                    let target = j * 2 + usize::from(m.path_attr);
                    edges[i * 2 + usize::from(m.in_path_file)].push((target, m.test_only));
                    incoming.insert(j);
                }
            }
        }
    }
    let mut tests = BTreeSet::new();
    let mut production = BTreeSet::new();
    let mut pending: Vec<_> = files
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let target = target_roots.contains(&s.rel);
            (!incoming.contains(&i) || target || s.scan.test_only).then_some((i * 2 + usize::from(target), s.scan.test_only))
        })
        .collect();
    while let Some((i, test_context)) = pending.pop() {
        let reached = if test_context { &mut tests } else { &mut production };
        if !reached.insert(i) {
            continue;
        }
        for &(j, test) in &edges[i] {
            pending.push((j, test_context || test || files[j / 2].scan.test_only));
        }
    }
    let test_files: BTreeSet<_> = tests.iter().map(|i| i / 2).collect();
    let production_files: BTreeSet<_> = production.iter().map(|i| i / 2).collect();
    test_files.difference(&production_files).map(|&i| files[i].rel.clone()).collect()
}

/// Narrow target hints, not a Cargo TOML parser. Recognized paths establish extra
/// production roots; ambiguous target syntax retains package sources conservatively.
fn production_roots(root: &Path, manifests: &[PathBuf], files: &[Scanned]) -> (BTreeSet<String>, bool) {
    fn plain_string(value: &str) -> Option<&str> {
        let quote = value.chars().next()?;
        if !matches!(quote, '\'' | '"') {
            return None;
        }
        let rest = &value[1..];
        let end = rest.find(quote)?;
        let tail = rest[end + 1..].trim();
        if (!tail.is_empty() && !tail.starts_with('#')) || (quote == '"' && rest[..end].contains('\\')) {
            return None;
        }
        Some(&rest[..end])
    }
    let mut roots = BTreeSet::new();
    let mut package_dirs = Vec::new();
    let mut uncertain = false;
    for manifest in manifests {
        let dir = manifest.parent().unwrap_or(root);
        package_dirs.push(dir);
        let mut hints = Vec::new();
        let (mut autolib, mut autobins, mut autoexamples, mut build_script, mut lib_table, mut lib_path) = (true, true, true, true, false, false);
        let mut ambiguous = false;
        let mut section = "";
        let source = fs::read_to_string(manifest).unwrap_or_else(|_| {
            ambiguous = true;
            String::new()
        });
        // Multiline values can contain apparent table headers. Do not guess.
        ambiguous |= source.contains("\"\"\"") || source.contains("'''");
        for raw in source.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                let header = line.split('#').next().unwrap_or("").trim();
                section = match header {
                    "[package]" => "package",
                    "[lib]" => {
                        lib_table = true;
                        "lib"
                    }
                    "[[bin]]" => {
                        hints.push(None);
                        "bin"
                    }
                    "[[example]]" => {
                        hints.push(None);
                        "example"
                    }
                    _ => {
                        let table = header.trim_matches(['[', ']', ' ', '\'', '"']);
                        if matches!(table, "lib" | "bin" | "example" | "package") || ["lib.", "bin.", "example.", "package."].iter().any(|prefix| table.starts_with(prefix)) {
                            ambiguous = true;
                        }
                        "other"
                    }
                };
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim().trim_matches(['\'', '"']), value.trim());
            if section.is_empty() && (matches!(key, "lib" | "bin" | "example" | "package") || ["lib.", "bin.", "example.", "package."].iter().any(|prefix| key.starts_with(prefix)))
            {
                ambiguous = true;
            }
            if section == "package" && matches!(key, "autolib" | "autobins" | "autoexamples") {
                let parsed = match value.split('#').next().unwrap_or("").trim() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
                match (key, parsed) {
                    ("autolib", Some(v)) => autolib = v,
                    ("autobins", Some(v)) => autobins = v,
                    ("autoexamples", Some(v)) => autoexamples = v,
                    _ => ambiguous = true,
                }
            }
            if section == "package" && key == "build" {
                match value.split('#').next().unwrap_or("").trim() {
                    "false" => build_script = false,
                    "true" => build_script = true,
                    _ => {
                        build_script = false;
                        if let Some(path) = plain_string(value).filter(|p| !Path::new(p).is_absolute()) {
                            hints.push(Some(dir.join(path)));
                        } else {
                            ambiguous = true;
                        }
                    }
                }
            }
            if matches!(section, "lib" | "bin" | "example") && key == "path" {
                if let Some(path) = plain_string(value) {
                    if Path::new(path).is_absolute() {
                        ambiguous = true;
                    } else {
                        let path = dir.join(path);
                        if section == "lib" {
                            lib_path = true;
                            hints.push(Some(path));
                        } else if let Some(hint) = hints.last_mut() {
                            *hint = Some(path);
                        }
                    }
                } else {
                    ambiguous = true;
                }
            }
        }
        // A named target declaration without a plain path needs Cargo's inference.
        ambiguous |= hints.iter().any(Option::is_none);
        let explicit: BTreeSet<_> = hints.into_iter().flatten().filter_map(|p| p.canonicalize().ok()).collect();
        for file in files {
            let path = root.join(&file.rel);
            if path.canonicalize().ok().is_some_and(|p| explicit.contains(&p)) {
                roots.insert(file.rel.clone());
                continue;
            }
            let Ok(local) = path.strip_prefix(dir) else {
                continue;
            };
            let local = local.to_string_lossy().replace('\\', "/");
            let default_lib = local == "src/lib.rs" && !lib_path && (autolib || lib_table);
            let default_bin = autobins
                && (local == "src/main.rs"
                    || local.strip_prefix("src/bin/").is_some_and(|p| {
                        let parts: Vec<_> = p.split('/').collect();
                        (parts.len() == 1 && p.ends_with(".rs")) || (parts.len() == 2 && parts[1] == "main.rs")
                    }));
            let default_example = autoexamples
                && local.strip_prefix("examples/").is_some_and(|p| {
                    let parts: Vec<_> = p.split('/').collect();
                    (parts.len() == 1 && p.ends_with(".rs")) || (parts.len() == 2 && parts[1] == "main.rs")
                });
            if ambiguous || default_lib || default_bin || default_example || (build_script && local == "build.rs") {
                roots.insert(file.rel.clone());
            }
        }
        uncertain |= ambiguous;
    }
    for file in files {
        let path = root.join(&file.rel);
        if !package_dirs.iter().any(|dir| path.starts_with(dir)) && matches!(file.rel.as_str(), "lib.rs" | "main.rs") {
            roots.insert(file.rel.clone());
        }
    }
    (roots, uncertain)
}

pub fn run(c: &ScanConfig) -> Result<ScanResult, String> {
    let use_ast = match c.engine {
        EngineChoice::Heuristic => false,
        EngineChoice::Auto => ast_available(),
        EngineChoice::Ast if ast_available() => true,
        EngineChoice::Ast => return Err("this build has no AST engine; rebuild with `--features ast` or use `--engine heuristic`".into()),
    };
    let mut inventory = crate::scope::discover(&c.root, &c.exclude, c.include_ignored)?;
    let cargo_manifests: Vec<PathBuf> = inventory.files.iter().filter(|p| p.file_name().is_some_and(|n| n == "Cargo.toml")).cloned().collect();
    let mut findings = Vec::new();
    let mut scanned_files = Vec::new();
    let mut test_files_skipped = 0usize;
    let mut fallbacks = 0usize;
    for path in inventory.files.into_iter().filter(|p| p.extension().is_some_and(|x| x == "rs")) {
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
    let (target_roots, uncertain_targets) = production_roots(&c.root, &cargo_manifests, &scanned_files);
    if uncertain_targets {
        inventory.note.push_str("; unsupported Cargo target syntax: package sources retained conservatively during test-module filtering; target identity is unverified");
    }
    let test_files = test_module_files(&scanned_files, &target_roots);
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
    Ok(ScanResult {
        files_scanned: scanned,
        lines_scanned: lines,
        engine,
        findings,
        test_files_skipped,
        dirs_skipped: inventory.dirs_skipped,
        scope_note: inventory.note,
        cargo_manifests,
    })
}

#[cfg(feature = "ast")]
fn ast_scan(c: &ScanConfig, rel: &str, content: &str) -> Result<FileScan, String> {
    ast::scan_with_tests(rel, content, c.max_fn_lines, c.include_tests)
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
        // A service profile cannot establish missing admission/backlog bounds.
        // These signals remain medium until a reviewer inspects the mechanism.
        (Profile::Library, "unwrap_in_non_test" | "panic_in_library" | "process_exit") => raise(f),
        (Profile::Ffi, "unsafe_impl" | "transmute" | "unsafe_keyword") => raise(f),
        (Profile::Prototype, _) if f.category == Category::Quality && f.severity == Severity::Medium => f.severity = Severity::Info,
        _ => {}
    }
}
