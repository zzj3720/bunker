//! The client side checked as SPT checks it when the game starts: every BepInEx plugin and patcher assembly read
//! (its .NET metadata, `clr_meta`), the SPT version it was built against taken from its references to SPT's own
//! assemblies (`spt-reflection`, `spt-core`, … , or `aki-*` before 3.10), and set against the instance's SPT by major
//! and minor (`4.1.5` runs on `4.1.6`, `4.0.1` does not). Which mods need which, from the assemblies they reference
//! and the plugins they say they depend on (`[BepInDependency]`). Turning off a mod with those that need it, and an
//! update to a build for this SPT (its old files backed up first, put back if it fails).

use crate::clr_meta::{self, Meta};
use crate::instance::Instance;
use crate::manifest::{self, Source};
use crate::mods::{self, ModView};
use crate::util::*;
use crate::{install, sources};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A client assembly not built for this instance's SPT.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct BadDll {
    /// relative to the instance root, `/`-separated (`BepInEx/plugins/UnityToolkit/UnityToolkit.dll`)
    pub path: String,
    /// as SPT names it: relative to `plugins` or `patchers`, `\`-separated (`UnityToolkit\UnityToolkit.dll`)
    pub shown: String,
    /// the SPT it was built for (`4.0.1`)
    pub built_for: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ModRef {
    pub id: String,
    pub name: String,
}

/// A client assembly of the instance, read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dll {
    pub path: String,
    pub meta: Meta,
    /// the SPT it was built for, `` when it references none of SPT's assemblies
    pub built_for: String,
    /// `compatible`, `incompatible`, `unknown` (no SPT reference, or the instance's SPT unknown)
    pub status: String,
}

/// Mods whose builds for each SPT are released somewhere Bunker can read, by the name they are known by: the base
/// libraries other mods need, kept up by the community when their authors stopped.
pub const KNOWN_SOURCES: &[(&str, &str)] = &[("unitytoolkit", "https://github.com/Tylevo/UnityToolkit-New")];

/// Files SPT itself puts in BepInEx: not checked, never a mod's.
pub fn is_spt_own(rel_path: &str) -> bool {
    let l = rel_path.to_ascii_lowercase();
    let name = l.rsplit('/').next().unwrap_or(&l);
    l.starts_with("bepinex/plugins/spt/") || l.starts_with("bepinex/patchers/spt/") || name == "spt-prepatch.dll" || name == "fixplugintypesserialization.dll"
}

fn is_spt_assembly(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.starts_with("spt-") || l.starts_with("aki-")
}

fn nums(v: &str) -> Vec<u64> {
    v.split(['.', '-', '+']).map_while(|p| p.parse::<u64>().ok()).collect()
}

/// `major.minor` of a version (`4.1.6` → `4.1`), `` when it has none.
pub fn major_minor(v: &str) -> String {
    let n = nums(v.trim_start_matches(['v', 'V']));
    if n.len() < 2 {
        return String::new();
    }
    format!("{}.{}", n[0], n[1])
}

/// What an assembly's references to SPT's say: the SPT it was built for, and whether that is this one's.
pub fn judge(meta: &Meta, spt: &str) -> (String, String) {
    let spt_refs: Vec<_> = meta.refs.iter().filter(|r| is_spt_assembly(&r.name)).collect();
    let Some(best) = spt_refs.iter().max_by(|a, b| version_cmp(&a.version, &b.version)) else {
        return (String::new(), "unknown".into());
    };
    let n = nums(&best.version);
    let have = nums(spt);
    let aki = best.name.to_ascii_lowercase().starts_with("aki-");
    if n.first().copied().unwrap_or(0) >= 3 {
        // (SPT's assemblies carry its version: 4.0.1.0 is SPT 4.0.1)
        let built = n.iter().take(3).map(|x| x.to_string()).collect::<Vec<_>>().join(".");
        if have.len() < 2 {
            return (built, "unknown".into());
        }
        let ok = n.get(0..2) == have.get(0..2);
        return (built, if ok { "compatible" } else { "incompatible" }.into());
    }
    if aki {
        // SPT 3.10 renamed its aki-* assemblies spt-*: a plugin still asking for them cannot load on it
        let built = "3.9 及更早（aki）".to_string();
        let renamed = have.len() >= 2 && (have[0], have[1]) >= (3, 10);
        return (built, if renamed { "incompatible" } else { "unknown" }.into());
    }
    (String::new(), "unknown".into())
}

