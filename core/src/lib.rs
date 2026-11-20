//! Bunker's local core, linked into the app as a static library. The app gives it a poster (`bunker_set_post`, which
//! hands a message to the UI between frames) and sends it requests (`bunker_call`): each runs on a thread of its own
//! and its answer, `{"id": n, "ok": true, "data": …}` or `{"id": n, "ok": false, "error": "…"}`, is posted back, as
//! are the core's own events (`{"ev": "log" | "server" | "game", …}`).

pub mod api;
pub mod archive;
pub mod cache;
pub mod clientcheck;
pub mod clr_meta;
pub mod config;
pub mod events;
pub mod game;
pub mod install;
pub mod instance;
pub mod manifest;
pub mod mods;
pub mod pever;
pub mod profiles;
pub mod server;
pub mod snapshot;
pub mod sources;
pub mod spt_http;
pub mod util;

use serde_json::{json, Value};
use std::sync::OnceLock;

/// The app's poster: a message (UTF-8 JSON, not NUL-terminated) handed to the UI.
pub type PostFn = extern "C" fn(*const u8, usize);

static POST: OnceLock<PostFn> = OnceLock::new();

fn post(text: &str) {
    if let Some(f) = POST.get() {
        f(text.as_ptr(), text.len());
    }
}

/// Sets the poster once; the core's events go to it from then on.
#[no_mangle]
pub extern "C" fn bunker_set_post(f: PostFn) {
    if POST.set(f).is_ok() {
        events::listen(post);
    }
}

/// Answers a request off the caller's thread. `json` is UTF-8, `len` bytes, copied before this returns.
#[no_mangle]
pub extern "C" fn bunker_call(id: i64, json: *const u8, len: usize) {
    let bytes = if json.is_null() || len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(json, len) }.to_vec() };
    std::thread::spawn(move || {
        let answer = answer(id, &bytes);
        post(&answer.to_string());
    });
}

/// As `bunker_call`, with bytes beside the request (a file let go on the window: `mods.plan_bytes`).
#[no_mangle]
pub extern "C" fn bunker_call_data(id: i64, json: *const u8, len: usize, data: *const u8, dlen: usize) {
    let bytes = if json.is_null() || len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(json, len) }.to_vec() };
    let data = if data.is_null() || dlen == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(data, dlen) }.to_vec() };
    std::thread::spawn(move || {
        let answer = answer_with(id, &bytes, Some(data));
        post(&answer.to_string());
    });
}

/// A request's answer (what `bunker_call` posts), a panic included as an error.
pub fn answer(id: i64, bytes: &[u8]) -> Value {
    answer_with(id, bytes, None)
}

pub fn answer_with(id: i64, bytes: &[u8], data: Option<Vec<u8>>) -> Value {
    let r = std::panic::catch_unwind(move || {
        let req: Value = serde_json::from_slice(bytes).map_err(|e| format!("请求不是 JSON：{e}"))?;
        api::dispatch_data(&req, data)
    });
    match r {
        Ok(Ok(data)) => json!({"id": id, "ok": true, "data": data}),
        Ok(Err(e)) => json!({"id": id, "ok": false, "error": e}),
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            json!({"id": id, "ok": false, "error": format!("core 内部错误：{msg}")})
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn answers_and_errors() {
        let a = super::answer(7, br#"{"op":"nope"}"#);
        assert_eq!(a["id"], 7);
        assert_eq!(a["ok"], false);
        assert!(a["error"].as_str().unwrap().contains("未知操作"));
        let b = super::answer(8, b"not json");
        assert_eq!(b["ok"], false);
    }
}
