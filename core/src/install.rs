//! Installing a mod from an archive in two steps: a plan (the archive unpacked into `.bunker/staging/<plan>/`, extra
//! top folders dropped, every entry mapped to where it goes in this instance, `SPT/user/mods` and
//! `SPT_Runtime/user/mods` mapped to this instance's own, conflicts named), then, once people confirm it, the
//! install: profiles snapshotted, files moved into place, a manifest written.

use crate::archive;
use crate::instance::Instance;
use crate::manifest::{self, Manifest, Source};
use crate::snapshot;
use crate::util::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mapping {
    /// In the archive (after its extra top folders), `/`-separated
    pub from: String,
    /// In the instance, relative to its root
    pub to: String,
    /// `server` or `client`
    pub side: String,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Conflict {
    pub path: String,
    /// The manifest that owns it, or `` (installed by hand)
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub id: String,
    pub instance: String,
    pub archive: String,
    pub name: String,
    pub version: String,
    pub mod_id: String,
    pub source: Source,
    /// The folder the mappings' `from` are in (the unpacked archive, its extra top folders gone)
    pub base: String,
    pub stripped: Vec<String>,
    pub mappings: Vec<Mapping>,
    /// Entries for another SPT layout mapped to this one's (`SPT/user/mods` → `SPT_Runtime/user/mods`)
    pub adapted: bool,
    /// The instance's layout said for people (`SPT 4.1`, `SPT 4.0`)
    pub target: String,
    pub conflicts: Vec<Conflict>,
    /// The managed mod this one replaces (an update), when its entries are the same
    pub replaces: String,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    pub files: usize,
}

const ROOT_NAMES: &[&str] = &["bepinex", "spt", "spt_runtime", "user", "plugins", "patchers", "config"];
const JUNK: &[&str] = &["__macosx", ".ds_store", "thumbs.db", "desktop.ini"];

fn is_junk(name: &str) -> bool {
    JUNK.contains(&name.to_ascii_lowercase().as_str())
}

fn is_doc(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".md") || n.ends_with(".txt") || n.ends_with(".pdf") || n.starts_with("license") || n.starts_with("readme") || n.ends_with(".url") || n.ends_with(".png") || n.ends_with(".jpg")
}

/// Which side a .NET assembly is for, by the assemblies it references: a BepInEx plugin, or an SPT server mod.
pub fn dll_side(p: &Path) -> Option<&'static str> {
    let b = fs::read(p).ok()?;
    let has = |s: &[u8]| b.windows(s.len()).any(|w| w == s);
    if has(b"SPTarkov.Server") || has(b"SPTarkov.DI") || has(b"SPTarkov.Common") {
        Some("server")
    } else if has(b"BepInEx") {
        Some("client")
    } else {
        None
    }
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    list_dir(dir).into_iter().filter(|p| !is_junk(&name_of(p))).collect()
}

fn side_of_dir(dir: &Path) -> Option<&'static str> {
    if dir.join("package.json").is_file() {
        return Some("server");
    }
    let mut server = 0;
    let mut client = 0;
    for f in walk_files(dir) {
        if name_of(&f).to_ascii_lowercase().ends_with(".dll") {
            match dll_side(&f) {
                Some("server") => server += 1,
                Some("client") => client += 1,
                _ => {}
            }
        }
    }
    if server > client {
        Some("server")
    } else if client > 0 {
        Some("client")
    } else {
        None
    }
}

fn layout_name(inst: &Instance) -> String {
    let v = inst.spt_version.split('.').take(2).collect::<Vec<_>>().join(".");
    match (inst.layout.as_str(), v.is_empty()) {
        (_, false) => format!("SPT {v}"),
        ("spt_runtime", true) => "SPT 4.1+".into(),
        ("spt", true) => "SPT 4.0".into(),
        _ => "SPT 3.x".into(),
    }
}

