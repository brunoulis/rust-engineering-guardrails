//! Shared finding model. Every analyzer produces `Finding`s; the report layer ranks them.

use std::fmt;

/// Audit dimension a finding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Safety,
    Concurrency,
    Ownership,
    Performance,
    Architecture,
    Quality,
}

impl Category {
    pub const ALL: [Category; 6] = [Category::Safety, Category::Concurrency, Category::Ownership, Category::Performance, Category::Architecture, Category::Quality];
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Safety => "safety",
            Category::Concurrency => "concurrency",
            Category::Ownership => "ownership",
            Category::Performance => "performance",
            Category::Architecture => "architecture",
            Category::Quality => "quality",
        }
    }
}

/// Severity follows SKILL.md §3. Pattern matches never reach `Blocker`; only compiler
/// errors and measured regressions beyond an explicit budget do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Medium,
    High,
    Blocker,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Blocker => "blocker",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Severity::Info),
            "medium" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            "blocker" => Some(Severity::Blocker),
            _ => None,
        }
    }
    pub fn weight(self) -> f64 {
        match self {
            Severity::Info => 1.0,
            Severity::Medium => 3.0,
            Severity::High => 7.0,
            Severity::Blocker => 15.0,
        }
    }
}

/// How the evidence was obtained, from weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Text pattern; may match strings, comments or test code.
    Heuristic,
    /// Parsed Rust syntax; no type information.
    Syntactic,
    /// Reported by rustc/clippy, i.e. with type information.
    Compiler,
    /// Measured (llvm-lines, benchmarks).
    Measured,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Heuristic => "heuristic",
            Confidence::Syntactic => "syntactic",
            Confidence::Compiler => "compiler",
            Confidence::Measured => "measured",
        }
    }
    pub fn weight(self) -> f64 {
        match self {
            Confidence::Heuristic => 0.5,
            Confidence::Syntactic => 1.0,
            Confidence::Compiler => 1.5,
            Confidence::Measured => 1.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Service,
    Engine,
    Library,
    Ffi,
    Prototype,
}

impl Profile {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "service" => Some(Profile::Service),
            "engine" => Some(Profile::Engine),
            "library" => Some(Profile::Library),
            "ffi" => Some(Profile::Ffi),
            "prototype" => Some(Profile::Prototype),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Service => "service",
            Profile::Engine => "engine",
            Profile::Library => "library",
            Profile::Ffi => "ffi",
            Profile::Prototype => "prototype",
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    /// Stable rule id (`clone_in_loop`, `clippy::redundant_clone`, ...).
    pub rule: String,
    pub category: Category,
    pub severity: Severity,
    pub confidence: Confidence,
    /// Path relative to the audited root, `/`-separated. Empty for project-wide findings.
    pub file: String,
    /// 1-based line, 0 when not applicable.
    pub line: usize,
    /// File matched a `--hot` glob.
    pub hot: bool,
    /// What the signal means and what to check.
    pub message: String,
    /// Source excerpt or tool output backing the finding.
    pub evidence: String,
}

impl Finding {
    pub fn new(rule: impl Into<String>, category: Category, severity: Severity, confidence: Confidence) -> Self {
        Finding { rule: rule.into(), category, severity, confidence, file: String::new(), line: 0, hot: false, message: String::new(), evidence: String::new() }
    }
    pub fn at(mut self, file: &str, line: usize) -> Self {
        self.file = file.to_string();
        self.line = line;
        self
    }
    pub fn msg(mut self, m: impl Into<String>) -> Self {
        self.message = m.into();
        self
    }
    pub fn evidence(mut self, e: impl Into<String>) -> Self {
        self.evidence = e.into();
        self
    }
    pub fn hot(mut self, hot: bool) -> Self {
        self.hot = hot;
        self
    }
}

/// Outcome of one analyzer, reported so the reader knows what was *not* checked.
#[derive(Debug, Clone)]
pub enum AnalyzerStatus {
    Ran { detail: String },
    Skipped { reason: String },
    Failed { reason: String },
}

#[derive(Debug, Clone)]
pub struct AnalyzerRun {
    pub name: &'static str,
    pub status: AnalyzerStatus,
}

impl AnalyzerRun {
    pub fn ran(name: &'static str, detail: impl Into<String>) -> Self {
        AnalyzerRun { name, status: AnalyzerStatus::Ran { detail: detail.into() } }
    }
    pub fn skipped(name: &'static str, reason: impl Into<String>) -> Self {
        AnalyzerRun { name, status: AnalyzerStatus::Skipped { reason: reason.into() } }
    }
    pub fn failed(name: &'static str, reason: impl Into<String>) -> Self {
        AnalyzerRun { name, status: AnalyzerStatus::Failed { reason: reason.into() } }
    }
    pub fn status_str(&self) -> (&'static str, &str) {
        match &self.status {
            AnalyzerStatus::Ran { detail } => ("ran", detail),
            AnalyzerStatus::Skipped { reason } => ("skipped", reason),
            AnalyzerStatus::Failed { reason } => ("failed", reason),
        }
    }
}

/// Truncates evidence to a bounded number of characters (not bytes).
pub fn excerpt(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let mut out: String = t.chars().take(max).collect();
        out.push('…');
        out
    }
}
