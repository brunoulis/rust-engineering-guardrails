//! Debt gates exercise the binary and real files. Real Clippy builds are opt-in.
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

#[allow(dead_code)]
#[path = "../src/json.rs"]
mod json;

struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!("rust-rails-baseline-{}-{}", std::process::id(), ID.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&root).unwrap();
        let p = Self(root);
        p.source(source);
        p
    }
    fn source(&self, s: &str) {
        std::fs::write(self.0.join("lib.rs"), s).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rust-guardrails")).arg("report").arg(&self.0).args(["--no-metadata", "--format", "json"]).args(args).output().unwrap()
    }
    fn baseline(&self) -> String {
        self.0.join("debt.json").to_string_lossy().into_owned()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn parsed(o: &Output) -> json::Value {
    json::parse(&String::from_utf8_lossy(&o.stdout)).unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&o.stderr)))
}

#[test]
fn line_motion_is_existing_debt_but_an_added_duplicate_fails() {
    let p = Project::new("pub fn a(v: String) { let _copy = v.clone(); }\n");
    let base = p.baseline();
    let saved = p.run(&["--write-baseline", &base]);
    assert!(saved.status.success(), "{}", String::from_utf8_lossy(&saved.stderr));
    p.source("// moved down\n\npub fn a(v: String) { let _copy = v.clone(); }\n");
    let moved = p.run(&["--baseline", &base, "--fail-on", "info"]);
    assert!(moved.status.success(), "{}", String::from_utf8_lossy(&moved.stdout));
    assert_eq!(parsed(&moved).path(&["baseline", "new_count"]).and_then(json::Value::as_f64), Some(0.0));
    p.source("// moved down\n\npub fn a(v: String) { let _copy = v.clone(); }\npub fn b(v: String) { let _copy = v.clone(); }\n");
    let added = p.run(&["--baseline", &base, "--fail-on", "info"]);
    assert_eq!(added.status.code(), Some(1));
    assert!(parsed(&added).path(&["baseline", "new_count"]).and_then(json::Value::as_f64).unwrap() > 0.0);
}

#[test]
fn malformed_and_missing_baselines_cannot_pass_a_gate() {
    let p = Project::new("pub fn a() {}\n");
    let base = p.baseline();
    assert_eq!(p.run(&["--baseline", &base, "--fail-on", "high"]).status.code(), Some(2));
    for contents in ["{}", "{\"schema_version\":1,\"findings\":[]}", "invalid"] {
        std::fs::write(&base, contents).unwrap();
        assert_eq!(p.run(&["--baseline", &base, "--fail-on", "high"]).status.code(), Some(2));
    }
}

#[test]
fn writing_a_baseline_does_not_make_existing_gate_failures_pass() {
    let p = Project::new("pub fn a(v: String) { let _copy = v.clone(); }\n");
    assert_eq!(p.run(&["--write-baseline", &p.baseline(), "--fail-on", "info"]).status.code(), Some(1));
}

#[test]
fn sarif_contains_locations_rules_and_execution_status() {
    let p = Project::new("pub fn a(v: String) { let _copy = v.clone(); }\n");
    std::fs::rename(p.0.join("lib.rs"), p.0.join("with space#.rs")).unwrap();
    let o = p.run(&["--format", "sarif"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let j = parsed(&o);
    assert_eq!(j.str_of("version"), "2.1.0");
    let run = &j.get("runs").unwrap().as_array()[0];
    assert!(!run.path(&["tool", "driver", "rules"]).unwrap().as_array().is_empty());
    assert_eq!(run.get("invocations").unwrap().as_array()[0].get("executionSuccessful").and_then(json::Value::as_bool), Some(true));
    let results = run.get("results").unwrap().as_array();
    assert!(results.iter().any(|v| v.str_of("ruleId") == "clone_call"));
    let uri = results[0].get("locations").unwrap().as_array()[0].path(&["physicalLocation", "artifactLocation", "uri"]).and_then(json::Value::as_str).unwrap();
    assert_eq!(uri, "with%20space%23.rs");
}

#[test]
fn explicitly_requested_unavailable_analyzers_fail_the_gate() {
    let p = Project::new("pub fn a() {}\n");
    std::fs::write(p.0.join("Cargo.toml"), "[package]\nname='unavailable'\nversion='0.1.0'\n[lib]\npath='lib.rs'\n[workspace]\n").unwrap();
    for name in ["metadata", "clippy", "llvm-lines"] {
        let o = Command::new(env!("CARGO_BIN_EXE_rust-guardrails"))
            .arg("report")
            .arg(&p.0)
            .args(["--no-metadata", "--with", name, "--fail-on", "high", "--format", "json"])
            .env("CARGO", p.0.join("missing-cargo-executable"))
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1), "a missing requested {name} executable must not pass");
        let j = parsed(&o);
        assert_eq!(j.get("analyzers").unwrap().as_array().iter().find(|a| a.str_of("name") == name).unwrap().str_of("status"), "failed");
    }
}

