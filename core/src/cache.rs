//! What is slow to read and seldom changes, kept between requests and between runs: programs' and libraries' version
//! resources (`pever`), assemblies' .NET metadata (`clr_meta`), and mods' Chinese localization counts (`mods::scan_loc`),
//! each keyed by what it was read from (a file's path, modification time and size; a mod's JSON files, all of them so),
//! so a file changed, added or removed is read anew. Kept in memory, and in Bunker's own data folder
//! (`<data_dir>/cache/reads.json`) once `api`'s `init` names it; never in a game's folder.

use crate::clr_meta::Meta;
use crate::mods::Loc;
use crate::pever::PeVersion;
use crate::util::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

/// A file as it is now: its modification time (ns since 1970) and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub mtime: u64,
    pub size: u64,
}

pub fn stamp(p: &Path) -> Option<Stamp> {
    let m = std::fs::metadata(p).ok()?;
    let mtime = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos() as u64).unwrap_or(0);
    Some(Stamp { mtime, size: m.len() })
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    pe: HashMap<String, (Stamp, Option<PeVersion>)>,
    #[serde(default)]
    clr: HashMap<String, (Stamp, Option<Meta>)>,
    /// by the mod's roots; the signature of the JSON files they held
    #[serde(default)]
    loc: HashMap<String, (u64, Loc)>,
}

struct Cache {
    store: Store,
    dir: Option<PathBuf>,
    dirty: bool,
}

fn cache() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Cache { store: Store::default(), dir: None, dirty: false }))
}

const FILE: &str = "reads.json";
/// (the format: a file of another is not read)
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    store: Store,
}

/// Where the cache is kept from now on (`<data_dir>/cache/`): what is there read in, unless already read.
pub fn open(data_dir: &Path) {
    let dir = data_dir.join("cache");
    {
        let c = cache().lock().unwrap();
        if c.dir.as_deref() == Some(dir.as_path()) {
            return;
        }
    }
    load(&dir);
    cache().lock().unwrap().dir = Some(dir);
}

/// What a cache folder holds merged in (what was read in this run already wins over it).
fn load(dir: &Path) {
    let Ok(b) = std::fs::read(dir.join(FILE)) else { return };
    let Ok(d) = serde_json::from_slice::<OnDisk>(&b) else { return };
    if d.version != VERSION {
        return;
    }
    let mut c = cache().lock().unwrap();
    for (k, v) in d.store.pe {
        c.store.pe.entry(k).or_insert(v);
    }
    for (k, v) in d.store.clr {
        c.store.clr.entry(k).or_insert(v);
    }
    for (k, v) in d.store.loc {
        c.store.loc.entry(k).or_insert(v);
    }
}

/// What changed written to the data folder (after a request), entries of files gone dropped.
pub fn save() {
    let dir = {
        let c = cache().lock().unwrap();
        if !c.dirty {
            return;
        }
        match c.dir.clone() {
            Some(d) => d,
            None => return,
        }
    };
    save_in(&dir);
}

fn save_in(dir: &Path) {
    let mut c = cache().lock().unwrap();
    c.store.pe.retain(|k, _| Path::new(k).exists());
    c.store.clr.retain(|k, _| Path::new(k).exists());
    let text = serde_json::to_vec(&OnDiskRef { version: VERSION, store: &c.store }).unwrap_or_default();
    if write_atomic(&dir.join(FILE), &text).is_ok() {
        c.dirty = false;
    }
}

#[derive(Serialize)]
struct OnDiskRef<'a> {
    version: u32,
    store: &'a Store,
}