/// Every plugin and patcher assembly in place (those of mods turned off are in `.bunker/disabled`, not loaded).
pub fn scan(inst: &Instance) -> Vec<Dll> {
    let root = inst.root_path();
    let mut out = Vec::new();
    for sub in ["plugins", "patchers"] {
        for f in walk_files(&root.join("BepInEx").join(sub)) {
            if !name_of(&f).to_ascii_lowercase().ends_with(".dll") {
                continue;
            }
            let r = rel(&root, &f);
            if is_spt_own(&r) {
                continue;
            }
            let Some(meta) = clr_meta::read(&f) else { continue };
            let (built_for, status) = judge(&meta, &inst.spt_version);
            out.push(Dll { path: r, meta, built_for, status });
        }
    }
    out
}

/// The path as SPT's warning says it: below `plugins` or `patchers`, with `\`.
fn shown(path: &str) -> String {
    let p = path.splitn(3, '/').nth(2).unwrap_or(path);
    p.replace('/', "\\")
}

fn under(path: &str, root: &str) -> bool {
    let (p, r) = (path.to_ascii_lowercase(), root.to_ascii_lowercase());
    p == r || p.starts_with(&format!("{r}/"))
}

/// What a set of assemblies is known by (their names, their plugins' GUIDs), and what it asks for (the assemblies
/// it references, the plugins it cannot load without), lowercased.
fn identity(dlls: &[&Dll]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut is = BTreeSet::new();
    let mut needs = BTreeSet::new();
    for d in dlls {
        if !d.meta.name.is_empty() {
            is.insert(d.meta.name.to_ascii_lowercase());
        }
        for p in &d.meta.plugins {
            is.insert(p.guid.to_ascii_lowercase());
        }
        for r in &d.meta.refs {
            needs.insert(r.name.to_ascii_lowercase());
        }
        for dep in d.meta.deps.iter().filter(|d| !d.soft) {
            needs.insert(dep.guid.to_ascii_lowercase());
        }
    }
    (is, needs)
}

/// Where a mod's update for this SPT comes from: its manifest's GitHub link, else a known source for its name.
fn fix_source(m: &ModView, is: &BTreeSet<String>) -> String {
    if m.source_kind == "github" && !m.source_url.is_empty() {
        return m.source_url.clone();
    }
    let key = mods::pair_key(&m.name);
    for (name, url) in KNOWN_SOURCES {
        if key == *name || is.contains(*name) {
            return url.to_string();
        }
    }
    String::new()
}

/// The mods' client check filled in: each one's assemblies judged, what needs it and what it needs (among mods
/// turned on), and where an update for this SPT would come from.
pub fn annotate(inst: &Instance, list: &mut [ModView]) {
    let dlls = scan(inst);
    let mine: Vec<Vec<&Dll>> = list
        .iter()
        .map(|m| if m.enabled { dlls.iter().filter(|d| m.roots.iter().any(|r| under(&d.path, r))).collect() } else { vec![] })
        .collect();
    let ids: Vec<(BTreeSet<String>, BTreeSet<String>)> = mine.iter().map(|d| identity(d)).collect();
    let refs: Vec<ModRef> = list.iter().map(|m| ModRef { id: m.id.clone(), name: m.name.clone() }).collect();
    for i in 0..list.len() {
        let ds = &mine[i];
        let bad: Vec<BadDll> = ds
            .iter()
            .filter(|d| d.status == "incompatible")
            .map(|d| BadDll { path: d.path.clone(), shown: shown(&d.path), built_for: d.built_for.clone() })
            .collect();
        let m = &mut list[i];
        m.dll_status = if ds.is_empty() {
            String::new()
        } else if !bad.is_empty() {
            "incompatible".into()
        } else if ds.iter().any(|d| d.status == "compatible") {
            "compatible".into()
        } else {
            "unknown".into()
        };
        m.built_for = match bad.first() {
            Some(b) => b.built_for.clone(),
            None => ds.iter().filter(|d| !d.built_for.is_empty()).map(|d| d.built_for.clone()).max_by(|a, b| version_cmp(a, b)).unwrap_or_default(),
        };
        m.bad_dlls = bad;
        m.fix_url = fix_source(m, &ids[i].0);
        let needs = |a: usize, b: usize| a != b && ids[a].1.iter().any(|n| ids[b].0.contains(n));
        m.required_by = (0..ids.len()).filter(|&j| needs(j, i)).map(|j| refs[j].clone()).collect();
        m.requires = (0..ids.len()).filter(|&j| needs(i, j)).map(|j| refs[j].clone()).collect();
    }
}

