//! Unified audit report: findings by dimension plus a prioritized, verifiable refactoring
//! plan. The plan is a proposal only — the tool never edits the audited code.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::analyzers::bench::{self, BenchResult};
use crate::json::{obj, Value};
use crate::model::{AnalyzerRun, Category, Confidence, Finding, Profile, Severity};
use crate::rules;

pub const DISCLAIMER: &str =
    "Syntactic and compiler signals are leads, not proof. Only compiler errors and benchmark regressions beyond an explicit budget are treated as blockers. Sources and Cargo.lock are never modified (clippy/llvm-lines only write build artifacts to target/).";

pub struct Report {
    pub root: String,
    pub profile: Profile,
    pub engine: &'static str,
    pub files_scanned: usize,
    pub lines_scanned: usize,
    pub analyzers: Vec<AnalyzerRun>,
    pub findings: Vec<Finding>,
    pub baseline: Option<crate::baseline::Comparison>,
    pub exceptions: Vec<Option<String>>,
    pub project_profile: Option<Value>,
    pub bench: Option<BenchResult>,
    pub plan: Vec<PlanItem>,
    pub plan_omitted: usize,
}

#[derive(Debug, Clone)]
pub struct PlanItem {
    pub id: String,
    pub priority: &'static str,
    pub score: f64,
    pub rule: String,
    pub category: Category,
    pub severity: Severity,
    pub confidence: Confidence,
    pub file: String,
    pub lines: Vec<usize>,
    pub count: usize,
    pub hot: bool,
    pub title: String,
    pub why: String,
    pub keep: String,
    pub minimal: String,
    pub structural: String,
    pub verify: Vec<String>,
    pub reference: String,
    pub characterize_first: bool,
    pub done_when: String,
}

/// Rules that are context, not work items.
const NOT_ACTIONABLE: &[&str] = &["workspace_summary", "bench_improvement", "bench_inconclusive", "parse_fallback", "read_error", "build_scripts_and_proc_macros", "unsafe_block"];

/// Syntactic rule ↔ clippy lint pairs that describe the same concern.
const CORROBORATIONS: &[(&str, &str)] = &[
    ("unsafe_without_safety_comment", "clippy::undocumented_unsafe_blocks"),
    ("unsafe_safety_comment_review", "clippy::undocumented_unsafe_blocks"),
    ("unsafe_fn_without_safety_doc", "clippy::missing_safety_doc"),
    ("clone_in_loop", "clippy::redundant_clone"),
    ("clone_call", "clippy::redundant_clone"),
    ("allocation_in_loop", "clippy::needless_collect"),
    ("sync_lock_in_async", "clippy::await_holding_lock"),
    ("unwrap_in_non_test", "clippy::unwrap_used"),
];

/// Folds a compiler diagnostic into the syntactic finding at the same location: one item,
/// upgraded to `compiler` confidence, instead of two plan entries for one problem.
/// Returns the number of merged pairs.
pub fn corroborate(findings: &mut Vec<Finding>) -> usize {
    let mut drop = vec![false; findings.len()];
    let mut merged = 0;
    for i in 0..findings.len() {
        let Some(&(_, lint)) = CORROBORATIONS.iter().find(|(r, _)| *r == findings[i].rule) else { continue };
        // Preserve an existing blocker diagnostic instead of folding it into a
        // lower-severity review signal. Analyzer failure gates are independent.
        let hit = (0..findings.len()).find(|&j| {
            !drop[j] && findings[j].severity != Severity::Blocker && findings[j].rule == lint && findings[j].file == findings[i].file && findings[j].line == findings[i].line
        });
        if let Some(j) = hit {
            drop[j] = true;
            merged += 1;
            let (sev, conf) = (findings[j].severity, findings[j].confidence);
            let f = &mut findings[i];
            // A merge confirms the lead; it never turns a pattern match into a blocker.
            let documentation = matches!(f.rule.as_str(), "unsafe_without_safety_comment" | "unsafe_fn_without_safety_doc" | "unsafe_safety_comment_review");
            f.severity = if documentation { Severity::Medium } else { f.severity.max(sev).min(Severity::High) };
            f.confidence = f.confidence.max(conf);
            f.message = if documentation {
                format!("{} [documentation convention confirmed by {lint}; soundness is unverified]", f.message)
            } else {
                format!("{} [confirmed by {lint}]", f.message)
            };
        }
    }
    let mut k = 0;
    findings.retain(|_| {
        k += 1;
        !drop[k - 1]
    });
    merged
}