fn key(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A file's version resource, read once for each state of the file.
pub fn pe(p: &Path, read: impl FnOnce(&Path) -> Option<PeVersion>) -> Option<PeVersion> {
    let Some(st) = stamp(p) else { return None };
    let k = key(p);
    if let Some((s, v)) = cache().lock().unwrap().store.pe.get(&k) {
        if *s == st {
            return v.clone();
        }
    }
    let v = read(p);
    let mut c = cache().lock().unwrap();
    c.store.pe.insert(k, (st, v.clone()));
    c.dirty = true;
    v
}

/// An assembly's metadata, read once for each state of the file.
pub fn clr(p: &Path, read: impl FnOnce(&Path) -> Option<Meta>) -> Option<Meta> {
    let Some(st) = stamp(p) else { return None };
    let k = key(p);
    if let Some((s, v)) = cache().lock().unwrap().store.clr.get(&k) {
        if *s == st {
            return v.clone();
        }
    }
    let v = read(p);
    let mut c = cache().lock().unwrap();
    c.store.clr.insert(k, (st, v.clone()));
    c.dirty = true;
    v
}

/// FNV-1a, 64 bits.
fn fnv(h: &mut u64, b: &[u8]) {
    for x in b {
        *h ^= *x as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}

/// A mod's localization, counted once for each state of its JSON files: `files` are those `scan_loc` reads (each
/// with its stamp), `count` counts them when they changed.
pub fn loc(roots: &[PathBuf], files: &[(PathBuf, Stamp)], count: impl FnOnce() -> Loc) -> Loc {
    let mut k = String::new();
    for r in roots {
        k.push_str(&key(r));
        k.push('\n');
    }
    let mut sig = 0xcbf2_9ce4_8422_2325u64;
    for (f, st) in files {
        fnv(&mut sig, key(f).as_bytes());
        fnv(&mut sig, &st.mtime.to_le_bytes());
        fnv(&mut sig, &st.size.to_le_bytes());
    }
    if let Some((s, v)) = cache().lock().unwrap().store.loc.get(&k) {
        if *s == sig {
            return v.clone();
        }
    }
    let v = count();
    let mut c = cache().lock().unwrap();
    c.store.loc.insert(k, (sig, v.clone()));
    c.dirty = true;
    v
}

#[cfg(test)]
pub fn has_clr(p: &Path) -> bool {
    cache().lock().unwrap().store.clr.contains_key(&key(p))
}

#[cfg(test)]
pub fn forget(p: &Path) {
    cache().lock().unwrap().store.clr.remove(&key(p));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clr_meta::tests::unity_toolkit;
    use crate::instance::tests::{temp_dir, touch};

    #[test]
    fn reads_follow_the_file_and_last_between_runs() {
        let d = temp_dir("cache");
        let dll = d.join("game/BepInEx/plugins/UnityToolkit/UnityToolkit.dll");
        touch(&dll, &unity_toolkit([4, 0, 1, 0]).bytes());
        let first = crate::clr_meta::read(&dll).unwrap();
        assert_eq!(first.refs[2].version, "4.0.1.0");
        // read again without reading the file: the cache answers (a reader that fails proves it is not asked)
        assert_eq!(clr(&dll, |_| None), Some(first.clone()));
        // replaced by another build (another size): read anew
        let mut newer = unity_toolkit([4, 1, 5, 0]);
        newer.refs.push(("ZLinq".into(), [1, 5, 3, 0]));
        touch(&dll, &newer.bytes());
        assert_eq!(crate::clr_meta::read(&dll).unwrap().refs[2].version, "4.1.5.0");
        // gone: nothing
        let gone = d.join("game/x.dll");
        assert_eq!(clr(&gone, |_| panic!("not read: it is not there")), None);
        // kept in the data folder, read back by the next run
        let data = d.join("data").join("cache");
        let _ = crate::clr_meta::read(&dll);
        save_in(&data);
        assert!(data.join(FILE).is_file());
        assert!(!data.join("game").exists() && !d.join("game/.bunker").exists(), "nothing written beside the game");
        // (as the next run: what this one read forgotten, the data folder read)
        forget(&dll);
        assert!(!has_clr(&dll));
        load(&data);
        assert!(has_clr(&dll));
        assert_eq!(clr(&dll, |_| None).unwrap().refs[2].version, "4.1.5.0");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn versions_cached_by_stamp() {
        let d = temp_dir("cache-pe");
        let f = d.join("SPT.Server.exe");
        touch(&f, b"not a program");
        assert_eq!(crate::pever::read(&f), None);
        assert_eq!(pe(&f, |_| Some(PeVersion { file: "9.9.9.9".into(), ..Default::default() })), None, "the stored answer, not read again");
        touch(&f, b"not a program, longer");
        assert_eq!(pe(&f, |_| Some(PeVersion { file: "1.2.3.4".into(), ..Default::default() })).unwrap().file, "1.2.3.4");
        let _ = std::fs::remove_dir_all(&d);
    }
}
