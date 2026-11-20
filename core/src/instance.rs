//! An SPT install found in a folder: which layout it has (4.1+ `SPT_Runtime/`, 3.11/4.0 `SPT/`, or a server at the
//! game's root as older releases had it), where its server, mods, profiles and BepInEx are, and their versions.

use crate::pever;
use crate::util::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Instance {
    pub root: String,
    /// `spt_runtime` (4.1+), `spt` (3.11 / 4.0), `root` (server beside the game)
    pub layout: String,
    /// The server's folder relative to the root: `SPT_Runtime`, `SPT` or `` (the root).
    pub server_rel: String,
    pub server_exe: String,
    pub has_server: bool,
    pub has_game: bool,
    pub has_bepinex: bool,
    pub spt_version: String,
    pub eft_version: String,
    pub bepinex_version: String,
    pub server_mods: usize,
    pub client_mods: usize,
    pub profiles: usize,
    pub port: u16,
    pub problems: Vec<String>,
}

impl Instance {
    pub fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.root)
    }
    pub fn server_dir(&self) -> PathBuf {
        if self.server_rel.is_empty() {
            self.root_path()
        } else {
            self.root_path().join(&self.server_rel)
        }
    }
    pub fn mods_dir(&self) -> PathBuf {
        self.server_dir().join("user").join("mods")
    }
    pub fn mods_rel(&self) -> String {
        if self.server_rel.is_empty() {
            "user/mods".into()
        } else {
            format!("{}/user/mods", self.server_rel)
        }
    }
    pub fn profiles_dir(&self) -> PathBuf {
        self.server_dir().join("user").join("profiles")
    }
    pub fn plugins_dir(&self) -> PathBuf {
        self.root_path().join("BepInEx").join("plugins")
    }
    pub fn game_exe(&self) -> PathBuf {
        self.root_path().join("EscapeFromTarkov.exe")
    }
    pub fn bunker_dir(&self) -> PathBuf {
        self.root_path().join(".bunker")
    }
    pub fn ok(&self) -> bool {
        self.has_server && self.has_game
    }
}

/// BepInEx/plugins entries SPT itself ships, never shown as mods.
pub const SYSTEM_PLUGINS: &[&str] = &["spt", "configurationmanager"];

pub fn is_system_plugin(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.trim_end_matches(".dll");
    SYSTEM_PLUGINS.contains(&n)
}

/// The server's port from its http config (`SPT_Data/configs/http.json`, or 3.x's `SPT_Data/Server/configs/`).
pub fn server_port(server_dir: &Path) -> u16 {
    for p in ["SPT_Data/configs/http.json", "SPT_Data/Server/configs/http.json"] {
        if let Ok(v) = read_json(&server_dir.join(p)) {
            if let Some(port) = v.get("port").and_then(|x| x.as_u64()) {
                return port as u16;
            }
        }
    }
    6969
}

fn spt_version_of(server_dir: &Path, exe: &Path) -> String {
    if let Some(v) = pever::read(exe) {
        let s = v.short();
        if !s.is_empty() && s != "0.0.0.0" {
            return s;
        }
    }
    for p in ["SPT_Data/configs/core.json", "SPT_Data/Server/configs/core.json"] {
        if let Ok(v) = read_json(&server_dir.join(p)) {
            if let Some(s) = v.get("sptVersion").and_then(|x| x.as_str()) {
                return s.to_string();
            }
        }
    }
    String::new()
}