pub fn build_plan(findings: &[Finding], root: &str, limit: usize) -> (Vec<PlanItem>, usize) {
    let mut groups: BTreeMap<(&str, &str), Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        if !NOT_ACTIONABLE.contains(&f.rule.as_str()) {
            groups.entry((f.rule.as_str(), f.file.as_str())).or_default().push(f);
        }
    }
    let mut items: Vec<PlanItem> = groups
        .into_iter()
        .filter_map(|((rule, file), hits)| {
            let severity = hits.iter().map(|f| f.severity).max()?;
            let confidence = hits.iter().map(|f| f.confidence).max()?;
            let hot = hits.iter().any(|f| f.hot);
            let category = hits[0].category;
            let actionable = severity >= Severity::Medium || (hot && matches!(category, Category::Performance | Category::Ownership));
            if !actionable {
                return None;
            }
            let count = hits.len();
            let score = severity.weight() * confidence.weight() * if hot { 2.0 } else { 1.0 } * (1.0 + (count as f64).ln());
            let mut lines: Vec<usize> = hits.iter().map(|f| f.line).filter(|l| *l > 0).collect();
            lines.sort_unstable();
            lines.dedup();
            let subst = |s: &str| s.replace("<root>", root);
            let (title, why, keep, minimal, structural, verify, reference, characterize_first) = match rules::get(rule) {
                Some(r) => (
                    if count == 1 { hits[0].message.clone() } else { r.title.to_string() },
                    r.why.to_string(),
                    r.keep.to_string(),
                    r.minimal.to_string(),
                    r.structural.to_string(),
                    r.verify.iter().map(|v| subst(v)).collect(),
                    r.reference.to_string(),
                    r.characterize_first,
                ),
                None if rule.starts_with("rustsec::") || rule.starts_with("dependency_warning::") => (
                    format!("`{rule}`: {}", hits[0].message),
                    format!("Imported dependency evidence ({count} occurrence(s)); freshness, current lockfile identity and reachability remain unverified."),
                    "Retain only with a reviewed advisory applicability assessment or justified dated acceptance; preserve the evidence.".to_string(),
                    "Verify the affected dependency version/features and advisory conditions; apply the smallest compatible remediation when warranted.".to_string(),
                    "Replace or isolate the dependency only when compatibility and maintenance evidence justify that cost.".to_string(),
                    vec![
                        "cargo audit --json (retain exit status, tool/database provenance, settings and lockfile identity)".to_string(),
                        "cargo deny check and cargo vet when adopted by project policy; verify affected consumer contracts".to_string(),
                    ],
                    "policies/security-supply-chain.md".to_string(),
                    true,
                ),
                None => {
                    // Compiler/clippy diagnostic.
                    let g = rules::generic_play(category);
                    let lint_flag = if rule.starts_with("clippy::") { format!(" -- -W {rule}") } else { String::new() };
                    let doc = rule.strip_prefix("clippy::").map(|n| format!(" Docs: https://rust-lang.github.io/rust-clippy/master/index.html#{n}")).unwrap_or_default();
                    (
                        format!("`{rule}`: {}", hits[0].message),
                        format!("Reported by the compiler with type information ({count} occurrence(s)).{doc}"),
                        g.keep.to_string(),
                        g.minimal.to_string(),
                        g.structural.to_string(),
                        vec![
                            format!("cargo clippy --workspace --all-targets{lint_flag} (no `{rule}` at {file})"),
                            "cargo test --workspace (must pass before and after)".to_string(),
                        ],
                        g.reference.to_string(),
                        matches!(category, Category::Ownership | Category::Concurrency),
                    )
                }
            };
            Some(PlanItem {
                id: String::new(),
                priority: "",
                score,
                rule: rule.to_string(),
                category,
                severity,
                confidence,
                file: file.to_string(),
                lines,
                count,
                hot,
                title,
                why,
                keep,
                minimal,
                structural,
                verify,
                reference,
                characterize_first,
                done_when: String::new(),
            })
        })
        .collect();
    // Blockers always come first so --plan-limit can never truncate them away.
    items.sort_by(|a, b| {
        (b.severity == Severity::Blocker)
            .cmp(&(a.severity == Severity::Blocker))
            .then(b.score.total_cmp(&a.score))
            .then(a.category.cmp(&b.category))
            .then(a.file.cmp(&b.file))
            .then(a.rule.cmp(&b.rule))
    });
    let omitted = items.len().saturating_sub(limit);
    items.truncate(limit);
    for (i, it) in items.iter_mut().enumerate() {
        it.id = format!("R{}", i + 1);
        it.priority = if it.severity == Severity::Blocker {
            "P0"
        } else if it.score >= 10.0 {
            "P1"
        } else if it.score >= 4.0 {
            "P2"
        } else {
            "P3"
        };
        let loc = if it.file.is_empty() { "the project".to_string() } else { it.file.clone() };
        it.done_when = format!(
            "Record one reviewed disposition for `{}` in {loc}: defect fixed with applicable verification results, legitimate use retained with code/test evidence, or investigation deferred with the unresolved question and next check. Valid code may remain unchanged and the signal may remain; no per-occurrence comment or lint suppression is required.",
            it.rule
        );
    }
    (items, omitted)
}

