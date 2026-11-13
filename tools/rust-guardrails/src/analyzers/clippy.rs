//! `cargo clippy --message-format=json` integration: type-aware diagnostics mapped onto
//! the audit dimensions. This **builds the project** (build scripts and proc-macros run),
//! so it is opt-in and should only be used on trusted code.

use std::collections::BTreeSet;
use std::path::Path;

use super::{cargo, first_line, relativize, run_timed};
use crate::json::{self, Value};
use crate::model::{excerpt, AnalyzerRun, Category, Confidence, Finding, Severity};

/// Extra lints enabled for the audit on top of clippy's defaults. Chosen because they
/// sharpen the syntactic signals with type information; none is from the contradictory
/// parts of `clippy::restriction` except the unsafe-documentation ones.
pub const AUDIT_LINTS: &[&str] = &[
    "clippy::undocumented_unsafe_blocks",
    "clippy::multiple_unsafe_ops_per_block",
    "clippy::redundant_clone",
    "clippy::needless_collect",
    "clippy::implicit_clone",
    "clippy::large_futures",
    "clippy::large_types_passed_by_value",
    "clippy::large_stack_arrays",
    "clippy::needless_pass_by_value",
    "clippy::unused_async",
    "clippy::inefficient_to_string",
    "clippy::significant_drop_tightening",
];

pub fn analyze(root: &Path, extra_args: &[String]) -> (Vec<Finding>, AnalyzerRun) {
    // Paths in cargo diagnostics are relative to the *workspace* root, which differs from
    // the audited root when auditing one member of a workspace.
    let ws_root = workspace_root(root).unwrap_or_else(|| root.to_path_buf());
    let mut c = cargo(root);
    // --locked: never create or rewrite Cargo.lock in the audited project.
    c.args(["clippy", "--locked", "--workspace", "--all-targets", "--message-format=json", "--quiet"]);
    c.args(extra_args);
    // --cap-lints warn: neither the audit lints below nor `#![deny(..)]` attributes in the
    // project can turn a lint into a build failure, so every error left is a real
    // compile error and no lint hides the others.
    c.args(["--", "--cap-lints", "warn"]);
    for l in AUDIT_LINTS {
        c.args(["-W", l]);
    }
    c.args(["-A", "unknown_lints"]);
    let t = match run_timed(c) {
        Ok(t) => t,
        Err(e) => return (vec![], AnalyzerRun::skipped("clippy", format!("cannot run cargo clippy: {e}"))),
    };
    let stdout = String::from_utf8_lossy(&t.output.stdout);
    if !stdout.contains("\"reason\":\"build-finished\"") && !t.output.status.success() {
        let err = first_line(&String::from_utf8_lossy(&t.output.stderr));
        let hint =
            if err.contains("lock file") { " (Cargo.lock missing or outdated; the audit never writes it — run `cargo generate-lockfile` yourself if appropriate)" } else { "" };
        return (vec![], AnalyzerRun::failed("clippy", format!("{err}{hint}")));
    }
    let parsed = parse_messages(root, &ws_root, &stdout);
    let errors = parsed.findings.iter().filter(|f| f.severity == Severity::Blocker).count();
    let detail = format!(
        "{} diagnostic(s), {} compile error(s), build {} in {:.1}s; extra lints: {}; lints capped at warn",
        parsed.findings.len(),
        errors,
        if parsed.build_ok { "succeeded" } else { "FAILED" },
        t.secs,
        AUDIT_LINTS.len()
    );
    (parsed.findings, AnalyzerRun::ran("clippy", detail))
}

fn workspace_root(root: &Path) -> Option<std::path::PathBuf> {
    let o = cargo(root).args(["locate-project", "--workspace", "--message-format", "plain"]).output().ok()?;
    let manifest = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (o.status.success() && !manifest.is_empty()).then(|| Path::new(&manifest).parent().map(Path::to_path_buf)).flatten()
}

pub struct Parsed {
    pub findings: Vec<Finding>,
    pub build_ok: bool,
}

pub fn parse_messages(root: &Path, ws_root: &Path, stdout: &str) -> Parsed {
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    let mut build_ok = true;
    for line in stdout.lines().filter(|l| l.starts_with('{')) {
        let Ok(v) = json::parse(line) else { continue };
        match v.str_of("reason") {
            "build-finished" => build_ok = v.get("success").and_then(Value::as_bool).unwrap_or(false),
            "compiler-message" => {
                let Some(msg) = v.get("message") else { continue };
                if let Some(f) = to_finding(root, ws_root, msg) {
                    if seen.insert((f.rule.clone(), f.file.clone(), f.line, f.message.clone())) {
                        findings.push(f);
                    }
                }
            }
            _ => {}
        }
    }
    Parsed { findings, build_ok }
}

