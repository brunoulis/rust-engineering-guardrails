//! Zero-dependency text engine (the legacy pre-1.0 rules). It blanks string/char literals and
//! comments before matching, which removes the most common false positives, but it still
//! has no notion of loops, async context or test code.

use super::FileScan;
use crate::model::{excerpt, Confidence};
use crate::rules;

pub fn scan(rel: &str, content: &str) -> FileScan {
    let code = strip_literals_and_comments(content);
    let original: Vec<&str> = content.lines().collect();
    let mut out = FileScan::default();
    for (i, line) in code.lines().enumerate() {
        for rule in signals(line) {
            let evidence = original.get(i).map(|l| excerpt(l, 180)).unwrap_or_default();
            out.findings.push(rules::finding(rule, Confidence::Heuristic).at(rel, i + 1).evidence(evidence));
        }
    }
    out
}

pub fn signals(s: &str) -> Vec<&'static str> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let has = |needle: &str| compact.contains(needle);
    let entries = [
        ("clone_call", has(".clone(")),
        ("collect_call", has(".collect")),
        ("to_vec", has(".to_vec(")),
        ("to_owned", has(".to_owned(")),
        ("format_macro", has("format!(")),
        ("vec_new", has("Vec::new(") || has("Vec::with_capacity(")),
        ("mem_take", has("mem::take(")),
        ("mem_replace", has("mem::replace(")),
        ("refcell", has("RefCell<")),
        ("rc", has("Rc<")),
        ("arc_mutex", has("Arc<Mutex<") || has("Arc<RwLock<") || has("Arc<std::sync::Mutex<") || has("Arc<tokio::sync::RwLock<") || has("Arc<tokio::sync::Mutex<")),
        ("os_thread_spawn", has("thread::spawn(") || has("thread::scope(")),
        ("task_spawn", has("tokio::spawn(") || has("async_std::spawn(")),
        ("unsafe_keyword", s.contains("unsafe {") || s.contains("unsafe fn") || s.contains("unsafe impl") || s.contains("unsafe trait") || has("unsafe{")),
    ];
    entries.into_iter().filter(|(_, yes)| *yes).map(|(rule, _)| rule).collect()
}

/// Replaces the contents of comments and string/char literals with spaces, preserving
/// newlines so line numbers stay valid. Handles nested block comments, escapes, raw
/// strings (`r#"..."#`, `br"..."`) and the char-literal vs lifetime ambiguity.
pub fn strip_literals_and_comments(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let blank = |ch: char, out: &mut String| out.push(if ch == '\n' { '\n' } else { ' ' });
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if ch == '/' && next == Some('*') {
            let mut depth = 0usize;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(c[i], &mut out);
                    i += 1;
                }
            }
        } else if let Some(hashes) = raw_string_start(&c, i) {
            // Keep the prefix (`r`, `br`, hashes, quote) as spaces too.
            let prefix_end = c[i..].iter().position(|&x| x == '"').map_or(c.len(), |p| i + p + 1);
            while i < prefix_end {
                out.push(' ');
                i += 1;
            }
            while i < c.len() {
                if c[i] == '"' && (0..hashes).all(|k| c.get(i + 1 + k) == Some(&'#')) {
                    for _ in 0..=hashes {
                        out.push(' ');
                    }
                    i += 1 + hashes;
                    break;
                }
                blank(c[i], &mut out);
                i += 1;
            }
        } else if ch == '"' {
            out.push(' ');
            i += 1;
            while i < c.len() {
                if c[i] == '\\' {
                    blank(c[i], &mut out);
                    if let Some(&n) = c.get(i + 1) {
                        blank(n, &mut out);
                    }
                    i += 2;
                } else if c[i] == '"' {
                    out.push(' ');
                    i += 1;
                    break;
                } else {
                    blank(c[i], &mut out);
                    i += 1;
                }
            }
        } else if ch == '\'' && is_char_literal(&c, i) {
            out.push(' ');
            i += 1;
            while i < c.len() && c[i] != '\'' {
                if c[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < c.len() {
                    blank(c[i], &mut out);
                    i += 1;
                }
            }
            if i < c.len() {
                out.push(' ');
                i += 1;
            }
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// `r"`, `r#"`, `br"`, `br#"` not preceded by an identifier character.
fn raw_string_start(c: &[char], i: usize) -> Option<usize> {
    if i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_') {
        return None;
    }
    let mut j = i;
    if c.get(j) == Some(&'b') {
        j += 1;
    }
    if c.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while c.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    (c.get(j) == Some(&'"')).then_some(hashes)
}

/// `'x'`, `'\n'`, `'\u{..}'` are char literals; `'a` (lifetime/label) is not.
fn is_char_literal(c: &[char], i: usize) -> bool {
    matches!((c.get(i + 1), c.get(i + 2)), (Some('\\'), _) | (Some(_), Some('\'')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signals_are_candidates() {
        let s = signals("let x = items.clone(); std::mem::take(&mut state);");
        assert!(s.contains(&"clone_call"));
        assert!(s.contains(&"mem_take"));
    }

    #[test]
    fn literals_and_comments_do_not_match() {
        let src = "let a = \"unsafe fn x.clone()\"; // Arc<Mutex<T>>\n/* nested /* .collect() */ Rc< */ let b = r#\"Vec::new()\"#;\nlet c = '\"'; let d: &'static str = x.clone();";
        let scan = scan("f.rs", src);
        let rules: Vec<_> = scan.findings.iter().map(|f| (f.rule.as_str(), f.line)).collect();
        assert_eq!(rules, vec![("clone_call", 3)], "only the real clone on line 3 should match");
    }

    #[test]
    fn stripping_preserves_line_count() {
        let src = "a\n\"multi\nline\"\n/* x\n y */ b";
        assert_eq!(strip_literals_and_comments(src).lines().count(), src.lines().count());
    }
}
