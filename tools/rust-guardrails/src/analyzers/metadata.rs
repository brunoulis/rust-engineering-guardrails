//! `cargo metadata` analysis: workspace layout, internal dependency edges and cycles,
//! duplicate crate versions, catch-all features, build scripts/proc-macros, editions.
//!
//! Runs with `--frozen` so it never touches the network or rewrites `Cargo.lock`;
//! if resolution is impossible it falls back to `--no-deps` and says so.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use super::{cargo, first_line};
use crate::json::{self, Value};
use crate::model::{AnalyzerRun, Confidence, Finding, Severity};
use crate::rules;

pub fn analyze(root: &Path) -> (Vec<Finding>, AnalyzerRun) {
    let full = cargo(root).args(["metadata", "--format-version", "1", "--frozen"]).output();
    let (out, resolved, note) = match full {
        Ok(o) if o.status.success() => (o, true, String::new()),
        Ok(o) => {
            let why = first_line(&String::from_utf8_lossy(&o.stderr));
            match cargo(root).args(["metadata", "--format-version", "1", "--no-deps"]).output() {
                Ok(o2) if o2.status.success() => (o2, false, format!("dependency graph unavailable ({why}); workspace-only checks")),
                Ok(o2) => return (vec![], AnalyzerRun::failed("metadata", first_line(&String::from_utf8_lossy(&o2.stderr)))),
                Err(e) => return (vec![], AnalyzerRun::failed("metadata", e.to_string())),
            }
        }
        Err(e) => return (vec![], AnalyzerRun::failed("metadata", format!("cargo not available: {e}"))),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    match json::parse(&text) {
        Ok(v) => {
            let findings = analyze_value(&v, resolved);
            let detail = if resolved { "resolved dependency graph (--frozen)".to_string() } else { note };
            (findings, AnalyzerRun::ran("metadata", detail))
        }
        Err(e) => (vec![], AnalyzerRun::failed("metadata", format!("invalid cargo metadata JSON: {e}"))),
    }
}

struct Pkg<'a> {
    name: &'a str,
    version: &'a str,
    manifest: String,
    edition: &'a str,
    v: &'a Value,
}

