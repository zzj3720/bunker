//! What the core says without being asked (a server's log line, its state, the game starting and ending): handed
//! to whoever listens (the app's poster, set over the C ABI; a test's collector).

use serde_json::Value;
use std::sync::{Mutex, OnceLock};

type Listener = Box<dyn Fn(&str) + Send + Sync>;

fn listeners() -> &'static Mutex<Vec<(u64, Listener)>> {
    static L: OnceLock<Mutex<Vec<(u64, Listener)>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(Vec::new()))
}

/// Adds a listener; returns its key, for `unlisten`.
pub fn listen(f: impl Fn(&str) + Send + Sync + 'static) -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    listeners().lock().unwrap().push((k, Box::new(f)));
    k
}

pub fn unlisten(k: u64) {
    listeners().lock().unwrap().retain(|(x, _)| *x != k);
}

pub fn emit(v: &Value) {
    let text = v.to_string();
    for (_, f) in listeners().lock().unwrap().iter() {
        f(&text);
    }
}