/// A mod's id and every mod turned on that needs it, those needing them too: what turning it off with its
/// dependents turns off (dependents first).
pub fn with_dependents(list: &[ModView], id: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut todo = vec![id.to_string()];
    while let Some(x) = todo.pop() {
        if out.contains(&x) {
            continue;
        }
        out.push(x.clone());
        if let Some(m) = list.iter().find(|m| m.id == x) {
            for r in &m.required_by {
                todo.push(r.id.clone());
            }
        }
    }
    out.reverse();
    out
}

/// Turns mods off, each with those that need it when `dependents`; the manifests' ids turned off.
pub fn disable(inst: &Instance, ids: &[String], dependents: bool) -> Res<Vec<String>> {
    let list = mods::list(inst);
    let mut order: Vec<String> = Vec::new();
    for id in ids {
        if !list.iter().any(|m| &m.id == id) {
            return err(format!("没有找到 Mod：{id}"));
        }
        let these = if dependents { with_dependents(&list, id) } else { vec![id.clone()] };
        for x in these {
            if !order.contains(&x) {
                order.push(x);
            }
        }
    }
    let mut done = Vec::new();
    for x in order {
        done.push(mods::set_enabled(inst, &x, false)?);
    }
    Ok(done)
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Fixed {
    pub id: String,
    pub name: String,
    pub from_version: String,
    pub version: String,
    pub asset: String,
    /// where the old files were copied, relative to the instance root (`.bunker/backups/unitytoolkit-…`)
    pub backup: String,
    pub built_for_before: String,
    /// the mod's client check after the update, rescanned
    pub status: String,
    pub built_for: String,
    pub still_bad: Vec<BadDll>,
}

pub fn backups_dir(root: &Path) -> PathBuf {
    root.join(".bunker").join("backups")
}

/// The old files put back where they were (an update that failed), its manifest with them.
fn put_back(root: &Path, backup: &Path, old: &manifest::Manifest) -> Res<()> {
    for r in &old.roots {
        let to = safe_join(root, r)?;
        let from = safe_join(backup, r)?;
        if from.exists() {
            remove_path(&to)?;
            copy_path(&from, &to)?;
        }
    }
    manifest::save(root, old)
}

/// Updates a mod to the build of its source made for this instance's SPT (its major and minor): the release
/// downloaded first, then the mod's files copied to `.bunker/backups/<id>-<time>/`, the old ones taken out, the new
/// installed with a manifest (profiles snapshotted, as every install), and the client check run again on it.
pub fn fix_update(inst: &Instance, id: &str) -> Res<Fixed> {
    let root = inst.root_path();
    let list = mods::list(inst);
    let v = list.iter().find(|m| m.id == id).cloned().ok_or_else(|| format!("没有找到 Mod：{id}"))?;
    if !v.enabled {
        return err(format!("{} 已禁用：先启用再更新", v.name));
    }
    if v.fix_url.is_empty() {
        return err(format!("{} 没有已知的更新来源：请到它的发布页下载适用于 SPT {} 的版本后拖入安装", v.name, major_minor(&inst.spt_version)));
    }
    let mm = major_minor(&inst.spt_version);
    if mm.is_empty() {
        return err("读不出这个实例的 SPT 版本，无法挑选兼容的发布");
    }
    let src = sources::source_for(&v.fix_url)?;
    let release = src.for_spt(&v.fix_url, &mm)?;
    let dl = install::staging(inst).join(format!("fix-{}", stamp_id(now_ms())));
    let file = match sources::download(&release, &dl) {
        Ok(f) => f,
        Err(e) => {
            let _ = remove_path(&dl);
            return Err(e);
        }
    };
    let old = mods::take(inst, id)?;
    let backup = backups_dir(&root).join(format!("{}-{}", old.id, stamp_id(now_ms())));
    for r in &old.roots {
        let from = safe_join(&root, r)?;
        if from.exists() {
            copy_path(&from, &safe_join(&backup, r)?)?;
        }
    }
    let result = (|| -> Res<manifest::Manifest> {
        mods::uninstall(inst, &old.id)?;
        let plan = install::plan(inst, &file, Source { kind: release.source.clone(), url: release.url.clone(), file: release.asset_name.clone() }, Some((old.name.clone(), release.version.clone())))?;
        match install::execute(inst, &plan.id, false) {
            Ok(done) => Ok(done.manifest),
            Err(e) => {
                let _ = install::cancel(inst, &plan.id);
                Err(e)
            }
        }
    })();
    let _ = remove_path(&dl);
    let man = match result {
        Ok(m) => m,
        Err(e) => {
            put_back(&root, &backup, &old).map_err(|e2| format!("{e}；还原旧文件也失败了：{e2}（备份在 {}）", rel(&root, &backup)))?;
            return err(format!("更新失败，已还原旧文件：{e}"));
        }
    };
    let now = mods::list(inst).into_iter().find(|m| m.id == man.id).unwrap_or_default();
    Ok(Fixed {
        id: man.id,
        name: man.name,
        from_version: v.version,
        version: man.version,
        asset: release.asset_name,
        backup: rel(&root, &backup),
        built_for_before: v.built_for,
        status: now.dll_status,
        built_for: now.built_for,
        still_bad: now.bad_dlls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clr_meta::tests::{unity_toolkit, Dll as Asm};
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, touch};
    use std::fs;

    #[test]
    fn versions_judged_by_major_and_minor() {
        let m = |v: [u16; 4]| clr_meta::parse(&unity_toolkit(v).bytes()).unwrap();
        assert_eq!(judge(&m([4, 0, 1, 0]), "4.1.6"), ("4.0.1".into(), "incompatible".into()));
        assert_eq!(judge(&m([4, 1, 5, 0]), "4.1.6"), ("4.1.5".into(), "compatible".into()));
        assert_eq!(judge(&m([4, 1, 6, 0]), "4.1.6"), ("4.1.6".into(), "compatible".into()));
        assert_eq!(judge(&m([4, 1, 5, 0]), ""), ("4.1.5".into(), "unknown".into()));
        let plain = clr_meta::parse(&Asm { name: "FOVFix".into(), refs: vec![("BepInEx".into(), [5, 4, 23, 0])], ..Default::default() }.bytes()).unwrap();
        assert_eq!(judge(&plain, "4.1.6").1, "unknown");
        let aki = clr_meta::parse(&Asm { name: "Old".into(), refs: vec![("aki-core".into(), [1, 0, 0, 0])], ..Default::default() }.bytes()).unwrap();
        assert_eq!(judge(&aki, "3.11.3").1, "incompatible");
        assert_eq!(judge(&aki, "3.8.0").1, "unknown");
        assert_eq!(major_minor("4.1.6"), "4.1");
        assert_eq!(shown("BepInEx/plugins/UnityToolkit/UnityToolkit.dll"), "UnityToolkit\\UnityToolkit.dll");
    }

    /// The real 4.1.6 install's shape: UnityToolkit (plugins and patchers) built for 4.0.1, ManimalIcebreaker
    /// (server and client) needing it by its plugin's GUID and BigBrain by assembly, ManimalInterchange needing it.
    pub fn manimal_instance(tag: &str, toolkit: [u16; 4]) -> PathBuf {
        let r = fake_instance(tag, "SPT_Runtime");
        touch(&r.join("SPT_Runtime/SPT_Data/configs/core.json"), br#"{"sptVersion":"4.1.6"}"#);
        let p = r.join("BepInEx/plugins");
        touch(&p.join("UnityToolkit/UnityToolkit.dll"), &unity_toolkit(toolkit).bytes());
        touch(&p.join("UnityToolkit/UniTask.dll"), &Asm { name: "UniTask".into(), ..Default::default() }.bytes());
        touch(&r.join("BepInEx/patchers/UnityToolkit/UnityToolkit-Prepatcher.dll"), &Asm { name: "UnityToolkit-Prepatcher".into(), refs: vec![("Mono.Cecil".into(), [0, 11, 4, 0])], ..Default::default() }.bytes());
        let ice = Asm {
            name: "ManimalIcebreakerClient".into(),
            version: [1, 2, 4, 0],
            refs: vec![("spt-reflection".into(), [4, 1, 6, 0]), ("DrakiaXYZ-BigBrain".into(), [1, 5, 0, 0])],
            plugin: Some(("com.manimal.icebreaker".into(), "Icebreaker".into(), "1.2.4".into())),
            deps: vec![("com.tylevo.unitytoolkit".into(), Some(1))],
            ..Default::default()
        };
        touch(&p.join("ManimalIcebreaker/ManimalIcebreakerClient.dll"), &ice.bytes());
        touch(&r.join("SPT_Runtime/user/mods/ManimalIcebreaker/ManimalIcebreaker.dll"), b"MZ");
        let inter = Asm { name: "ManimalInterchange".into(), refs: vec![("UnityToolkit".into(), [2, 0, 2, 0]), ("UniTask".into(), [2, 5, 0, 0])], ..Default::default() };
        touch(&p.join("ManimalInterchange/ManimalInterchange.dll"), &inter.bytes());
        let bb = Asm { name: "DrakiaXYZ-BigBrain".into(), refs: vec![("spt-reflection".into(), [4, 1, 6, 0])], plugin: Some(("xyz.drakia.bigbrain".into(), "BigBrain".into(), "1.5.0".into())), ..Default::default() };
        touch(&p.join("DrakiaXYZ-BigBrain.dll"), &bb.bytes());
        touch(&p.join("FOVFix.dll"), &Asm { name: "FOVFix".into(), refs: vec![("BepInEx".into(), [5, 4, 23, 0])], ..Default::default() }.bytes());
        // SPT's own: never judged
        touch(&p.join("spt/spt-core.dll"), &Asm { name: "spt-core".into(), refs: vec![("spt-reflection".into(), [4, 0, 1, 0])], ..Default::default() }.bytes());
        touch(&r.join("BepInEx/patchers/spt-prepatch.dll"), &Asm { name: "spt-prepatch".into(), refs: vec![("spt-reflection".into(), [4, 0, 1, 0])], ..Default::default() }.bytes());
        r
    }

    fn by_name<'a>(l: &'a [ModView], n: &str) -> &'a ModView {
        l.iter().find(|m| m.name == n).unwrap_or_else(|| panic!("no {n} in {:?}", l.iter().map(|m| &m.name).collect::<Vec<_>>()))
    }

    #[test]
    fn finds_the_outdated_toolkit_and_who_needs_it() {
        let r = manimal_instance("cc-find", [4, 0, 1, 0]);
        let inst = detect(&r);
        assert_eq!(inst.spt_version, "4.1.6");
        let l = mods::list(&inst);
        let utk = by_name(&l, "UnityToolkit");
        assert_eq!(utk.roots, vec!["BepInEx/plugins/UnityToolkit", "BepInEx/patchers/UnityToolkit"], "the patcher is the plugin's");
        assert_eq!(utk.dll_status, "incompatible");
        assert_eq!(utk.built_for, "4.0.1");
        assert_eq!(utk.bad_dlls, vec![BadDll { path: "BepInEx/plugins/UnityToolkit/UnityToolkit.dll".into(), shown: "UnityToolkit\\UnityToolkit.dll".into(), built_for: "4.0.1".into() }]);
        assert_eq!(utk.fix_url, "https://github.com/Tylevo/UnityToolkit-New");
        let needs: Vec<&str> = utk.required_by.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(needs, vec!["ManimalIcebreaker", "ManimalInterchange"]);
        let ice = by_name(&l, "ManimalIcebreaker");
        assert!(ice.server && ice.client);
        assert_eq!(ice.dll_status, "compatible");
        assert_eq!(ice.built_for, "4.1.6");
        let req: Vec<&str> = ice.requires.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(req, vec!["DrakiaXYZ-BigBrain", "UnityToolkit"]);
        assert_eq!(by_name(&l, "FOVFix").dll_status, "unknown");
        assert!(!l.iter().any(|m| m.name.eq_ignore_ascii_case("spt") || m.name.contains("prepatch")));
        let _ = fs::remove_dir_all(&r);
        // rebuilt for 4.1.5: compatible on 4.1.6
        let r = manimal_instance("cc-ok", [4, 1, 5, 0]);
        let l = mods::list(&detect(&r));
        assert_eq!(by_name(&l, "UnityToolkit").dll_status, "compatible");
        assert!(by_name(&l, "UnityToolkit").bad_dlls.is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    fn files(r: &Path) -> Vec<String> {
        walk_files(r).iter().map(|f| rel(r, f)).filter(|p| !p.starts_with(".bunker/")).collect()
    }

    #[test]
    fn turns_off_with_dependents_and_back_without_residue() {
        let r = manimal_instance("cc-off", [4, 0, 1, 0]);
        let inst = detect(&r);
        let before = files(&r);
        let utk = by_name(&mods::list(&inst), "UnityToolkit").id.clone();
        // only it: plugins and patchers both go
        let done = disable(&inst, &[utk.clone()], false).unwrap();
        assert_eq!(done.len(), 1);
        assert!(!r.join("BepInEx/plugins/UnityToolkit").exists());
        assert!(!r.join("BepInEx/patchers/UnityToolkit").exists());
        assert!(r.join("BepInEx/plugins/ManimalInterchange").exists());
        assert!(manifest::disabled_dir(&r, &done[0]).join("BepInEx/patchers/UnityToolkit/UnityToolkit-Prepatcher.dll").is_file());
        let l = mods::list(&inst);
        assert!(by_name(&l, "UnityToolkit").required_by.is_empty(), "what is off is needed by none: it is not checked");
        mods::set_enabled(&inst, &done[0], true).unwrap();
        assert_eq!(files(&r), before);
        // with those that need it
        let l = mods::list(&inst);
        let utk = by_name(&l, "UnityToolkit").id.clone();
        let done = disable(&inst, &[utk], true).unwrap();
        assert_eq!(done.len(), 3, "{done:?}");
        for gone in ["BepInEx/plugins/UnityToolkit", "BepInEx/patchers/UnityToolkit", "BepInEx/plugins/ManimalIcebreaker", "SPT_Runtime/user/mods/ManimalIcebreaker", "BepInEx/plugins/ManimalInterchange"] {
            assert!(!r.join(gone).exists(), "{gone}");
        }
        assert!(r.join("BepInEx/plugins/DrakiaXYZ-BigBrain.dll").exists());
        let l = mods::list(&inst);
        assert!(l.iter().filter(|m| !m.enabled).count() == 3);
        assert!(l.iter().all(|m| m.dll_status != "incompatible"));
        for id in &done {
            mods::set_enabled(&inst, id, true).unwrap();
        }
        assert_eq!(files(&r), before);
        assert!(!r.join(".bunker/disabled").exists());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn an_adopted_mod_takes_its_patcher_too() {
        // a manifest made before patchers were paired (plugins only): the patcher joins it when it is taken
        let r = manimal_instance("cc-adopt", [4, 0, 1, 0]);
        let inst = detect(&r);
        let files = manifest::files_under(&r, "BepInEx/plugins/UnityToolkit");
        let m = manifest::Manifest { id: "unitytoolkit".into(), name: "UnityToolkit".into(), source: Source { kind: "adopted".into(), ..Default::default() }, enabled: true, roots: vec!["BepInEx/plugins/UnityToolkit".into()], files, ..Default::default() };
        manifest::save(&r, &m).unwrap();
        let l = mods::list(&inst);
        assert_eq!(by_name(&l, "UnityToolkit").roots.len(), 2);
        assert!(!l.iter().any(|x| x.id != "unitytoolkit" && x.name == "UnityToolkit"));
        mods::set_enabled(&inst, "unitytoolkit", false).unwrap();
        assert!(!r.join("BepInEx/patchers/UnityToolkit").exists());
        assert_eq!(manifest::load(&r, "unitytoolkit").unwrap().roots.len(), 2);
        mods::set_enabled(&inst, "unitytoolkit", true).unwrap();
        assert!(r.join("BepInEx/patchers/UnityToolkit/UnityToolkit-Prepatcher.dll").is_file());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn picks_dependents_in_order() {
        let mk = |id: &str, req: &[&str]| ModView { id: id.into(), name: id.into(), enabled: true, required_by: req.iter().map(|x| ModRef { id: x.to_string(), name: x.to_string() }).collect(), ..Default::default() };
        let l = vec![mk("a", &["b", "c"]), mk("b", &["d"]), mk("c", &[]), mk("d", &[])];
        let o = with_dependents(&l, "a");
        assert_eq!(o.last().unwrap(), "a");
        assert_eq!(o.len(), 4);
        let pos = |x: &str| o.iter().position(|y| y == x).unwrap();
        assert!(pos("d") < pos("b"));
    }

    #[test]
    fn backup_put_back() {
        let r = manimal_instance("cc-back", [4, 0, 1, 0]);
        let inst = detect(&r);
        let id = by_name(&mods::list(&inst), "UnityToolkit").id.clone();
        let old = mods::take(&inst, &id).unwrap();
        let b = backups_dir(&r).join("t");
        for x in &old.roots {
            copy_path(&r.join(x), &b.join(x)).unwrap();
        }
        mods::uninstall(&inst, &old.id).unwrap();
        assert!(!r.join("BepInEx/patchers/UnityToolkit").exists());
        put_back(&r, &b, &old).unwrap();
        let l = mods::list(&inst);
        assert_eq!(by_name(&l, "UnityToolkit").built_for, "4.0.1");
        assert!(r.join("BepInEx/patchers/UnityToolkit/UnityToolkit-Prepatcher.dll").is_file());
        let _ = fs::remove_dir_all(&r);
    }

    /// A plain-HTTP stand-in for GitHub: UnityToolkit-New's releases, and its asset's download (what `zip` holds).
    fn fake_github(zip: std::sync::Arc<std::sync::Mutex<Vec<u8>>>) -> String {
        use std::io::{BufRead, BufReader, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
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
                let body: Vec<u8> = if path.starts_with("/repos/Tylevo/UnityToolkit-New/releases") {
                    serde_json::json!([{"tag_name": "v2.0.2", "name": "UnityToolkit 2.0.2 for SPT 4.1.5", "draft": false, "prerelease": false, "assets": [
                        {"name": "SHA256SUMS.txt", "browser_download_url": format!("{b2}/dl/sums")},
                        {"name": "UnityToolkit-v2.0.2-SPT4.1.5.zip", "browser_download_url": format!("{b2}/dl/utk.zip")}
                    ]}])
                    .to_string()
                    .into_bytes()
                } else if path == "/dl/utk.zip" {
                    zip.lock().unwrap().clone()
                } else {
                    let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes());
                let _ = s.write_all(&body);
            }
        });
        base
    }

    #[test]
    fn fix_update_backs_up_replaces_and_rechecks() {
        let r = manimal_instance("cc-fix", [4, 0, 1, 0]);
        touch(&r.join("BepInEx/plugins/UnityToolkit/Old-Only.txt"), b"from 4.0.1");
        let inst = detect(&r);
        // the release's zip as Tylevo packs it: `\` between its parts
        let z = r.join("../utk-fix.zip");
        let new_dll = unity_toolkit([4, 1, 5, 0]).bytes();
        let pre = Asm { name: "UnityToolkit-Prepatcher".into(), version: [2, 0, 2, 0], ..Default::default() }.bytes();
        let uni = Asm { name: "UniTask".into(), ..Default::default() }.bytes();
        crate::install::tests::make_zip(
            &z,
            &[
                ("BepInEx\\patchers\\UnityToolkit\\UnityToolkit-Prepatcher.dll", &pre),
                ("BepInEx\\plugins\\UnityToolkit\\UnityToolkit.dll", &new_dll),
                ("BepInEx\\plugins\\UnityToolkit\\UniTask.dll", &uni),
                ("BepInEx\\plugins\\UnityToolkit\\Assemblies.jsonc", b"{}"),
            ],
        );
        let zip = std::sync::Arc::new(std::sync::Mutex::new(fs::read(&z).unwrap()));
        let _ = fs::remove_file(&z);
        std::env::set_var("BUNKER_GITHUB_API", fake_github(zip.clone()));
        let id = by_name(&mods::list(&inst), "UnityToolkit").id.clone();

        // an archive with nothing to install: the old files are put back as they were
        *zip.lock().unwrap() = {
            let bad = r.join("../utk-bad.zip");
            crate::install::tests::make_zip(&bad, &[("README.md", b"nothing")]);
            let b = fs::read(&bad).unwrap();
            let _ = fs::remove_file(&bad);
            b
        };
        let before = files(&r);
        let e = fix_update(&inst, &id).unwrap_err();
        assert!(e.contains("已还原旧文件"), "{e}");
        assert_eq!(files(&r), before);
        let l = mods::list(&inst);
        let utk = by_name(&l, "UnityToolkit");
        assert_eq!(utk.built_for, "4.0.1");
        let id = utk.id.clone();

        *zip.lock().unwrap() = {
            let z = r.join("../utk-fix2.zip");
            crate::install::tests::make_zip(
                &z,
                &[
                    ("BepInEx\\patchers\\UnityToolkit\\UnityToolkit-Prepatcher.dll", &pre),
                    ("BepInEx\\plugins\\UnityToolkit\\UnityToolkit.dll", &new_dll),
                    ("BepInEx\\plugins\\UnityToolkit\\UniTask.dll", &uni),
                ],
            );
            let b = fs::read(&z).unwrap();
            let _ = fs::remove_file(&z);
            b
        };
        let f = fix_update(&inst, &id).unwrap();
        assert_eq!(f.built_for_before, "4.0.1");
        assert_eq!(f.status, "compatible", "{f:?}");
        assert_eq!(f.built_for, "4.1.5");
        assert!(f.still_bad.is_empty());
        assert_eq!(f.version, "2.0.2");
        assert_eq!(f.asset, "UnityToolkit-v2.0.2-SPT4.1.5.zip");
        assert!(f.backup.starts_with(".bunker/backups/unitytoolkit-"), "{}", f.backup);
        // the old files, plugins and patchers, kept in the backup
        let b = r.join(&f.backup);
        assert!(b.join("BepInEx/plugins/UnityToolkit/Old-Only.txt").is_file());
        assert!(b.join("BepInEx/patchers/UnityToolkit/UnityToolkit-Prepatcher.dll").is_file());
        let old = clr_meta::read(&b.join("BepInEx/plugins/UnityToolkit/UnityToolkit.dll")).unwrap();
        assert_eq!(judge(&old, "4.1.6").0, "4.0.1");
        // the new ones in place, the old-only file gone, a manifest naming its source
        assert!(!r.join("BepInEx/plugins/UnityToolkit/Old-Only.txt").exists());
        assert!(r.join("BepInEx/patchers/UnityToolkit/UnityToolkit-Prepatcher.dll").is_file());
        let m = manifest::load(&r, &f.id).unwrap();
        assert_eq!(m.source.kind, "github");
        assert_eq!(m.source.url, "https://github.com/Tylevo/UnityToolkit-New");
        assert_eq!(m.roots, vec!["BepInEx/patchers/UnityToolkit", "BepInEx/plugins/UnityToolkit"]);
        // rechecked: nothing outdated, and those that need it still do
        let l = mods::list(&inst);
        assert!(l.iter().all(|x| x.dll_status != "incompatible"));
        let utk = by_name(&l, "UnityToolkit");
        assert!(utk.managed);
        assert_eq!(utk.required_by.len(), 2);
        assert!(!r.join(".bunker/staging").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false), "nothing left in staging");
        let _ = fs::remove_dir_all(&r);
    }
}
