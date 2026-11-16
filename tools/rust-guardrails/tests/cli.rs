//! End-to-end tests of the CLI against fixtures in tests/fixtures.
//! The clippy test builds the fixture and is `#[ignore]`d; run with `cargo test -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rust-guardrails"))
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("binary runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn sample() -> String {
    fixtures().join("sample").to_string_lossy().into_owned()
}

#[test]
#[cfg(feature = "ast")]
fn bounded_spawn_is_a_review_lead_not_a_high_gate_failure() {
    let dir = std::env::temp_dir().join(format!("rust-guardrails-bounded-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Mini-Redis's admission pattern: the permit is acquired BEFORE spawning,
    // then retained inside the task until the handler finishes.
    std::fs::write(
        dir.join("lib.rs"),
        "async fn accept(limit: Semaphore) { loop { let permit = limit.acquire_owned().await.unwrap(); tokio::spawn(async move { handle().await; drop(permit); }); } }",
    )
    .unwrap();
    let o = run(&["report", &dir.to_string_lossy(), "--profile", "service", "--no-metadata", "--fail-on", "high", "--format", "json"]);
    std::fs::remove_file(dir.join("lib.rs")).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    assert!(o.status.success(), "{}", stdout(&o));
    assert!(stdout(&o).contains("\"rule\":\"spawn_in_loop\""));
}

#[test]
#[cfg(feature = "ast")]
fn documentation_findings_do_not_fail_high_gate_but_production_hazards_do() {
    let dir = std::env::temp_dir().join(format!("rust-guardrails-doc-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.rs"), "pub unsafe fn read(p: *const u8) -> u8 { unsafe { *p } }").unwrap();
    let args = ["report", dir.to_str().unwrap(), "--profile", "ffi", "--no-metadata", "--fail-on", "high", "--format", "json"];
    let docs = run(&args);
    assert!(docs.status.success(), "documentation alone must not fail the high gate: {}", stdout(&docs));
    for rule in ["unsafe_fn_without_safety_doc", "unsafe_without_safety_comment"] {
        assert!(has_finding(&stdout(&docs), rule, "lib.rs", 1), "{}", stdout(&docs));
    }
    assert!(!stdout(&docs).contains("\"severity\":\"high\""), "{}", stdout(&docs));
    std::fs::write(dir.join("lib.rs"), "static mut LIVE_STATE: u8 = 0;\npub unsafe fn read(p: *const u8) -> u8 { unsafe { *p } }").unwrap();
    let hazard = run(&args);
    std::fs::remove_file(dir.join("lib.rs")).unwrap();
    std::fs::remove_dir(dir).unwrap();
    assert_eq!(hazard.status.code(), Some(1), "{}", stdout(&hazard));
    assert!(has_finding(&stdout(&hazard), "static_mut", "lib.rs", 1));
}

#[test]
fn missing_benchmark_uncertainty_is_an_error_not_an_improvement() {
    let dir = std::env::temp_dir().join(format!("rust-guardrails-missing-ci-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = dir.join("before.json");
    let cand = dir.join("after.json");
    std::fs::write(&base, r#"{"results":[{"command":"work","mean":2}]}"#).unwrap();
    std::fs::write(&cand, r#"{"results":[{"command":"work","mean":1}]}"#).unwrap();
    let o = run(&["bench-compare", "--hyperfine", &base.to_string_lossy(), &cand.to_string_lossy(), "--format", "json"]);
    std::fs::remove_file(base).unwrap();
    std::fs::remove_file(cand).unwrap();
    std::fs::remove_dir(dir).unwrap();
    assert!(!o.status.success());
    assert!(!stdout(&o).contains("improved"));
    assert!(String::from_utf8_lossy(&o.stderr).contains("requires a positive finite mean/stddev"));
}

/// Tiny helpers over the JSON report without a JSON dependency in tests:
/// each finding is emitted as `{"rule":"...","category":...,"file":"...","line":N,...}`.
fn has_finding(json: &str, rule: &str, file: &str, line: usize) -> bool {
    json.contains(&format!("{{\"rule\":\"{rule}\","))
        && json.split("{\"rule\":").any(|chunk| chunk.starts_with(&format!("\"{rule}\"")) && chunk.contains(&format!("\"file\":\"{file}\",\"line\":{line},")))
}

#[test]
#[cfg_attr(not(feature = "ast"), ignore = "needs the ast feature")]
fn report_json_on_fixture() {
    let o = run(&["report", &sample(), "--hot", "src/hot*", "--format", "json", "--plan-limit", "50"]);
    assert!(o.status.success(), "stderr: {}", String::from_utf8_lossy(&o.stderr));
    let j = stdout(&o);
    assert!(j.starts_with("{\"schema_version\":2,"));
    assert!(j.contains("\"engine\":\"ast\""), "AST engine is the default build");
    for (rule, file, line) in [
        ("rc_refcell", "src/lib.rs", 11),
        ("arc_mutex", "src/lib.rs", 12),
        ("clone_in_loop", "src/lib.rs", 24),
        ("allocation_in_loop", "src/lib.rs", 25),
        ("unbounded_channel", "src/lib.rs", 31),
        ("clone_in_loop", "src/lib.rs", 33),
        ("spawn_in_loop", "src/lib.rs", 34),
        ("blocking_in_async", "src/lib.rs", 39),
        ("unsafe_without_safety_comment", "src/lib.rs", 43),
        ("unfinished_code", "src/lib.rs", 47),
        ("allocation_in_loop", "src/hot.rs", 6),
    ] {
        assert!(has_finding(&j, rule, file, line), "missing {rule} at {file}:{line}\n{j}");
    }
    // Decoys in strings/comments (lines 16-19) and the #[cfg(test)] module (line 51+) are ignored.
    for line in [17, 18, 55, 56] {
        assert!(!j.contains(&format!("\"file\":\"src/lib.rs\",\"line\":{line},")), "false positive at line {line}");
    }
    // `names.to_vec()` is the for-loop iterator expression: evaluated once, not per iteration.
    assert!(!has_finding(&j, "allocation_in_loop", "src/lib.rs", 23));
    // Metadata ran read-only on the fixture (it has a lockfile and no dependencies).
    assert!(j.contains("{\"name\":\"metadata\",\"status\":\"ran\""), "{j}");
    assert!(j.contains("{\"name\":\"clippy\",\"status\":\"skipped\""));
    // Hot file is marked and every plan item is verifiable.
    assert!(j.contains("\"file\":\"src/hot.rs\",\"line\":6,\"hot\":true"));
    assert!(j.contains("\"plan\":[{\"id\":\"R1\""));
    assert!(!j.contains("\"verify\":[]"));
    assert!(j.contains("\"done_when\":"));
}

#[test]
fn heuristic_engine_is_available_and_coarser() {
    let o = run(&["audit", &sample(), "--engine", "heuristic", "--format", "json"]);
    assert!(o.status.success());
    let j = stdout(&o);
    assert!(j.starts_with("{\"schema_version\":1,") && j.contains("\"engine\":\"heuristic\""));
    // No loop awareness: the heuristic only knows `clone_call`, including test code.
    assert!(j.contains("\"rule\":\"clone_call\",\"file\":\"src/lib.rs\",\"line\":55"));
    assert!(!j.contains("clone_in_loop"));
}

#[test]
fn audit_and_triage_keep_legacy_shape() {
    let a = stdout(&run(&["audit", &sample(), "--format", "json"]));
    for key in ["\"files_scanned\":2", "\"by_rule\":", "\"by_file_rule\":", "\"priority\":\"review\""] {
        assert!(a.contains(key), "{key} missing in {a}");
    }
    let t = stdout(&run(&["triage", &sample(), "--format", "json"]));
    assert!(t.contains("\"inspection_queue\":\"review-safety-boundary\""), "{t}");
}

#[test]
fn markdown_report_has_plan_sections() {
    let o = run(&["report", &sample(), "--format", "markdown", "--no-metadata"]);
    assert!(o.status.success());
    let md = stdout(&o);
    for s in [
        "# Rust audit report",
        "## What was checked",
        "## Summary by dimension",
        "## Prioritized refactoring plan",
        "| Keep as is |",
        "**Verify:**",
        "**Done when:**",
        "## Limitations",
    ] {
        assert!(md.contains(s), "missing {s}");
    }
    assert!(md.contains("| metadata | skipped | --no-metadata |"));
}

#[test]
#[cfg_attr(not(feature = "ast"), ignore = "needs the ast feature")]
fn fail_on_sets_exit_code() {
    let o = run(&["report", &sample(), "--no-metadata", "--fail-on", "high"]);
    assert_eq!(o.status.code(), Some(1), "fixture has high findings");
    let o = run(&["report", &sample(), "--no-metadata", "--fail-on", "blocker"]);
    assert_eq!(o.status.code(), Some(0), "pattern matches never become blockers");
}

#[test]
fn bench_compare_criterion() {
    let dir = fixtures().join("criterion");
    let dir = dir.to_string_lossy();
    let o = run(&["bench-compare", "--criterion", &dir, "--baseline", "before", "--candidate", "after", "--format", "json"]);
    assert_eq!(o.status.code(), Some(0), "no budget → no failure");
    let j = stdout(&o);
    assert!(j.contains("{\"name\":\"parse\",\"verdict\":\"improved\",\"change_pct\":-20,"), "{j}");
    assert!(j.contains("{\"name\":\"render\",\"verdict\":\"regressed\",\"change_pct\":20,"), "{j}");
    assert!(j.contains("{\"name\":\"group/encode\",\"verdict\":\"inconclusive\""), "{j}");
    let o = run(&["bench-compare", "--criterion", &dir, "--baseline", "before", "--candidate", "after", "--max-regression", "10"]);
    assert_eq!(o.status.code(), Some(1), "render regressed 20% > 10% budget");
    assert!(stdout(&o).contains("OVER-BUDGET render"));
}

#[test]
fn bench_compare_hyperfine_pairs_single_commands() {
    let (b, c) = (fixtures().join("hyperfine-before.json"), fixtures().join("hyperfine-after.json"));
    let o = run(&["bench-compare", "--hyperfine", &b.to_string_lossy(), &c.to_string_lossy()]);
    assert!(o.status.success());
    let s = stdout(&o);
    assert!(s.contains("improved") && s.contains("./app-old input.txt → ./app-new input.txt") && s.contains("(-25.0%)"), "{s}");
}

#[test]
fn report_includes_llvm_lines_and_bench() {
    let llvm = fixtures().join("llvm-lines.txt");
    let crit = fixtures().join("criterion");
    let o = run(&[
        "report",
        &sample(),
        "--no-metadata",
        "--llvm-lines-file",
        &llvm.to_string_lossy(),
        "--criterion",
        &crit.to_string_lossy(),
        "--baseline",
        "before",
        "--candidate",
        "after",
        "--max-regression",
        "10",
        "--format",
        "json",
    ]);
    let j = stdout(&o);
    assert!(j.contains("total 30737 IR lines"), "{j}");
    assert!(j.contains("`core::ptr::drop_in_place<T>` generates 9.4% of LLVM IR"));
    assert!(!j.contains("sample::tiny"), "rows below 1% are not reported");
    assert!(j.contains("\"rule\":\"bench_regression\",\"category\":\"performance\",\"severity\":\"blocker\""), "over budget → blocker");
    assert!(j.contains("\"plan\":[{\"id\":\"R1\",\"priority\":\"P0\",\"score\":"), "{j}");
}

#[test]
fn explain_and_errors() {
    let o = run(&["explain", "spawn_in_loop"]);
    assert!(o.status.success() && stdout(&o).contains("cap admission before spawning"));
    assert!(stdout(&run(&["explain"])).contains("unsafe_without_safety_comment"));
    assert_eq!(run(&["explain", "nope"]).status.code(), Some(2));
    assert_eq!(run(&["report", "does/not/exist"]).status.code(), Some(2));
    assert_eq!(run(&["audit", ".", "--profile", "web"]).status.code(), Some(2));
}

#[test]
#[cfg(feature = "ast")]
#[ignore = "builds the fixture with cargo clippy"]
fn clippy_corroborates_syntactic_findings() {
    let o = run(&["report", &sample(), "--with", "clippy", "--format", "json", "--plan-limit", "50"]);
    let j = stdout(&o);
    assert!(j.contains("{\"name\":\"clippy\",\"status\":\"ran\""), "{j}");
    // The syntactic unsafe finding absorbed clippy's lint at the same line.
    assert!(j.contains("[documentation convention confirmed by clippy::undocumented_unsafe_blocks; soundness is unverified]"), "{j}");
    assert!(!has_finding(&j, "clippy::undocumented_unsafe_blocks", "src/lib.rs", 43));
    assert!(j.contains("\"rule\":\"clippy::unused_async\""));
    let _ = std::fs::remove_dir_all(fixtures().join("sample").join("target"));
}

// ---- regressions from the independent review ----

fn layout() -> String {
    fixtures().join("layout").to_string_lossy().into_owned()
}

#[test]
#[cfg_attr(not(feature = "ast"), ignore = "needs the ast feature")]
fn test_files_and_build_dirs() {
    let j = stdout(&run(&["report", &layout(), "--no-metadata", "--format", "json"]));
    assert!(has_finding(&j, "unwrap_in_non_test", "src/lib.rs", 7), "{j}");
    assert!(has_finding(&j, "unfinished_code", "src/build/mod.rs", 3), "src/build is source, not a build dir: {j}");
    assert!(!j.contains("src/tests.rs"), "file of `#[cfg(test)] mod tests;` is test code: {j}");
    assert!(!j.contains("tests/it.rs") && !j.contains("gen.rs"), "{j}");
    assert!(j.contains("2 test file(s) skipped") && j.contains("skipped dirs: target"), "{j}");
    let all = stdout(&run(&["report", &layout(), "--no-metadata", "--include-tests", "--format", "json"]));
    assert!(has_finding(&all, "unwrap_in_non_test", "tests/it.rs", 7), "{all}");
}

#[test]
fn deep_nesting_does_not_overflow_the_stack() {
    let dir = std::env::temp_dir().join(format!("rust-guardrails-deep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sum = (0..3000).map(|_| "x").collect::<Vec<_>>().join(" + ");
    let chain = ".trim().to_string()".repeat(3000);
    std::fs::write(dir.join("gen.rs"), format!("fn a(x: u64) -> u64 {{ 0 + {sum} }}\nfn b(s: &str) -> String {{ s{chain} }}\n")).unwrap();
    let o = run(&["report", &dir.to_string_lossy(), "--no-metadata", "--format", "json"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(o.status.success(), "exit {:?}: {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
    assert!(stdout(&o).contains("\"files_scanned\":1"));
}

#[test]
fn misspelled_benchmark_names_are_errors() {
    let dir = fixtures().join("criterion");
    let dir = dir.to_string_lossy();
    let o = run(&["bench-compare", "--criterion", &dir, "--baseline", "before", "--candidate", "aftr", "--max-regression", "5"]);
    assert_eq!(o.status.code(), Some(2), "typo must not pass a gate");
    assert!(String::from_utf8_lossy(&o.stderr).contains("no benchmark has both"));
    let o = run(&["report", &sample(), "--no-metadata", "--criterion", &dir, "--baseline", "bfore", "--fail-on", "blocker"]);
    assert_eq!(o.status.code(), Some(2), "report with unreadable benchmark input fails");
}

#[test]
#[cfg(feature = "ast")]
#[ignore = "builds a temporary project with cargo clippy"]
fn clippy_never_writes_the_lockfile_and_caps_deny_lints() {
    let dir = std::env::temp_dir().join(format!("rust-guardrails-nolock-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"nolock\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n").unwrap();
    std::fs::write(dir.join("src/lib.rs"), "#![deny(warnings)]\npub fn f(p: *const u8) -> u8 { unsafe { *p } }\n").unwrap();
    let root = dir.to_string_lossy().into_owned();
    let o = run(&["report", &root, "--with", "clippy", "--format", "json"]);
    let j = stdout(&o);
    assert!(!dir.join("Cargo.lock").exists(), "audit must not create Cargo.lock");
    assert!(j.contains("{\"name\":\"clippy\",\"status\":\"failed\"") && j.contains("never writes it"), "{j}");
    // With a lockfile present, deny(warnings) + audit lints must not produce a blocker.
    let g = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())).args(["generate-lockfile", "--offline"]).current_dir(&dir).status().unwrap();
    assert!(g.success());
    let j = stdout(&run(&["report", &root, "--with", "clippy", "--format", "json"]));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(j.contains("{\"name\":\"clippy\",\"status\":\"ran\",\"detail\":\"") && j.contains("build succeeded"), "{j}");
    assert!(!j.contains("\"severity\":\"blocker\""), "{j}");
    assert!(j.contains("[documentation convention confirmed by clippy::undocumented_unsafe_blocks; soundness is unverified]"), "{j}");
}
