//! Consumer-visible project profile behavior; no source-text assertions.
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Project(PathBuf);
impl Project {
    fn new(config: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("rust-rails-config-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "pub fn snapshot(s: &String) -> String { s.clone() }\n").unwrap();
        std::fs::write(dir.join(".rust-rails.toml"), config).unwrap();
        Self(dir)
    }
    fn run(&self, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rust-guardrails")).args(["report", &self.0.to_string_lossy(), "--no-metadata", "--format", "json"]).args(extra).output().unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn discovered_profile_controls_ci_and_cli_overrides_it() {
    let p = Project::new("schema_version = 1\nprofile = 'library'\n[ci]\nfail_on = 'info'\n");
    let o = p.run(&[]);
    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    assert!(out(&o).contains("\"profile\":\"library\""));
    let o = p.run(&["--profile", "prototype", "--fail-on", "blocker"]);
    assert!(o.status.success(), "{}", out(&o));
    assert!(out(&o).contains("\"profile\":\"prototype\""));
}

#[test]
fn configuration_exclusions_apply_and_explicit_config_is_used() {
    let p = Project::new("schema_version = 1\n[scan]\nexclude = ['src/**']\n");
    let o = p.run(&[]);
    assert!(o.status.success());
    assert!(out(&o).contains("\"files_scanned\":0"), "{}", out(&o));
    let alternate = p.0.join("alternate.toml");
    std::fs::write(&alternate, "schema_version = 1\nprofile = 'engine'\n").unwrap();
    let o = p.run(&["--config", &alternate.to_string_lossy()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out(&o).contains("\"profile\":\"engine\""));
    assert!(out(&o).contains("\"files_scanned\":1"));
}

#[test]
fn malformed_unknown_and_unsupported_profiles_fail_closed() {
    for config in [
        "schema_version = 2",
        "schema_version = 1\n[ci]\nfial_on = 'high'",
        "schema_version = 1\nprofile = 'other'",
        "schema_version = 1\n[scan]\nmax_fn_lines = 0",
        "schema_version = 1\nprofile = 'engine'\nprofile = 'service'",
        "schema_version = 1\n[contracts]\nedition = '2030'",
        "schema_version = 1\n[scan]\nhot = ['src/**', 1]",
    ] {
        let p = Project::new(config);
        let o = p.run(&[]);
        assert_eq!(o.status.code(), Some(2), "accepted {config}: {}", out(&o));
    }
    let p = Project::new("schema_version = 1");
    assert_eq!(p.run(&["--config", "missing-rust-rails.toml"]).status.code(), Some(2));
}

#[test]
fn descriptive_contracts_are_visible_without_claiming_enforcement() {
    let p = Project::new("schema_version = 1\n[contracts]\nedition = '2021'\nmsrv = '1.85'\ncritical_paths = ['src/**']\nunsafe_policy = 'reviewed boundaries only'\nallocation_budget = 'zero per simulation tick'\ndeterminism = 'identical replay digest'\n");
    let o = p.run(&[]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let j = out(&o);
    assert!(j.contains("zero per simulation tick") && j.contains("not automatically enforced"), "{j}");
}

#[test]
fn justified_exceptions_expire_and_do_not_hide_findings() {
    let config = |expires: &str| {
        format!("schema_version = 1\n[ci]\nfail_on = 'info'\n[[exceptions]]\nrule = 'clone_call'\npath = 'src/lib.rs'\nmax_severity = 'info'\nreason = 'owned snapshot outlives borrowed source'\nowner = 'runtime-team'\nexpires = '{expires}'\n")
    };
    let p = Project::new(&config("9999-12-31"));
    let o = p.run(&[]);
    assert!(o.status.success(), "{}", out(&o));
    assert!(out(&o).contains("clone_call") && out(&o).contains("owned snapshot outlives borrowed source"), "{}", out(&o));
    std::fs::write(p.0.join(".rust-rails.toml"), config("2000-01-01")).unwrap();
    let o = p.run(&[]);
    assert_eq!(o.status.code(), Some(1), "expired exception must not waive gate: {}", out(&o));
}
