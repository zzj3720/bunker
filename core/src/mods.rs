//! The mods of an instance, server and client parts of one mod tracked as a pair: those Bunker installed (their
//! manifests) and those found installed (paired by name), their versions and Chinese localization; turning one off
//! and on without leaving anything behind, and uninstalling one by its manifest.

use crate::clientcheck::{self, BadDll, ModRef};
use crate::instance::{is_system_patcher, is_system_plugin, Instance};
use crate::manifest::{self, Manifest, Source};
use crate::pever;
use crate::util::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Default)]
pub struct Loc {
    /// `full` (Chinese for all its text), `partial`, `none` (has text in other languages but no Chinese), `na` (no text)
    pub status: String,
    /// Chinese entries
    pub ch: usize,
    /// Entries of its English (or first other) text, to compare with
    pub other: usize,
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ModView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub server: bool,
    pub client: bool,
    pub enabled: bool,
    pub managed: bool,
    pub roots: Vec<String>,
    pub files: usize,
    pub loc: Loc,
    pub source_kind: String,
    pub source_url: String,
    /// `ok` (says it works with this SPT), `mismatch`, `unknown`
    pub compat: String,
    pub spt_range: String,
    /// its client assemblies against this SPT (`clientcheck`): `compatible`, `incompatible`, `unknown`, `` (none)
    pub dll_status: String,
    /// the SPT its client assemblies were built for (the outdated one's, if any)
    pub built_for: String,
    pub bad_dlls: Vec<BadDll>,
    /// mods turned on that need it, and those it needs
    pub required_by: Vec<ModRef>,
    pub requires: Vec<ModRef>,
    /// where an update built for this SPT comes from, `` when unknown
    pub fix_url: String,
}

/// A name reduced for pairing a server part with a client part: the extension, separators, and the words that only
/// say which side it is (`Server`, `ServerMod`, `Client`, `Backend`, `Plugin`) left out.
pub fn pair_key(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let base = lower.strip_suffix(".dll").unwrap_or(&lower);
    let stop = ["server", "servermod", "client", "backend", "plugin", "plugins", "mod", "spt", "bepinex"];
    let sides = ["servermod", "server", "client", "backend", "plugin"];
    let mut out = String::new();
    for tok in base.split(|c: char| !c.is_ascii_alphanumeric()) {
        if tok.is_empty() || stop.contains(&tok) {
            continue;
        }
        let mut t = tok;
        for s in sides {
            if t.len() > s.len() + 2 {
                if let Some(rest) = t.strip_suffix(s) {
                    t = rest;
                }
                if let Some(rest) = t.strip_prefix(s) {
                    t = rest;
                }
            }
        }
        out.push_str(t);
    }
    out
}

/// Whether two names are one mod's two parts: one's key holds the other's (`solarintsain` ⊇ `sain`).
pub fn pairs_with(a: &str, b: &str) -> bool {
    let (x, y) = (pair_key(a), pair_key(b));
    if x.len() < 4 || y.len() < 4 {
        return x == y && !x.is_empty();
    }
    x.contains(&y) || y.contains(&x)
}

fn count_strings(v: &Value) -> usize {
    match v {
        Value::String(_) => 1,
        Value::Array(a) => a.iter().map(count_strings).sum(),
        Value::Object(o) => o.values().map(count_strings).sum(),
        _ => 0,
    }
}

const LANGS: &[&str] = &["ch", "en", "ru", "fr", "ge", "de", "es", "es-mx", "it", "jp", "kr", "pl", "po", "sk", "cz", "hu", "tu", "ro"];

/// Localization of a mod's files: `ch.json` files (as `db/CustomLocales/ch.json`), and inline `"ch": {…}` beside
/// another language's key in its JSON. Counted once for each state of those files (`cache`).
pub fn scan_loc(paths: &[PathBuf]) -> Loc {
    let mut files = Vec::new();
    let mut seen = 0usize;
    'all: for p in paths {
        for f in files_of(p) {
            seen += 1;
            if seen > 20000 {
                break 'all;
            }
            let name = name_of(&f).to_ascii_lowercase();
            if !(name.ends_with(".json") || name.ends_with(".jsonc")) {
                continue;
            }
            if let Some(st) = crate::cache::stamp(&f) {
                files.push((f, st));
            }
        }
    }
    crate::cache::loc(paths, &files, || count_loc(&files))
}

