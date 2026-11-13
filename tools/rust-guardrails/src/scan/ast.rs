//! Syntactic engine built on `syn`. Compared with the text heuristic it
//! - ignores strings, comments and test code (`#[cfg(test)]`, `#![cfg(test)]`, `#[test]`,
//!   and files of out-of-line `#[cfg(test)] mod x;` declarations),
//! - knows loop bodies (vs. the iterator expression evaluated once),
//! - knows async context, reset for closures handed to blocking offload APIs
//!   (`spawn_blocking`, `block_in_place`, thread spawns),
//! - treats error-path closures (`map_err`, `ok_or_else`, ...) and `return` as cold,
//! - looks inside format-like macro arguments,
//! - checks `SAFETY:` comments and `# Safety` docs next to unsafe code,
//! - measures functions and types.
//!
//! It still has no type information: `x.clone()` on an `Arc` and on a `Vec` look the same.

use proc_macro2::Span;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, GenericArgument, PathArguments, Token, Type};

use super::{FileScan, TypeStat};
use crate::model::{excerpt, Confidence, Finding};
use crate::rules;

pub fn scan(rel: &str, content: &str, max_fn_lines: usize) -> Result<FileScan, String> {
    let file = syn::parse_file(content).map_err(|e| {
        let at = e.span().start();
        format!("syn parse error at {}:{}: {e}", at.line, at.column + 1)
    })?;
    if is_test_attr(&file.attrs) {
        // `#![cfg(test)]`: the whole file is test code.
        return Ok(FileScan { test_only: true, ..FileScan::default() });
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut v =
        Visitor { rel, lines: &lines, max_fn_lines, out: FileScan::default(), loop_depth: 0, async_depth: 0, awaited_next: false, async_loop_base: 0, stmt_line: 0, fn_name: None };
    v.visit_file(&file);
    Ok(v.out)
}

struct Visitor<'a> {
    rel: &'a str,
    lines: &'a [&'a str],
    max_fn_lines: usize,
    out: FileScan,
    loop_depth: usize,
    async_depth: usize,
    /// Set by `visit_expr_await`; consumed by the next call/method-call so that
    /// `x.lock().await` or `fs::read(p).await` are recognized as async APIs.
    awaited_next: bool,
    /// Loop depth when the innermost async body was entered: an `.await` is sequential
    /// per iteration only if a loop encloses it *inside* that async body.
    async_loop_base: usize,
    /// First line of the innermost enclosing statement (for SAFETY comments above a
    /// multi-line statement).
    stmt_line: usize,
    fn_name: Option<String>,
}

/// The 1-based source line, trimmed, for evidence.
fn line_text(lines: &[&str], line: usize) -> String {
    lines.get(line.wrapping_sub(1)).map(|l| excerpt(l, 180)).unwrap_or_default()
}

fn line_of(span: Span) -> usize {
    span.start().line
}

/// Splits `a, b(c, d), e` at top-level commas.
fn top_level_args(s: &str) -> Vec<&str> {
    let (mut depth, mut start, mut out) = (0i32, 0usize, Vec::new());
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out.retain(|a| !a.is_empty());
    out
}

/// True when a `cfg(...)` predicate can only hold in test builds: `test`, or `all(..)`
/// with `test` (or a test-only predicate) as a direct argument. `any(test, x)` and
/// `not(test)` are not test-only.
pub fn cfg_is_test_only(pred: &str) -> bool {
    let p: String = pred.chars().filter(|c| !c.is_whitespace()).collect();
    if p == "test" {
        return true;
    }
    match p.strip_prefix("all(").and_then(|r| r.strip_suffix(')')) {
        Some(inner) => top_level_args(inner).into_iter().any(cfg_is_test_only),
        None => false,
    }
}

fn is_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        let p = a.path();
        if p.segments.last().is_some_and(|s| s.ident == "test" || s.ident == "bench") {
            return true;
        }
        if p.is_ident("cfg") {
            if let syn::Meta::List(l) = &a.meta {
                return cfg_is_test_only(&l.tokens.to_string());
            }
        }
        false
    })
}