impl Report {
    pub fn counts(&self) -> BTreeMap<(Category, Severity), usize> {
        let mut m = BTreeMap::new();
        for f in &self.findings {
            *m.entry((f.category, f.severity)).or_default() += 1;
        }
        m
    }

    pub fn max_severity(&self) -> Option<Severity> {
        self.findings
            .iter()
            .enumerate()
            .filter(|(i, _)| self.baseline.as_ref().is_none_or(|b| b.is_new[*i]) && self.exceptions.get(*i).is_none_or(Option::is_none))
            .map(|(_, f)| f.severity)
            .max()
    }

    fn gate_notes(&self) -> String {
        let mut out = String::new();
        if let Some(b) = &self.baseline {
            let new = b.is_new.iter().filter(|n| **n).count();
            let _ = writeln!(
                out,
                "\nDebt baseline: {new} new/escalated, {} existing, {} resolved/unmatched. Gate considers new, non-exempt findings; all findings remain in the report.",
                b.is_new.len() - new,
                b.resolved_count
            );
        }
        for (i, reason) in self.exceptions.iter().enumerate() {
            if let Some(reason) = reason {
                let f = &self.findings[i];
                let _ = writeln!(out, "Exception: {} [{}] — {}", loc(&f.file, &[f.line]), f.rule, reason);
            }
        }
        if let Some(profile) = &self.project_profile {
            let _ = writeln!(out, "\nProject profile contracts (review context; not proof of enforcement): {}", profile.to_json());
        }
        out.push_str(&crate::verification::to_text(&self.findings, self.profile));
        out
    }

    fn sorted_findings(&self, cat: Category) -> Vec<&Finding> {
        let mut v: Vec<&Finding> = self.findings.iter().filter(|f| f.category == cat).collect();
        v.sort_by(|a, b| b.severity.cmp(&a.severity).then(b.hot.cmp(&a.hot)).then(b.confidence.cmp(&a.confidence)).then(a.file.cmp(&b.file)).then(a.line.cmp(&b.line)));
        v
    }

