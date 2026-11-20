//! What Bunker knows of a mod it installed or took over: `.bunker/manifests/<id>.json` in the instance, naming every
//! file it put down (for an uninstall that leaves nothing) and the top-level entries it owns (a server mod's folder,
//! a plugin's dll or folder), which disabling moves into `.bunker/disabled/<id>/` and back.

use crate::util::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Source {
    /// `archive` (a file people gave), `github` (a release), `adopted` (found installed, taken over)
    pub kind: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source: Source,
    #[serde(default)]
    pub installed_at: i64,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Top-level entries, relative to the instance root (`SPT_Runtime/user/mods/UIFixes`, `BepInEx/plugins/X.dll`).
    pub roots: Vec<String>,
    /// Every file, relative to the instance root, where it is when enabled.
    pub files: Vec<String>,
}

fn yes() -> bool {
    true
}

pub fn manifests_dir(root: &Path) -> PathBuf {
    root.join(".bunker").join("manifests")
}

pub fn disabled_dir(root: &Path, id: &str) -> PathBuf {
    root.join(".bunker").join("disabled").join(id)
}

pub fn load_all(root: &Path) -> Vec<Manifest> {
    let mut out: Vec<Manifest> = list_dir(&manifests_dir(root))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| fs::read(&p).ok().and_then(|b| serde_json::from_slice(strip_bom(&b)).ok()))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn load(root: &Path, id: &str) -> Option<Manifest> {
    let p = manifests_dir(root).join(format!("{id}.json"));
    fs::read(&p).ok().and_then(|b| serde_json::from_slice(strip_bom(&b)).ok())
}

pub fn save(root: &Path, m: &Manifest) -> Res<()> {
    let p = manifests_dir(root).join(format!("{}.json", m.id));
    let text = serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?;
    write_atomic(&p, &text)
}

pub fn delete(root: &Path, id: &str) -> Res<()> {
    remove_path(&manifests_dir(root).join(format!("{id}.json")))
}

/// An id for a new manifest from a name, not taken by another.
pub fn fresh_id(root: &Path, name: &str) -> String {
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while manifests_dir(root).join(format!("{id}.json")).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// The files under a root, relative to the instance root.
pub fn files_under(root: &Path, entry_rel: &str) -> Vec<String> {
    let p = match safe_join(root, entry_rel) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    files_of(&p).iter().map(|f| rel(root, f)).collect()
}
