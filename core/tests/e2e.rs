//! The core end to end in a made-up instance: a stand-in server (src/bin/fake_spt.rs) started, its log read, its
//! launcher routes over HTTPS, a character logged in and the "game" started with SPT's arguments, the server stopped
//! by Ctrl+C; a mod installed from a GitHub release (a local stand-in for GitHub's API); the C ABI's answers.

use bunker_core::{api, events, game, instance, server, spt_http};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bunker-e2e-{tag}-{}-{}", std::process::id(), bunker_core::util::now_ms()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn touch(p: &Path, data: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, data).unwrap();
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

const FAKE: &str = env!("CARGO_BIN_EXE_fake_spt");

/// A 4.1 layout whose server and game are the stand-in, on a port of its own (never the real 6969).
fn instance_with_fake(tag: &str) -> (PathBuf, u16) {
    let r = temp(tag);
    let port = free_port();
    fs::create_dir_all(r.join("SPT_Runtime")).unwrap();
    fs::copy(FAKE, r.join("SPT_Runtime/SPT.Server.exe")).unwrap();
    fs::copy(FAKE, r.join("EscapeFromTarkov.exe")).unwrap();
    touch(&r.join("SPT_Runtime/SPT_Data/configs/http.json"), format!("{{\"ip\":\"127.0.0.1\",\"port\":{port}}}").as_bytes());
    touch(&r.join("BepInEx/core/BepInEx.dll"), b"MZ");
    touch(&r.join("BepInEx/cache/stale.dat"), b"old");
    touch(&r.join("SPT_Runtime/user/profiles/6afeab5dc8ca1f0ed4b321be.json"), br#"{"info":{"id":"6afeab5dc8ca1f0ed4b321be","username":"bear","edition":"Edge Of Darkness"}}"#);
    (r, port)
}

#[test]
fn server_login_game_and_graceful_stop() {
    let (r, port) = instance_with_fake("srv");
    let inst = instance::detect(&r);
    assert_eq!(inst.port, port);
    assert!(inst.ok(), "{:?}", inst.problems);
    let seen: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let root = inst.root.clone();
    let k = events::listen(move |t| {
        let v: Value = serde_json::from_str(t).unwrap();
        if v["instance"] == root.as_str() || v["status"]["instance"] == root.as_str() {
            s2.lock().unwrap().push(v);
        }
    });

    let st = server::start(&inst).unwrap();
    assert_eq!(st.state, "starting");
    assert!(server::start(&inst).is_err(), "one server per instance");
    let st = server::wait_ready(&inst, Duration::from_secs(30)).unwrap();
    assert_eq!(st.state, "running");
    // the lines before ready are all read by now
    let t0 = Instant::now();
    while server::status(&inst).errors == 0 && t0.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let st = server::status(&inst);
    assert_eq!((st.warns, st.errors), (1, 1), "{st:?}");
    let log = server::log(&inst, 0);
    let levels: Vec<(&str, &str)> = log.iter().map(|l| (l.level.as_str(), l.text.as_str())).collect();
    assert!(levels.contains(&("warn", "[WARN] GuidingLight: config value out of range, using default")), "{levels:?}");
    assert!(levels.contains(&("error", "[ERROR] Mod Broken failed to load")), "{levels:?}");
    assert!(levels.iter().any(|(l, t)| *l == "info" && t.contains("跳蚤")), "{levels:?}");
    assert!(levels.iter().any(|(l, t)| *l == "ok" && t.contains("Happy playing")), "{levels:?}");

    // the launcher's routes, over HTTPS with the self-signed certificate
    let s = spt_http::Server::new(port);
    let profiles = s.profiles().unwrap();
    assert_eq!(profiles[0].nickname, "BEAR_Main");
    assert_eq!(profiles[0].level, 34);
    assert_eq!(s.editions().unwrap(), vec!["Standard".to_string(), "Edge Of Darkness".to_string()]);
    assert!(s.login("nobody", "").is_err());
    s.register("newbie", "", "Standard").unwrap();

    // the game, for that character, with the caches cleaned
    let tmp = temp("tmp");
    touch(&tmp.join("Battlestate Games/EscapeFromTarkov/x.tmp"), b"t");
    let l = game::launch(&inst, "bear", "", true, &tmp).unwrap();
    assert!(!l.started_server);
    assert_eq!(l.token, "6afeab5dc8ca1f0ed4b321be");
    assert_eq!(l.cleaned.as_ref().unwrap().files, 2);
    let args_file = r.join("game_args.txt");
    let t0 = Instant::now();
    while !args_file.exists() && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let args = fs::read_to_string(&args_file).unwrap();
    assert_eq!(args, game::game_args("6afeab5dc8ca1f0ed4b321be", port));
    let t0 = Instant::now();
    while !seen.lock().unwrap().iter().any(|v| v["ev"] == "game" && v["state"] == "exited") && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(seen.lock().unwrap().iter().any(|v| v["ev"] == "game" && v["state"] == "running"));
    assert!(seen.lock().unwrap().iter().any(|v| v["ev"] == "game" && v["state"] == "exited"));

    // stopped: Ctrl+C first
    let st = server::stop(&inst, Duration::from_secs(10)).unwrap();
    assert_eq!(st.state, "stopped");
    assert!(!spt_http::port_open(port));
    if cfg!(windows) {
        assert!(st.message.is_empty(), "stopped by Ctrl+C, not ended: {st:?}");
        let t0 = Instant::now();
        while !server::log(&inst, 0).iter().any(|l| l.text.contains("Saving profiles")) && t0.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(server::log(&inst, 0).iter().any(|l| l.text.contains("Saving profiles")), "the server had its Ctrl+C");
    }
    let states: Vec<String> = seen.lock().unwrap().iter().filter(|v| v["ev"] == "server").map(|v| v["status"]["state"].as_str().unwrap().to_string()).collect();
    assert_eq!(states.first().map(String::as_str), Some("starting"));
    assert!(states.contains(&"running".to_string()));
    assert!(states.contains(&"stopping".to_string()));
    assert_eq!(states.last().map(String::as_str), Some("stopped"), "{states:?}");
    assert!(seen.lock().unwrap().iter().any(|v| v["ev"] == "log" && v["line"]["level"] == "warn"));
    events::unlisten(k);

    // started by the game's launch when it is not running
    let l = game::launch(&inst, "bear", "", false, &tmp).unwrap();
    assert!(l.started_server);
    server::stop(&inst, Duration::from_secs(10)).unwrap();
    let _ = fs::remove_dir_all(&r);
    let _ = fs::remove_dir_all(&tmp);
}

/// A plain-HTTP stand-in for GitHub's API and its downloads.
fn fake_github(version: Arc<Mutex<String>>, zip: Vec<u8>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let b2 = base.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut rd = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            let _ = rd.read_line(&mut line);
            loop {
                let mut h = String::new();
                if rd.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let v = version.lock().unwrap().clone();
            let (ct, body): (&str, Vec<u8>) = if path == "/repos/Tyfon/UIFixes/releases/latest" {
                let j = json!({"tag_name": format!("v{v}"), "name": format!("UI Fixes {v}"), "assets": [
                    {"name": "source.zip", "browser_download_url": format!("{b2}/dl/source.zip")},
                    {"name": format!("Tyfon-UIFixes-{v}.zip"), "browser_download_url": format!("{b2}/dl/uifixes.zip")}
                ]});
                ("application/json", j.to_string().into_bytes())
            } else if path == "/dl/uifixes.zip" {
                ("application/zip", zip.clone())
            } else {
                let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                continue;
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes());
            let _ = s.write_all(&body);
            let mut rest = Vec::new();
            let _ = rd.read(&mut rest);
        }
    });
    base
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        for (n, d) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(d).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn github_release_install_and_update_check_through_requests() {
    let version = Arc::new(Mutex::new("6.0.4".to_string()));
    let zip = zip_of(&[
        ("BepInEx/plugins/Tyfon.UIFixes.dll", b"MZ BepInEx"),
        ("SPT/user/mods/UIFixes/UIFixes.dll", b"MZ SPTarkov.Server.Core"),
    ]);
    let api_base = fake_github(version.clone(), zip);
    std::env::set_var("BUNKER_GITHUB_API", &api_base);
    let (r, _port) = instance_with_fake("gh");
    let data = temp("ghdata");
    api::dispatch(&json!({"op": "init", "data_dir": data, "guess": false})).unwrap();
    let plan = api::dispatch(&json!({"op": "mods.plan_url", "instance": r, "url": "https://github.com/Tyfon/UIFixes/releases/latest"})).unwrap();
    assert_eq!(plan["name"], "UIFixes");
    assert_eq!(plan["version"], "6.0.4");
    assert_eq!(plan["archive"], "Tyfon-UIFixes-6.0.4.zip");
    assert_eq!(plan["adapted"], true);
    assert_eq!(plan["mappings"][1]["to"], "SPT_Runtime/user/mods/UIFixes");
    let done = api::dispatch(&json!({"op": "mods.install", "instance": r, "plan": plan["id"]})).unwrap();
    assert_eq!(done["manifest"]["source"]["kind"], "github");
    assert_eq!(done["manifest"]["source"]["url"], "https://github.com/Tyfon/UIFixes");
    let none = api::dispatch(&json!({"op": "mods.check_updates", "instance": r})).unwrap();
    assert_eq!(none["updates"].as_array().unwrap().len(), 0);
    *version.lock().unwrap() = "6.1.0".into();
    let some = api::dispatch(&json!({"op": "mods.check_updates", "instance": r})).unwrap();
    assert_eq!(some["updates"][0]["latest"], "6.1.0");
    assert_eq!(some["updates"][0]["current"], "6.0.4");
    let list = api::dispatch(&json!({"op": "mods.list", "instance": r})).unwrap();
    assert_eq!(list["mods"][0]["source_kind"], "github");
    assert_eq!(list["enabled"], 1);
    // a dropped file's bytes
    let z2 = zip_of(&[("plugins/Other.dll", b"MZ BepInEx")]);
    let b64 = base64(&z2);
    let p2 = api::dispatch(&json!({"op": "mods.plan_bytes", "instance": r, "name": "Other-1.0.zip", "data": b64})).unwrap();
    assert_eq!(p2["mappings"][0]["to"], "BepInEx/plugins/Other.dll");
    api::dispatch(&json!({"op": "mods.cancel_plan", "instance": r, "plan": p2["id"]})).unwrap();
    let _ = fs::remove_dir_all(&r);
    let _ = fs::remove_dir_all(&data);
}

fn base64(b: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

static GOT: Mutex<Vec<String>> = Mutex::new(Vec::new());

extern "C" fn collect(p: *const u8, n: usize) {
    let s = unsafe { std::slice::from_raw_parts(p, n) };
    GOT.lock().unwrap().push(String::from_utf8_lossy(s).into_owned());
}

#[test]
fn c_abi_posts_answers() {
    bunker_core::bunker_set_post(collect);
    let req = br#"{"op":"instances.probe","path":"/no/such/dir"}"#;
    bunker_core::bunker_call(41, req.as_ptr(), req.len());
    let t0 = Instant::now();
    loop {
        let got = GOT.lock().unwrap().iter().filter_map(|s| serde_json::from_str::<Value>(s).ok()).find(|v| v["id"] == 41);
        if let Some(v) = got {
            assert_eq!(v["ok"], true);
            assert_eq!(v["data"]["has_server"], false);
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(5), "no answer posted");
        std::thread::sleep(Duration::from_millis(20));
    }
}