fn count_loc(files: &[(PathBuf, crate::cache::Stamp)]) -> Loc {
    let mut ch = 0;
    let mut other_by: BTreeMap<String, usize> = BTreeMap::new();
    for (f, st) in files {
        let name = name_of(f).to_ascii_lowercase();
        let Some((stem, _)) = name.rsplit_once('.') else { continue };
        if LANGS.contains(&stem) {
            let n = fs::read(f).ok().and_then(|b| parse_jsonc(&b).ok()).map(|v| count_strings(&v)).unwrap_or(0);
            if stem == "ch" {
                ch += n;
            } else {
                *other_by.entry(stem.to_string()).or_default() += n;
            }
            continue;
        }
        if st.size > 2 << 20 {
            continue;
        }
        let Ok(b) = fs::read(f) else { continue };
        // (inline text needs a `"ch"` key: a file without those four bytes has none, and is not parsed)
        if !b.windows(4).any(|w| w == b"\"ch\"") {
            continue;
        }
        let Ok(v) = parse_jsonc(&b) else { continue };
        inline_locales(&v, &mut ch, &mut other_by, 0);
    }
    loc_of(ch, &other_by)
}

fn loc_of(ch: usize, other_by: &BTreeMap<String, usize>) -> Loc {
    let other = other_by.get("en").copied().unwrap_or_else(|| other_by.values().copied().max().unwrap_or(0));
    let mut langs: Vec<String> = other_by.keys().cloned().collect();
    if ch > 0 {
        langs.insert(0, "ch".into());
    }
    let status = if ch == 0 && other == 0 {
        "na"
    } else if ch == 0 {
        "none"
    } else if other == 0 || ch * 10 >= other * 9 {
        "full"
    } else {
        "partial"
    };
    Loc { status: status.into(), ch, other, langs }
}

fn inline_locales(v: &Value, ch: &mut usize, other: &mut BTreeMap<String, usize>, depth: usize) {
    if depth > 12 {
        return;
    }
    match v {
        Value::Object(o) => {
            let has_ch = o.contains_key("ch");
            let others: Vec<&String> = o.keys().filter(|k| *k != "ch" && LANGS.contains(&k.as_str())).collect();
            if has_ch && !others.is_empty() {
                *ch += count_strings(&o["ch"]);
                for k in others {
                    *other.entry(k.clone()).or_default() += count_strings(&o[k.as_str()]);
                }
                return;
            }
            for x in o.values() {
                inline_locales(x, ch, other, depth + 1);
            }
        }
        Value::Array(a) => {
            for x in a {
                inline_locales(x, ch, other, depth + 1);
            }
        }
        _ => {}
    }
}

/// JSON with `//` and `/* */` comments and trailing commas (the `.jsonc` mods write), parsed.
pub fn parse_jsonc(b: &[u8]) -> Result<Value, String> {
    // (most files are plain JSON: parsed as they are; comments and trailing commas only when that fails)
    if let Ok(v) = serde_json::from_slice(strip_bom(b)) {
        return Ok(v);
    }
    let text = String::from_utf8_lossy(strip_bom(b));
    let clean = strip_jsonc(&text);
    serde_json::from_str(&clean).map_err(|e| format!("第 {} 行第 {} 列：{e}", e.line(), e.column()))
}

