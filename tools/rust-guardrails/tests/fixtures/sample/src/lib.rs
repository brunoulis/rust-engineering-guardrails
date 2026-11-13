//! Fixture: every construct below is intentional. Line numbers are asserted by tests/cli.rs.
#![allow(dead_code, clippy::all)]

pub mod hot;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct Shared {
    pub graph: Rc<RefCell<Vec<u32>>>,
    pub counter: Arc<Mutex<u64>>,
}

/// Mentions `unsafe fn` and `.clone()` only inside strings and comments: must not be flagged by AST.
pub fn decoy() -> &'static str {
    // x.clone(); unsafe { } Arc<Mutex<T>>
    "unsafe fn x.clone() Arc<Mutex<T>>"
}

pub fn per_item(names: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for n in names.to_vec() {
        let owned = n.clone();
        out.push(format!("{owned}!"));
    }
    out
}

pub fn spawn_all(n: usize) {
    let (tx, _rx) = std::sync::mpsc::channel::<usize>();
    for i in 0..n {
        let tx = tx.clone();
        std::thread::spawn(move || tx.send(i).unwrap());
    }
}

pub async fn slow() {
    std::thread::sleep(std::time::Duration::from_millis(1));
}

pub fn read(p: *const u8) -> u8 {
    unsafe { *p }
}

pub fn later() -> u8 {
    todo!()
}

#[cfg(test)]
mod tests {
    #[test]
    fn clones_in_tests_are_ignored() {
        let v = vec![1];
        let _ = v.clone();
        Some(1).unwrap();
    }
}
