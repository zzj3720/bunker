//! Snapshots of an instance's profiles (`user/profiles/*.json`): taken by hand with a note, before a mod is installed
//! and before a rollback; kept in `.bunker/snapshots/<id>/` with what they hold, and put back whole.

use crate::instance::Instance;
use crate::util::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub id: String,
    pub created_at: i64,
    pub note: String,
    /// `manual`, `before_install`, `before_restore`
    pub kind: String,
    pub files: Vec<String>,
    pub bytes: u64,
    /// The characters it holds (nicknames and levels), to tell snapshots apart.
    #[serde(default)]
    pub characters: Vec<String>,
}

pub fn dir(inst: &Instance) -> PathBuf {
    inst.bunker_dir().join("snapshots")
}

fn profile_files(inst: &Instance) -> Vec<PathBuf> {
    list_dir(&inst.profiles_dir())
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .collect()
}

fn character_of(p: &std::path::Path) -> Option<String> {
    let v = read_json(p).ok()?;
    let info = v.pointer("/characters/pmc/Info")?;
    let nick = info.get("Nickname")?.as_str()?;
    let lvl = info.get("Level").and_then(|x| x.as_i64()).unwrap_or(0);
    Some(format!("{nick} Lv.{lvl}"))
}

pub fn create(inst: &Instance, note: &str, kind: &str) -> Res<Snapshot> {
    let files = profile_files(inst);
    let mut ms = now_ms();
    let mut id = stamp_id(ms);
    while dir(inst).join(&id).exists() {
        ms += 1;
        id = stamp_id(ms);
    }
    let d = dir(inst).join(&id);
    let pd = d.join("profiles");
    fs::create_dir_all(&pd).at("创建快照目录", &pd)?;
    let mut bytes = 0;
    let mut names = Vec::new();
    let mut chars = Vec::new();
    for f in &files {
        let n = name_of(f);
        bytes += fs::copy(f, pd.join(&n)).at("复制存档", f)?;
        if let Some(c) = character_of(f) {
            chars.push(c);
        }
        names.push(n);
    }
    let s = Snapshot { id, created_at: ms, note: note.to_string(), kind: kind.to_string(), files: names, bytes, characters: chars };
    write_atomic(&d.join("meta.json"), &serde_json::to_vec_pretty(&s).unwrap())?;
    Ok(s)
}

/// Newest first.
pub fn list(inst: &Instance) -> Vec<Snapshot> {
    let mut v: Vec<Snapshot> = list_dir(&dir(inst))
        .into_iter()
        .filter_map(|d| fs::read(d.join("meta.json")).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .collect();
    v.sort_by(|a: &Snapshot, b: &Snapshot| b.id.cmp(&a.id));
    v
}

/// Puts a snapshot's profiles back in place of the current ones (themselves snapshotted first). Refused while a
/// server runs on the instance's port: it holds the profiles in memory and would write over them.
pub fn restore(inst: &Instance, id: &str, server_up: bool) -> Res<Snapshot> {
    if server_up {
        return err("服务端正在运行，先停止服务端再回滚（否则它会用内存里的存档覆盖回滚结果）");
    }
    let d = dir(inst).join(id);
    let meta: Snapshot = serde_json::from_slice(&fs::read(d.join("meta.json")).at("读取快照", &d)?).map_err(|e| e.to_string())?;
    let before = create(inst, &format!("回滚到 {} 之前", id), "before_restore")?;
    let pd = inst.profiles_dir();
    fs::create_dir_all(&pd).at("创建存档目录", &pd)?;
    for f in profile_files(inst) {
        if !meta.files.contains(&name_of(&f)) {
            fs::remove_file(&f).at("删除存档", &f)?;
        }
    }
    for n in &meta.files {
        let src = d.join("profiles").join(n);
        let data = fs::read(&src).at("读取快照存档", &src)?;
        write_atomic(&pd.join(n), &data)?;
    }
    let _ = before;
    Ok(meta)
}

pub fn delete(inst: &Instance, id: &str) -> Res<()> {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return err("快照编号无效");
    }
    remove_path(&dir(inst).join(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, touch};

    #[test]
    fn snapshot_restore_roundtrip() {
        let r = fake_instance("snap", "SPT_Runtime");
        let inst = detect(&r);
        let p = r.join("SPT_Runtime/user/profiles/aaa.json");
        let original = fs::read(&p).unwrap();
        let s = create(&inst, "打 Labs 之前", "manual").unwrap();
        assert_eq!(s.files, vec!["aaa.json".to_string()]);
        assert_eq!(s.characters, vec!["BEAR_Main Lv.34".to_string()]);
        // play on: the profile changes, a new one appears
        fs::write(&p, b"{\"changed\":true}").unwrap();
        touch(&r.join("SPT_Runtime/user/profiles/bbb.json"), b"{}");
        assert!(restore(&inst, &s.id, true).is_err(), "refused while the server runs");
        restore(&inst, &s.id, false).unwrap();
        assert_eq!(fs::read(&p).unwrap(), original);
        assert!(!r.join("SPT_Runtime/user/profiles/bbb.json").exists());
        let all = list(&inst);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].kind, "before_restore", "newest first: the state before the rollback, kept");
        assert_eq!(all[0].files.len(), 2);
        delete(&inst, &all[0].id).unwrap();
        assert_eq!(list(&inst).len(), 1);
        assert!(delete(&inst, "../x").is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