/// rustc error codes (`E0425`) or uncoded errors are compile errors; anything else is a lint.
fn is_compile_error(code: &str) -> bool {
    code.is_empty() || (code.len() == 5 && code.starts_with('E') && code[1..].chars().all(|c| c.is_ascii_digit()))
}

fn to_finding(root: &Path, ws_root: &Path, msg: &Value) -> Option<Finding> {
    let level = msg.str_of("level");
    let text = msg.str_of("message");
    let code = msg.path(&["code", "code"]).and_then(Value::as_str).unwrap_or("");
    if level != "warning" && level != "error" {
        return None;
    }
    if code == "unknown_lints"
        || text.starts_with("aborting due to")
        || text.contains("warnings emitted")
        || text.contains("warning emitted")
        || text.starts_with("could not compile")
    {
        return None;
    }
    let compile_error = level == "error" && is_compile_error(code);
    let primary = msg.get("spans").map(Value::as_array).unwrap_or_default().iter().find(|s| s.get("is_primary").and_then(Value::as_bool) == Some(true));
    let mut outside = None;
    let (file, line, snippet) = match primary {
        Some(s) => {
            let raw = s.str_of("file_name");
            let p = Path::new(raw);
            let abs = if p.is_absolute() || p.has_root() { p.to_path_buf() } else { ws_root.join(p) };
            let file = match relativize(root, &abs.to_string_lossy()) {
                Some(f) => f,
                // Lints in other crates are out of scope; a compile error there still breaks the build.
                None if compile_error => {
                    outside = Some(raw.replace('\\', "/"));
                    String::new()
                }
                None => return None,
            };
            let line = s.get("line_start").and_then(Value::as_f64).unwrap_or(0.0) as usize;
            let snippet = s.get("text").map(Value::as_array).unwrap_or_default().first().map(|t| t.str_of("text").to_string()).unwrap_or_default();
            (file, line, snippet)
        }
        None if compile_error => (String::new(), 0, String::new()),
        None => return None,
    };
    let rule = if code.is_empty() { format!("rustc::{level}") } else { code.to_string() };
    let (category, mut severity) = categorize(&rule);
    let mut message = text.to_string();
    if compile_error {
        severity = Severity::Blocker;
    }
    if let Some(o) = outside {
        message = format!("{message} (in {o}, outside the audited root)");
    }
    Some(Finding::new(rule, category, severity, Confidence::Compiler).at(&file, if file.is_empty() { 0 } else { line }).msg(message).evidence(excerpt(&snippet, 180)))
}

