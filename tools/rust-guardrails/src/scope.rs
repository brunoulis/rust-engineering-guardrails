//! Shared source/manifest inventory. Git is an optional, read-only source selector;
//! filesystem discovery remains available without Git and through --include-ignored.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::glob;

const ALWAYS_SKIP: &[&str] = &[".git", ".hg", ".svn", "node_modules", ".venv", "venv", ".next", ".idea", ".vscode", ".cache", "__pycache__"];

pub struct Inventory {
    pub files: Vec<PathBuf>,
    pub dirs_skipped: Vec<String>,
    pub note: String,
}

fn selected_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "rs") || path.file_name().is_some_and(|n| n == "Cargo.toml")
}

fn key(path: &str) -> String {
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_string()
    }
}

fn git_files(root: &Path) -> Option<BTreeSet<String>> {
    let output = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "."])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let names = String::from_utf8(output.stdout).ok()?;
    let mut selected = BTreeSet::new();
    for name in names.split('\0').filter(|n| !n.is_empty()) {
        // Git paths are root-relative; never let inventory paths escape the root.
        let p = Path::new(name);
        if !p.components().all(|c| matches!(c, Component::Normal(_))) {
            return None;
        }
        if selected_file(p) {
            selected.insert(key(&name.replace('\\', "/")));
        }
    }
    Some(selected)
}

pub fn discover(root: &Path, exclude: &[String], include_ignored: bool) -> Result<Inventory, String> {
    let selected = if include_ignored { None } else { git_files(root) };
    let note = if include_ignored {
        "filesystem inventory (--include-ignored); child repositories and caches remain excluded"
    } else if selected.is_some() {
        "Git source inventory: tracked and non-ignored untracked files; ignored files excluded"
    } else {
        "filesystem inventory: Git selection unavailable or root is not a Git repository; ignored files may be included"
    };
    let mut inventory = Inventory { files: Vec::new(), dirs_skipped: Vec::new(), note: note.into() };
    gather(root, root, exclude, selected.as_ref(), &mut inventory)?;
    Ok(inventory)
}

fn skip_dir(dir: &Path, name: &str) -> bool {
    ALWAYS_SKIP.contains(&name)
        || dir.join("CACHEDIR.TAG").is_file()
        || (name == "target" && dir.parent().is_some_and(|p| p.join("Cargo.toml").is_file()))
        || dir.join(".git").exists()
        || matches!(name, ".worktrees")
}

fn gather(root: &Path, here: &Path, exclude: &[String], selected: Option<&BTreeSet<String>>, out: &mut Inventory) -> Result<(), String> {
    let mut entries = fs::read_dir(here).map_err(|e| format!("{}: {e}", here.display()))?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        if ty.is_symlink() {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if ty.is_dir() {
            let prefix = format!("{rel}/");
            // Only prune globs whose trailing star necessarily excludes descendants.
            let excluded = exclude.iter().any(|p| p.ends_with('*') && glob::matches(p, &prefix));
            let outside_git = selected.is_some_and(|files| {
                let prefix = key(&prefix);
                !files.range(prefix.clone()..).next().is_some_and(|f| f.starts_with(&prefix))
            });
            // Worktree stores are generated data even without a usable Git inventory.
            let tool_worktrees = here.file_name().is_some_and(|n| n == ".claude" || n == ".codex") && entry.file_name() == "worktrees";
            if skip_dir(&path, &entry.file_name().to_string_lossy()) || excluded || outside_git || tool_worktrees {
                out.dirs_skipped.push(rel);
            } else {
                gather(root, &path, exclude, selected, out)?;
            }
        } else if ty.is_file() && selected_file(&path) && !glob::any(exclude, &rel) && selected.is_none_or(|files| files.contains(&key(&rel))) {
            out.files.push(path);
        }
    }
    Ok(())
}
