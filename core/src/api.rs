//! The core's requests: `{"op": "...", ...}` in, a JSON answer out. The C ABI (lib.rs) runs each on a thread of its
//! own and posts the answer back; tests call `dispatch` directly.

use crate::instance::{self, Instance};
use crate::manifest::Source;
use crate::util::*;
use crate::{config, game, install, manifest, mods, profiles, server, snapshot, sources, spt_http};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Registry {
    pub instances: Vec<Entry>,
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub settings: Value,
}

fn data_dir() -> &'static Mutex<Option<PathBuf>> {
    static D: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(None))
}

fn registry_path() -> Res<PathBuf> {
    data_dir().lock().unwrap().clone().map(|d| d.join("instances.json")).ok_or_else(|| "core 未初始化".into())
}

fn load_registry() -> Res<Registry> {
    let p = registry_path()?;
    Ok(std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default())
}

fn save_registry(r: &Registry) -> Res<()> {
    write_atomic(&registry_path()?, &serde_json::to_vec_pretty(r).unwrap())
}

/// Where the user's own SPT installs are likely to be: each drive's `Games`, `SPT` folders (Windows), for the first run.
fn guesses() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(windows)]
    for d in b'C'..=b'H' {
        let drive = format!("{}:\\", d as char);
        for base in ["Games", "SPT", "SPTarkov", "Program Files (x86)\\Games"] {
            let b = Path::new(&drive).join(base);
            if b.is_dir() {
                for e in list_dir(&b) {
                    if e.is_dir() && instance::detect(&e).ok() {
                        out.push(e);
                    }
                }
            }
            if b.is_dir() && instance::detect(&b).ok() {
                out.push(b);
            }
        }
    }
    out
}

fn default_name(i: &Instance) -> String {
    let v = if i.spt_version.is_empty() { String::new() } else { format!("SPT {}", i.spt_version) };
    let folder = name_of(Path::new(&i.root));
    if v.is_empty() {
        folder
    } else {
        v
    }
}

#[derive(Serialize)]
struct InstanceView {
    #[serde(flatten)]
    inst: Instance,
    name: String,
    current: bool,
    mods: usize,
}

fn arg<'a>(req: &'a Value, k: &str) -> Res<&'a str> {
    req.get(k).and_then(|v| v.as_str()).ok_or_else(|| format!("缺少参数 {k}"))
}

fn inst_of(req: &Value) -> Res<Instance> {
    let p = arg(req, "instance")?;
    let i = instance::detect(Path::new(p));
    if !Path::new(p).is_dir() {
        return err(format!("实例目录不存在：{p}"));
    }
    Ok(i)
}

