//! Optional analyzers backed by external cargo tooling. Each one reports an
//! `AnalyzerRun` so the report states explicitly what was and was not checked.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

pub mod bench;
pub mod clippy;
pub mod llvm_lines;
pub mod metadata;

/// The cargo binary: `$CARGO` when invoked through cargo, otherwise `cargo` on PATH.
pub fn cargo(root: &Path) -> Command {
    let bin = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut c = Command::new(bin);
    c.current_dir(root);
    c
}

pub struct Timed {
    pub output: Output,
    pub secs: f64,
}

pub fn run_timed(mut c: Command) -> std::io::Result<Timed> {
    let start = Instant::now();
    let output = c.output()?;
    Ok(Timed { output, secs: start.elapsed().as_secs_f64() })
}

/// Makes a path reported by cargo relative to `root` with `/` separators. Returns `None`
/// for files outside the root (dependencies, std).
pub fn relativize(root: &Path, file: &str) -> Option<String> {
    let p = Path::new(file);
    // Existing files: canonical paths (resolves symlinks and Windows `\\?\` prefixes).
    if let (Ok(r), Ok(f)) = (root.canonicalize(), p.canonicalize()) {
        return f.strip_prefix(&r).ok().map(|rel| rel.to_string_lossy().replace('\\', "/"));
    }
    // Otherwise compare lexically normalized absolute paths.
    let cwd = std::env::current_dir().ok()?;
    let (r, f) = (normalize(&cwd.join(root)), normalize(&cwd.join(p)));
    f.strip_prefix(&r).ok().map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// Removes `.` and resolves `..` without touching the file system.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

pub fn first_line(s: &str) -> String {
    s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}