/// What the unpacked archive at `x` holds, mapped into `inst`. `stem` names the mod where its files do not.
pub fn analyze(inst: &Instance, x: &Path, stem: &str) -> (PathBuf, Vec<String>, Vec<Mapping>, bool, Vec<String>, Vec<String>) {
    let mut base = x.to_path_buf();
    let mut stripped = Vec::new();
    // drop wrapper folders: one folder alone (beside documents) that is not itself where things go
    loop {
        let es = entries(&base);
        let dirs: Vec<&PathBuf> = es.iter().filter(|p| p.is_dir()).collect();
        let loose: Vec<&PathBuf> = es.iter().filter(|p| p.is_file() && !is_doc(&name_of(p))).collect();
        if dirs.len() == 1 && loose.is_empty() {
            let d = dirs[0];
            let dn = name_of(d).to_ascii_lowercase();
            let inner = entries(d);
            let inner_has_root = inner.iter().any(|p| p.is_dir() && ROOT_NAMES.contains(&name_of(p).to_ascii_lowercase().as_str()));
            if !ROOT_NAMES.contains(&dn.as_str()) && inner_has_root {
                stripped.push(name_of(d));
                base = d.clone();
                continue;
            }
        }
        break;
    }
    let mods_rel = inst.mods_rel();
    let mut maps = Vec::new();
    let mut adapted = false;
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    let count = |p: &Path| files_of(p).len();
    let push = |maps: &mut Vec<Mapping>, from: &Path, to: String, side: &str| {
        maps.push(Mapping { from: rel(&base.clone(), from), to, side: side.into(), files: count(from) });
    };
    let es = entries(&base);
    for e in &es {
        let n = name_of(e);
        let ln = n.to_ascii_lowercase();
        if e.is_dir() {
            match ln.as_str() {
                "bepinex" => {
                    for sub in entries(e) {
                        let sn = name_of(&sub);
                        let lsn = sn.to_ascii_lowercase();
                        if ["plugins", "patchers", "config"].contains(&lsn.as_str()) && sub.is_dir() {
                            for item in entries(&sub) {
                                push(&mut maps, &item, format!("BepInEx/{lsn}/{}", name_of(&item)), "client");
                            }
                        } else {
                            skipped.push(format!("BepInEx/{sn}"));
                        }
                    }
                }
                "spt" | "spt_runtime" | "user" => {
                    let mods = if ln == "user" { e.join("mods") } else { e.join("user").join("mods") };
                    let from_rel = if ln == "user" { "user/mods".to_string() } else { format!("{n}/user/mods") };
                    if mods.is_dir() {
                        let theirs = if ln == "user" { "" } else if ln == "spt" { "SPT" } else { "SPT_Runtime" };
                        if theirs != inst.server_rel {
                            adapted = true;
                        }
                        for m in entries(&mods) {
                            push(&mut maps, &m, format!("{mods_rel}/{}", name_of(&m)), "server");
                        }
                    }
                    for other in entries(e) {
                        let on = name_of(&other).to_ascii_lowercase();
                        if on != "user" && on != "mods" {
                            skipped.push(format!("{n}/{}", name_of(&other)));
                        }
                    }
                    let _ = from_rel;
                }
                "plugins" | "patchers" | "config" => {
                    for item in entries(e) {
                        push(&mut maps, &item, format!("BepInEx/{ln}/{}", name_of(&item)), "client");
                    }
                }
                _ => match side_of_dir(e) {
                    Some("server") => push(&mut maps, e, format!("{mods_rel}/{n}"), "server"),
                    Some(_) => push(&mut maps, e, format!("BepInEx/plugins/{n}"), "client"),
                    None => {
                        skipped.push(n.clone());
                        warnings.push(format!("无法判断 {n}/ 属于服务端还是客户端，已跳过"));
                    }
                },
            }
        } else if ln.ends_with(".dll") {
            match dll_side(e) {
                Some("server") => push(&mut maps, e, format!("{mods_rel}/{}/{n}", slug(stem)), "server"),
                _ => push(&mut maps, e, format!("BepInEx/plugins/{n}"), "client"),
            }
        } else if ln == "package.json" {
            // a 3.x server mod packed without its folder: the whole archive is the mod
            maps.clear();
            push(&mut maps, &base.clone(), format!("{mods_rel}/{}", slug(stem)), "server");
            break;
        } else if !is_junk(&n) {
            skipped.push(n);
        }
    }
    (base, stripped, maps, adapted, skipped, warnings)
}

