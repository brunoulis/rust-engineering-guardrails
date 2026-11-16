//! Scoped Cargo roots. Cargo's workspace identity, rather than directory names or
//! source size, decides which discovered manifests share one analyzer invocation.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::analyzers::{cargo, relativize};
use crate::model::{AnalyzerRun, AnalyzerStatus, Finding};

pub struct Project {
    pub root: PathBuf,
    pub manifest: String,
    /// A nested manifest may inherit a workspace outside the selected inventory.
    /// Keep it visible as a failed run; do not build that external workspace.
    pub scope_error: Option<String>,
    selected_root: bool,
}

pub fn projects(root: &Path, selected_manifests: &[PathBuf]) -> Result<Vec<Project>, String> {
    let root = root.canonicalize().map_err(|e| format!("{}: {e}", root.display()))?;
    if root.join("Cargo.toml").is_file() {
        return Ok(vec![Project { root, manifest: "Cargo.toml".into(), scope_error: None, selected_root: true }]);
    }
    let manifests: BTreeSet<PathBuf> = selected_manifests
        .iter()
        .map(|p| {
            let path = p.canonicalize().map_err(|e| format!("selected Cargo manifest {}: {e}", p.display()))?;
            if !path.starts_with(&root) {
                return Err(format!("selected Cargo manifest escaped the audited root: {}", p.display()));
            }
            Ok(path)
        })
        .collect::<Result<_, String>>()?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for manifest in &manifests {
        let dir = manifest.parent().ok_or("Cargo manifest has no parent directory")?;
        let workspace = cargo(dir).args(["locate-project", "--workspace", "--message-format", "plain"]).output().ok().filter(|o| o.status.success()).and_then(|o| {
            let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
            p.canonicalize().ok()
        });
        // If Cargo cannot identify the workspace, retain this manifest so that
        // analyzer failures and their provenance still reach configured gates.
        let workspace = workspace.as_ref().unwrap_or(manifest);
        let scope_error = (!manifests.contains(workspace)).then(|| "Cargo workspace manifest is outside the selected scope; workspace analysis not run".into());
        let chosen = if scope_error.is_some() { manifest } else { workspace };
        if !seen.insert(chosen.clone()) {
            continue;
        }
        out.push(Project {
            root: chosen.parent().ok_or("Cargo workspace manifest has no parent directory")?.to_path_buf(),
            manifest: relativize(&root, &chosen.to_string_lossy()).ok_or("Cargo manifest escaped the audited root")?,
            scope_error,
            selected_root: false,
        });
    }
    Ok(out)
}

impl Project {
    /// Paths from source diagnostics use the invocation root; metadata supplies
    /// its actual workspace root. Empty project-wide locations gain the nested
    /// manifest identity so independent crates cannot collapse into one finding.
    pub fn rebase(&self, audited_root: &Path, base: &Path, (mut findings, mut run): (Vec<Finding>, AnalyzerRun)) -> (Vec<Finding>, AnalyzerRun) {
        for f in &mut findings {
            if f.file.is_empty() {
                if !self.selected_root {
                    f.file = self.manifest.clone();
                }
            } else {
                let path = base.join(&f.file);
                if let Some(relative) = relativize(audited_root, &path.to_string_lossy()) {
                    f.file = relative;
                } else {
                    // Selected workspace members retain external Cargo evidence
                    // without inventing a source location within the audit scope.
                    f.evidence.push_str(&format!("; location outside audited root: {}", path.display()));
                    f.file = self.manifest.clone();
                    f.line = 0;
                }
            }
        }
        let detail = match &mut run.status {
            AnalyzerStatus::Ran { detail } => detail,
            AnalyzerStatus::Skipped { reason } | AnalyzerStatus::Failed { reason } => reason,
        };
        *detail = format!("manifest {}: {detail}", self.manifest);
        (findings, run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Confidence, Severity};

    #[test]
    fn nested_compiler_and_project_findings_get_root_relative_identity() {
        let audited = std::env::current_dir().unwrap();
        let project = Project { root: audited.join("nested"), manifest: "nested/Cargo.toml".into(), scope_error: None, selected_root: false };
        let source = Finding::new("E0425", Category::Quality, Severity::Blocker, Confidence::Compiler).at("src/lib.rs", 4);
        let wide = Finding::new("llvm_lines_hotspot", Category::Performance, Severity::Medium, Confidence::Measured);
        let (findings, run) = project.rebase(&audited, &project.root, (vec![source, wide], AnalyzerRun::failed("clippy", "compilation failed")));
        assert_eq!(findings[0].file, "nested/src/lib.rs");
        assert_eq!(findings[0].line, 4);
        assert_eq!(findings[1].file, "nested/Cargo.toml");
        assert_eq!(findings[1].line, 0);
        assert!(matches!(run.status, AnalyzerStatus::Failed { .. }));
        assert!(run.status_str().1.contains("nested/Cargo.toml"));
    }
}