/// What is in `root`: an instance even when parts are missing (`problems` says which), so the UI can tell why.
pub fn detect(root: &Path) -> Instance {
    let root_s = root.to_string_lossy().into_owned();
    let (layout, server_rel) = if root.join("SPT_Runtime").join("SPT.Server.exe").is_file() {
        ("spt_runtime", "SPT_Runtime")
    } else if root.join("SPT").join("SPT.Server.exe").is_file() {
        ("spt", "SPT")
    } else if root.join("SPT.Server.exe").is_file() {
        ("root", "")
    } else if root.join("SPT_Runtime").is_dir() {
        ("spt_runtime", "SPT_Runtime")
    } else if root.join("SPT").is_dir() {
        ("spt", "SPT")
    } else {
        ("root", "")
    };
    let server_dir = if server_rel.is_empty() { root.to_path_buf() } else { root.join(server_rel) };
    let exe = server_dir.join("SPT.Server.exe");
    let has_server = exe.is_file();
    let game = root.join("EscapeFromTarkov.exe");
    let has_game = game.is_file();
    let bep = root.join("BepInEx");
    let has_bepinex = bep.join("core").is_dir() || bep.join("plugins").is_dir();
    let mut problems = Vec::new();
    if !root.is_dir() {
        problems.push("目录不存在".to_string());
    } else {
        if !has_game {
            problems.push("没有找到 EscapeFromTarkov.exe".into());
        }
        if !has_server {
            problems.push("没有找到 SPT.Server.exe（SPT_Runtime/、SPT/ 或根目录）".into());
        }
        if !has_bepinex {
            problems.push("没有找到 BepInEx".into());
        }
    }
    let mods_dir = server_dir.join("user").join("mods");
    let server_mods = list_dir(&mods_dir).iter().filter(|p| p.is_dir()).count();
    let client_mods = list_dir(&bep.join("plugins"))
        .iter()
        .filter(|p| {
            let n = name_of(p);
            (p.is_dir() || n.to_ascii_lowercase().ends_with(".dll")) && !is_system_plugin(&n)
        })
        .count();
    let profiles = list_dir(&server_dir.join("user").join("profiles"))
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .count();
    Instance {
        root: root_s,
        layout: layout.into(),
        server_rel: server_rel.into(),
        server_exe: exe.to_string_lossy().into_owned(),
        has_server,
        has_game,
        has_bepinex,
        spt_version: if has_server { spt_version_of(&server_dir, &exe) } else { String::new() },
        eft_version: pever::read(&game).map(|v| v.short()).unwrap_or_default(),
        bepinex_version: pever::read(&bep.join("core").join("BepInEx.dll")).map(|v| v.short()).unwrap_or_default(),
        server_mods,
        client_mods,
        profiles,
        port: server_port(&server_dir),
        problems,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fs;

    pub fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bunker-test-{tag}-{}-{}", std::process::id(), now_ms()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    pub fn touch(p: &Path, data: &[u8]) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    /// A made-up instance: `server_rel` its server folder, mods named as the real 4.1.6 install's are.
    pub fn fake_instance(tag: &str, server_rel: &str) -> PathBuf {
        let r = temp_dir(tag);
        let s = if server_rel.is_empty() { r.clone() } else { r.join(server_rel) };
        touch(&r.join("EscapeFromTarkov.exe"), b"MZ");
        touch(&s.join("SPT.Server.exe"), b"MZ");
        touch(&r.join("BepInEx/core/BepInEx.dll"), b"MZ");
        touch(&r.join("BepInEx/plugins/spt/spt-core.dll"), b"MZ");
        touch(&s.join("user/profiles/aaa.json"), br#"{"info":{"id":"aaa","username":"bear","edition":"Standard"},"characters":{"pmc":{"Info":{"Nickname":"BEAR_Main","Level":34,"Side":"Bear"},"Inventory":{"items":[{"_tpl":"5449016a4bdc2d6f028b456f","upd":{"StackObjectsCount":500000}}]}}}}"#);
        r
    }

    #[test]
    fn detects_41_layout() {
        let r = fake_instance("det41", "SPT_Runtime");
        touch(&r.join("SPT_Runtime/user/mods/GuidingLight/GuidingLight.dll"), b"x");
        touch(&r.join("BepInEx/plugins/Tyfon.UIFixes.dll"), b"x");
        touch(&r.join("SPT_Runtime/SPT_Data/configs/http.json"), br#"{"ip":"127.0.0.1","port":7070}"#);
        let i = detect(&r);
        assert_eq!(i.layout, "spt_runtime");
        assert_eq!(i.server_rel, "SPT_Runtime");
        assert!(i.ok(), "{:?}", i.problems);
        assert_eq!(i.server_mods, 1);
        assert_eq!(i.client_mods, 1, "spt's own plugin folder is not a mod");
        assert_eq!(i.profiles, 1);
        assert_eq!(i.port, 7070);
        assert_eq!(i.mods_rel(), "SPT_Runtime/user/mods");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn detects_40_and_root_layouts() {
        let r = fake_instance("det40", "SPT");
        let i = detect(&r);
        assert_eq!(i.layout, "spt");
        assert_eq!(i.port, 6969);
        assert!(i.problems.is_empty());
        let _ = fs::remove_dir_all(&r);
        let r = fake_instance("detroot", "");
        touch(&r.join("SPT_Data/Server/configs/core.json"), br#"{"sptVersion":"3.11.3"}"#);
        let i = detect(&r);
        assert_eq!(i.layout, "root");
        assert_eq!(i.spt_version, "3.11.3");
        assert_eq!(i.mods_rel(), "user/mods");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn says_what_is_missing() {
        let r = temp_dir("detnone");
        let i = detect(&r);
        assert!(!i.ok());
        assert_eq!(i.problems.len(), 3);
        let _ = fs::remove_dir_all(&r);
    }
}
