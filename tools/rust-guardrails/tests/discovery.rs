//! Nested Cargo projects must use the same scope as the source scan.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../src/json.rs"]
#[allow(dead_code)]
mod json;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("rust-guardrails-discovery-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn write(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn package(&self, dir: &str, name: &str) {
        self.write(&format!("{dir}/Cargo.toml"), &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"));
        self.write(&format!("{dir}/src/lib.rs"), "pub fn read() {\n    let _ = Some(1).unwrap().clone();\n}\n");
    }
    fn report(&self, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rust-guardrails")).arg("report").arg(&self.0).args(["--format", "json"]).args(extra).output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the uniquely allocated test directory is removed.
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn value(output: &Output) -> json::Value {
    json::parse(&String::from_utf8_lossy(&output.stdout)).unwrap_or_else(|e| panic!("{e}; stderr: {}", String::from_utf8_lossy(&output.stderr)))
}
fn runs(v: &json::Value, name: &str) -> Vec<(String, String)> {
    v.get("analyzers").unwrap().as_array().iter().filter(|a| a.str_of("name") == name).map(|a| (a.str_of("status").into(), a.str_of("detail").into())).collect()
}
fn summaries(v: &json::Value) -> Vec<String> {
    v.get("findings").unwrap().as_array().iter().filter(|f| f.str_of("rule") == "workspace_summary").map(|f| f.str_of("file").to_string()).collect()
}

#[test]
fn independent_nested_crates_have_root_relative_provenance_without_builds() {
    let f = Fixture::new();
    f.package("tools/alpha", "discovery_alpha");
    f.package("native/beta", "discovery_beta");
    let o = f.report(&[]);
    assert!(o.status.success());
    let v = value(&o);
    let mut locations = summaries(&v);
    locations.sort();
    assert_eq!(locations, ["native/beta/Cargo.toml", "tools/alpha/Cargo.toml"]);
    let metadata = runs(&v, "metadata");
    assert_eq!(metadata.len(), 2);
    assert!(metadata.iter().all(|(status, detail)| status == "ran" && detail.contains("Cargo.toml")));
    assert!(v.get("findings").unwrap().as_array().iter().any(|f| f.str_of("file") == "tools/alpha/src/lib.rs"));
    assert!(!f.0.join("tools/alpha/target").exists());
    assert!(!f.0.join("native/beta/target").exists());
    assert!(!f.0.join("tools/alpha/Cargo.lock").exists());
    assert!(!f.0.join("native/beta/Cargo.lock").exists());
}

#[test]
fn cargo_workspace_members_are_analyzed_once() {
    let f = Fixture::new();
    f.write("rust/Cargo.toml", "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"2\"\n");
    f.package("rust/a", "discovery_a");
    f.package("rust/b", "discovery_b");
    f.write(
        "rust/a/Cargo.toml",
        "[package]\nname = \"discovery_a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\ndiscovery_b = { path = \"../b\", version = \"*\" }\n",
    );
    let v = value(&f.report(&[]));
    assert_eq!(summaries(&v), ["rust/Cargo.toml"]);
    assert_eq!(runs(&v, "metadata").len(), 1);
    assert!(v.get("findings").unwrap().as_array().iter().any(|f| f.str_of("rule") == "wildcard_dependency" && f.str_of("file") == "rust/a/Cargo.toml"));
    assert!(!f.0.join("rust/target").exists());
}

#[test]
fn selected_cargo_root_retains_single_project_analysis() {
    let f = Fixture::new();
    f.package(".", "discovery_selected");
    f.package("nested", "discovery_unrelated");
    let v = value(&f.report(&[]));
    assert_eq!(runs(&v, "metadata").len(), 1);
    assert_eq!(summaries(&v), [""]);
    assert!(runs(&v, "metadata")[0].1.contains("manifest Cargo.toml:"));
    assert!(!f.0.join("target").exists());
}

#[test]
fn excluded_manifests_are_not_discovered() {
    let f = Fixture::new();
    f.package("maintained", "discovery_kept");
    f.package("comparison", "discovery_copy");
    let v = value(&f.report(&["--exclude", "comparison/**"]));
    assert_eq!(summaries(&v), ["maintained/Cargo.toml"]);
}

#[test]
fn ignored_manifests_follow_the_explicit_scope_override_and_config() {
    let f = Fixture::new();
    assert!(Command::new("git").arg("init").arg("--quiet").arg(&f.0).status().unwrap().success());
    f.write(".gitignore", "comparison/\n");
    f.package("maintained", "discovery_kept");
    f.package("comparison", "discovery_copy");
    assert_eq!(summaries(&value(&f.report(&[]))), ["maintained/Cargo.toml"]);
    assert_eq!(summaries(&value(&f.report(&["--include-ignored"]))).len(), 2);
    f.write(".rust-rails.toml", "schema_version = 1\n[scan]\ninclude_ignored = true\n");
    assert_eq!(summaries(&value(&f.report(&[]))).len(), 2);
}

#[test]
fn broken_discovered_manifest_keeps_requested_analyzer_gate_failed() {
    let f = Fixture::new();
    f.write("broken/Cargo.toml", "not valid Cargo TOML");
    let o = f.report(&["--with", "clippy", "--fail-on", "blocker"]);
    assert_eq!(o.status.code(), Some(1));
    let v = value(&o);
    assert!(runs(&v, "clippy").iter().any(|(status, detail)| status == "failed" && detail.contains("broken/Cargo.toml")));
    assert!(runs(&v, "metadata").iter().any(|(status, _)| status == "failed"));
}

#[test]
fn inherited_workspace_outside_selected_scope_is_visible_and_not_built() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[workspace]\nmembers = [\"selected/member\"]\nresolver = \"2\"\n");
    f.package("selected/member", "discovery_member");
    let o = Command::new(env!("CARGO_BIN_EXE_rust-guardrails"))
        .arg("report")
        .arg(f.0.join("selected"))
        .args(["--with", "clippy", "--fail-on", "blocker", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let v = value(&o);
    assert!(runs(&v, "metadata").iter().any(|(status, detail)| status == "failed" && detail.contains("outside the selected scope")));
    assert!(runs(&v, "clippy").iter().any(|(status, detail)| status == "failed" && detail.contains("member/Cargo.toml")));
    assert!(!f.0.join("target").exists());
}

#[test]
fn saved_llvm_lines_input_is_read_once_for_multiple_crates() {
    let f = Fixture::new();
    f.package("a", "discovery_a");
    f.package("b", "discovery_b");
    f.write("llvm.txt", "100 (100%) 1 (100%) (TOTAL)\n60 (60%) 1 (100%) example::work\n");
    let input = f.0.join("llvm.txt");
    let v = value(&f.report(&["--llvm-lines-file", input.to_str().unwrap()]));
    assert_eq!(runs(&v, "llvm-lines").len(), 1);
    assert_eq!(v.get("findings").unwrap().as_array().iter().filter(|f| f.str_of("rule") == "llvm_lines_hotspot").count(), 1);
}
