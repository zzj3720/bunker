//! Starting the game for a character, as SPT's own launcher does: the server running (started and waited for when
//! it is not), the character logged in, optionally the caches cleaned, then `EscapeFromTarkov.exe` in the game's
//! folder with `-token=<profile id>` and `-config={'BackendUrl':…}`.

use crate::events;
use crate::instance::Instance;
use crate::server;
use crate::spt_http;
use crate::util::*;
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// The game's arguments, exactly as SPT's launcher gives them.
pub fn game_args(token: &str, port: u16) -> String {
    format!("-force-gfx-jobs native -token={token} -config={{'BackendUrl':'https://127.0.0.1:{port}','Version':'live','MatchingVersion':'live'}}")
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Cleaned {
    pub files: usize,
    pub bytes: u64,
    pub dirs: Vec<String>,
}

/// The caches a stale mod leaves behind: BepInEx's (`BepInEx/cache`) and the game's temporary folder
/// (`%TEMP%\Battlestate Games\EscapeFromTarkov`).
pub fn cache_dirs(inst: &Instance, temp: &Path) -> Vec<PathBuf> {
    vec![inst.root_path().join("BepInEx").join("cache"), temp.join("Battlestate Games").join("EscapeFromTarkov")]
}

pub fn clean_cache(inst: &Instance, temp: &Path) -> Res<Cleaned> {
    let mut c = Cleaned::default();
    for d in cache_dirs(inst, temp) {
        if !d.is_dir() {
            continue;
        }
        for f in walk_files(&d) {
            c.bytes += std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            c.files += 1;
        }
        for e in list_dir(&d) {
            remove_path(&e)?;
        }
        c.dirs.push(d.to_string_lossy().into_owned());
    }
    Ok(c)
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Launched {
    pub pid: u32,
    pub token: String,
    pub args: String,
    pub cleaned: Option<Cleaned>,
    pub started_server: bool,
}

pub fn launch(inst: &Instance, username: &str, password: &str, clean: bool, temp: &Path) -> Res<Launched> {
    if !inst.has_game {
        return err("这个实例没有 EscapeFromTarkov.exe");
    }
    let mut started_server = false;
    let st = server::status(inst);
    if !matches!(st.state.as_str(), "running" | "external") {
        if st.state != "starting" {
            server::start(inst)?;
            started_server = true;
        }
        server::wait_ready(inst, Duration::from_secs(240))?;
    }
    let token = spt_http::Server::new(inst.port).login(username, password)?;
    let cleaned = if clean { Some(clean_cache(inst, temp)?) } else { None };
    let args = game_args(&token, inst.port);
    let mut cmd = Command::new(inst.game_exe());
    cmd.current_dir(inst.root_path());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(&args);
    }
    #[cfg(not(windows))]
    {
        cmd.args(args.split(' '));
    }
    let mut child = cmd.spawn().map_err(|e| format!("启动游戏失败：{e}"))?;
    let pid = child.id();
    let root = inst.root.clone();
    events::emit(&json!({"ev": "game", "instance": root, "state": "running", "pid": pid}));
    thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        events::emit(&json!({"ev": "game", "instance": root, "state": "exited", "pid": pid, "code": code}));
    });
    Ok(Launched { pid, token, args, cleaned, started_server })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, temp_dir, touch};

    #[test]
    fn args_as_the_launcher_gives_them() {
        assert_eq!(
            game_args("6afeab5dc8ca1f0ed4b321be", 6969),
            "-force-gfx-jobs native -token=6afeab5dc8ca1f0ed4b321be -config={'BackendUrl':'https://127.0.0.1:6969','Version':'live','MatchingVersion':'live'}"
        );
    }

    #[test]
    fn cleans_both_caches_only() {
        let r = fake_instance("cache", "SPT_Runtime");
        let t = temp_dir("tmp");
        touch(&r.join("BepInEx/cache/chainloader_typeloader.dat"), b"1234");
        touch(&t.join("Battlestate Games/EscapeFromTarkov/Temp/x.bin"), b"12");
        touch(&t.join("other/keep.txt"), b"k");
        let c = clean_cache(&detect(&r), &t).unwrap();
        assert_eq!(c.files, 2);
        assert_eq!(c.bytes, 6);
        assert!(r.join("BepInEx/cache").is_dir() && list_dir(&r.join("BepInEx/cache")).is_empty());
        assert!(t.join("other/keep.txt").exists());
        assert!(r.join("BepInEx/core/BepInEx.dll").exists());
        let _ = std::fs::remove_dir_all(&r);
        let _ = std::fs::remove_dir_all(&t);
    }
}