    // ------------------------------------------------------------------ text
    pub fn to_text(&self, limit: usize) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "Rust audit [{}] {}: {} files, {} lines, engine={}", self.profile, self.root, self.files_scanned, self.lines_scanned, self.engine);
        let _ = writeln!(o, "NOTE: {DISCLAIMER}");
        let _ = writeln!(o, "\nAnalyzers:");
        for a in &self.analyzers {
            let (s, d) = a.status_str();
            let _ = writeln!(o, "  {:<11} {s:<8} {d}", a.name);
        }
        let counts = self.counts();
        let _ = writeln!(o, "\nFindings by dimension (blocker/high/medium/info):");
        for c in Category::ALL {
            let n = |s| counts.get(&(c, s)).copied().unwrap_or(0);
            let _ = writeln!(o, "  {:<13} {:>3} {:>4} {:>6} {:>5}", c.as_str(), n(Severity::Blocker), n(Severity::High), n(Severity::Medium), n(Severity::Info));
        }
        if let Some(b) = &self.bench {
            let _ = writeln!(o, "\nBenchmarks ({}):", b.source);
            for c in &b.comparisons {
                let _ = writeln!(o, "  {:<12} {}  {}", c.verdict.as_str(), c.name, bench::describe(c));
            }
        }
        let _ = writeln!(o, "\nRefactoring plan (proposed, not applied):");
        if self.plan.is_empty() {
            let _ = writeln!(o, "  nothing above info severity");
        }
        for it in &self.plan {
            let _ = writeln!(
                o,
                "  {} {} [{}/{}/{}] {}{} — {}",
                it.id,
                it.priority,
                it.category.as_str(),
                it.severity.as_str(),
                it.confidence.as_str(),
                loc(&it.file, &it.lines),
                if it.hot { " (hot)" } else { "" },
                it.title
            );
            let _ = writeln!(o, "      minimal: {}", it.minimal);
            if let Some(v) = it.verify.first() {
                let _ = writeln!(o, "      verify:  {v}");
            }
        }
        if self.plan_omitted > 0 {
            let _ = writeln!(o, "  … {} more item(s); raise --plan-limit", self.plan_omitted);
        }
        let mut top: Vec<&Finding> = self.findings.iter().filter(|f| f.severity >= Severity::Medium).collect();
        top.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.file.cmp(&b.file)).then(a.line.cmp(&b.line)));
        let _ = writeln!(o, "\nTop findings (medium and above, first {limit}):");
        for f in top.iter().take(limit) {
            let _ = writeln!(o, "  {:<7} {:<12} {} [{}] {}", f.severity.as_str(), f.category.as_str(), loc(&f.file, &[f.line]), f.rule, f.message);
        }
        o.push_str(&self.gate_notes());
        o
    }

    // ------------------------------------------------------------------ markdown
    pub fn to_markdown(&self, limit: usize) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "# Rust audit report\n");
        let _ = writeln!(o, "| Field | Value |\n| --- | --- |");
        let _ = writeln!(o, "| Root | `{}` |", self.root);
        let _ = writeln!(o, "| Profile | {} |", self.profile);
        let _ = writeln!(o, "| Source engine | {} |", self.engine);
        let _ = writeln!(o, "| Files / lines | {} / {} |", self.files_scanned, self.lines_scanned);
        let _ = writeln!(o, "| Tool | rust-guardrails {} |\n", env!("CARGO_PKG_VERSION"));
        let _ = writeln!(o, "> {DISCLAIMER}\n");

        let _ = writeln!(o, "## What was checked\n\n| Analyzer | Status | Detail |\n| --- | --- | --- |");
        for a in &self.analyzers {
            let (s, d) = a.status_str();
            let _ = writeln!(o, "| {} | {} | {} |", a.name, s, md(d));
        }

        let counts = self.counts();
        let _ = writeln!(o, "\n## Summary by dimension\n\n| Dimension | Blocker | High | Medium | Info |\n| --- | ---: | ---: | ---: | ---: |");
        for c in Category::ALL {
            let n = |s| counts.get(&(c, s)).copied().unwrap_or(0);
            let _ = writeln!(o, "| {} | {} | {} | {} | {} |", c.as_str(), n(Severity::Blocker), n(Severity::High), n(Severity::Medium), n(Severity::Info));
        }

        if let Some(b) = &self.bench {
            let _ = writeln!(o, "\n## Benchmark comparison\n\nSource: {}\n\n| Benchmark | Verdict | Change | Detail |\n| --- | --- | ---: | --- |", md(&b.source));
            for c in &b.comparisons {
                let _ = writeln!(
                    o,
                    "| {} | {}{} | {:+.1}% | {} |",
                    md(&c.name),
                    c.verdict.as_str(),
                    if c.over_budget { " (over budget)" } else { "" },
                    c.change_pct,
                    md(&bench::describe(c))
                );
            }
            if !b.unmatched.is_empty() {
                let _ = writeln!(o, "\nUnmatched: {}", md(&b.unmatched.join(", ")));
            }
        }

        let _ = writeln!(o, "\n## Prioritized refactoring plan\n");
        let _ = writeln!(o, "Proposed only — nothing was changed. Work one item per change set; keep pure moves separate from behavior changes; for items marked *characterize first*, add tests pinning current behavior before editing. Priorities: P0 blocker, P1 ≥10, P2 ≥4, P3 below (score = severity × confidence × hot × log-count).\n");
        if self.plan.is_empty() {
            let _ = writeln!(o, "No item above info severity.\n");
        }
        for it in &self.plan {
            let _ = writeln!(o, "### {} · {} · {} — {}\n", it.id, it.priority, it.category.as_str(), md(&it.title));
            let _ = writeln!(
                o,
                "- **Where:** {}{} ({} occurrence(s))\n- **Rule:** `{}` · severity {} · confidence {} · score {:.1}\n- **Why:** {}\n",
                loc(&it.file, &it.lines),
                if it.hot { " — **hot path**" } else { "" },
                it.count,
                it.rule,
                it.severity.as_str(),
                it.confidence.as_str(),
                it.score,
                md(&it.why)
            );
            let _ = writeln!(
                o,
                "| Option | When / what |\n| --- | --- |\n| Keep as is | {} |\n| Minimal fix | {} |\n| Structural | {} |\n",
                md(&it.keep),
                md(&it.minimal),
                md(&it.structural)
            );
            if it.characterize_first {
                let _ = writeln!(o, "**Characterize first:** add tests that pin the current observable behavior before changing this code.\n");
            }
            let _ = writeln!(o, "**Verify:**\n");
            for v in &it.verify {
                let _ = writeln!(o, "- [ ] {}", md(v));
            }
            let _ = writeln!(o, "\n**Done when:** {}\n\nReference: `{}`\n", md(&it.done_when), it.reference);
        }
        if self.plan_omitted > 0 {
            let _ = writeln!(o, "_{} further item(s) omitted; raise `--plan-limit`._\n", self.plan_omitted);
        }

        let _ = writeln!(o, "## Findings by dimension\n");
        for c in Category::ALL {
            let all = self.sorted_findings(c);
            if all.is_empty() {
                continue;
            }
            let _ = writeln!(o, "### {} ({})\n\n| Sev | Conf | Location | Rule | Message | Evidence |\n| --- | --- | --- | --- | --- | --- |", c.as_str(), all.len());
            for f in all.iter().take(limit) {
                let _ = writeln!(
                    o,
                    "| {} | {} | {}{} | `{}` | {} | {} |",
                    f.severity.as_str(),
                    f.confidence.as_str(),
                    md(&loc(&f.file, &[f.line])),
                    if f.hot { " 🔥" } else { "" },
                    f.rule,
                    md(&f.message),
                    if f.evidence.is_empty() { String::new() } else { format!("`{}`", f.evidence.replace('`', "'").replace('|', "\\|")) }
                );
            }
            if all.len() > limit {
                let _ = writeln!(o, "\n_{} more; raise `--limit` or use `--format json`._", all.len() - limit);
            }
            let _ = writeln!(o);
        }
        let _ = writeln!(o, "## Limitations\n\n- The syntactic engine has no type information: a `.clone()` of an `Arc` and of a `Vec` look the same. Run with `--with clippy` for type-aware lints.\n- Loop and async context are lexical; calls through helper functions are not followed.\n- `large_type` merges same-named types from different modules.\n- llvm-lines measures compile-time IR volume, not runtime speed. Benchmarks are only as representative as their workload.\n- Thresholds (`--max-file-lines`, `--max-fn-lines`, `--max-fields`, `--max-methods`) are project choices, not universal rules.");
        o.push_str(&self.gate_notes());
        o
    }

    // ------------------------------------------------------------------ json
    pub fn to_json(&self) -> String {
        let finding = |(i, f): (usize, &Finding)| {
            obj([
                ("rule", f.rule.as_str().into()),
                ("category", f.category.as_str().into()),
                ("severity", f.severity.as_str().into()),
                ("confidence", f.confidence.as_str().into()),
                ("file", f.file.as_str().into()),
                ("line", f.line.into()),
                ("hot", f.hot.into()),
                ("message", f.message.as_str().into()),
                ("evidence", f.evidence.as_str().into()),
                ("fingerprint", crate::baseline::fingerprint(f).into()),
                ("baseline_state", self.baseline.as_ref().map_or(Value::Null, |b| if b.is_new[i] { "new" } else { "existing" }.into())),
                ("exception", self.exceptions.get(i).and_then(Option::as_ref).map_or(Value::Null, |s| s.as_str().into())),
            ])
        };
        let counts = self.counts();
        let summary = obj(Category::ALL.iter().map(|c| {
            (
                c.as_str(),
                obj([Severity::Blocker, Severity::High, Severity::Medium, Severity::Info].iter().map(|s| (s.as_str(), counts.get(&(*c, *s)).copied().unwrap_or(0).into()))),
            )
        }));
        let plan = Value::Arr(
            self.plan
                .iter()
                .map(|it| {
                    obj([
                        ("id", it.id.as_str().into()),
                        ("priority", it.priority.into()),
                        ("score", ((it.score * 10.0).round() / 10.0).into()),
                        ("rule", it.rule.as_str().into()),
                        ("category", it.category.as_str().into()),
                        ("severity", it.severity.as_str().into()),
                        ("confidence", it.confidence.as_str().into()),
                        ("file", it.file.as_str().into()),
                        ("lines", Value::Arr(it.lines.iter().map(|l| (*l).into()).collect())),
                        ("occurrences", it.count.into()),
                        ("hot", it.hot.into()),
                        ("title", it.title.as_str().into()),
                        ("why", it.why.as_str().into()),
                        ("options", obj([("keep", it.keep.as_str().into()), ("minimal", it.minimal.as_str().into()), ("structural", it.structural.as_str().into())])),
                        ("characterize_first", it.characterize_first.into()),
                        ("verify", Value::Arr(it.verify.iter().map(|v| v.as_str().into()).collect())),
                        ("done_when", it.done_when.as_str().into()),
                        ("reference", it.reference.as_str().into()),
                    ])
                })
                .collect(),
        );
        let bench = match &self.bench {
            None => Value::Null,
            Some(b) => obj([
                ("source", b.source.as_str().into()),
                (
                    "comparisons",
                    Value::Arr(
                        b.comparisons
                            .iter()
                            .map(|c| {
                                obj([
                                    ("name", c.name.as_str().into()),
                                    ("verdict", c.verdict.as_str().into()),
                                    ("change_pct", ((c.change_pct * 100.0).round() / 100.0).into()),
                                    ("over_budget", c.over_budget.into()),
                                    (
                                        "baseline_ns",
                                        obj([
                                            ("mean", c.base.mean.into()),
                                            ("lo", c.base.lo.into()),
                                            ("hi", c.base.hi.into()),
                                            ("confidence_level", c.base.confidence_level.into()),
                                            ("method", c.base.method.into()),
                                        ]),
                                    ),
                                    (
                                        "candidate_ns",
                                        obj([
                                            ("mean", c.cand.mean.into()),
                                            ("lo", c.cand.lo.into()),
                                            ("hi", c.cand.hi.into()),
                                            ("confidence_level", c.cand.confidence_level.into()),
                                            ("method", c.cand.method.into()),
                                        ]),
                                    ),
                                ])
                            })
                            .collect(),
                    ),
                ),
                ("unmatched", Value::Arr(b.unmatched.iter().map(|u| u.as_str().into()).collect())),
            ]),
        };
        obj([
            ("schema_version", 2usize.into()),
            ("baseline", self.baseline.as_ref().map_or(Value::Null, crate::baseline::Comparison::to_json)),
            ("project_profile", self.project_profile.clone().unwrap_or(Value::Null)),
            ("verification", crate::verification::to_json(&self.findings, self.profile)),
            ("tool_version", env!("CARGO_PKG_VERSION").into()),
            ("disclaimer", DISCLAIMER.into()),
            ("root", self.root.as_str().into()),
            ("profile", self.profile.as_str().into()),
            ("engine", self.engine.into()),
            ("files_scanned", self.files_scanned.into()),
            ("lines_scanned", self.lines_scanned.into()),
            (
                "analyzers",
                Value::Arr(
                    self.analyzers
                        .iter()
                        .map(|a| {
                            let (s, d) = a.status_str();
                            obj([("name", a.name.into()), ("status", s.into()), ("detail", d.into())])
                        })
                        .collect(),
                ),
            ),
            ("summary", summary),
            ("plan", plan),
            ("plan_omitted", self.plan_omitted.into()),
            ("benchmarks", bench),
            ("findings", Value::Arr(self.findings.iter().enumerate().map(finding).collect())),
        ])
        .to_json()
    }
}