/// Name and version from an archive's file name: `Tyfon-UIFixes-6.0.4.zip` → (`Tyfon-UIFixes`, `6.0.4`).
pub fn name_version(file: &str) -> (String, String) {
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    let stem = stem.strip_suffix(".tar").unwrap_or(stem);
    match version_in(stem) {
        Some(v) => {
            let i = stem.rfind(&v).unwrap();
            let mut name = stem[..i].trim_end_matches(['-', '_', ' ', '.', 'v', 'V']).to_string();
            if name.is_empty() {
                name = stem.to_string();
            }
            (name, v)
        }
        None => (stem.to_string(), String::new()),
    }
}

pub fn staging(inst: &Instance) -> PathBuf {
    inst.bunker_dir().join("staging")
}

/// Unpacks `archive` and plans its install into `inst`.
pub fn plan(inst: &Instance, archive: &Path, source: Source, name_hint: Option<(String, String)>) -> Res<Plan> {
    if !inst.ok() {
        return err(format!("实例不完整：{}", inst.problems.join("；")));
    }
    let id = format!("p{}", stamp_id(now_ms()));
    let dir = staging(inst).join(&id);
    let x = dir.join("x");
    let n = archive::extract(archive, &x)?;
    if n == 0 {
        let _ = fs::remove_dir_all(&dir);
        return err("压缩包是空的");
    }
    let file = name_of(archive);
    let (name, version) = name_hint.unwrap_or_else(|| name_version(&file));
    let (base, stripped, mappings, adapted, skipped, warnings) = analyze(inst, &x, &name);
    if mappings.is_empty() {
        let _ = fs::remove_dir_all(&dir);
        return err(format!("没有在压缩包里找到可安装的内容（BepInEx/、SPT/user/mods/、user/mods/ 或 dll）{}", if skipped.is_empty() { String::new() } else { format!("，已跳过：{}", skipped.join("、")) }));
    }
    let root = inst.root_path();
    let manifests = manifest::load_all(&root);
    let mut conflicts = Vec::new();
    let mut replaces = String::new();
    for m in &mappings {
        let to = safe_join(&root, &m.to)?;
        let owner = manifests.iter().find(|mf| mf.roots.iter().any(|r| r.eq_ignore_ascii_case(&m.to)));
        match owner {
            Some(mf) if mf.source.kind != "adopted" => replaces = mf.id.clone(),
            Some(mf) => conflicts.push(Conflict { path: m.to.clone(), owner: mf.id.clone() }),
            None if to.exists() => conflicts.push(Conflict { path: m.to.clone(), owner: String::new() }),
            None => {
                // disabled managed mods keep their entries in the store: installing over them is an update too
                if let Some(mf) = manifests.iter().find(|mf| !mf.enabled && mf.roots.iter().any(|r| r.eq_ignore_ascii_case(&m.to))) {
                    replaces = mf.id.clone();
                }
            }
        }
    }
    let mod_id = if replaces.is_empty() { manifest::fresh_id(&root, &name) } else { replaces.clone() };
    let files = mappings.iter().map(|m| m.files).sum();
    let p = Plan {
        id: id.clone(),
        instance: inst.root.clone(),
        archive: file,
        name,
        version,
        mod_id,
        source,
        base: base.to_string_lossy().into_owned(),
        stripped,
        mappings,
        adapted,
        target: layout_name(inst),
        conflicts,
        replaces,
        skipped,
        warnings,
        files,
    };
    write_atomic(&dir.join("plan.json"), &serde_json::to_vec_pretty(&p).unwrap())?;
    Ok(p)
}

