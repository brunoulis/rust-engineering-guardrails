use std::process::Command;

#[path = "../src/json.rs"]
#[allow(dead_code)]
mod json;

fn run(root: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rust-guardrails")).args(["report", &root.to_string_lossy(), "--no-metadata", "--format", "json"]).args(args).output().unwrap()
}

#[test]
fn imported_advisories_gate_without_claiming_exploitability() {
    let root = std::env::temp_dir().join(format!("rails-security-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("audit.json");
    std::fs::write(&file, r#"{"vulnerabilities":{"found":true,"count":1,"list":[{"advisory":{"id":"RUSTSEC-2026-0001","title":"Test advisory"},"package":{"name":"example","version":"1.0.0"}}]},"warnings":{"unmaintained":[{"kind":"unmaintained","package":{"name":"old","version":"0.1.0"}}]}}"#).unwrap();
    let out = run(&root, &["--audit-json", &file.to_string_lossy(), "--fail-on", "high"]);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    let report = json::parse(&String::from_utf8(out.stdout).unwrap()).unwrap();
    let findings = report.get("findings").unwrap().as_array();
    let advisory = findings.iter().find(|f| f.str_of("rule") == "rustsec::RUSTSEC-2026-0001").unwrap();
    assert_eq!(advisory.str_of("severity"), "high");
    assert!(advisory.str_of("message").contains("reachability unverified"));
    assert!(findings.iter().any(|f| f.str_of("rule") == "dependency_warning::unmaintained" && f.str_of("severity") == "medium"));
    let plan = report.get("plan").unwrap().as_array().iter().find(|p| p.str_of("rule") == "rustsec::RUSTSEC-2026-0001").unwrap();
    assert!(!plan.str_of("why").contains("compiler"));
    assert!(plan.get("verify").unwrap().as_array().iter().any(|v| v.as_str().unwrap().contains("cargo audit")));
    std::fs::write(&file, r#"{"vulnerabilities":{"found":true,"count":1,"list":[]},"warnings":{}}"#).unwrap();
    assert_eq!(run(&root, &["--audit-json", &file.to_string_lossy()]).status.code(), Some(2));
    std::fs::write(&file, "{}").unwrap();
    assert_eq!(run(&root, &["--audit-json", &file.to_string_lossy()]).status.code(), Some(2));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ffi_profile_selects_memory_verification_but_does_not_run_it() {
    let root = std::env::temp_dir().join(format!("rails-verification-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn value() -> u32 { 42 }\n").unwrap();
    let out = run(&root, &["--profile", "ffi"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = json::parse(&String::from_utf8(out.stdout).unwrap()).unwrap();
    let recommendations = report.get("verification").unwrap().as_array();
    assert!(recommendations.iter().any(|r| r.str_of("technique") == "miri" && r.str_of("status") == "not_run"));
    assert!(recommendations.iter().any(|r| r.str_of("technique") == "sanitizers"));
    assert!(!recommendations.iter().any(|r| r.str_of("technique") == "loom"));
    let plain = run(&root, &[]);
    let plain = json::parse(&String::from_utf8(plain.stdout).unwrap()).unwrap();
    assert!(!plain.get("verification").unwrap().as_array().iter().any(|r| r.str_of("technique") == "miri"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn heuristic_unsafe_signal_selects_memory_checks() {
    let root = std::env::temp_dir().join(format!("rails-heuristic-memory-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("lib.rs"), "pub unsafe fn memory(p: *const u8) -> u8 { unsafe { *p } }\n").unwrap();
    let out = run(&root, &["--engine", "heuristic"]);
    assert!(out.status.success());
    let report = json::parse(&String::from_utf8(out.stdout).unwrap()).unwrap();
    let selected = report.get("verification").unwrap().as_array();
    assert!(selected.iter().any(|r| r.str_of("technique") == "miri"));
    assert!(selected.iter().any(|r| r.str_of("technique") == "sanitizers"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_new_warning_advisory_is_new_debt_for_the_same_package() {
    let root = std::env::temp_dir().join(format!("rails-warning-identity-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let audit = root.join("audit.json");
    let baseline = root.join("baseline.json");
    let input = r#"{"vulnerabilities":{"found":false,"count":0,"list":[]},"warnings":{"unmaintained":[{"package":{"name":"old","version":"0.1.0"},"advisory":{"id":"RUSTSEC-2026-0001","title":"First notice"}}]}}"#;
    std::fs::write(&audit, input).unwrap();
    let first = run(&root, &["--audit-json", &audit.to_string_lossy(), "--write-baseline", &baseline.to_string_lossy()]);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    std::fs::write(&audit, input.replace("RUSTSEC-2026-0001", "RUSTSEC-2026-0002").replace("First notice", "Second notice")).unwrap();
    let second = run(&root, &["--audit-json", &audit.to_string_lossy(), "--baseline", &baseline.to_string_lossy(), "--fail-on", "medium"]);
    assert_eq!(second.status.code(), Some(1), "{}", String::from_utf8_lossy(&second.stdout));
    let report = json::parse(&String::from_utf8(second.stdout).unwrap()).unwrap();
    assert!(report.get("findings").unwrap().as_array().iter().any(|f| f.str_of("message").contains("RUSTSEC-2026-0002") && f.str_of("message").contains("Second notice")));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicitly_requested_analyzer_without_manifest_is_failed() {
    let root = std::env::temp_dir().join(format!("rails-missing-manifest-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for analyzer in ["clippy", "llvm-lines"] {
        let out = run(&root, &["--with", analyzer, "--fail-on", "high"]);
        assert_eq!(out.status.code(), Some(1));
        let report = json::parse(&String::from_utf8(out.stdout).unwrap()).unwrap();
        assert!(report.get("analyzers").unwrap().as_array().iter().any(|a| a.str_of("name") == analyzer && a.str_of("status") == "failed"));
    }
    std::fs::remove_dir_all(root).unwrap();
}