fn has_safety_doc(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("doc") {
            return false;
        }
        if let syn::Meta::NameValue(nv) = &a.meta {
            if let Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                let v = s.value();
                return v.contains("# Safety") || v.contains("SAFETY") || v.contains("# Panics and Safety");
            }
        }
        false
    })
}

fn path_string(p: &syn::Path) -> String {
    p.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
}

/// Last-segment name and its first generic type argument (e.g. `Rc<RefCell<T>>` → ("Rc", RefCell<T>)).
fn generic_head(ty: &Type) -> Option<(String, Option<&Type>)> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    let inner = match &seg.arguments {
        PathArguments::AngleBracketed(a) => a.args.iter().find_map(|g| match g {
            GenericArgument::Type(t) => Some(t),
            _ => None,
        }),
        _ => None,
    };
    Some((seg.ident.to_string(), inner))
}

impl Visitor<'_> {
    fn push(&mut self, rule: &'static str, line: usize, detail: Option<String>) {
        // One finding per rule and line: `(a.clone(), b.clone())` is one review item.
        if self.out.findings.iter().rev().take(16).any(|f| f.rule == rule && f.line == line) {
            return;
        }
        let mut f = rules::finding(rule, Confidence::Syntactic).at(self.rel, line).evidence(line_text(self.lines, line));
        if let Some(d) = detail {
            f.message = format!("{} — {d}", f.message);
        }
        if let Some(name) = &self.fn_name {
            f.message = format!("{} (in `{name}`)", f.message);
        }
        self.out.findings.push(f);
    }

    fn push_finding(&mut self, f: Finding) {
        self.out.findings.push(f);
    }

    /// True if `line` itself, or the contiguous comment/attribute block directly above it,
    /// mentions SAFETY (same convention as `clippy::undocumented_unsafe_blocks`): a comment
    /// justifying one unsafe block does not cover the next one.
    fn has_safety_comment(&self, line: usize) -> bool {
        let mentions = |l: &str| l.contains("SAFETY") || l.contains("Safety:");
        let Some(own) = line.checked_sub(1).and_then(|i| self.lines.get(i)) else { return false };
        if mentions(own) {
            return true;
        }
        let mut i = line - 1; // index of the line above, exclusive upper bound
        while i > 0 {
            i -= 1;
            let t = self.lines[i].trim();
            if t.starts_with("//") || t.starts_with("#[") || t.starts_with("#!") {
                if mentions(t) {
                    return true;
                }
            } else if t.ends_with("*/") {
                // Block comment ending here: scan up to its opening `/*`.
                loop {
                    if mentions(self.lines[i]) {
                        return true;
                    }
                    if self.lines[i].contains("/*") || i == 0 {
                        break;
                    }
                    i -= 1;
                }
            } else {
                return false;
            }
        }
        false
    }

    /// SAFETY justification on the unsafe line, above it, or above its enclosing statement.
    fn is_justified(&self, line: usize) -> bool {
        self.has_safety_comment(line) || (self.stmt_line > 0 && self.stmt_line < line && self.has_safety_comment(self.stmt_line))
    }

    /// Visits `f` as code that runs on a blocking thread, not on the async executor.
    fn outside_async<F: FnOnce(&mut Self)>(&mut self, f: F) {
        let saved = (self.async_depth, self.async_loop_base);
        self.async_depth = 0;
        f(self);
        (self.async_depth, self.async_loop_base) = saved;
    }

    /// Visits `f` as code that runs at most once per call (cold path), even inside a loop.
    fn cold<F: FnOnce(&mut Self)>(&mut self, f: F) {
        let saved = (self.loop_depth, self.async_loop_base);
        (self.loop_depth, self.async_loop_base) = (0, 0);
        f(self);
        (self.loop_depth, self.async_loop_base) = saved;
    }

    fn in_fn<F: FnOnce(&mut Self)>(&mut self, name: String, is_async: bool, f: F) {
        let saved = (self.loop_depth, self.async_depth, self.async_loop_base, self.fn_name.take());
        self.loop_depth = 0;
        self.async_depth = usize::from(is_async);
        self.async_loop_base = 0;
        self.fn_name = Some(name);
        f(self);
        self.loop_depth = saved.0;
        self.async_depth = saved.1;
        self.async_loop_base = saved.2;
        self.fn_name = saved.3;
    }

    fn check_fn_len(&mut self, name: &str, start: Span, block: &syn::Block) {
        let (s, e) = (line_of(start), block.brace_token.span.close().end().line);
        let len = e.saturating_sub(s) + 1;
        if len > self.max_fn_lines {
            let f = rules::finding("long_function", Confidence::Syntactic)
                .at(self.rel, s)
                .msg(format!("Long function `{name}`"))
                .evidence(format!("{len} lines (threshold {})", self.max_fn_lines));
            self.push_finding(f);
        }
    }

    fn visit_loop_body(&mut self, body: &syn::Block) {
        self.loop_depth += 1;
        self.visit_block(body);
        self.loop_depth -= 1;
    }

    fn type_stat(&mut self, name: String) -> &mut TypeStat {
        if let Some(i) = self.out.types.iter().position(|(n, _)| *n == name) {
            return &mut self.out.types[i].1;
        }
        self.out.types.push((name, TypeStat { file: self.rel.to_string(), ..Default::default() }));
        &mut self.out.types.last_mut().expect("just pushed").1
    }
}