/// Pure analysis of `cargo metadata` JSON; `resolved` is false for `--no-deps` output.
pub fn analyze_value(v: &Value, resolved: bool) -> Vec<Finding> {
    let ws_root = v.str_of("workspace_root").replace('\\', "/");
    let rel = |manifest: &str| {
        let m = manifest.replace('\\', "/");
        m.strip_prefix(&ws_root).map(|s| s.trim_start_matches('/').to_string()).unwrap_or(m)
    };
    let pkgs: BTreeMap<&str, Pkg> = v
        .get("packages")
        .map(Value::as_array)
        .unwrap_or_default()
        .iter()
        .map(|p| (p.str_of("id"), Pkg { name: p.str_of("name"), version: p.str_of("version"), manifest: rel(p.str_of("manifest_path")), edition: p.str_of("edition"), v: p }))
        .collect();
    let members: Vec<&Pkg> = v.get("workspace_members").map(Value::as_array).unwrap_or_default().iter().filter_map(|id| id.as_str().and_then(|id| pkgs.get(id))).collect();
    let member_names: BTreeSet<&str> = members.iter().map(|p| p.name).collect();
    let mut findings = Vec::new();

    // Per-member dependency declarations.
    let mut edges: Vec<(&str, &str, &str)> = Vec::new(); // (from, to, kind)
    for m in &members {
        for d in m.v.get("dependencies").map(Value::as_array).unwrap_or_default() {
            let name = d.str_of("name");
            let kind = d.get("kind").and_then(Value::as_str).unwrap_or("normal");
            if member_names.contains(name) && d.get("path").is_some() {
                edges.push((m.name, member_names.get(name).copied().unwrap_or(name), kind));
            }
            if d.str_of("req") == "*" {
                findings.push(rules::finding("wildcard_dependency", Confidence::Measured).at(&m.manifest, 0).evidence(format!("{} → {name} = \"*\"", m.name)));
            }
            if d.str_of("source").starts_with("git+") && !d.str_of("source").contains("rev=") {
                findings.push(rules::finding("git_dependency", Confidence::Measured).at(&m.manifest, 0).evidence(format!("{} → {name} from {}", m.name, d.str_of("source"))));
            }
            let feats: Vec<&str> = d.get("features").map(Value::as_array).unwrap_or_default().iter().filter_map(Value::as_str).collect();
            if feats.contains(&"full") {
                findings.push(rules::finding("broad_feature", Confidence::Measured).at(&m.manifest, 0).evidence(format!("{} → {name} features = [\"full\"]", m.name)));
            }
        }
    }

    // Cycles among members (only possible through dev-dependencies).
    for cycle in cycles(&member_names, &edges) {
        findings.push(rules::finding("dev_dependency_cycle", Confidence::Measured).evidence(cycle.join(" → ")));
    }

    let editions: BTreeSet<&str> = members.iter().map(|m| m.edition).collect();
    if editions.len() > 1 {
        let detail = members.iter().map(|m| format!("{}={}", m.name, m.edition)).collect::<Vec<_>>().join(", ");
        findings.push(rules::finding("mixed_editions", Confidence::Measured).evidence(detail));
    }

    let mut summary = format!(
        "{} workspace member(s) [{}]; {} internal edge(s) ({} dev)",
        members.len(),
        member_names.iter().copied().collect::<Vec<_>>().join(", "),
        edges.len(),
        edges.iter().filter(|e| e.2 == "dev").count()
    );

    if resolved {
        // Duplicate versions across the whole graph.
        let mut versions: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for p in pkgs.values() {
            versions.entry(p.name).or_default().insert(p.version);
        }
        for (name, vs) in versions.iter().filter(|(_, vs)| vs.len() > 1) {
            let mut f = rules::finding("duplicate_dependency_versions", Confidence::Measured)
                .msg(format!("`{name}` compiled in {} versions", vs.len()))
                .evidence(vs.iter().copied().collect::<Vec<_>>().join(", "));
            if vs.len() >= 3 {
                f.severity = Severity::Medium;
            }
            findings.push(f);
        }
        let (mut build_scripts, mut proc_macros) = (Vec::new(), Vec::new());
        for p in pkgs.values() {
            for t in p.v.get("targets").map(Value::as_array).unwrap_or_default() {
                let kinds: Vec<&str> = t.get("kind").map(Value::as_array).unwrap_or_default().iter().filter_map(Value::as_str).collect();
                if kinds.contains(&"custom-build") {
                    build_scripts.push(p.name);
                }
                if kinds.contains(&"proc-macro") {
                    proc_macros.push(p.name);
                }
            }
        }
        if !build_scripts.is_empty() || !proc_macros.is_empty() {
            build_scripts.sort_unstable();
            build_scripts.dedup();
            proc_macros.sort_unstable();
            proc_macros.dedup();
            findings.push(rules::finding("build_scripts_and_proc_macros", Confidence::Measured).evidence(format!(
                "{} build script(s): {}; {} proc-macro crate(s): {}",
                build_scripts.len(),
                build_scripts.join(", "),
                proc_macros.len(),
                proc_macros.join(", ")
            )));
        }
        let ids: Vec<&str> = v.get("workspace_members").map(Value::as_array).unwrap_or_default().iter().filter_map(Value::as_str).collect();
        let graph = resolve_graph(v);
        let per_member: Vec<String> = ids.iter().map(|id| format!("{}={}", pkgs.get(id).map_or("?", |p| p.name), transitive_count(&graph, id))).collect();
        summary.push_str(&format!("; {} packages in graph; transitive non-dev deps per member: {}", pkgs.len(), per_member.join(", ")));
    }
    findings.push(rules::finding("workspace_summary", Confidence::Measured).evidence(summary));
    findings
}

/// node id → (dep id, is_dev_only)
fn resolve_graph(v: &Value) -> BTreeMap<&str, Vec<(&str, bool)>> {
    let mut g = BTreeMap::new();
    for n in v.path(&["resolve", "nodes"]).map(Value::as_array).unwrap_or_default() {
        let deps = n
            .get("deps")
            .map(Value::as_array)
            .unwrap_or_default()
            .iter()
            .map(|d| {
                let kinds = d.get("dep_kinds").map(Value::as_array).unwrap_or_default();
                let dev_only = !kinds.is_empty() && kinds.iter().all(|k| k.get("kind").and_then(Value::as_str) == Some("dev"));
                (d.str_of("pkg"), dev_only)
            })
            .collect();
        g.insert(n.str_of("id"), deps);
    }
    g
}

fn transitive_count(g: &BTreeMap<&str, Vec<(&str, bool)>>, start: &str) -> usize {
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::from([start]);
    while let Some(id) = q.pop_front() {
        for (dep, dev_only) in g.get(id).map(Vec::as_slice).unwrap_or_default() {
            if !dev_only && seen.insert(*dep) {
                q.push_back(dep);
            }
        }
    }
    seen.len()
}