fn temp_dir_of(req: &Value) -> PathBuf {
    req.get("temp").and_then(|t| t.as_str()).map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

fn to_json<T: Serialize>(v: T) -> Res<Value> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

pub fn dispatch(req: &Value) -> Res<Value> {
    dispatch_data(req, None)
}

/// A request with bytes beside it (`data`: a file's, for `mods.plan_bytes`).
pub fn dispatch_data(req: &Value, data: Option<Vec<u8>>) -> Res<Value> {
    let op = arg(req, "op")?;
    match op {
        "init" => {
            let d = PathBuf::from(arg(req, "data_dir")?);
            std::fs::create_dir_all(&d).at("创建数据目录", &d)?;
            *data_dir().lock().unwrap() = Some(d.clone());
            let mut r = load_registry()?;
            if r.instances.is_empty() && req.get("guess").and_then(|g| g.as_bool()).unwrap_or(true) {
                // (the newest SPT first: the one chosen)
                let mut found: Vec<Instance> = guesses().iter().map(|g| instance::detect(g)).collect();
                found.sort_by(|a, b| version_cmp(&b.spt_version, &a.spt_version));
                for i in found {
                    r.instances.push(Entry { path: i.root.clone(), name: default_name(&i), added_at: now_ms() });
                }
                if let Some(first) = r.instances.first() {
                    r.current = first.path.clone();
                }
                save_registry(&r)?;
            }
            Ok(json!({"data_dir": d, "version": env!("CARGO_PKG_VERSION")}))
        }
        "instances.list" => {
            let r = load_registry()?;
            let list: Vec<InstanceView> = r
                .instances
                .iter()
                .map(|e| {
                    let i = instance::detect(Path::new(&e.path));
                    let mods = manifest::load_all(&i.root_path()).len();
                    InstanceView { current: e.path == r.current, name: e.name.clone(), mods: i.server_mods.max(mods), inst: i }
                })
                .collect();
            Ok(json!({"instances": list, "current": r.current}))
        }
        "instances.probe" => to_json(instance::detect(Path::new(arg(req, "path")?))),
        "instances.scan" => {
            // SPT installs found where they usually are, not added yet (the add-instance step's suggestions)
            let r = load_registry()?;
            let found: Vec<Instance> = guesses()
                .into_iter()
                .map(|g| instance::detect(&g))
                .filter(|i| !r.instances.iter().any(|e| Path::new(&e.path) == Path::new(&i.root)))
                .collect();
            to_json(found)
        }
        "instances.add" => {
            let p = arg(req, "path")?;
            let i = instance::detect(Path::new(p));
            if !i.ok() {
                return err(format!("不是可用的 SPT 目录：{}", i.problems.join("；")));
            }
            let mut r = load_registry()?;
            if !r.instances.iter().any(|e| Path::new(&e.path) == Path::new(&i.root)) {
                let name = req.get("name").and_then(|n| n.as_str()).filter(|n| !n.trim().is_empty()).map(String::from).unwrap_or_else(|| default_name(&i));
                r.instances.push(Entry { path: i.root.clone(), name, added_at: now_ms() });
            }
            r.current = i.root.clone();
            save_registry(&r)?;
            to_json(i)
        }
        "instances.remove" => {
            let p = arg(req, "path")?;
            let mut r = load_registry()?;
            r.instances.retain(|e| e.path != p);
            if r.current == p {
                r.current = r.instances.first().map(|e| e.path.clone()).unwrap_or_default();
            }
            save_registry(&r)?;
            Ok(json!({"current": r.current}))
        }
        "instances.select" => {
            let p = arg(req, "path")?;
            let mut r = load_registry()?;
            if !r.instances.iter().any(|e| e.path == p) {
                return err("没有这个实例");
            }
            r.current = p.to_string();
            save_registry(&r)?;
            Ok(json!({"current": p}))
        }
        "instances.rename" => {
            let p = arg(req, "path")?;
            let n = arg(req, "name")?;
            let mut r = load_registry()?;
            for e in r.instances.iter_mut().filter(|e| e.path == p) {
                e.name = n.to_string();
            }
            save_registry(&r)?;
            Ok(json!({}))
        }
        "settings.get" => Ok(load_registry()?.settings),
        "settings.set" => {
            let mut r = load_registry()?;
            if !r.settings.is_object() {
                r.settings = json!({});
            }
            if let (Some(o), Some(new)) = (r.settings.as_object_mut(), req.get("values").and_then(|v| v.as_object())) {
                for (k, v) in new {
                    o.insert(k.clone(), v.clone());
                }
            }
            save_registry(&r)?;
            Ok(r.settings)
        }
        "mods.list" => {
            let i = inst_of(req)?;
            let list = mods::list(&i);
            let enabled = list.iter().filter(|m| m.enabled).count();
            let server = list.iter().filter(|m| m.server && m.enabled).count();
            let client = list.iter().filter(|m| m.client && m.enabled).count();
            let content: Vec<&mods::ModView> = list.iter().filter(|m| m.loc.status != "na").collect();
            let localized = content.iter().filter(|m| m.loc.status == "full").count();
            Ok(json!({
                "mods": list,
                "enabled": enabled,
                "server": server,
                "client": client,
                "content": content.len(),
                "localized": localized,
            }))
        }
        "mods.set_enabled" => {
            let i = inst_of(req)?;
            let on = req.get("enabled").and_then(|b| b.as_bool()).ok_or("缺少参数 enabled")?;
            if server::up(&i) {
                return err("服务端正在运行：先停止服务端再启用/禁用 Mod");
            }
            let id = mods::set_enabled(&i, arg(req, "id")?, on)?;
            Ok(json!({"id": id}))
        }
        "mods.uninstall" => {
            let i = inst_of(req)?;
            if server::up(&i) {
                return err("服务端正在运行：先停止服务端再卸载 Mod");
            }
            Ok(json!({"removed": mods::uninstall(&i, arg(req, "id")?)?}))
        }
        "mods.plan_file" => {
            let i = inst_of(req)?;
            let p = PathBuf::from(arg(req, "path")?);
            to_json(install::plan(&i, &p, Source { kind: "archive".into(), url: String::new(), file: name_of(&p) }, None)?)
        }
        "mods.plan_bytes" => {
            // a file let go on the window: its bytes (base64) and name
            let i = inst_of(req)?;
            let name = arg(req, "name")?;
            let data = match data {
                Some(d) if !d.is_empty() => d,
                _ => b64_decode(arg(req, "data")?)?,
            };
            let dir = install::staging(&i).join(format!("drop-{}", stamp_id(now_ms())));
            let safe: String = name.chars().map(|c| if c.is_alphanumeric() || "-_. ".contains(c) { c } else { '_' }).collect();
            let p = dir.join(safe);
            write_atomic(&p, &data)?;
            let r = install::plan(&i, &p, Source { kind: "archive".into(), url: String::new(), file: name.to_string() }, None);
            let _ = remove_path(&dir);
            to_json(r?)
        }
        "mods.plan_url" => {
            let i = inst_of(req)?;
            let url = arg(req, "url")?;
            let src = sources::source_for(url)?;
            let rel = src.release(url)?;
            let dir = install::staging(&i).join(format!("dl-{}", stamp_id(now_ms())));
            let file = sources::download(&rel, &dir)?;
            let name = rel.repo.rsplit('/').next().unwrap_or(&rel.repo).to_string();
            let r = install::plan(&i, &file, Source { kind: rel.source.clone(), url: rel.url.clone(), file: rel.asset_name.clone() }, Some((name, rel.version.clone())));
            let _ = remove_path(&dir);
            to_json(r?)
        }
        "mods.install" => {
            let i = inst_of(req)?;
            if server::up(&i) {
                return err("服务端正在运行：先停止服务端再安装 Mod");
            }
            let overwrite = req.get("overwrite").and_then(|b| b.as_bool()).unwrap_or(false);
            to_json(install::execute(&i, arg(req, "plan")?, overwrite)?)
        }
        "mods.cancel_plan" => {
            install::cancel(&inst_of(req)?, arg(req, "plan")?)?;
            Ok(json!({}))
        }
        "mods.check_updates" => {
            let i = inst_of(req)?;
            let mut out = Vec::new();
            let mut failed = Vec::new();
            for m in manifest::load_all(&i.root_path()) {
                if m.source.url.is_empty() {
                    continue;
                }
                let Ok(src) = sources::source_for(&m.source.url) else { continue };
                match src.latest(&m.source.url) {
                    Ok(rel) => {
                        if version_cmp(&rel.version, &m.version) == std::cmp::Ordering::Greater {
                            out.push(json!({"id": m.id, "name": m.name, "current": m.version, "latest": rel.version, "url": rel.url, "asset": rel.asset_name}));
                        }
                    }
                    Err(e) => failed.push(json!({"id": m.id, "error": e})),
                }
            }
            Ok(json!({"updates": out, "failed": failed}))
        }
        "snapshots.list" => to_json(snapshot::list(&inst_of(req)?)),
        "snapshots.create" => {
            let note = req.get("note").and_then(|n| n.as_str()).unwrap_or("");
            to_json(snapshot::create(&inst_of(req)?, note, "manual")?)
        }
        "snapshots.restore" => {
            let i = inst_of(req)?;
            to_json(snapshot::restore(&i, arg(req, "id")?, server::up(&i))?)
        }
        "snapshots.delete" => {
            snapshot::delete(&inst_of(req)?, arg(req, "id")?)?;
            Ok(json!({}))
        }
        "config.list" => to_json(config::list(&inst_of(req)?)),
        "config.read" => Ok(json!({"text": config::read(&inst_of(req)?, arg(req, "path")?)?})),
        "config.check" => {
            let p = arg(req, "path")?;
            let kind = if p.ends_with(".jsonc") { "jsonc" } else if p.ends_with(".json") { "json" } else { "cfg" };
            Ok(match config::check(kind, arg(req, "text")?) {
                Ok(()) => json!({"ok": true}),
                Err(e) => json!({"ok": false, "error": e}),
            })
        }
        "config.save" => Ok(json!({"backup": config::save(&inst_of(req)?, arg(req, "path")?, arg(req, "text")?)?})),
        "profiles.list" => {
            let i = inst_of(req)?;
            Ok(json!({"characters": profiles::list(&i)}))
        }
        "profiles.create" => {
            let i = inst_of(req)?;
            let s = spt_http::Server::new(i.port);
            s.register(arg(req, "username")?, req.get("password").and_then(|p| p.as_str()).unwrap_or(""), arg(req, "edition")?)?;
            Ok(json!({}))
        }
        "profiles.editions" => Ok(json!({"editions": spt_http::Server::new(inst_of(req)?.port).editions()?})),
        "profiles.remote" => to_json(spt_http::Server::new(inst_of(req)?.port).profiles()?),
        "server.status" => to_json(server::status(&inst_of(req)?)),
        "server.log" => to_json(server::log(&inst_of(req)?, req.get("after").and_then(|a| a.as_u64()).unwrap_or(0))),
        "server.start" => to_json(server::start(&inst_of(req)?)?),
        "server.stop" => to_json(server::stop(&inst_of(req)?, Duration::from_secs(15))?),
        "server.restart" => {
            let i = inst_of(req)?;
            let st = server::status(&i);
            if st.state == "external" {
                return err("运行中的服务端不是 Bunker 启动的，Bunker 不会去重启它");
            }
            if matches!(st.state.as_str(), "running" | "starting") {
                server::stop(&i, Duration::from_secs(15))?;
            }
            to_json(server::start(&i)?)
        }
        "game.launch" => {
            let i = inst_of(req)?;
            let clean = req.get("clean_cache").and_then(|b| b.as_bool()).unwrap_or(false);
            to_json(game::launch(&i, arg(req, "username")?, req.get("password").and_then(|p| p.as_str()).unwrap_or(""), clean, &temp_dir_of(req))?)
        }
        "cache.clean" => to_json(game::clean_cache(&inst_of(req)?, &temp_dir_of(req))?),
        _ => err(format!("未知操作：{op}")),
    }
}

/// Standard base64 (the UI hands dropped files' bytes this way).
pub fn b64_decode(s: &str) -> Res<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c).ok_or("base64 数据无效")?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::tests::{fake_instance, temp_dir};

    #[test]
    fn base64() {
        assert_eq!(b64_decode("aGVsbG8gd29ybGQ=").unwrap(), b"hello world");
        assert_eq!(b64_decode("UEsDBA==").unwrap(), b"PK\x03\x04");
    }

    // (one test: the registry is the process's)
    #[test]
    fn registry_and_requests() {
        let d = temp_dir("data");
        dispatch(&json!({"op": "init", "data_dir": d, "guess": false})).unwrap();
        let r = fake_instance("api", "SPT_Runtime");
        let bad = temp_dir("notspt");
        assert!(dispatch(&json!({"op": "instances.add", "path": bad})).is_err());
        dispatch(&json!({"op": "instances.add", "path": r, "name": "SPT 4.1.6 主力档"})).unwrap();
        let l = dispatch(&json!({"op": "instances.list"})).unwrap();
        assert_eq!(l["instances"].as_array().unwrap().len(), 1);
        assert_eq!(l["instances"][0]["name"], "SPT 4.1.6 主力档");
        assert_eq!(l["instances"][0]["current"], true);
        assert_eq!(l["instances"][0]["layout"], "spt_runtime");
        let s = dispatch(&json!({"op": "settings.set", "values": {"clean_cache": true}})).unwrap();
        assert_eq!(s["clean_cache"], true);
        let p = dispatch(&json!({"op": "profiles.list", "instance": r})).unwrap();
        assert_eq!(p["characters"][0]["nickname"], "BEAR_Main");
        let st = dispatch(&json!({"op": "server.status", "instance": r})).unwrap();
        assert!(st["state"] == "stopped" || st["state"] == "external");
        assert!(dispatch(&json!({"op": "nope"})).is_err());
        dispatch(&json!({"op": "instances.remove", "path": r})).unwrap();
        assert_eq!(dispatch(&json!({"op": "instances.list"})).unwrap()["instances"].as_array().unwrap().len(), 0);
        for x in [&d, &r, &bad] {
            let _ = std::fs::remove_dir_all(x);
        }
    }
}
