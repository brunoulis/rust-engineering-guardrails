//! Path glob used by `--hot` and `--exclude`. Supports `*` and `?`; `*` also crosses `/`,
//! so `crates/sim/**` and `crates/sim/*` both match nested files. Not `.gitignore` syntax.

pub fn matches(pattern: &str, input: &str) -> bool {
    let p: Vec<char> = pattern.replace('\\', "/").chars().collect();
    let s: Vec<char> = input.chars().collect();
    let (mut i, mut j, mut star, mut matched) = (0, 0, None, 0);
    while j < s.len() {
        if i < p.len() && (p[i] == s[j] || p[i] == '?') {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            matched = j;
            i += 1;
        } else if let Some(x) = star {
            i = x + 1;
            matched += 1;
            j = matched;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

pub fn any(patterns: &[String], input: &str) -> bool {
    patterns.iter().any(|g| matches(g, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_nested_paths() {
        assert!(matches("crates/sim/**", "crates/sim/src/lib.rs"));
        assert!(!matches("crates/api/**", "crates/sim/src/lib.rs"));
        assert!(matches("src/?ib.rs", "src/lib.rs"));
        assert!(matches("*.rs", "a/b.rs"));
        assert!(matches("crates\\sim\\*", "crates/sim/x.rs"));
    }
}