/// Comments blanked (newlines kept, so errors keep their line numbers) and commas before `}`/`]` removed. One pass:
/// what follows a comma is looked at past spaces and comments without copying anything.
pub fn strip_jsonc(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut in_str = false;
    while i < c.len() {
        let ch = c[i];
        if in_str {
            out.push(ch);
            if ch == '\\' && i + 1 < c.len() {
                out.push(c[i + 1]);
                i += 2;
                continue;
            }
            if ch == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if ch == '"' {
            in_str = true;
            out.push(ch);
            i += 1;
        } else if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
            while i < c.len() && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
            out.push_str("  ");
            i += 2;
            while i < c.len() && !(c[i] == '*' && i + 1 < c.len() && c[i + 1] == '/') {
                out.push(if c[i] == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            if i < c.len() {
                out.push_str("  ");
                i += 2;
            }
        } else if ch == ',' {
            let next = next_significant(&c, i + 1);
            out.push(if matches!(next, Some('}') | Some(']')) { ' ' } else { ',' });
            i += 1;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// The first character from `j` that is not a space or in a comment.
fn next_significant(c: &[char], mut j: usize) -> Option<char> {
    while j < c.len() {
        if c[j].is_whitespace() {
            j += 1;
        } else if c[j] == '/' && j + 1 < c.len() && c[j + 1] == '/' {
            while j < c.len() && c[j] != '\n' {
                j += 1;
            }
        } else if c[j] == '/' && j + 1 < c.len() && c[j + 1] == '*' {
            j += 2;
            while j < c.len() && !(c[j] == '*' && j + 1 < c.len() && c[j + 1] == '/') {
                j += 1;
            }
            j += 2;
        } else {
            return Some(c[j]);
        }
    }
    None
}

fn dll_version(p: &Path) -> String {
    pever::read(p).map(|v| v.short()).filter(|s| s != "0.0.0.0" && s != "0.0.0").unwrap_or_default()
}

/// A server mod folder's name, version, author and SPT range: its package.json (3.x), else its dll's version.
fn server_meta(dir: &Path) -> (String, String, String, String) {
    if let Ok(v) = read_json(&dir.join("package.json")) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        return (s("name"), s("version"), s("author"), s("sptVersion"));
    }
    let n = name_of(dir);
    let dlls: Vec<PathBuf> = list_dir(dir).into_iter().filter(|p| name_of(p).to_ascii_lowercase().ends_with(".dll")).collect();
    let main = dlls
        .iter()
        .find(|p| pair_key(&name_of(p)) == pair_key(&n))
        .or(dlls.first())
        .cloned();
    let version = main.map(|p| dll_version(&p)).unwrap_or_default();
    (String::new(), version, String::new(), String::new())
}

fn client_meta(p: &Path) -> String {
    if p.is_file() {
        return dll_version(p);
    }
    let n = name_of(p);
    let dlls: Vec<PathBuf> = walk_files(p).into_iter().filter(|f| name_of(f).to_ascii_lowercase().ends_with(".dll")).collect();
    dlls.iter()
        .find(|f| pair_key(&name_of(f)) == pair_key(&n))
        .or(dlls.first())
        .map(|f| dll_version(f))
        .unwrap_or_default()
}

/// Whether an SPT version satisfies a package.json `sptVersion` range (`~3.11`, `^3.10.0`, `>=3.9.0`, `3.11.x`).
pub fn spt_compat(range: &str, spt: &str) -> &'static str {
    let r = range.trim();
    if r.is_empty() || spt.is_empty() {
        return "unknown";
    }
    let nums = |s: &str| -> Vec<u64> {
        s.trim_start_matches(['~', '^', '>', '=', '<', 'v', ' '])
            .split('.')
            .map_while(|p| p.parse::<u64>().ok())
            .collect()
    };
    let want = nums(r);
    let have = nums(spt);
    if want.is_empty() || have.is_empty() {
        return "unknown";
    }
    let at = |v: &Vec<u64>, i: usize| v.get(i).copied().unwrap_or(0);
    let ok = if r.starts_with(">=") {
        (at(&have, 0), at(&have, 1), at(&have, 2)) >= (at(&want, 0), at(&want, 1), at(&want, 2))
    } else if r.starts_with('^') {
        at(&have, 0) == at(&want, 0) && (at(&have, 1), at(&have, 2)) >= (at(&want, 1), at(&want, 2))
    } else {
        // ~x.y, x.y.x, x.y.z: the same major and minor
        at(&have, 0) == at(&want, 0) && (want.len() < 2 || at(&have, 1) == at(&want, 1))
    };
    if ok {
        "ok"
    } else {
        "mismatch"
    }
}

/// Where a root's files are now: in place when enabled, in the disabled store when not.
fn root_now(root: &Path, m: &Manifest, r: &str) -> PathBuf {
    if m.enabled {
        safe_join(root, r).unwrap_or_default()
    } else {
        safe_join(&manifest::disabled_dir(root, &m.id), r).unwrap_or_default()
    }
}

fn is_server_rel(inst: &Instance, r: &str) -> bool {
    r.starts_with(&inst.mods_rel())
}

/// Every mod of an instance, managed ones first by name, then those found installed.
pub fn list(inst: &Instance) -> Vec<ModView> {
    let root = inst.root_path();
    let mut manifests = manifest::load_all(&root);
    let mut loose = loose_patchers(inst, &manifests);
    for m in manifests.iter_mut().filter(|m| m.source.kind == "adopted") {
        let mine: Vec<String> = loose.iter().filter(|p| claims(&m.roots, p)).cloned().collect();
        loose.retain(|p| !mine.contains(p));
        m.roots.extend(mine);
    }
    let mut owned: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for m in &manifests {
        owned.extend(m.roots.iter().map(|r| r.to_ascii_lowercase()));
        let server_paths: Vec<PathBuf> = m.roots.iter().filter(|r| is_server_rel(inst, r)).map(|r| root_now(&root, m, r)).collect();
        let (mut author, mut range) = (String::new(), String::new());
        for p in &server_paths {
            let (_, _, a, s) = server_meta(p);
            if !a.is_empty() {
                author = a;
            }
            if !s.is_empty() {
                range = s;
            }
        }
        out.push(ModView {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            author,
            server: !server_paths.is_empty(),
            client: m.roots.iter().any(|r| r.starts_with("BepInEx/")),
            enabled: m.enabled,
            managed: m.source.kind != "adopted",
            roots: m.roots.clone(),
            files: m.files.len().max(m.roots.iter().map(|r| manifest::files_under(&root, r).len()).sum()),
            loc: scan_loc(&server_paths),
            source_kind: m.source.kind.clone(),
            source_url: m.source.url.clone(),
            compat: spt_compat(&range, &inst.spt_version).into(),
            spt_range: range,
            ..Default::default()
        });
    }
    // found installed: server folders and plugins not owned by a manifest
    let mods_rel = inst.mods_rel();
    let servers: Vec<PathBuf> = list_dir(&inst.mods_dir())
        .into_iter()
        .filter(|p| p.is_dir() && !owned.contains(&format!("{mods_rel}/{}", name_of(p)).to_ascii_lowercase()))
        .collect();
    let mut clients: Vec<PathBuf> = list_dir(&inst.plugins_dir())
        .into_iter()
        .filter(|p| {
            let n = name_of(p);
            (p.is_dir() || n.to_ascii_lowercase().ends_with(".dll"))
                && !is_system_plugin(&n)
                && !owned.contains(&format!("BepInEx/plugins/{n}").to_ascii_lowercase())
        })
        .collect();
    let mut found = Vec::new();
    for s in servers {
        let sn = name_of(&s);
        let mut mine: Vec<PathBuf> = clients.iter().filter(|c| pairs_with(&sn, &name_of(c))).cloned().collect();
        clients.retain(|c| !mine.contains(c));
        let names: Vec<String> = std::iter::once(sn.clone()).chain(mine.iter().map(|c| name_of(c))).collect();
        let pats: Vec<String> = loose.iter().filter(|p| names.iter().any(|n| pairs_with(n, &name_of(Path::new(p))))).cloned().collect();
        loose.retain(|p| !pats.contains(p));
        mine.extend(pats.iter().map(|p| root.join(p)));
        let (pkg_name, mut version, author, range) = server_meta(&s);
        if version.is_empty() {
            version = mine.iter().map(|c| client_meta(c)).find(|v| !v.is_empty()).unwrap_or_default();
        }
        let mut roots = vec![rel(&root, &s)];
        roots.extend(mine.iter().map(|c| rel(&root, c)));
        let files: usize = roots.iter().map(|r| manifest::files_under(&root, r).len()).sum();
        found.push(ModView {
            id: String::new(),
            name: if pkg_name.is_empty() { sn.clone() } else { pkg_name },
            version,
            author,
            server: true,
            client: !mine.is_empty(),
            enabled: true,
            managed: false,
            roots,
            files,
            loc: scan_loc(&[s.clone()]),
            source_kind: String::new(),
            source_url: String::new(),
            compat: spt_compat(&range, &inst.spt_version).into(),
            spt_range: range,
            ..Default::default()
        });
    }
    let mut lone: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for c in clients {
        let r = rel(&root, &c);
        let pats: Vec<String> = loose.iter().filter(|p| claims(&[r.clone()], p)).cloned().collect();
        loose.retain(|p| !pats.contains(p));
        lone.push((c, std::iter::once(r).chain(pats).collect()));
    }
    lone.extend(loose.into_iter().map(|p| (root.join(&p), vec![p])));
    for (c, roots) in lone {
        let n = name_of(&c);
        found.push(ModView {
            id: String::new(),
            name: n.strip_suffix(".dll").unwrap_or(&n).to_string(),
            version: client_meta(&c),
            client: true,
            enabled: true,
            files: roots.iter().map(|r| manifest::files_under(&root, r).len()).sum(),
            roots,
            loc: Loc { status: "na".into(), ..Default::default() },
            compat: "unknown".into(),
            ..Default::default()
        });
    }
    found.sort_by_key(|m| m.name.to_ascii_lowercase());
    let mut used: Vec<String> = out.iter().map(|m| m.id.clone()).collect();
    for mut m in found {
        let base = format!("u-{}", slug(&m.name));
        let mut id = base.clone();
        let mut n = 2;
        while used.contains(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        used.push(id.clone());
        m.id = id;
        out.push(m);
    }
    clientcheck::annotate(inst, &mut out);
    out
}

/// `BepInEx/patchers` entries no manifest names (SPT's own left out), relative to the root.
fn loose_patchers(inst: &Instance, manifests: &[Manifest]) -> Vec<String> {
    let root = inst.root_path();
    let owned: Vec<String> = manifests.iter().flat_map(|m| m.roots.iter().map(|r| r.to_ascii_lowercase())).collect();
    list_dir(&root.join("BepInEx").join("patchers"))
        .into_iter()
        .filter(|p| {
            let n = name_of(p);
            (p.is_dir() || n.to_ascii_lowercase().ends_with(".dll")) && !is_system_patcher(&n)
        })
        .map(|p| rel(&root, &p))
        .filter(|r| !owned.contains(&r.to_ascii_lowercase()))
        .collect()
}

/// Whether a patcher entry is the mod's whose roots these are: one of its plugins has its name (`plugins/UnityToolkit`
/// and `patchers/UnityToolkit`).
fn claims(roots: &[String], patcher: &str) -> bool {
    let pn = name_of(Path::new(patcher));
    roots.iter().any(|r| r.to_ascii_lowercase().starts_with("bepinex/plugins/") && pairs_with(&name_of(Path::new(r)), &pn))
}

/// A mod by its id; one found installed is taken over first (a manifest of what it has), so it can be turned off
/// and uninstalled as precisely as one Bunker installed.
pub fn take(inst: &Instance, id: &str) -> Res<Manifest> {
    let root = inst.root_path();
    if let Some(mut m) = manifest::load(&root, id) {
        if m.source.kind == "adopted" {
            // (a patcher of its plugin's name, found loose: its own too, so it goes off and on with it)
            let all = manifest::load_all(&root);
            let mine: Vec<String> = loose_patchers(inst, &all).into_iter().filter(|p| claims(&m.roots, p)).collect();
            if !mine.is_empty() {
                for p in &mine {
                    m.files.extend(manifest::files_under(&root, p));
                }
                m.roots.extend(mine);
                manifest::save(&root, &m)?;
            }
        }
        return Ok(m);
    }
    let v = list(inst).into_iter().find(|m| m.id == id).ok_or_else(|| format!("没有找到 Mod：{id}"))?;
    let mid = manifest::fresh_id(&root, &v.name);
    let files = v.roots.iter().flat_map(|r| manifest::files_under(&root, r)).collect();
    let m = Manifest {
        id: mid,
        name: v.name,
        version: v.version,
        source: Source { kind: "adopted".into(), ..Default::default() },
        installed_at: now_ms(),
        enabled: true,
        roots: v.roots,
        files,
    };
    manifest::save(&root, &m)?;
    Ok(m)
}

/// Turns a mod off (its entries moved into `.bunker/disabled/<id>/`) or back on; returns its manifest's id.
pub fn set_enabled(inst: &Instance, id: &str, on: bool) -> Res<String> {
    let root = inst.root_path();
    let mut m = take(inst, id)?;
    if m.enabled == on {
        return Ok(m.id);
    }
    let store = manifest::disabled_dir(&root, &m.id);
    // nothing moves unless all can
    for r in &m.roots {
        let (from, to) = if on { (safe_join(&store, r)?, safe_join(&root, r)?) } else { (safe_join(&root, r)?, safe_join(&store, r)?) };
        if from.exists() && to.exists() {
            return err(format!("目标已存在，无法{}：{}", if on { "启用" } else { "禁用" }, to.display()));
        }
    }
    for r in &m.roots {
        let (from, to) = if on { (safe_join(&store, r)?, safe_join(&root, r)?) } else { (safe_join(&root, r)?, safe_join(&store, r)?) };
        if from.exists() {
            move_path(&from, &to)?;
            // (folders left empty in the store go; the instance's own folders, as plugins/, stay)
            if let (true, Some(parent)) = (on, from.parent()) {
                prune_empty(parent.to_path_buf(), &store);
            }
        }
    }
    if on {
        let _ = fs::remove_dir(&store);
        prune_empty(store.parent().unwrap().to_path_buf(), &root.join(".bunker"));
    }
    m.enabled = on;
    manifest::save(&root, &m)?;
    Ok(m.id)
}

/// Removes a mod: every file its manifest names, the entries it owned (with what the mod wrote into them since),
/// folders left empty, its disabled copy and its manifest.
pub fn uninstall(inst: &Instance, id: &str) -> Res<usize> {
    let root = inst.root_path();
    let m = take(inst, id)?;
    let store = manifest::disabled_dir(&root, &m.id);
    let mut removed = 0;
    for f in &m.files {
        for base in [&root, &store] {
            let p = safe_join(base, f)?;
            if p.is_file() {
                fs::remove_file(&p).at("删除文件", &p)?;
                removed += 1;
                if let Some(parent) = p.parent() {
                    prune_empty(parent.to_path_buf(), base);
                }
            }
        }
    }
    for r in &m.roots {
        for base in [&root, &store] {
            let p = safe_join(base, r)?;
            if p.exists() {
                removed += files_of(&p).len();
                remove_path(&p)?;
            }
            if let Some(parent) = p.parent() {
                prune_empty(parent.to_path_buf(), base);
            }
        }
    }
    remove_path(&store)?;
    prune_empty(store.parent().unwrap().to_path_buf(), &root.join(".bunker"));
    manifest::delete(&root, &m.id)?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, touch};

    #[test]
    fn pairs_the_real_names() {
        let yes = [
            ("Solarint-SAIN-ServerMod", "SAIN"),
            ("BlackDivServer", "BlackDiv"),
            ("CSGasServer", "CSGas"),
            ("Skwizzy.LootingBots", "Skwizzy.LootingBots"),
            ("Skwizzy-LootingBots-ServerMod", "skwizzy.LootingBots.dll"),
            ("Tyfon.UIFixes.Server", "Tyfon.UIFixes.dll"),
            ("Tyfon.UIFixes.Server", "Tyfon.UIFixes.Net.dll"),
            ("swiftxp-showmethemoney", "SwiftXP.ShowMeTheMoney.dll"),
            ("WTT-ServerCommonLib", "WTT-ClientCommonLib"),
            ("WTT-ContentBackport", "WTT-ContentBackportClient"),
            ("MoreBotsServer", "MoreBotsPlugin"),
            ("MoreCheckmarksBackend", "MoreCheckmarks"),
            ("mpstark-dynamicmaps", "DynamicMaps"),
            ("VisitAPI-Server", "VisitAPI"),
        ];
        for (s, c) in yes {
            assert!(pairs_with(s, c), "{s} / {c}: {} {}", pair_key(s), pair_key(c));
        }
        let no = [("GuidingLight", "FOVFix.dll"), ("HideoutBackport", "WTT-ContentBackportClient"), ("QuickSell", "SAIN")];
        for (s, c) in no {
            assert!(!pairs_with(s, c), "{s} / {c}");
        }
    }

    #[test]
    fn jsonc_comments_and_commas() {
        let v = parse_jsonc(b"{\n // a\n \"a\": 1, /* b */ \"s\": \"x // not a comment\",\n \"l\": [1,2,],\n}").unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["s"], "x // not a comment");
        assert_eq!(v["l"].as_array().unwrap().len(), 2);
        let e = parse_jsonc(b"{\n\"a\": 1\n\"b\": 2}").unwrap_err();
        assert!(e.contains("第 3 行"), "{e}");
    }

    #[test]
    fn localization_status() {
        let r = fake_instance("loc", "SPT_Runtime");
        let m = r.join("SPT_Runtime/user/mods");
        touch(&m.join("Full/db/CustomLocales/ch.json"), br#"{"a":"jia","b":"yi"}"#);
        touch(&m.join("Full/db/CustomLocales/en.json"), br#"{"a":"A","b":"B"}"#);
        touch(&m.join("Half/locales/ch.json"), br#"{"a":"x"}"#);
        touch(&m.join("Half/locales/en.json"), br#"{"a":"A","b":"B","c":"C"}"#);
        touch(&m.join("None/locales/en.json"), br#"{"a":"A"}"#);
        touch(&m.join("Inline/quests.jsonc"), b"{ // q\n \"q1\": {\"name\": {\"en\": \"Go\", \"ch\": \"\xe5\x8e\xbb\"}} }");
        touch(&m.join("Logic/Logic.dll"), b"MZ");
        let l = |n: &str| scan_loc(&[m.join(n)]);
        assert_eq!(l("Full").status, "full");
        assert_eq!(l("Full").ch, 2);
        assert_eq!(l("Half").status, "partial");
        assert_eq!(l("None").status, "none");
        assert_eq!(l("Inline").status, "full");
        assert_eq!(l("Inline").ch, 1);
        assert_eq!(l("Logic").status, "na");
        let _ = fs::remove_dir_all(&r);
    }

    /// What `strip_jsonc` did before (each comma before a comment copied the rest and stripped it again): the reference
    /// its one pass must agree with.
    fn strip_jsonc_before(s: &str) -> String {
        let c: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        let mut in_str = false;
        while i < c.len() {
            let ch = c[i];
            if in_str {
                out.push(ch);
                if ch == '\\' && i + 1 < c.len() {
                    out.push(c[i + 1]);
                    i += 2;
                    continue;
                }
                if ch == '"' {
                    in_str = false;
                }
                i += 1;
                continue;
            }
            if ch == '"' {
                in_str = true;
                out.push(ch);
                i += 1;
            } else if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
                while i < c.len() && c[i] != '\n' {
                    out.push(' ');
                    i += 1;
                }
            } else if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
                out.push_str("  ");
                i += 2;
                while i < c.len() && !(c[i] == '*' && i + 1 < c.len() && c[i + 1] == '/') {
                    out.push(if c[i] == '\n' { '\n' } else { ' ' });
                    i += 1;
                }
                out.push_str("  ");
                i += 2;
            } else if ch == ',' {
                let mut j = i + 1;
                while j < c.len() && c[j].is_whitespace() {
                    j += 1;
                }
                let rest: String = c[j..c.len().min(j + 2)].iter().collect();
                if j < c.len() && (c[j] == '}' || c[j] == ']') {
                    out.push(' ');
                } else if rest == "//" || rest == "/*" {
                    let tail: String = c[j..].iter().collect();
                    let after = strip_jsonc_before(&tail);
                    out.push(if after.trim_start().starts_with('}') || after.trim_start().starts_with(']') { ' ' } else { ',' });
                } else {
                    out.push(',');
                }
                i += 1;
            } else {
                out.push(ch);
                i += 1;
            }
        }
        out
    }

    #[test]
    fn strip_jsonc_agrees_with_before_and_is_linear() {
        let cases = [
            "{\n \"a\": 1, // one\n \"b\": [1, 2, /* two */ ],\n \"c\": {\"d\": 3, // last\n }, /* x */ \n}",
            "[1, // a\n /* b */ 2, // c\n]",
            "{\"s\": \"a, // not\", \"t\": \"\\\" ,}\" , }",
        ];
        for c in cases {
            assert_eq!(strip_jsonc(c), strip_jsonc_before(c), "{c}");
        }
        assert_eq!(parse_jsonc(cases[0].as_bytes()).unwrap()["c"]["d"], 3);
        // (an unclosed comment: blanked to the end, as before less the two spaces it added past the end)
        assert!(parse_jsonc(b"{\"a\": 1 /* unclosed").is_err());
        assert_eq!(strip_jsonc("1 /* x"), "1     ");
        // a config of many entries, each with a comment after its comma: the old way copied and stripped the rest at
        // each, again within each copy (exponential); 40 already took longer than this whole test may
        let mut big = String::from("{\n");
        for i in 0..20000 {
            big.push_str(&format!("  \"key{i}\": {i}, // what key{i} does\n"));
        }
        big.push_str("  \"last\": true, // the end\n}\n");
        let t = std::time::Instant::now();
        let v = parse_jsonc(big.as_bytes()).unwrap();
        let ms = t.elapsed().as_millis();
        assert_eq!(v["key19999"], 19999);
        assert_eq!(v["last"], true);
        assert!(ms < 1500, "20000 commented entries took {ms} ms");
        let small: String = big.lines().take(12).chain(["}"]).collect::<Vec<_>>().join("\n");
        assert_eq!(strip_jsonc(&small), strip_jsonc_before(&small));
    }

    /// `scan_loc` as it was: every JSON file parsed, no `"ch"` filter, no cache.
    fn scan_loc_before(paths: &[PathBuf]) -> Loc {
        let mut ch = 0;
        let mut other_by: BTreeMap<String, usize> = BTreeMap::new();
        for p in paths {
            for f in files_of(p) {
                let name = name_of(&f).to_ascii_lowercase();
                let Some((stem, ext)) = name.rsplit_once('.') else { continue };
                if ext != "json" && ext != "jsonc" {
                    continue;
                }
                let size = fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
                if LANGS.contains(&stem) {
                    let n = fs::read(&f).ok().and_then(|b| parse_jsonc(&b).ok()).map(|v| count_strings(&v)).unwrap_or(0);
                    if stem == "ch" {
                        ch += n;
                    } else {
                        *other_by.entry(stem.to_string()).or_default() += n;
                    }
                    continue;
                }
                if size > 2 << 20 {
                    continue;
                }
                let Some(v) = fs::read(&f).ok().and_then(|b| parse_jsonc(&b).ok()) else { continue };
                inline_locales(&v, &mut ch, &mut other_by, 0);
            }
        }
        loc_of(ch, &other_by)
    }

    #[test]
    fn scan_loc_filter_agrees_and_the_cache_follows_changes() {
        let r = fake_instance("locfast", "SPT_Runtime");
        let m = r.join("SPT_Runtime/user/mods/Mixed");
        touch(&m.join("quests.jsonc"), b"{ // q\n \"q1\": {\"name\": {\"en\": \"Go\", \"ch\": \"\xe5\x8e\xbb\"}, }, }");
        touch(&m.join("items.json"), br#"{"i": {"name": {"en": "Gun", "ru": "Pushka"}}}"#);
        touch(&m.join("values.json"), br#"{"lang": "ch", "x": ["ch", "en"]}"#);
        touch(&m.join("only_ch.json"), br#"{"t": {"ch": "a"}}"#);
        touch(&m.join("locales/en.json"), br#"{"a": "A", "b": "B"}"#);
        touch(&m.join("broken.json"), b"{\"ch\": ");
        let want = scan_loc_before(&[m.clone()]);
        assert_eq!(scan_loc(&[m.clone()]), want);
        assert_eq!(want.ch, 1);
        // (and again, from the cache)
        assert_eq!(scan_loc(&[m.clone()]), want);
        // a file added: counted anew
        touch(&m.join("locales/ch.json"), br#"{"a": "jia", "b": "yi"}"#);
        let now = scan_loc(&[m.clone()]);
        assert_eq!(now, scan_loc_before(&[m.clone()]));
        assert_eq!(now.ch, 3);
        // changed (another size), and removed
        touch(&m.join("locales/ch.json"), br#"{"a": "jia"}"#);
        assert_eq!(scan_loc(&[m.clone()]).ch, 2);
        fs::remove_file(m.join("locales/ch.json")).unwrap();
        assert_eq!(scan_loc(&[m.clone()]), want);
        // turned off: its files moved, nothing of it left to count at its old place
        assert_eq!(scan_loc(&[r.join("SPT_Runtime/user/mods/Gone")]).status, "na");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn compat_ranges() {
        assert_eq!(spt_compat("~3.11", "3.11.3"), "ok");
        assert_eq!(spt_compat("~3.10", "3.11.3"), "mismatch");
        assert_eq!(spt_compat("^3.9.0", "3.11.0"), "ok");
        assert_eq!(spt_compat(">=3.8.0", "4.1.6"), "ok");
        assert_eq!(spt_compat("", "4.1.6"), "unknown");
    }

    fn snapshot_of(r: &Path) -> Vec<String> {
        walk_files(r).iter().map(|f| rel(r, f)).filter(|p| !p.starts_with(".bunker/")).collect()
    }

    #[test]
    fn lists_pairs_and_toggles_without_residue() {
        let r = fake_instance("toggle", "SPT_Runtime");
        let m = r.join("SPT_Runtime/user/mods");
        touch(&m.join("Solarint-SAIN-ServerMod/package.json"), br#"{"name":"SAIN","version":"4.2.0","author":"Solarint","sptVersion":"~4.1"}"#);
        touch(&m.join("Solarint-SAIN-ServerMod/config.json"), b"{}");
        touch(&r.join("BepInEx/plugins/SAIN/SAIN.dll"), b"MZ");
        touch(&r.join("BepInEx/plugins/SAIN/presets/a.json"), b"{}");
        touch(&m.join("GuidingLight/GuidingLight.dll"), b"MZ");
        touch(&r.join("BepInEx/plugins/FOVFix.dll"), b"MZ");
        let inst = detect(&r);
        let mods = list(&inst);
        assert_eq!(mods.len(), 3, "{mods:#?}");
        let sain = mods.iter().find(|m| m.name == "SAIN").unwrap();
        assert!(sain.server && sain.client);
        assert_eq!(sain.version, "4.2.0");
        assert_eq!(sain.files, 4);
        assert_eq!(sain.compat, "unknown", "a made-up instance has no SPT version");
        let fov = mods.iter().find(|m| m.name == "FOVFix").unwrap();
        assert!(fov.client && !fov.server);

        let before = snapshot_of(&r);
        let id = set_enabled(&inst, &sain.id, false).unwrap();
        let mid = snapshot_of(&r);
        assert!(!mid.iter().any(|p| p.contains("SAIN")), "{mid:?}");
        assert_eq!(walk_files(&manifest::disabled_dir(&r, &id)).len(), 4);
        let listed = list(&inst);
        let off = listed.iter().find(|m| m.id == id).unwrap();
        assert!(!off.enabled && off.server && off.client);
        set_enabled(&inst, &id, true).unwrap();
        assert_eq!(snapshot_of(&r), before);
        assert!(!r.join(".bunker/disabled").exists(), "the disabled store is gone once empty");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn uninstall_by_manifest_leaves_others() {
        let r = fake_instance("uninst", "SPT_Runtime");
        let m = r.join("SPT_Runtime/user/mods");
        touch(&m.join("BlackDivServer/BlackDivServer.dll"), b"MZ");
        touch(&m.join("BlackDivServer/db/CustomLocales/ch.json"), b"{}");
        touch(&r.join("BepInEx/plugins/BlackDiv/BlackDiv.dll"), b"MZ");
        touch(&m.join("GuidingLight/GuidingLight.dll"), b"MZ");
        let inst = detect(&r);
        let bd = list(&inst).into_iter().find(|m| m.name == "BlackDivServer").unwrap();
        assert!(bd.client);
        // disabled first: uninstall removes its stored copy too
        let id = set_enabled(&inst, &bd.id, false).unwrap();
        let n = uninstall(&inst, &id).unwrap();
        assert_eq!(n, 3);
        assert!(!m.join("BlackDivServer").exists());
        assert!(!r.join("BepInEx/plugins/BlackDiv").exists());
        assert!(m.join("GuidingLight/GuidingLight.dll").exists());
        assert!(manifest::load_all(&r).is_empty());
        assert!(!r.join(".bunker/disabled").exists());
        let _ = fs::remove_dir_all(&r);
    }
}