const ALLOC_METHODS: &[&str] = &["collect", "to_vec", "to_owned", "to_string", "into_owned"];
/// Methods whose closure arguments run only on the error/absent path (cold), so work in
/// them is not per-iteration work even inside a loop.
const COLD_ADAPTERS: &[&str] = &["map_err", "ok_or_else", "unwrap_or_else", "or_else", "with_context", "inspect_err"];
/// Calls whose closure argument runs on a blocking thread, outside the async executor.
const BLOCKING_OFFLOAD: &[&str] = &["spawn_blocking", "block_in_place", "spawn", "scope"];
const ALLOC_MACROS: &[&str] = &["format", "vec"];
/// Macros whose bodies are comma-separated expressions worth visiting.
const EXPR_MACROS: &[&str] = &[
    "format",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "vec",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "panic",
    "dbg",
    "format_args",
];

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        if !is_test_attr(&m.attrs) {
            visit::visit_item_mod(self, m);
        } else if m.content.is_none() {
            // `#[cfg(test)] mod tests;` — its file is test code; scan/mod.rs drops it.
            self.out.test_modules.push(m.ident.to_string());
        }
    }

    fn visit_stmt(&mut self, s: &'ast syn::Stmt) {
        let saved = self.stmt_line;
        self.stmt_line = s.span().start().line;
        visit::visit_stmt(self, s);
        self.stmt_line = saved;
    }

    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        if is_test_attr(&f.attrs) {
            return;
        }
        let name = f.sig.ident.to_string();
        if let Some(u) = &f.sig.unsafety {
            if !has_safety_doc(&f.attrs) {
                self.push("unsafe_fn_without_safety_doc", line_of(u.span), Some(format!("`{name}`")));
            }
        }
        self.check_fn_len(&name, f.sig.fn_token.span, &f.block);
        // `fn_name` is also how `process::exit` in `main` is recognized as expected.
        self.in_fn(name, f.sig.asyncness.is_some(), |v| visit::visit_item_fn(v, f));
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        if is_test_attr(&f.attrs) {
            return;
        }
        let name = f.sig.ident.to_string();
        if let Some(u) = &f.sig.unsafety {
            if !has_safety_doc(&f.attrs) {
                self.push("unsafe_fn_without_safety_doc", line_of(u.span), Some(format!("`{name}`")));
            }
        }
        self.check_fn_len(&name, f.sig.fn_token.span, &f.block);
        self.in_fn(name, f.sig.asyncness.is_some(), |v| visit::visit_impl_item_fn(v, f));
    }

    fn visit_trait_item_fn(&mut self, f: &'ast syn::TraitItemFn) {
        let name = f.sig.ident.to_string();
        self.in_fn(name, f.sig.asyncness.is_some(), |v| visit::visit_trait_item_fn(v, f));
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        if is_test_attr(&i.attrs) {
            return;
        }
        if let Some(u) = &i.unsafety {
            let line = line_of(u.span);
            if !self.has_safety_comment(line) {
                self.push("unsafe_impl", line, Some("no `SAFETY:` comment".into()));
            } else {
                self.push("unsafe_impl", line, None);
            }
        }
        if let Some((name, _)) = generic_head(&i.self_ty) {
            let methods = if i.trait_.is_none() { i.items.iter().filter(|it| matches!(it, syn::ImplItem::Fn(_))).count() } else { 0 };
            let line = line_of(i.impl_token.span);
            let t = self.type_stat(name);
            if t.line == 0 {
                t.line = line;
            }
            t.inherent_methods += methods;
            t.impl_blocks += 1;
        }
        visit::visit_item_impl(self, i);
    }

    fn visit_item_struct(&mut self, s: &'ast syn::ItemStruct) {
        if is_test_attr(&s.attrs) {
            return;
        }
        let line = line_of(s.struct_token.span);
        let fields = s.fields.len();
        let t = self.type_stat(s.ident.to_string());
        t.fields = fields;
        t.line = line;
        visit::visit_item_struct(self, s);
    }

    fn visit_item_static(&mut self, s: &'ast syn::ItemStatic) {
        if let syn::StaticMutability::Mut(m) = &s.mutability {
            self.push("static_mut", line_of(m.span), Some(format!("`{}`", s.ident)));
        }
        visit::visit_item_static(self, s);
    }

    fn visit_expr_for_loop(&mut self, e: &'ast syn::ExprForLoop) {
        // The iterator expression runs once; only the body is per-iteration.
        self.visit_pat(&e.pat);
        self.visit_expr(&e.expr);
        self.visit_loop_body(&e.body);
    }

    fn visit_expr_while(&mut self, e: &'ast syn::ExprWhile) {
        // The condition runs per iteration too, but it is the loop's driver
        // (`while let Some(x) = rx.recv().await` is the canonical consumer), so it is
        // treated like a `for` iterator expression.
        self.visit_expr(&e.cond);
        self.visit_loop_body(&e.body);
    }

    fn visit_expr_loop(&mut self, e: &'ast syn::ExprLoop) {
        self.visit_loop_body(&e.body);
    }

    fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
        // A sync closure inside async code (e.g. `.map(|p| fs::read(p))`) still runs on
        // the executor; only blocking-offload calls reset the context (see `outside_async`).
        if c.asyncness.is_some() {
            let saved = (self.async_depth, self.async_loop_base);
            self.async_depth += 1;
            self.async_loop_base = self.loop_depth;
            visit::visit_expr_closure(self, c);
            (self.async_depth, self.async_loop_base) = saved;
        } else {
            visit::visit_expr_closure(self, c);
        }
    }

    fn visit_expr_async(&mut self, a: &'ast syn::ExprAsync) {
        let saved = self.async_loop_base;
        self.async_depth += 1;
        self.async_loop_base = self.loop_depth;
        visit::visit_expr_async(self, a);
        self.async_depth -= 1;
        self.async_loop_base = saved;
    }

    fn visit_expr_await(&mut self, a: &'ast syn::ExprAwait) {
        if self.loop_depth > self.async_loop_base {
            self.push("await_in_loop", line_of(a.await_token.span), None);
        }
        self.awaited_next = matches!(&*a.base, Expr::Call(_) | Expr::MethodCall(_));
        visit::visit_expr_await(self, a);
        self.awaited_next = false;
    }

    fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
        // Consume the await marker so it does not leak to calls inside the receiver/args.
        self.awaited_next = false;
        let name = m.method.to_string();
        let line = line_of(m.method.span());
        match name.as_str() {
            "clone" if m.args.is_empty() => {
                let rule = if self.loop_depth > 0 { "clone_in_loop" } else { "clone_call" };
                self.push(rule, line, None);
            }
            n if self.loop_depth > 0 && ALLOC_METHODS.contains(&n) => self.push("allocation_in_loop", line, Some(format!("`.{n}()`"))),
            "unwrap" | "expect" => {
                self.push("unwrap_in_non_test", line, Some(format!("`.{name}()`")));
                // `m.lock().unwrap()` is a std (poisoning) lock; async locks are awaited
                // instead. Bare `.lock()` (parking_lot) is not flagged: it is
                // indistinguishable from an async lock future without types.
                if let Expr::MethodCall(inner) = &*m.receiver {
                    let lock = inner.method.to_string();
                    if self.async_depth > 0 && inner.args.is_empty() && matches!(lock.as_str(), "lock" | "read" | "write") {
                        self.push("sync_lock_in_async", line_of(inner.method.span()), Some(format!("`.{lock}().{name}()`")));
                    }
                }
            }
            "block_on" if self.async_depth > 0 => self.push("block_on_in_async", line, None),
            "spawn" | "spawn_blocking" | "spawn_local" if self.loop_depth > 0 && !m.args.is_empty() => self.push("spawn_in_loop", line, Some(format!("`.{name}(..)`"))),
            _ => {}
        }
        if COLD_ADAPTERS.contains(&name.as_str()) && self.loop_depth > 0 {
            self.visit_expr(&m.receiver);
            self.cold(|v| m.args.iter().for_each(|a| v.visit_expr(a)));
        } else if BLOCKING_OFFLOAD.contains(&name.as_str()) {
            self.visit_expr(&m.receiver);
            self.outside_async(|v| m.args.iter().for_each(|a| v.visit_expr(a)));
        } else {
            visit::visit_expr_method_call(self, m);
        }
    }

    fn visit_expr_return(&mut self, r: &'ast syn::ExprReturn) {
        // The returned expression is evaluated once per call, not per iteration.
        self.cold(|v| visit::visit_expr_return(v, r));
    }

    fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
        let awaited = std::mem::take(&mut self.awaited_next);
        if let Expr::Path(p) = &*c.func {
            let path = path_string(&p.path);
            let last = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            let line = p.path.segments.last().map_or(0, |s| line_of(s.ident.span()));
            let ends = |suffix: &str| path == suffix || path.ends_with(&format!("::{suffix}"));
            let in_loop = self.loop_depth > 0;
            if ends("thread::spawn") {
                self.push(if in_loop { "spawn_in_loop" } else { "thread_spawn" }, line, Some(format!("`{path}`")));
            } else if ends("tokio::spawn") || ends("task::spawn") || ends("task::spawn_local") || ends("task::spawn_blocking") || ends("async_std::spawn") || path == "spawn_local"
            {
                self.push(if in_loop { "spawn_in_loop" } else { "task_spawn" }, line, Some(format!("`{path}`")));
            } else if last == "unbounded_channel" || ends("crossbeam_channel::unbounded") || ends("flume::unbounded") || (last == "unbounded" && path.contains("channel")) {
                self.push("unbounded_channel", line, Some(format!("`{path}`")));
            } else if (ends("mpsc::channel") || ends("sync::mpsc::channel")) && c.args.is_empty() {
                self.push("unbounded_channel", line, Some("`std::sync::mpsc::channel()` is unbounded".into()));
            } else if ends("mem::take") {
                self.push("mem_take", line, None);
            } else if ends("mem::replace") {
                self.push("mem_replace", line, None);
            } else if ends("mem::transmute") || path == "transmute" {
                self.push("transmute", line, None);
            } else if ends("Box::leak") {
                self.push("box_leak", line, None);
            } else if ends("process::exit") && self.fn_name.as_deref() != Some("main") {
                self.push("process_exit", line, None);
            } else if self.async_depth > 0 && ends("block_on") {
                self.push("block_on_in_async", line, Some(format!("`{path}`")));
            } else if self.async_depth > 0 && !awaited && (ends("thread::sleep") || path.starts_with("std::fs::") || path.starts_with("fs::") && !path.contains("tokio")) {
                self.push("blocking_in_async", line, Some(format!("`{path}`")));
            } else if in_loop && (ends("Vec::with_capacity") || ends("String::with_capacity") || ends("Box::new")) {
                self.push("allocation_in_loop", line, Some(format!("`{path}`")));
            }
            if BLOCKING_OFFLOAD.contains(&last.as_str()) {
                // `spawn_blocking(|| ..)`, `thread::spawn(|| ..)`, `thread::scope(|s| ..)`:
                // the closure runs on a blocking thread. (`tokio::spawn(async {..})` stays
                // async because the async block re-enters async context.)
                self.visit_expr(&c.func);
                self.outside_async(|v| c.args.iter().for_each(|a| v.visit_expr(a)));
                return;
            }
        }
        visit::visit_expr_call(self, c);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let Some(last) = mac.path.segments.last() else { return };
        let name = last.ident.to_string();
        let line = line_of(last.ident.span());
        match name.as_str() {
            n if self.loop_depth > 0 && ALLOC_MACROS.contains(&n) => self.push("allocation_in_loop", line, Some(format!("`{n}!`"))),
            "todo" | "unimplemented" => self.push("unfinished_code", line, Some(format!("`{name}!`"))),
            "panic" => self.push("panic_in_library", line, None),
            _ => {}
        }
        if EXPR_MACROS.contains(&name.as_str()) {
            // Best effort: format strings parse as literal expressions; failures are ignored.
            if let Ok(args) = mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
                for a in &args {
                    self.visit_expr(a);
                }
            } else if name == "vec" {
                // `vec![elem; n]`
                let repeat = |input: ParseStream| -> syn::Result<(Expr, Expr)> {
                    let elem: Expr = input.parse()?;
                    input.parse::<Token![;]>()?;
                    Ok((elem, input.parse()?))
                };
                if let Ok((elem, n)) = mac.parse_body_with(repeat) {
                    self.visit_expr(&elem);
                    self.visit_expr(&n);
                }
            }
        }
    }

    fn visit_expr_unsafe(&mut self, u: &'ast syn::ExprUnsafe) {
        let line = line_of(u.unsafe_token.span);
        let rule = if self.is_justified(line) { "unsafe_block" } else { "unsafe_without_safety_comment" };
        self.push(rule, line, None);
        visit::visit_expr_unsafe(self, u);
    }

    fn visit_type(&mut self, ty: &'ast Type) {
        if let Some((outer, Some(inner))) = generic_head(ty) {
            if let Some((inner_name, _)) = generic_head(inner) {
                let line = match ty {
                    Type::Path(tp) => tp.path.segments.last().map_or(0, |s| line_of(s.ident.span())),
                    _ => 0,
                };
                match (outer.as_str(), inner_name.as_str()) {
                    ("Rc", "RefCell") => self.push("rc_refcell", line, None),
                    ("Arc", "Mutex" | "RwLock") => self.push("arc_mutex", line, Some(format!("`Arc<{inner_name}<_>>`"))),
                    _ => {}
                }
            }
        }
        visit::visit_type(self, ty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_of(src: &str) -> Vec<(String, usize)> {
        scan("f.rs", src, 80).unwrap().findings.into_iter().map(|f| (f.rule, f.line)).collect()
    }

    #[test]
    fn ignores_strings_comments_and_tests() {
        let src = r#"
fn f() -> &'static str {
    // x.clone() unsafe { }
    "unsafe fn Arc<Mutex<T>> .clone()"
}
#[cfg(test)]
mod tests { fn t() { let _ = vec![1].clone(); } }
#[test]
fn t2() { None::<u8>.unwrap(); }
"#;
        assert!(rules_of(src).is_empty(), "{:?}", rules_of(src));
    }

    #[test]
    fn distinguishes_loop_body_from_iterator() {
        let src = "fn f(v: &Vec<String>) {\n    for s in v.clone() {\n        let t = s.clone();\n        let u: Vec<u8> = t.bytes().collect();\n        let w = format!(\"{}\", u.len());\n    }\n}\n";
        let r = rules_of(src);
        assert!(r.contains(&("clone_call".into(), 2)), "{r:?}");
        assert!(r.contains(&("clone_in_loop".into(), 3)), "{r:?}");
        assert!(r.contains(&("allocation_in_loop".into(), 4)), "{r:?}");
        assert!(r.contains(&("allocation_in_loop".into(), 5)), "{r:?}");
    }

    #[test]
    fn async_context_and_spawn_blocking() {
        let src = "async fn f(m: std::sync::Mutex<u8>, t: tokio::sync::Mutex<u8>) {\n    std::thread::sleep(d);\n    let g = m.lock().unwrap();\n    let h = t.lock().await;\n    tokio::task::spawn_blocking(move || std::fs::read(\"x\"));\n    let x = tokio::fs::read(\"y\").await;\n}\n";
        let r = rules_of(src);
        assert!(r.contains(&("blocking_in_async".into(), 2)), "{r:?}");
        assert!(r.contains(&("sync_lock_in_async".into(), 3)), "{r:?}");
        assert_eq!(r.iter().filter(|(k, _)| k == "sync_lock_in_async").count(), 1, "{r:?}");
        assert_eq!(r.iter().filter(|(k, _)| k == "blocking_in_async").count(), 1, "{r:?}");
    }

    #[test]
    fn spawn_in_loop_and_channels() {
        let src = "fn f() {\n    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();\n    let (a, b) = std::sync::mpsc::channel();\n    let (c, d) = tokio::sync::mpsc::channel(8);\n    for i in 0..n {\n        tokio::spawn(async move { work(i).await });\n    }\n}\n";
        let r = rules_of(src);
        assert!(r.contains(&("unbounded_channel".into(), 2)), "{r:?}");
        assert!(r.contains(&("unbounded_channel".into(), 3)), "{r:?}");
        assert!(!r.iter().any(|(k, l)| k == "unbounded_channel" && *l == 4), "{r:?}");
        assert!(r.contains(&("spawn_in_loop".into(), 6)), "{r:?}");
    }

    #[test]
    fn unsafe_documentation() {
        let src = "fn f(p: *const u8) -> u8 {\n    // SAFETY: p is valid for reads.\n    let a = unsafe { *p };\n    let b = unsafe { *p };\n    a + b\n}\nunsafe fn g() {}\n/// # Safety\n/// caller ensures x\nunsafe fn h() {}\nstatic mut X: u8 = 0;\n";
        let r = rules_of(src);
        assert!(r.contains(&("unsafe_block".into(), 3)), "{r:?}");
        assert!(r.contains(&("unsafe_without_safety_comment".into(), 4)), "{r:?}");
        assert!(r.contains(&("unsafe_fn_without_safety_doc".into(), 7)), "{r:?}");
        assert!(!r.iter().any(|(k, l)| k == "unsafe_fn_without_safety_doc" && *l == 10), "{r:?}");
        assert!(r.contains(&("static_mut".into(), 11)), "{r:?}");
    }

    #[test]
    fn types_and_macro_bodies() {
        let src = "struct S { a: std::rc::Rc<std::cell::RefCell<u8>>, b: Arc<Mutex<u8>> }\nimpl S { fn x(&self) {} fn y(&self) { println!(\"{}\", self.a.clone()); todo!() } }\n";
        let out = scan("f.rs", src, 80).unwrap();
        let r: Vec<_> = out.findings.iter().map(|f| (f.rule.as_str(), f.line)).collect();
        assert!(r.contains(&("rc_refcell", 1)), "{r:?}");
        assert!(r.contains(&("arc_mutex", 1)), "{r:?}");
        assert!(r.contains(&("clone_call", 2)), "{r:?}");
        assert!(r.contains(&("unfinished_code", 2)), "{r:?}");
        let s = &out.types.iter().find(|(n, _)| n == "S").unwrap().1;
        assert_eq!((s.fields, s.inherent_methods, s.impl_blocks), (2, 2, 1));
    }

    #[test]
    fn cold_paths_and_duplicates() {
        let src = "fn f(v: &[u8]) -> Result<(), String> {\n    for x in v {\n        let y = g(x).map_err(|e| format!(\"{e}\"))?;\n        if y { return Err(format!(\"bad {x}\")); }\n        let t = (a.clone(), b.clone());\n    }\n    Ok(())\n}\n";
        let r = rules_of(src);
        assert!(!r.iter().any(|(k, _)| k == "allocation_in_loop"), "{r:?}");
        assert_eq!(r.iter().filter(|(k, l)| k == "clone_in_loop" && *l == 5).count(), 1, "{r:?}");
    }

    // ---- regressions from the independent review ----

    #[test]
    fn safety_comment_does_not_bridge_through_deref_lines() {
        let src = "fn f(p: *mut u8, q: *const u8) {\n    // SAFETY: q valid\n    unsafe { *p = *q };\n    *p += 1;\n    let y = unsafe { *q.add(1) };\n}\n";
        assert!(rules_of(src).contains(&("unsafe_without_safety_comment".into(), 5)), "{:?}", rules_of(src));
    }

    #[test]
    fn safety_in_block_comment_and_above_multiline_statement() {
        let src = "fn f(p: *const u8) -> u8 {\n    /* SAFETY:\n       p is valid */\n    let a = unsafe { *p };\n    // SAFETY: p is valid\n    let v = g(\n        unsafe { *p },\n    );\n    a + v\n}\n";
        let r = rules_of(src);
        assert!(!r.iter().any(|(k, _)| k == "unsafe_without_safety_comment"), "{r:?}");
    }

    #[test]
    fn async_context_edge_cases() {
        let src = "async fn f(v: Vec<String>, m: tokio::sync::Mutex<u8>, mut rx: R) {\n    let _: Vec<_> = v.iter().map(|p| std::fs::read(p)).collect();\n    futures::executor::block_on(g());\n    let _ = tokio::time::timeout(d, m.lock()).await;\n    while let Some(x) = rx.recv().await { h(x); }\n    std::thread::scope(|s| { std::thread::sleep(d); });\n}\n";
        let r = rules_of(src);
        assert!(r.contains(&("blocking_in_async".into(), 2)), "sync closure still on executor: {r:?}");
        assert!(r.contains(&("block_on_in_async".into(), 3)), "{r:?}");
        assert!(!r.iter().any(|(k, _)| k == "sync_lock_in_async"), "{r:?}");
        assert!(!r.iter().any(|(k, _)| k == "await_in_loop"), "while-let consumer: {r:?}");
        assert!(!r.iter().any(|(k, l)| k == "blocking_in_async" && *l == 6), "thread::scope runs off-executor: {r:?}");
    }

    #[test]
    fn method_spawns_and_success_path_closures_in_loops() {
        let src = "fn f(o: Option<&str>, s: &S) {\n    for i in 0..9 {\n        std::thread::Builder::new().spawn(move || i).unwrap();\n        set.spawn(async move { i });\n        let t = o.map_or_else(String::new, |s| s.to_string());\n        let z = vec![String::new(); 3];\n    }\n}\n";
        let r = rules_of(src);
        assert!(r.contains(&("spawn_in_loop".into(), 3)), "{r:?}");
        assert!(r.contains(&("spawn_in_loop".into(), 4)), "{r:?}");
        assert!(r.contains(&("allocation_in_loop".into(), 5)), "map_or_else success closure: {r:?}");
        assert!(r.contains(&("allocation_in_loop".into(), 6)), "vec![x; n]: {r:?}");
    }

    #[test]
    fn cfg_test_predicates() {
        for (p, test_only) in [
            ("test", true),
            ("all(test, feature = \"notify\")", true),
            ("any(test, feature = \"x\")", false),
            ("not(test)", false),
            ("all(unix, any(test, x))", false),
            ("feature = \"testing\"", false),
        ] {
            assert_eq!(cfg_is_test_only(p), test_only, "{p}");
        }
        let src = "#[cfg(all(test, feature = \"notify\"))]\nmod t { fn f() { None::<u8>.unwrap(); } }\n#[cfg(any(test, feature = \"x\"))]\nfn g() { None::<u8>.unwrap(); }\n";
        assert_eq!(rules_of(src), vec![("unwrap_in_non_test".to_string(), 4)]);
        let inner = scan("t.rs", "#![cfg(test)]\nfn f() { None::<u8>.unwrap(); }\n", 80).unwrap();
        assert!(inner.test_only && inner.findings.is_empty());
        let outline = scan("lib.rs", "#[cfg(test)]\nmod tests;\nmod real;\n", 80).unwrap();
        assert_eq!(outline.test_modules, vec!["tests".to_string()]);
    }

    #[test]
    fn long_functions_are_measured() {
        let body = "    let x = 1;\n".repeat(90);
        let src = format!("fn big() {{\n{body}}}\n");
        assert!(rules_of(&src).contains(&("long_function".into(), 1)));
    }

    #[test]
    fn reports_parse_errors() {
        assert!(scan("f.rs", "fn (", 80).is_err());
    }
}