pub fn load_plan(inst: &Instance, id: &str) -> Res<Plan> {
    if id.contains(['/', '\\', '.']) {
        return err("安装计划编号无效");
    }
    let p = staging(inst).join(id).join("plan.json");
    serde_json::from_slice(&fs::read(&p).map_err(|_| "安装计划已失效，请重新选择压缩包".to_string())?).map_err(|e| e.to_string())
}

pub fn cancel(inst: &Instance, id: &str) -> Res<()> {
    if id.contains(['/', '\\', '.']) {
        return err("安装计划编号无效");
    }
    remove_path(&staging(inst).join(id))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Installed {
    pub manifest: Manifest,
    pub snapshot: String,
    /// Where entries installed by hand that were in the way were moved
    pub replaced_to: String,
}

/// Carries out a plan. Entries in the way that Bunker does not own need `overwrite` (they are moved aside to
/// `.bunker/replaced/<time>/`, not deleted); an update replaces the managed mod's files.
pub fn execute(inst: &Instance, plan_id: &str, overwrite: bool) -> Res<Installed> {
    let p = load_plan(inst, plan_id)?;
    if !p.conflicts.is_empty() && !overwrite {
        return err(format!("有 {} 处文件冲突：{}", p.conflicts.len(), p.conflicts.iter().map(|c| c.path.as_str()).collect::<Vec<_>>().join("、")));
    }
    let root = inst.root_path();
    let snap = snapshot::create(inst, &format!("安装 {} 之前", p.name), "before_install")?;
    if !p.replaces.is_empty() {
        crate::mods::uninstall(inst, &p.replaces)?;
    }
    let mut replaced_to = String::new();
    if !p.conflicts.is_empty() {
        let aside = inst.bunker_dir().join("replaced").join(stamp_id(now_ms()));
        for c in &p.conflicts {
            let from = safe_join(&root, &c.path)?;
            if from.exists() {
                move_path(&from, &safe_join(&aside, &c.path)?)?;
            }
            if !c.owner.is_empty() {
                manifest::delete(&root, &c.owner)?;
            }
        }
        replaced_to = rel(&root, &aside);
    }
    let base = PathBuf::from(&p.base);
    let mut files = Vec::new();
    let mut roots = Vec::new();
    for m in &p.mappings {
        let from = safe_join(&base, &m.from)?;
        let to = safe_join(&root, &m.to)?;
        let these = files_of(&from).into_iter().map(|f| match rel(&from, &f) {
            r if r.is_empty() => m.to.clone(),
            r => format!("{}/{r}", m.to),
        });
        files.extend(these);
        move_path(&from, &to)?;
        roots.push(m.to.clone());
    }
    let man = Manifest {
        id: p.mod_id.clone(),
        name: p.name.clone(),
        version: p.version.clone(),
        source: Source { file: p.archive.clone(), ..p.source.clone() },
        installed_at: now_ms(),
        enabled: true,
        roots,
        files,
    };
    manifest::save(&root, &man)?;
    let _ = remove_path(&staging(inst).join(&p.id));
    Ok(Installed { manifest: man, snapshot: snap.id, replaced_to })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, temp_dir, touch};
    use std::io::Write;

    pub fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let f = fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (n, data) in files {
            z.start_file(*n, opt).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    const CLIENT_DLL: &[u8] = b"MZ....BepInEx.BaseUnityPlugin....";
    const SERVER_DLL: &[u8] = b"MZ....SPTarkov.Server.Core....";

    #[test]
    fn names_from_archives() {
        assert_eq!(name_version("Tyfon-UIFixes-6.0.4.zip"), ("Tyfon-UIFixes".into(), "6.0.4".into()));
        assert_eq!(name_version("SAIN-v4.2.1.7z"), ("SAIN".into(), "4.2.1".into()));
        assert_eq!(name_version("Plain.zip"), ("Plain".into(), "".into()));
    }

    #[test]
    fn zip_wrapped_40_layout_into_41_with_manifest() {
        let r = fake_instance("inst41", "SPT_Runtime");
        let inst = detect(&r);
        let t = temp_dir("zips");
        let z = t.join("Tyfon-UIFixes-6.0.4.zip");
        make_zip(
            &z,
            &[
                ("UIFixes-6.0.4/BepInEx/plugins/Tyfon.UIFixes.dll", CLIENT_DLL),
                ("UIFixes-6.0.4/SPT/user/mods/UIFixes/UIFixes.dll", SERVER_DLL),
                ("UIFixes-6.0.4/SPT/user/mods/UIFixes/config/config.json", b"{}"),
                ("UIFixes-6.0.4/README.md", b"read me"),
            ],
        );
        let p = plan(&inst, &z, Source { kind: "archive".into(), ..Default::default() }, None).unwrap();
        assert_eq!(p.stripped, vec!["UIFixes-6.0.4".to_string()]);
        assert!(p.adapted, "SPT/user/mods mapped to SPT_Runtime/user/mods");
        assert_eq!(p.name, "Tyfon-UIFixes");
        assert_eq!(p.version, "6.0.4");
        let pairs: Vec<(String, String)> = p.mappings.iter().map(|m| (m.from.clone(), m.to.clone())).collect();
        assert_eq!(
            pairs,
            vec![
                ("BepInEx/plugins/Tyfon.UIFixes.dll".into(), "BepInEx/plugins/Tyfon.UIFixes.dll".into()),
                ("SPT/user/mods/UIFixes".into(), "SPT_Runtime/user/mods/UIFixes".into()),
            ]
        );
        assert!(p.conflicts.is_empty());
        assert_eq!(p.skipped, vec!["README.md".to_string()]);
        let done = execute(&inst, &p.id, false).unwrap();
        assert!(r.join("SPT_Runtime/user/mods/UIFixes/config/config.json").is_file());
        assert!(r.join("BepInEx/plugins/Tyfon.UIFixes.dll").is_file());
        let mut files = done.manifest.files.clone();
        files.sort();
        assert_eq!(
            files,
            vec![
                "BepInEx/plugins/Tyfon.UIFixes.dll".to_string(),
                "SPT_Runtime/user/mods/UIFixes/UIFixes.dll".into(),
                "SPT_Runtime/user/mods/UIFixes/config/config.json".into(),
            ]
        );
        assert_eq!(crate::snapshot::list(&inst)[0].kind, "before_install");
        assert!(!staging(&inst).join(&p.id).exists());
        let listed = crate::mods::list(&inst);
        let m = listed.iter().find(|m| m.id == done.manifest.id).unwrap();
        assert!(m.server && m.client && m.managed);
        // the same archive again: an update of that mod, not a conflict
        let p2 = plan(&inst, &z, Source { kind: "archive".into(), ..Default::default() }, None).unwrap();
        assert_eq!(p2.replaces, done.manifest.id);
        assert!(p2.conflicts.is_empty());
        execute(&inst, &p2.id, false).unwrap();
        assert_eq!(crate::manifest::load_all(&r).len(), 1);
        // and its uninstall leaves the instance as it was
        crate::mods::uninstall(&inst, &done.manifest.id).unwrap();
        assert!(!r.join("SPT_Runtime/user/mods/UIFixes").exists());
        assert!(!r.join("BepInEx/plugins/Tyfon.UIFixes.dll").exists());
        let _ = fs::remove_dir_all(&r);
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn loose_dlls_packages_and_conflicts() {
        let r = fake_instance("inst40", "SPT");
        let inst = detect(&r);
        let t = temp_dir("zips2");
        // loose dlls: sorted by what they reference
        let z = t.join("Mixed-1.2.zip");
        make_zip(&z, &[("Client.dll", CLIENT_DLL), ("ServerSide.dll", SERVER_DLL)]);
        let p = plan(&inst, &z, Source { kind: "archive".into(), ..Default::default() }, None).unwrap();
        let tos: Vec<&str> = p.mappings.iter().map(|m| m.to.as_str()).collect();
        assert_eq!(tos, vec!["BepInEx/plugins/Client.dll", "SPT/user/mods/mixed/ServerSide.dll"]);
        assert!(!p.adapted);
        cancel(&inst, &p.id).unwrap();
        // a 3.x mod with package.json at the top
        let z = t.join("cool-mod-2.0.0.zip");
        make_zip(&z, &[("package.json", br#"{"name":"cool"}"#), ("src/mod.js", b"x")]);
        let p = plan(&inst, &z, Source { kind: "archive".into(), ..Default::default() }, None).unwrap();
        assert_eq!(p.mappings.len(), 1);
        assert_eq!(p.mappings[0].to, "SPT/user/mods/cool-mod");
        assert_eq!(p.mappings[0].files, 2);
        cancel(&inst, &p.id).unwrap();
        // a folder installed by hand in the way: a conflict, moved aside only when people say so
        touch(&r.join("BepInEx/plugins/Thing/Thing.dll"), b"old");
        let z = t.join("Thing-1.0.zip");
        make_zip(&z, &[("plugins/Thing/Thing.dll", CLIENT_DLL)]);
        let p = plan(&inst, &z, Source { kind: "archive".into(), ..Default::default() }, None).unwrap();
        assert_eq!(p.conflicts, vec![Conflict { path: "BepInEx/plugins/Thing".into(), owner: String::new() }]);
        assert!(execute(&inst, &p.id, false).is_err());
        let done = execute(&inst, &p.id, true).unwrap();
        assert_eq!(fs::read(r.join("BepInEx/plugins/Thing/Thing.dll")).unwrap(), CLIENT_DLL);
        assert_eq!(fs::read(r.join(&done.replaced_to).join("BepInEx/plugins/Thing/Thing.dll")).unwrap(), b"old");
        let _ = fs::remove_dir_all(&r);
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn refuses_archives_climbing_out() {
        let r = fake_instance("slip", "SPT_Runtime");
        let inst = detect(&r);
        let t = temp_dir("zips3");
        let z = t.join("evil.zip");
        make_zip(&z, &[("../../evil.dll", CLIENT_DLL)]);
        let e = plan(&inst, &z, Source::default(), None).unwrap_err();
        assert!(e.contains("越界"), "{e}");
        assert!(!t.parent().unwrap().join("evil.dll").exists());
        let _ = fs::remove_dir_all(&r);
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn installs_from_7z() {
        let r = fake_instance("inst7z", "SPT_Runtime");
        let inst = detect(&r);
        let t = temp_dir("7z");
        let src = t.join("src");
        touch(&src.join("SAIN-4.2.1/BepInEx/plugins/SAIN/SAIN.dll"), CLIENT_DLL);
        touch(&src.join("SAIN-4.2.1/SPT_Runtime/user/mods/SAIN/SAIN.Server.dll"), SERVER_DLL);
        let z = t.join("SAIN-4.2.1.7z");
        sevenz_rust::compress_to_path(&src, &z).unwrap();
        let p = plan(&inst, &z, Source { kind: "archive".into(), ..Default::default() }, None).unwrap();
        assert!(!p.adapted);
        let tos: Vec<&str> = p.mappings.iter().map(|m| m.to.as_str()).collect();
        assert_eq!(tos, vec!["BepInEx/plugins/SAIN", "SPT_Runtime/user/mods/SAIN"]);
        execute(&inst, &p.id, false).unwrap();
        assert!(r.join("SPT_Runtime/user/mods/SAIN/SAIN.Server.dll").is_file());
        let _ = fs::remove_dir_all(&r);
        let _ = fs::remove_dir_all(&t);
    }
}