/// Strongly connected components with more than one node (Tarjan), as name paths.
fn cycles(nodes: &BTreeSet<&str>, edges: &[(&str, &str, &str)]) -> Vec<Vec<String>> {
    struct T<'a> {
        adj: BTreeMap<&'a str, Vec<&'a str>>,
        index: BTreeMap<&'a str, usize>,
        low: BTreeMap<&'a str, usize>,
        stack: Vec<&'a str>,
        on: BTreeSet<&'a str>,
        next: usize,
        out: Vec<Vec<String>>,
    }
    fn strong<'a>(t: &mut T<'a>, v: &'a str) {
        t.index.insert(v, t.next);
        t.low.insert(v, t.next);
        t.next += 1;
        t.stack.push(v);
        t.on.insert(v);
        for w in t.adj.get(v).cloned().unwrap_or_default() {
            if !t.index.contains_key(w) {
                strong(t, w);
                let lw = t.low[w];
                let lv = t.low.get_mut(v).expect("visited");
                *lv = (*lv).min(lw);
            } else if t.on.contains(w) {
                let iw = t.index[w];
                let lv = t.low.get_mut(v).expect("visited");
                *lv = (*lv).min(iw);
            }
        }
        if t.low[v] == t.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = t.stack.pop() {
                t.on.remove(w);
                comp.push(w.to_string());
                if w == v {
                    break;
                }
            }
            if comp.len() > 1 {
                comp.reverse();
                comp.push(comp[0].clone());
                t.out.push(comp);
            }
        }
    }
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (a, b, _) in edges {
        adj.entry(a).or_default().push(b);
    }
    let mut t = T { adj, index: BTreeMap::new(), low: BTreeMap::new(), stack: vec![], on: BTreeSet::new(), next: 0, out: vec![] };
    for n in nodes {
        if !t.index.contains_key(n) {
            strong(&mut t, n);
        }
    }
    t.out
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r#"{
      "workspace_root": "/w",
      "workspace_members": ["a 0.1.0 (path+file:///w/a)", "b 0.1.0 (path+file:///w/b)"],
      "packages": [
        {"id":"a 0.1.0 (path+file:///w/a)","name":"a","version":"0.1.0","edition":"2021","manifest_path":"/w/a/Cargo.toml",
         "targets":[{"kind":["lib"]}],
         "dependencies":[
           {"name":"b","kind":null,"path":"/w/b","req":"*","features":[]},
           {"name":"tokio","kind":null,"req":"1","features":["full"],"source":"registry+https://github.com/rust-lang/crates.io-index"}]},
        {"id":"b 0.1.0 (path+file:///w/b)","name":"b","version":"0.1.0","edition":"2018","manifest_path":"/w/b/Cargo.toml",
         "targets":[{"kind":["lib"]},{"kind":["custom-build"]}],
         "dependencies":[{"name":"a","kind":"dev","path":"/w/a","req":"^0.1","features":[]}]},
        {"id":"tokio 1.0.0","name":"tokio","version":"1.0.0","targets":[{"kind":["lib"]}],"dependencies":[]},
        {"id":"syn 1.0.0","name":"syn","version":"1.0.0","targets":[{"kind":["lib"]}],"dependencies":[]},
        {"id":"syn 2.0.0","name":"syn","version":"2.0.0","targets":[{"kind":["lib"]}],"dependencies":[]}
      ],
      "resolve": {"nodes": [
        {"id":"a 0.1.0 (path+file:///w/a)","deps":[{"pkg":"b 0.1.0 (path+file:///w/b)","dep_kinds":[{"kind":null}]},{"pkg":"tokio 1.0.0","dep_kinds":[{"kind":null}]}]},
        {"id":"b 0.1.0 (path+file:///w/b)","deps":[{"pkg":"a 0.1.0 (path+file:///w/a)","dep_kinds":[{"kind":"dev"}]},{"pkg":"syn 2.0.0","dep_kinds":[{"kind":null}]}]},
        {"id":"tokio 1.0.0","deps":[{"pkg":"syn 1.0.0","dep_kinds":[{"kind":null}]}]}
      ]}
    }"#;

    #[test]
    fn detects_workspace_issues() {
        let v = json::parse(META).unwrap();
        let f = analyze_value(&v, true);
        let has = |r: &str| f.iter().find(|x| x.rule == r);
        assert!(has("wildcard_dependency").is_some_and(|x| x.file == "a/Cargo.toml"));
        assert!(has("broad_feature").is_some());
        assert_eq!(has("dev_dependency_cycle").map(|x| x.evidence.as_str()), Some("a → b → a"));
        assert!(has("mixed_editions").is_some());
        assert!(has("duplicate_dependency_versions").is_some_and(|x| x.evidence == "1.0.0, 2.0.0"));
        assert!(has("build_scripts_and_proc_macros").is_some_and(|x| x.evidence.contains("1 build script(s): b")));
        let summary = &has("workspace_summary").unwrap().evidence;
        assert!(summary.contains("a=4") && summary.contains("b=1"), "{summary}");
    }

    #[test]
    fn no_deps_mode_skips_graph_checks() {
        let v = json::parse(META).unwrap();
        let f = analyze_value(&v, false);
        assert!(!f.iter().any(|x| x.rule == "duplicate_dependency_versions"));
        assert!(f.iter().any(|x| x.rule == "workspace_summary"));
    }
}