fn loc(file: &str, lines: &[usize]) -> String {
    let lines: Vec<String> = lines.iter().filter(|l| **l > 0).take(8).map(|l| l.to_string()).collect();
    match (file.is_empty(), lines.is_empty()) {
        (true, _) => "project".to_string(),
        (false, true) => file.to_string(),
        (false, false) => format!("{file}:{}", lines.join(",")),
    }
}

/// Escapes text for a Markdown table cell.
fn md(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(rule: &'static str, file: &str, line: usize, hot: bool) -> Finding {
        rules::finding(rule, Confidence::Syntactic).at(file, line).hot(hot)
    }

    #[test]
    fn plan_ranks_and_filters() {
        let findings = vec![
            f("clone_call", "a.rs", 1, false),        // info, not hot → excluded
            f("clone_call", "hot.rs", 2, true),       // info but hot ownership → included
            f("allocation_in_loop", "a.rs", 3, true), // info, but hot → in plan
            f("allocation_in_loop", "a.rs", 9, true),
            f("unsafe_without_safety_comment", "ffi.rs", 4, false), // documentation review, medium
            f("workspace_summary", "", 0, false),                   // context only
            Finding::new("E0425", Category::Quality, Severity::Blocker, Confidence::Compiler).at("main.rs", 1).msg("cannot find value"),
        ];
        let (plan, omitted) = build_plan(&findings, ".", 10);
        assert_eq!(omitted, 0);
        let ids: Vec<_> = plan.iter().map(|p| (p.id.as_str(), p.priority, p.rule.as_str(), p.file.as_str())).collect();
        assert_eq!(ids[0], ("R1", "P0", "E0425", "main.rs"));
        assert_eq!(ids[1].2, "allocation_in_loop");
        assert_eq!(plan.iter().find(|p| p.rule == "unsafe_without_safety_comment").unwrap().severity, Severity::Medium);
        assert!(ids.iter().any(|i| i.2 == "clone_call" && i.3 == "hot.rs"));
        assert!(!ids.iter().any(|i| i.2 == "clone_call" && i.3 == "a.rs"));
        assert!(!ids.iter().any(|i| i.2 == "workspace_summary"));
        let alloc = plan.iter().find(|p| p.rule == "allocation_in_loop").unwrap();
        assert_eq!((alloc.count, alloc.lines.clone()), (2, vec![3, 9]));
        assert!(plan[0].verify[0].contains("cargo clippy"));
        assert!(plan.iter().all(|p| !p.done_when.is_empty() && !p.verify.is_empty()));
    }

    #[test]
    fn corroboration_merges_same_location() {
        let mut v = vec![
            f("unsafe_without_safety_comment", "a.rs", 4, false),
            Finding::new("clippy::undocumented_unsafe_blocks", Category::Safety, Severity::Medium, Confidence::Compiler).at("a.rs", 4),
            Finding::new("clippy::undocumented_unsafe_blocks", Category::Safety, Severity::Medium, Confidence::Compiler).at("a.rs", 9),
        ];
        assert_eq!(corroborate(&mut v), 1);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].severity, v[0].confidence), (Severity::Medium, Confidence::Compiler));
        assert!(v[0].message.contains("documentation convention confirmed by clippy::undocumented_unsafe_blocks"));
        assert!(v[0].message.contains("soundness is unverified"));
        assert_eq!(v[1].line, 9);
    }

    #[test]
    fn safety_comment_corroboration_confirms_documentation_convention_only() {
        let mut source = Finding::new("unsafe_safety_comment_review", Category::Safety, Severity::Medium, Confidence::Syntactic)
            .at("ffi.rs", 3)
            .msg("Review the adjacent explanation; soundness is unverified");
        source.evidence = "// valid for the FFI owner".into();
        let mut findings = vec![source, Finding::new("clippy::undocumented_unsafe_blocks", Category::Safety, Severity::Medium, Confidence::Compiler).at("ffi.rs", 3)];
        assert_eq!(corroborate(&mut findings), 1);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Medium);
        assert!(findings[0].message.contains("documentation convention confirmed"));
        assert!(findings[0].message.contains("soundness is unverified"));
    }

    #[test]
    fn blockers_survive_plan_limit_and_merges_never_create_blockers() {
        let mut v: Vec<Finding> = (1..=12).map(|l| f("unsafe_without_safety_comment", "ffi.rs", l, false)).collect();
        v.push(rules::finding("bench_regression", Confidence::Measured).msg("render regressed"));
        v.last_mut().unwrap().severity = Severity::Blocker;
        let (plan, omitted) = build_plan(&v, ".", 1);
        assert_eq!((plan[0].priority, plan[0].rule.as_str(), omitted), ("P0", "bench_regression", 1));

        let mut m = vec![
            f("unsafe_without_safety_comment", "a.rs", 4, false),
            Finding::new("clippy::undocumented_unsafe_blocks", Category::Safety, Severity::Blocker, Confidence::Compiler).at("a.rs", 4),
        ];
        assert_eq!(corroborate(&mut m), 0, "compiler errors must remain independent blockers");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].severity, Severity::Medium);
        assert_eq!(m[1].severity, Severity::Blocker);
    }

    #[test]
    fn documentation_corroboration_does_not_promote_warnings_to_high() {
        for (rule, lint) in [
            ("unsafe_without_safety_comment", "clippy::undocumented_unsafe_blocks"),
            ("unsafe_fn_without_safety_doc", "clippy::missing_safety_doc"),
            ("unsafe_safety_comment_review", "clippy::undocumented_unsafe_blocks"),
        ] {
            let mut findings = vec![f(rule, "ffi.rs", 3, false), Finding::new(lint, Category::Safety, Severity::High, Confidence::Compiler).at("ffi.rs", 3)];
            assert_eq!(corroborate(&mut findings), 1);
            assert_eq!(findings[0].severity, Severity::Medium, "{rule}");
            assert_eq!(findings[0].confidence, Confidence::Compiler);
            assert!(findings[0].message.contains("soundness is unverified"));
        }
    }

    #[test]
    fn plan_limit_reports_omitted() {
        let findings: Vec<_> = (0..5).map(|i| f("allocation_in_loop", &format!("f{i}.rs"), 1, true)).collect();
        let (plan, omitted) = build_plan(&findings, ".", 3);
        assert_eq!((plan.len(), omitted), (3, 2));
    }
}