#[test]
fn malformed_llvm_lines_input_fails_but_total_only_input_is_valid() {
    let p = Project::new("pub fn a() {}\n");
    let saved = p.0.join("llvm-lines.txt");
    let input = saved.to_string_lossy().into_owned();
    for contents in ["", "Lines Copies Function name\n----- ----- -----\n", "not an llvm-lines table"] {
        std::fs::write(&saved, contents).unwrap();
        let o = p.run(&["--llvm-lines-file", &input, "--fail-on", "high"]);
        assert_eq!(o.status.code(), Some(1), "unreadable measurements must not pass as an empty analysis");
        let j = parsed(&o);
        assert_eq!(j.get("analyzers").unwrap().as_array().iter().find(|a| a.str_of("name") == "llvm-lines").unwrap().str_of("status"), "failed");
    }
    std::fs::write(&saved, "Lines Copies Function name\n0 0 (TOTAL)\n").unwrap();
    let o = p.run(&["--llvm-lines-file", &input, "--fail-on", "high"]);
    assert!(o.status.success());
    let j = parsed(&o);
    assert_eq!(j.get("analyzers").unwrap().as_array().iter().find(|a| a.str_of("name") == "llvm-lines").unwrap().str_of("status"), "ran");
    assert!(!j.get("findings").unwrap().as_array().iter().any(|f| f.str_of("rule") == "llvm_lines_hotspot"));
}

#[test]
#[ignore = "builds a temporary project with cargo clippy"]
fn failed_clippy_cannot_write_baseline_or_pass_against_preexisting_compile_errors() {
    let p = Project::new("pub fn broken() { missing(); }\n");
    std::fs::write(p.0.join("Cargo.toml"), "[package]\nname='broken_baseline'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='lib.rs'\n[workspace]\n").unwrap();
    let lock = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())).args(["generate-lockfile", "--offline"]).current_dir(&p.0).output().unwrap();
    assert!(lock.status.success());
    let base = p.baseline();
    let attempted = p.run(&["--with", "clippy", "--write-baseline", &base]);
    assert_eq!(attempted.status.code(), Some(2), "an incomplete build must not become accepted debt");
    assert!(!std::path::Path::new(&base).exists());
    // Simulate a debt snapshot written by an older auditor that incorrectly
    // accepted compile errors. Analyzer failure must override even exact matches.
    let report = parsed(&attempted);
    let entries = report
        .get("findings")
        .unwrap()
        .as_array()
        .iter()
        .map(|f| {
            let evidence = f.str_of("evidence");
            let key = json::obj([
                ("rule", f.str_of("rule").into()),
                ("file", f.str_of("file").into()),
                ("evidence", evidence.trim().into()),
                ("message", if evidence.is_empty() { f.str_of("message") } else { "" }.into()),
            ])
            .to_json();
            json::obj([("key", key.into()), ("severity", f.str_of("severity").into())])
        })
        .collect();
    std::fs::write(&base, json::obj([("tool", "rust-guardrails-debt".into()), ("schema_version", 1usize.into()), ("entries", json::Value::Arr(entries))]).to_json()).unwrap();
    let gate = p.run(&["--with", "clippy", "--baseline", &base, "--fail-on", "high"]);
    assert_eq!(gate.status.code(), Some(1));
    let j = parsed(&gate);
    assert_eq!(j.path(&["baseline", "new_count"]).and_then(json::Value::as_f64), Some(0.0));
    assert!(j.get("findings").unwrap().as_array().iter().any(|f| f.str_of("rule") == "E0425" && f.str_of("severity") == "blocker"));
    assert_eq!(j.get("analyzers").unwrap().as_array().iter().find(|a| a.str_of("name") == "clippy").unwrap().str_of("status"), "failed");
    let sarif = p.run(&["--with", "clippy", "--baseline", &base, "--format", "sarif", "--fail-on", "high"]);
    assert_eq!(sarif.status.code(), Some(1));
    let j = parsed(&sarif);
    let invocation = &j.get("runs").unwrap().as_array()[0].get("invocations").unwrap().as_array()[0];
    assert_eq!(invocation.get("executionSuccessful").and_then(json::Value::as_bool), Some(false));
}