/// Maps a lint/diagnostic code to a dimension and default severity.
pub fn categorize(code: &str) -> (Category, Severity) {
    let name = code.strip_prefix("clippy::").unwrap_or(code);
    use Category::*;
    use Severity::*;
    match name {
        "mut_from_ref"
        | "uninit_assumed_init"
        | "uninit_vec"
        | "not_unsafe_ptr_arg_deref"
        | "transmute_ptr_to_ref"
        | "cast_ptr_alignment"
        | "invalid_reference_casting"
        | "static_mut_refs" => (Safety, High),
        "undocumented_unsafe_blocks" | "missing_safety_doc" | "multiple_unsafe_ops_per_block" => (Safety, Medium),
        n if n.starts_with("transmute") || n.contains("unsafe") || n.contains("_ptr") => (Safety, Medium),
        "await_holding_lock" | "await_holding_refcell_ref" | "await_holding_invalid_type" | "arc_with_non_send_sync" => (Concurrency, High),
        "mutex_atomic"
        | "mutex_integer"
        | "future_not_send"
        | "large_futures"
        | "significant_drop_in_scrutinee"
        | "significant_drop_tightening"
        | "unused_async"
        | "rc_clone_in_vec_init" => (Concurrency, Medium),
        "redundant_clone" | "needless_pass_by_value" | "unnecessary_to_owned" | "redundant_allocation" | "clone_on_ref_ptr" | "assigning_clones" | "needless_pass_by_ref_mut" => {
            (Ownership, Medium)
        }
        "clone_on_copy" | "implicit_clone" | "ptr_arg" | "rc_buffer" | "cloned_instead_of_copied" | "unnecessary_clone" => (Ownership, Info),
        "box_collection"
        | "boxed_local"
        | "cmp_owned"
        | "collapsible_str_replace"
        | "expect_fun_call"
        | "extend_with_drain"
        | "format_collect"
        | "iter_nth"
        | "large_const_arrays"
        | "large_enum_variant"
        | "manual_memcpy"
        | "manual_retain"
        | "manual_str_repeat"
        | "map_entry"
        | "missing_spin_loop"
        | "needless_collect"
        | "result_large_err"
        | "single_char_pattern"
        | "slow_vector_initialization"
        | "to_string_in_format_args"
        | "useless_vec"
        | "vec_init_then_push"
        | "large_types_passed_by_value"
        | "large_stack_arrays"
        | "large_stack_frames"
        | "inefficient_to_string"
        | "iter_overeager_cloned"
        | "or_fun_call"
        | "format_push_string"
        | "drain_collect"
        | "unnecessary_box_returns"
        | "trivially_copy_pass_by_ref"
        | "vec_box"
        | "manual_flatten"
        | "needless_range_loop" => (Performance, Medium),
        "too_many_arguments" | "too_many_lines" | "cognitive_complexity" | "type_complexity" | "struct_excessive_bools" | "fn_params_excessive_bools" => (Architecture, Medium),
        "module_inception" | "module_name_repetitions" | "self_named_module_files" | "mod_module_files" => (Architecture, Info),
        "unused_must_use" | "let_underscore_future" | "unwrap_used" | "expect_used" | "panic" | "todo" | "unimplemented" | "dbg_macro" => (Quality, Medium),
        _ => (Quality, Info),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_json_stream() {
        let root = Path::new(".");
        let stream = [
            r#"{"reason":"compiler-artifact","package_id":"x"}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"redundant clone","code":{"code":"clippy::redundant_clone"},"spans":[{"file_name":"src\\lib.rs","line_start":7,"is_primary":true,"text":[{"text":"    let b = a.clone();"}]}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"redundant clone","code":{"code":"clippy::redundant_clone"},"spans":[{"file_name":"src\\lib.rs","line_start":7,"is_primary":true,"text":[{"text":"    let b = a.clone();"}]}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"this `MutexGuard` is held across an await point","code":{"code":"clippy::await_holding_lock"},"spans":[{"file_name":"src/svc.rs","line_start":3,"is_primary":true,"text":[]}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"error","message":"cannot find value `x`","code":{"code":"E0425"},"spans":[{"file_name":"src/main.rs","line_start":1,"is_primary":true,"text":[]}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"error","message":"aborting due to 1 previous error","code":null,"spans":[]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"dep lint","code":{"code":"dead_code"},"spans":[{"file_name":"/elsewhere/dep/src/lib.rs","line_start":1,"is_primary":true,"text":[]}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"error","message":"deny-level lint","code":{"code":"clippy::not_unsafe_ptr_arg_deref"},"spans":[{"file_name":"src/lib.rs","line_start":9,"is_primary":true,"text":[]}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"error","message":"mismatched types","code":{"code":"E0308"},"spans":[{"file_name":"/elsewhere/dep/src/lib.rs","line_start":4,"is_primary":true,"text":[]}]}}"#,
            r#"{"reason":"build-finished","success":false}"#,
        ]
        .join("\n");
        let p = parse_messages(root, root, &stream);
        assert!(!p.build_ok);
        assert!(p.findings.iter().any(|f| f.message.contains("outside the audited root")));
        let got: Vec<_> = p.findings.iter().map(|f| (f.rule.as_str(), f.file.as_str(), f.line, f.severity, f.category)).collect();
        assert_eq!(
            got,
            vec![
                ("clippy::redundant_clone", "src/lib.rs", 7, Severity::Medium, Category::Ownership),
                ("clippy::await_holding_lock", "src/svc.rs", 3, Severity::High, Category::Concurrency),
                ("E0425", "src/main.rs", 1, Severity::Blocker, Category::Quality),
                ("clippy::not_unsafe_ptr_arg_deref", "src/lib.rs", 9, Severity::High, Category::Safety),
                ("E0308", "", 0, Severity::Blocker, Category::Quality),
            ]
        );
    }
}
