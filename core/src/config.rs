//! Mods' config files: `.json` / `.jsonc` in server mods' config folders and BepInEx's `config/*.cfg`, read and
//! saved after their syntax is checked, the previous text kept as `<file>.bak`.

use crate::instance::Instance;
use crate::mods::parse_jsonc;
use crate::util::*;
use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigFile {
    /// Relative to the instance root
    pub path: String,
    /// `json`, `jsonc`, `cfg`
    pub kind: String,
    /// The mod (server folder) or `BepInEx` it belongs to
    pub owner: String,
    pub bytes: u64,
}

const MOST: u64 = 1 << 20;

fn kind_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.ends_with(".jsonc") {
        Some("jsonc")
    } else if n.ends_with(".json") {
        Some("json")
    } else if n.ends_with(".cfg") {
        Some("cfg")
    } else {
        None
    }
}

/// A server mod's config: files under a folder named config (any case), or named config.json(c) themselves.
fn is_config_path(rel_in_mod: &str) -> bool {
    let l = rel_in_mod.to_ascii_lowercase();
    let parts: Vec<&str> = l.split('/').collect();
    let file = parts.last().copied().unwrap_or("");
    parts[..parts.len().saturating_sub(1)].iter().any(|p| *p == "config" || *p == "configs" || *p == "settings")
        || file.starts_with("config.")
        || file.starts_with("settings.")
}

pub fn list(inst: &Instance) -> Vec<ConfigFile> {
    let root = inst.root_path();
    let mut out = Vec::new();
    for m in list_dir(&inst.mods_dir()).into_iter().filter(|p| p.is_dir()) {
        let owner = name_of(&m);
        for f in walk_files(&m) {
            let n = name_of(&f);
            let Some(kind) = kind_of(&n) else { continue };
            if kind == "cfg" || n.eq_ignore_ascii_case("package.json") {
                continue;
            }
            let bytes = fs::metadata(&f).map(|x| x.len()).unwrap_or(0);
            if bytes > MOST || !is_config_path(&rel(&m, &f)) {
                continue;
            }
            out.push(ConfigFile { path: rel(&root, &f), kind: kind.into(), owner: owner.clone(), bytes });
        }
    }
    let cfg = root.join("BepInEx").join("config");
    for f in walk_files(&cfg) {
        if kind_of(&name_of(&f)) == Some("cfg") {
            let bytes = fs::metadata(&f).map(|x| x.len()).unwrap_or(0);
            if bytes <= MOST {
                out.push(ConfigFile { path: rel(&root, &f), kind: "cfg".into(), owner: "BepInEx".into(), bytes });
            }
        }
    }
    out
}

fn file_in(inst: &Instance, relp: &str) -> Res<std::path::PathBuf> {
    let p = safe_join(&inst.root_path(), relp)?;
    if kind_of(&name_of(&p)).is_none() {
        return err("只能编辑 .json / .jsonc / .cfg 文件");
    }
    Ok(p)
}

pub fn read(inst: &Instance, relp: &str) -> Res<String> {
    let p = file_in(inst, relp)?;
    let b = fs::read(&p).at("读取配置", &p)?;
    Ok(String::from_utf8_lossy(strip_bom(&b)).into_owned())
}

/// A BepInEx config's syntax: `[Section]`, `Key = Value` inside a section, `#` comments, blank lines.
pub fn check_cfg(text: &str) -> Res<()> {
    let mut in_section = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with(';') {
            continue;
        }
        if t.starts_with('[') {
            if !t.ends_with(']') || t.len() < 3 {
                return err(format!("第 {} 行：节名缺少 ]：{t}", i + 1));
            }
            in_section = true;
            continue;
        }
        match t.split_once('=') {
            Some((k, _)) if !k.trim().is_empty() => {
                if !in_section {
                    return err(format!("第 {} 行：配置项不在任何 [节] 之下：{t}", i + 1));
                }
            }
            _ => return err(format!("第 {} 行：应为「键 = 值」：{t}", i + 1)),
        }
    }
    Ok(())
}

pub fn check(kind: &str, text: &str) -> Res<()> {
    match kind {
        "json" => serde_json::from_str::<serde_json::Value>(text)
            .map(|_| ())
            .map_err(|e| format!("JSON 语法错误：第 {} 行第 {} 列：{e}", e.line(), e.column())),
        "jsonc" => parse_jsonc(text.as_bytes()).map(|_| ()).map_err(|e| format!("JSONC 语法错误：{e}")),
        _ => check_cfg(text),
    }
}

/// Saves after checking; the old text kept beside as `<file>.bak`. Returns the `.bak`'s path.
pub fn save(inst: &Instance, relp: &str, text: &str) -> Res<String> {
    let p = file_in(inst, relp)?;
    let kind = kind_of(&name_of(&p)).unwrap();
    check(kind, text)?;
    let bak = p.with_file_name(format!("{}.bak", name_of(&p)));
    if p.exists() {
        fs::copy(&p, &bak).at("备份配置", &p)?;
    }
    write_atomic(&p, text.as_bytes())?;
    Ok(rel(&inst.root_path(), &bak))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, touch};

    #[test]
    fn lists_checks_and_saves_with_backup() {
        let r = fake_instance("cfg", "SPT_Runtime");
        let m = r.join("SPT_Runtime/user/mods");
        touch(&m.join("Skwizzy.LootingBots/Config/config.json"), b"{\"a\":1}");
        touch(&m.join("GuidingLight/db/CustomBuffs/MarkOfTheBeastBuffs.jsonc"), b"{}");
        touch(&m.join("Realism/config/settings.jsonc"), b"{ // x\n \"b\": true, }");
        touch(&m.join("Realism/package.json"), b"{}");
        touch(&r.join("BepInEx/config/com.mpstark.dynamicmaps.cfg"), b"[General]\nEnabled = true\n");
        let inst = detect(&r);
        let l = list(&inst);
        let paths: Vec<&str> = l.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "SPT_Runtime/user/mods/Realism/config/settings.jsonc",
                "SPT_Runtime/user/mods/Skwizzy.LootingBots/Config/config.json",
                "BepInEx/config/com.mpstark.dynamicmaps.cfg",
            ],
            "db files are data, not config"
        );
        let p = "SPT_Runtime/user/mods/Skwizzy.LootingBots/Config/config.json";
        let e = save(&inst, p, "{\"a\":2,}").unwrap_err();
        assert!(e.contains("第 1 行"), "{e}");
        assert_eq!(read(&inst, p).unwrap(), "{\"a\":1}", "a refused save changes nothing");
        let bak = save(&inst, p, "{\"a\":2}").unwrap();
        assert_eq!(read(&inst, p).unwrap(), "{\"a\":2}");
        assert_eq!(fs::read_to_string(r.join(bak)).unwrap(), "{\"a\":1}");
        save(&inst, "SPT_Runtime/user/mods/Realism/config/settings.jsonc", "{ /* ok */ \"b\": false, }").unwrap();
        let c = "BepInEx/config/com.mpstark.dynamicmaps.cfg";
        assert!(save(&inst, c, "Enabled = false\n").unwrap_err().contains("第 1 行"));
        assert!(save(&inst, c, "[General]\nEnabled false\n").unwrap_err().contains("第 2 行"));
        save(&inst, c, "## Settings file\n[General]\n# a comment\nEnabled = false\n").unwrap();
        assert!(read(&inst, "../../etc/passwd.json").is_err());
        assert!(read(&inst, "SPT_Runtime/SPT.Server.exe").is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
