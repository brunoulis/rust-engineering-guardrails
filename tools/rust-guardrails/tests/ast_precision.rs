#![cfg(feature = "ast")]

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("rust-guardrails-ast-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&dir).unwrap();
        for (path, text) in files {
            let path = dir.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Self(dir)
    }
    fn report(&self, include_tests: bool) -> String {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rust-guardrails"));
        cmd.args(["report", self.0.to_str().unwrap(), "--no-metadata", "--format", "json"]);
        if include_tests {
            cmd.arg("--include-tests");
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn finding(json: &str, rule: &str, file: &str) -> bool {
    json.split("{\"rule\":").any(|chunk| chunk.starts_with(&format!("\"{rule}\",")) && chunk.contains(&format!("\"file\":\"{file}\",")))
}

#[test]
fn path_test_modules_and_transitive_helpers_are_excluded() {
    let f = Fixture::new(&[
        ("src/lib.rs", "#[cfg(test)] #[path = \"checks.rs\"] mod tests;"),
        ("src/checks.rs", "mod helper; fn f() { None::<u8>.unwrap(); }"),
        ("src/helper.rs", "fn helper() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(!finding(&j, "unwrap_in_non_test", "src/checks.rs"), "{j}");
    assert!(!finding(&j, "unwrap_in_non_test", "src/helper.rs"), "{j}");
    let j = f.report(true);
    assert!(finding(&j, "unwrap_in_non_test", "src/checks.rs"), "{j}");
    assert!(finding(&j, "unwrap_in_non_test", "src/helper.rs"), "{j}");
}

#[test]
fn nested_inline_test_modules_resolve_paths_and_preserve_shared_production_files() {
    let f = Fixture::new(&[
        ("src/lib.rs", "mod outer { #[cfg(test)] mod checks { #[path = \"helper.rs\"] mod child; } } #[path = \"outer/checks/shared.rs\"] mod production;"),
        ("src/outer/checks/helper.rs", "#[path = \"shared.rs\"] mod shared; mod nested; fn f() { None::<u8>.unwrap(); }"),
        ("src/outer/checks/nested.rs", "fn f() { None::<u8>.unwrap(); }"),
        ("src/outer/checks/shared.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(!finding(&j, "unwrap_in_non_test", "src/outer/checks/helper.rs"), "{j}");
    assert!(!finding(&j, "unwrap_in_non_test", "src/outer/checks/nested.rs"), "{j}");
    assert!(finding(&j, "unwrap_in_non_test", "src/outer/checks/shared.rs"), "{j}");
}

#[test]
fn include_tests_restores_inline_functions_and_file_cfg() {
    let f =
        Fixture::new(&[("lib.rs", "#[cfg(test)] mod checks { #[test] fn f() { None::<u8>.unwrap(); } }"), ("test_support.rs", "#![cfg(test)] fn f() { None::<u8>.unwrap(); }")]);
    let j = f.report(true);
    assert!(finding(&j, "unwrap_in_non_test", "lib.rs"), "{j}");
    assert!(finding(&j, "unwrap_in_non_test", "test_support.rs"), "{j}");
}

#[test]
fn conditional_production_modules_remain_in_scope() {
    let f = Fixture::new(&[("lib.rs", "#[cfg(any(test, feature = \"x\"))] #[path = \"shared.rs\"] mod shared;"), ("shared.rs", "fn f() { None::<u8>.unwrap(); }")]);
    assert!(finding(&f.report(false), "unwrap_in_non_test", "shared.rs"));
}

#[test]
fn test_only_statements_expressions_and_match_arms_are_excluded() {
    let f = Fixture::new(&[(
        "lib.rs",
        r#"
fn live() {
    #[cfg(test)]
    probe(unsafe { std::mem::transmute::<u32, f32>(0) });
    #[cfg(all(test, feature = "probe"))]
    let _value = None::<u8>.unwrap();
    #[cfg(test)]
    panic!("test macro");
    #[cfg(test)]
    unsafe { std::mem::transmute::<u32, f32>(0); }
    match 0 {
        #[cfg(test)] 1 => { None::<u8>.unwrap(); },
        _ => {},
    }
}
"#,
    )]);
    let default = f.report(false);
    for rule in ["unsafe_without_safety_comment", "transmute", "unwrap_in_non_test", "panic_in_library"] {
        assert!(!finding(&default, rule, "lib.rs"), "{rule}: {default}");
        assert!(finding(&f.report(true), rule, "lib.rs"), "--include-tests must restore {rule}");
    }
}

#[test]
fn cfg_test_items_and_associated_items_are_excluded() {
    let f = Fixture::new(&[(
        "lib.rs",
        r#"
#[cfg(test)] static mut TEST_ONLY: u8 = 0;
#[cfg(test)] const TEST_VALUE: u8 = None::<u8>.unwrap();
#[cfg(test)] enum TestEnum { Value = { None::<u8>.unwrap() } }
#[cfg(test)] struct TestType { value: std::rc::Rc<std::cell::RefCell<u8>> }
trait LiveTrait {
    #[cfg(test)] fn test_default() { None::<u8>.unwrap(); }
    #[cfg(test)] const TEST_ASSOC: u8 = None::<u8>.unwrap();
}
struct Live;
impl Live {
    #[cfg(test)] const TEST_ASSOC: u8 = None::<u8>.unwrap();
}
"#,
    )]);
    let default = f.report(false);
    for rule in ["static_mut", "unwrap_in_non_test", "rc_refcell"] {
        assert!(!finding(&default, rule, "lib.rs"), "{rule}: {default}");
    }
    let tests = f.report(true);
    assert!(finding(&tests, "static_mut", "lib.rs"), "{tests}");
    assert!(finding(&tests, "unwrap_in_non_test", "lib.rs"), "{tests}");
    assert!(finding(&tests, "rc_refcell", "lib.rs"), "{tests}");
}

#[test]
fn production_capable_cfg_statements_and_following_code_remain_visible() {
    let f = Fixture::new(&[(
        "lib.rs",
        r#"
fn live() {
    #[cfg(test)] let _ = None::<u8>.unwrap();
    #[cfg(not(test))] let _ = None::<u8>.unwrap();
    #[cfg(any(test, feature = "live"))] let _ = None::<u8>.unwrap();
    #[cfg_attr(feature = "test_mode", cfg(test))] let _ = None::<u8>.unwrap();
    unsafe { std::mem::transmute::<u32, f32>(0); }
}
"#,
    )]);
    let default = f.report(false);
    let findings = default.split("\"findings\":[").nth(1).unwrap();
    assert_eq!(findings.matches("\"rule\":\"unwrap_in_non_test\"").count(), 3, "{default}");
    assert!(finding(&default, "transmute", "lib.rs"), "{default}");
    assert!(finding(&default, "unsafe_without_safety_comment", "lib.rs"), "{default}");
}

#[test]
fn test_only_function_and_statement_modules_propagate_file_scope() {
    let f = Fixture::new(&[
        (
            "src/lib.rs",
            r#"
#[cfg(test)] fn checks() { #[path = "fn_support.rs"] mod support; }
fn live() {
    #[cfg(test)] { #[path = "block_support.rs"] mod support; }
}
#[path = "shared.rs"] mod production;
#[cfg(test)] fn shared_checks() { #[path = "shared.rs"] mod support; }
"#,
        ),
        ("src/fn_support.rs", "fn check() { None::<u8>.unwrap(); }"),
        ("src/block_support.rs", "fn check() { None::<u8>.unwrap(); }"),
        ("src/shared.rs", "fn live() { None::<u8>.unwrap(); }"),
    ]);
    let default = f.report(false);
    for file in ["src/fn_support.rs", "src/block_support.rs"] {
        assert!(!finding(&default, "unwrap_in_non_test", file), "{file}: {default}");
        assert!(finding(&f.report(true), "unwrap_in_non_test", file), "{file}");
    }
    assert!(finding(&default, "unwrap_in_non_test", "src/shared.rs"), "{default}");
}

#[test]
fn test_only_variants_and_struct_literal_fields_are_excluded() {
    let f = Fixture::new(&[(
        "lib.rs",
        r#"
enum LiveEnum { #[cfg(test)] Test = { None::<u8>.unwrap() }, Live = 0 }
struct Live { value: u8 }
fn live() { let _ = Live { #[cfg(test)] value: None::<u8>.unwrap() }; }
"#,
    )]);
    assert!(!finding(&f.report(false), "unwrap_in_non_test", "lib.rs"));
    assert!(finding(&f.report(true), "unwrap_in_non_test", "lib.rs"));
}

#[test]
fn test_only_function_and_generic_parameters_are_excluded() {
    let f = Fixture::new(&[(
        "lib.rs",
        r#"
fn live(#[cfg(test)] value: std::rc::Rc<std::cell::RefCell<u8>>) {}
struct Generic<#[cfg(test)] T = std::rc::Rc<std::cell::RefCell<u8>>> { value: u8 }
"#,
    )]);
    assert!(!finding(&f.report(false), "rc_refcell", "lib.rs"));
    let included = f.report(true);
    let findings = included.split("\"findings\":[").nth(1).unwrap();
    assert_eq!(findings.matches("\"rule\":\"rc_refcell\"").count(), 2, "{included}");
}

#[test]
fn commas_in_cfg_string_values_do_not_hide_production_code() {
    let f = Fixture::new(&[(
        "lib.rs",
        r#"
#[cfg(all(build_flavor = "live,test,native"))] static mut LIVE: u8 = 0;
#[cfg(all(build_flavor = "live, test ,native"))] fn live() { None::<u8>.unwrap(); }
"#,
    )]);
    let report = f.report(false);
    assert!(finding(&report, "static_mut", "lib.rs"), "{report}");
    assert!(finding(&report, "unwrap_in_non_test", "lib.rs"), "{report}");
}

#[test]
fn ordinary_module_children_and_path_attributes_inside_non_root_files() {
    let f = Fixture::new(&[
        ("src/lib.rs", "mod feature;"),
        ("src/feature.rs", "#[cfg(test)] mod checks; mod nested { #[cfg(test)] #[path = \"fixture.rs\"] mod support; }"),
        ("src/feature/checks.rs", "mod helper; fn f() { None::<u8>.unwrap(); }"),
        ("src/feature/checks/helper.rs", "fn f() { None::<u8>.unwrap(); }"),
        ("src/feature/nested/fixture.rs", "fn f() { None::<u8>.unwrap(); }"),
        ("src/feature/helper.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    for path in ["src/feature/checks.rs", "src/feature/checks/helper.rs", "src/feature/nested/fixture.rs"] {
        assert!(!finding(&j, "unwrap_in_non_test", path), "{path}: {j}");
    }
    assert!(finding(&j, "unwrap_in_non_test", "src/feature/helper.rs"), "{j}");
}

#[test]
fn nested_lib_basename_is_not_a_production_target() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"checks/lib.rs\"] mod tests;"),
        ("src/checks/lib.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(!finding(&j, "unwrap_in_non_test", "src/checks/lib.rs"), "{j}");
}

#[test]
fn cargo_default_bin_remains_production_when_reused_by_tests() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"bin/worker.rs\"] mod tests;"),
        ("src/bin/worker.rs", "mod helper; fn main() { None::<u8>.unwrap(); }"),
        ("src/bin/helper.rs", "fn helper() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(finding(&j, "unwrap_in_non_test", "src/bin/worker.rs"), "{j}");
    assert!(finding(&j, "unwrap_in_non_test", "src/bin/helper.rs"), "{j}");
}

#[test]
fn cargo_custom_target_and_its_children_remain_production() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n[lib]\npath = 'custom/entry.rs'\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"../custom/entry.rs\"] mod tests;"),
        ("custom/entry.rs", "mod helper; fn f() { None::<u8>.unwrap(); }"),
        ("custom/helper.rs", "fn helper() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(finding(&j, "unwrap_in_non_test", "custom/entry.rs"), "{j}");
    assert!(finding(&j, "unwrap_in_non_test", "custom/helper.rs"), "{j}");
}

#[test]
fn unsupported_custom_target_syntax_retains_sources_and_reports_uncertainty() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n[lib]\npath = '''custom/entry.rs'''\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"../custom/entry.rs\"] mod tests;"),
        ("custom/entry.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(finding(&j, "unwrap_in_non_test", "custom/entry.rs"), "{j}");
    assert!(j.contains("unsupported Cargo target syntax"), "{j}");
    assert!(j.contains("target identity is unverified"), "{j}");
}

#[test]
fn disabled_cargo_bin_discovery_does_not_protect_test_sources() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\nautobins = false\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"bin/worker.rs\"] mod tests;"),
        ("src/bin/worker.rs", "fn main() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(!finding(&j, "unwrap_in_non_test", "src/bin/worker.rs"), "{j}");
}

#[test]
fn explicit_target_outside_package_directory_is_preserved_inside_scan_root() {
    let f = Fixture::new(&[
        ("pkg/Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n[lib]\npath = '../shared.rs'\n"),
        ("pkg/src/lib.rs", "#[cfg(test)] #[path = \"../../shared.rs\"] mod tests;"),
        ("shared.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(finding(&j, "unwrap_in_non_test", "shared.rs"), "{j}");
}

#[test]
fn cargo_build_script_roots_and_children_are_preserved() {
    for (setting, script) in [("build = 'custom.rs'", "custom.rs"), ("", "build.rs")] {
        let manifest = format!("[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n{setting}\n");
        let lib = format!("#[cfg(test)] #[path = \"../{script}\"] mod tests;");
        let f = Fixture::new(&[
            ("Cargo.toml", &manifest),
            ("src/lib.rs", &lib),
            (script, "mod support; fn main() { None::<u8>.unwrap(); }"),
            ("support.rs", "fn f() { None::<u8>.unwrap(); }"),
        ]);
        let j = f.report(false);
        assert!(finding(&j, "unwrap_in_non_test", script), "{script}: {j}");
        assert!(finding(&j, "unwrap_in_non_test", "support.rs"), "{script}: {j}");
    }
}

#[test]
fn disabled_build_script_does_not_protect_test_sources() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\nbuild = false\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"../build.rs\"] mod tests;"),
        ("build.rs", "mod support; fn main() { None::<u8>.unwrap(); }"),
        ("support.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(!finding(&j, "unwrap_in_non_test", "build.rs"), "{j}");
    assert!(!finding(&j, "unwrap_in_non_test", "support.rs"), "{j}");
}

#[test]
fn default_and_custom_examples_remain_production_roots() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\n[[example]]\nname = 'custom'\npath = 'custom/entry.rs'\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"../examples/demo.rs\"] mod demo; #[cfg(test)] #[path = \"../examples/nested/main.rs\"] mod nested; #[cfg(test)] #[path = \"../custom/entry.rs\"] mod custom;"),
        ("examples/demo.rs", "mod support; fn main() { None::<u8>.unwrap(); }"),
        ("examples/support.rs", "fn f() { None::<u8>.unwrap(); }"),
        ("examples/nested/main.rs", "fn main() { None::<u8>.unwrap(); }"),
        ("custom/entry.rs", "mod helper; fn main() { None::<u8>.unwrap(); }"),
        ("custom/helper.rs", "fn f() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    for path in ["custom/entry.rs", "custom/helper.rs", "examples/demo.rs", "examples/support.rs", "examples/nested/main.rs"] {
        assert!(finding(&j, "unwrap_in_non_test", path), "{path}: {j}");
    }
}

#[test]
fn disabled_autoexamples_does_not_protect_test_sources() {
    let f = Fixture::new(&[
        ("Cargo.toml", "[package]\nname = 'root_context'\nversion = '0.1.0'\nedition = '2021'\nautoexamples = false\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"../examples/demo.rs\"] mod tests;"),
        ("examples/demo.rs", "fn main() { None::<u8>.unwrap(); }"),
    ]);
    let j = f.report(false);
    assert!(!finding(&j, "unwrap_in_non_test", "examples/demo.rs"), "{j}");
}

#[test]
fn safety_prose_and_marker_only_are_unverified_review_signals() {
    let f = Fixture::new(&[("lib.rs", "fn f() {\n// Zero-initialize because this C POD contains only integers and pointers.\nlet x: Pod = unsafe { std::mem::zeroed() };\n// SAFETY:\nlet y = unsafe { *p };\n// SAFETY: p is valid for reads.\nlet z = unsafe { *p };\n}")]);
    let j = f.report(false);
    assert!(!finding(&j, "unsafe_without_safety_comment", "lib.rs"), "{j}");
    assert!(finding(&j, "unsafe_safety_comment_review", "lib.rs"), "{j}");
    let findings = j.split("\"findings\":[").nth(1).unwrap();
    assert_eq!(findings.matches("\"rule\":\"unsafe_safety_comment_review\"").count(), 2, "{j}");
    assert!(finding(&j, "unsafe_block", "lib.rs"), "{j}");
    assert!(j.contains("unverified"), "{j}");
}

#[test]
fn unrelated_strings_and_previous_statements_do_not_document_unsafe() {
    let f = Fixture::new(&[("lib.rs", "fn f() { let label = \"SAFETY: p valid\"; let x = unsafe { *p };\n// SAFETY: prior statement explanation.\nlet value = 1;\nlet y = unsafe { *p };\nlet raw = r#\"\n// SAFETY: text inside a raw string.\n\"#;\nlet z = unsafe { *p };\n}")]);
    let j = f.report(false);
    let findings = j.split("\"findings\":[").nth(1).unwrap();
    assert_eq!(findings.matches("\"rule\":\"unsafe_without_safety_comment\"").count(), 3, "{j}");
}
