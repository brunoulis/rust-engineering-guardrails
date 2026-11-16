use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../src/json.rs"]
#[allow(dead_code)]
mod json;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);
impl Project {
    fn new(git: bool) -> Self {
        let p = Self(std::env::temp_dir().join(format!("rust-guardrails-scope-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
        std::fs::create_dir_all(&p.0).unwrap();
        if git {
            p.git(&["init", "--quiet"]);
        }
        p
    }
    fn git(&self, args: &[&str]) {
        let out = Command::new("git").arg("-C").arg(&self.0).args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    fn write(&self, path: &str, contents: &str) {
        let target = self.0.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, contents).unwrap();
    }
    fn source(&self, path: &str) {
        self.write(path, "pub fn f(v: String) -> String { v.clone() }\n");
    }
    fn run_at(&self, root: &Path, args: &[&str], missing_git: bool) -> Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_rust-guardrails"));
        c.arg("report").arg(root).args(["--no-metadata", "--format", "json"]).args(args);
        if missing_git {
            c.env("PATH", self.0.join("no-programs"));
        }
        c.output().unwrap()
    }
    fn report(&self, args: &[&str]) -> json::Value {
        parsed(self.run_at(&self.0, args, false))
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn parsed(out: Output) -> json::Value {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    json::parse(&String::from_utf8(out.stdout).unwrap()).unwrap()
}
fn files(report: &json::Value) -> f64 {
    report.get("files_scanned").unwrap().as_f64().unwrap()
}

#[test]
fn git_scope_keeps_untracked_sources_and_tracked_ignored_sources() {
    let p = Project::new(true);
    p.write(".gitignore", "build/\nignored.rs\ntracked.rs\n");
    for file in ["lib.rs", "new.rs", "tracked.rs", "ignored.rs", "build/before/lib.rs"] {
        p.source(file);
    }
    p.git(&["add", "-f", "tracked.rs"]);
    let report = p.report(&[]);
    assert_eq!(files(&report), 3.0, "{}", report.to_json());
    let found = report.get("findings").unwrap().as_array();
    for file in ["lib.rs", "new.rs", "tracked.rs"] {
        assert!(found.iter().any(|f| f.str_of("file") == file));
    }
    assert!(!found.iter().any(|f| f.str_of("file") == "ignored.rs" || f.str_of("file").starts_with("build/")));
    assert_eq!(files(&p.report(&["--include-ignored"])), 5.0);
}

#[test]
fn filesystem_scope_skips_child_repositories_but_retains_source_build_modules() {
    let p = Project::new(false);
    p.source("lib.rs");
    p.source("src/build/mod.rs");
    p.source("copy/src/lib.rs");
    std::fs::create_dir_all(p.0.join("copy/.git")).unwrap();
    let report = p.report(&[]);
    assert_eq!(files(&report), 2.0, "{}", report.to_json());
    assert!(report.get("findings").unwrap().as_array().iter().any(|f| f.str_of("file") == "src/build/mod.rs"));
    assert_eq!(files(&p.report(&["--include-ignored"])), 2.0);
    // An explicitly selected child repository remains auditable.
    assert_eq!(files(&parsed(p.run_at(&p.0.join("copy"), &[], false))), 1.0);
}

#[test]
fn exclusions_prune_the_selected_directory_and_report_it() {
    let p = Project::new(false);
    p.source("lib.rs");
    p.source("snapshots/before/lib.rs");
    let report = p.report(&["--exclude", "snapshots/**"]);
    assert_eq!(files(&report), 1.0);
    let source = report.get("analyzers").unwrap().as_array().iter().find(|a| a.str_of("name") == "source").unwrap();
    assert!(source.str_of("detail").contains("snapshots"), "{}", source.to_json());
}

#[test]
fn missing_git_uses_a_visible_filesystem_fallback() {
    let p = Project::new(true);
    p.write(".gitignore", "ignored.rs\n");
    p.source("lib.rs");
    p.source("ignored.rs");
    let report = parsed(p.run_at(&p.0, &[], true));
    assert_eq!(files(&report), 2.0);
    let source = report.get("analyzers").unwrap().as_array().iter().find(|a| a.str_of("name") == "source").unwrap();
    assert!(source.str_of("detail").to_lowercase().contains("filesystem"), "{}", source.to_json());
}

#[test]
fn git_subdirectory_paths_are_relative_to_the_selected_root() {
    let p = Project::new(true);
    p.source("src/lib.rs");
    p.source("other.rs");
    let report = parsed(p.run_at(&p.0.join("src"), &[], false));
    assert_eq!(files(&report), 1.0);
    assert!(report.get("findings").unwrap().as_array().iter().any(|f| f.str_of("file") == "lib.rs"));
}
