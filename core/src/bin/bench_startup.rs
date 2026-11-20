//! What the window asks the core as it opens, timed, headless and read-only: `bench_startup <instance> <data dir>
//! <out.json> [rounds]`. The instance is registered in `<data dir>` (Bunker's own registry, made there), then each
//! round asks `instances.list`, `mods.list`, `profiles.list`, `config.list`, `snapshots.list` one after another, then
//! all at once on threads of their own as the app does. `out.json` holds the timings and, of each mod, what the client
//! check and the localization count said (to set one build's answers against another's). Nothing is written in the
//! instance: these requests only read it.

use bunker_core::api;
use serde_json::{json, Value};
use std::time::Instant;

const OPS: &[&str] = &["instances.list", "mods.list", "profiles.list", "config.list", "snapshots.list"];

fn ask(op: &str, inst: &str) -> (f64, Value) {
    let t = Instant::now();
    let r = api::dispatch(&json!({"op": op, "instance": inst}));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    match r {
        Ok(v) => (ms, v),
        Err(e) => panic!("{op}: {e}"),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("bench_startup <instance> <data dir> <out.json> [rounds]");
        std::process::exit(2);
    }
    let (inst, data, out) = (&a[1], &a[2], &a[3]);
    let rounds: usize = a.get(4).and_then(|r| r.parse().ok()).unwrap_or(3);
    let t = Instant::now();
    api::dispatch(&json!({"op": "init", "data_dir": data, "guess": false})).expect("init");
    let init_ms = t.elapsed().as_secs_f64() * 1000.0;
    api::dispatch(&json!({"op": "instances.add", "path": inst})).expect("instances.add");
    let mut report = Vec::new();
    let mut mods = Value::Null;
    for round in 1..=rounds {
        let mut seq = serde_json::Map::new();
        let t = Instant::now();
        for op in OPS {
            let (ms, v) = ask(op, inst);
            seq.insert(op.to_string(), json!((ms * 10.0).round() / 10.0));
            if *op == "mods.list" {
                mods = v;
            }
        }
        let seq_ms = t.elapsed().as_secs_f64() * 1000.0;
        // all at once, as the window sends them when it opens
        let t = Instant::now();
        let hs: Vec<_> = OPS.iter().map(|op| {
            let (op, inst) = (op.to_string(), inst.clone());
            std::thread::spawn(move || ask(&op, &inst).0)
        }).collect();
        let par: Vec<f64> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        let par_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("round {round}: one after another {seq_ms:.1} ms {seq:?}; all at once {par_ms:.1} ms");
        report.push(json!({"round": round, "sequential_ms": (seq_ms * 10.0).round() / 10.0, "each_ms": seq, "parallel_ms": (par_ms * 10.0).round() / 10.0, "parallel_each_ms": par}));
    }
    let answers: Vec<Value> = mods["mods"].as_array().cloned().unwrap_or_default().iter().map(|m| {
        let names = |k: &str| -> Vec<Value> { m[k].as_array().cloned().unwrap_or_default().iter().map(|r| r["name"].clone()).collect() };
        json!({
            "name": m["name"], "roots": m["roots"], "enabled": m["enabled"], "version": m["version"],
            "loc": m["loc"], "dll_status": m["dll_status"], "built_for": m["built_for"], "bad_dlls": m["bad_dlls"],
            "required_by": names("required_by"), "requires": names("requires"), "fix_url": m["fix_url"],
        })
    }).collect();
    let doc = json!({"instance": inst, "init_ms": init_ms, "rounds": report, "mods": answers});
    std::fs::write(out, serde_json::to_vec_pretty(&doc).unwrap()).expect("write out");
    println!("mods: {}; written {out}", answers.len());
}
