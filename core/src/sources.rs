//! Where mods come from besides a file people have: a release page's link. GitHub Releases are read through
//! GitHub's API (the latest release, or the one a link names), its `.zip`/`.7z` asset downloaded for the install's
//! plan, and installed mods from GitHub checked for newer releases. Other sites plug in as another `ModSource`
//! (sp-mod's is named and refused until it has an API to read).

use crate::util::*;
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Release {
    pub source: String,
    /// The link that names the mod (its repository), kept in its manifest for update checks
    pub url: String,
    pub repo: String,
    pub tag: String,
    pub version: String,
    pub asset_name: String,
    pub asset_url: String,
}

pub trait ModSource: Send + Sync {
    fn id(&self) -> &'static str;
    fn handles(&self, url: &str) -> bool;
    /// The release a link names (or its latest).
    fn release(&self, url: &str) -> Res<Release>;
    /// The newest release of the mod a manifest's link names.
    fn latest(&self, url: &str) -> Res<Release>;
    /// The newest release built for an SPT of this `major.minor` (`4.1`), where releases say which SPT they are for.
    fn for_spt(&self, url: &str, _major_minor: &str) -> Res<Release> {
        self.latest(url)
    }
}

/// The SPT versions a text says it is for (`UnityToolkit-v2.0.2-SPT4.1.5.zip`, `for SPT 4.1.x`, `SPT-v4.0`), as
/// `major.minor`.
pub fn spt_in(text: &str) -> Vec<String> {
    let l = text.to_ascii_lowercase();
    let b = l.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = l[from..].find("spt").map(|i| i + from) {
        let mut j = i + 3;
        while j < b.len() && matches!(b[j], b' ' | b'-' | b'_' | b'v' | b'.') && j < i + 6 {
            j += 1;
        }
        let start = j;
        while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
            j += 1;
        }
        let mm = crate::clientcheck::major_minor(l[start..j].trim_end_matches('.'));
        if !mm.is_empty() && !out.contains(&mm) {
            out.push(mm);
        }
        from = i + 3;
    }
    out
}

pub struct GitHub {
    pub api: String,
}

impl Default for GitHub {
    fn default() -> Self {
        GitHub { api: std::env::var("BUNKER_GITHUB_API").unwrap_or_else(|_| "https://api.github.com".into()) }
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).user_agent("Bunker (SPT launcher; github.com/zzj3720/bunker)").build()
}

/// `owner/repo`, and what the link names in it: a tag, or an asset's download.
fn parse_github(url: &str) -> Option<(String, String, Option<String>, Option<String>)> {
    let u = url.trim().trim_end_matches('/');
    let rest = u.strip_prefix("https://github.com/").or_else(|| u.strip_prefix("http://github.com/")).or_else(|| u.strip_prefix("github.com/"))?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return None;
    }
    let (o, r) = (parts[0].to_string(), parts[1].trim_end_matches(".git").to_string());
    let tag = match parts.get(2..) {
        Some(["releases", "tag", t, ..]) => Some(t.to_string()),
        Some(["releases", "download", t, _asset]) => Some(t.to_string()),
        _ => None,
    };
    let asset = match parts.get(2..) {
        Some(["releases", "download", _t, a]) => Some(a.to_string()),
        _ => None,
    };
    Some((o, r, tag, asset))
}

fn pick_asset(rel: &Value, want: Option<&str>) -> Option<(String, String)> {
    let assets = rel.get("assets")?.as_array()?;
    let named = |a: &Value| -> Option<(String, String)> {
        Some((a.get("name")?.as_str()?.to_string(), a.get("browser_download_url")?.as_str()?.to_string()))
    };
    let all: Vec<(String, String)> = assets.iter().filter_map(named).collect();
    if let Some(w) = want {
        return all.into_iter().find(|(n, _)| n == w);
    }
    let ok = |n: &str| {
        let l = n.to_ascii_lowercase();
        l.ends_with(".zip") || l.ends_with(".7z")
    };
    all.iter().find(|(n, _)| ok(n) && !n.to_ascii_lowercase().contains("source")).or_else(|| all.iter().find(|(n, _)| ok(n))).cloned()
}

impl GitHub {
    fn get_release(&self, o: &str, r: &str, tag: Option<&str>) -> Res<Value> {
        let path = match tag {
            Some(t) => format!("{}/repos/{o}/{r}/releases/tags/{t}", self.api),
            None => format!("{}/repos/{o}/{r}/releases/latest", self.api),
        };
        let resp = agent().get(&path).set("Accept", "application/vnd.github+json").call().map_err(|e| match e {
            ureq::Error::Status(404, _) => format!("GitHub 上没有找到 {o}/{r} 的发布（Release）"),
            ureq::Error::Status(403, _) => "GitHub API 请求次数已用完，请稍后再试".to_string(),
            e => format!("访问 GitHub 失败：{e}"),
        })?;
        resp.into_json().map_err(|e| format!("GitHub 返回无法解析：{e}"))
    }

    fn to_release(&self, o: &str, r: &str, v: &Value, asset: Option<&str>) -> Res<Release> {
        let tag = v.get("tag_name").and_then(|t| t.as_str()).unwrap_or("").to_string();
        let name = v.get("name").and_then(|t| t.as_str()).unwrap_or("");
        let (asset_name, asset_url) = pick_asset(v, asset).ok_or_else(|| format!("{o}/{r} 的发布 {tag} 里没有 .zip / .7z 附件"))?;
        let version = version_in(&tag).or_else(|| version_in(name)).unwrap_or_else(|| tag.trim_start_matches(['v', 'V']).to_string());
        Ok(Release { source: "github".into(), url: format!("https://github.com/{o}/{r}"), repo: format!("{o}/{r}"), tag, version, asset_name, asset_url })
    }
}

impl GitHub {
    fn releases(&self, o: &str, r: &str) -> Res<Vec<Value>> {
        let path = format!("{}/repos/{o}/{r}/releases?per_page=30", self.api);
        let resp = agent().get(&path).set("Accept", "application/vnd.github+json").call().map_err(|e| match e {
            ureq::Error::Status(404, _) => format!("GitHub 上没有找到 {o}/{r}"),
            ureq::Error::Status(403, _) => "GitHub API 请求次数已用完，请稍后再试".to_string(),
            e => format!("访问 GitHub 失败：{e}"),
        })?;
        let v: Value = resp.into_json().map_err(|e| format!("GitHub 返回无法解析：{e}"))?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }
}

/// Of releases (newest first, as GitHub lists them), the newest for SPT `mm`: an asset that names it, else a release
/// whose tag or title does; when none names any SPT, the newest. Drafts are skipped, pre-releases taken only when no
/// full release is for it.
pub fn pick_for_spt(rels: &[Value], mm: &str) -> Result<(usize, Option<String>), String> {
    let s = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let ok_asset = |n: &str| {
        let l = n.to_ascii_lowercase();
        l.ends_with(".zip") || l.ends_with(".7z")
    };
    let mut named_any = Vec::new();
    for pre in [false, true] {
        for (i, r) in rels.iter().enumerate() {
            if r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) || r.get("prerelease").and_then(|d| d.as_bool()).unwrap_or(false) != pre {
                continue;
            }
            let assets: Vec<String> = r.get("assets").and_then(|a| a.as_array()).map(|a| a.iter().map(|x| s(x, "name")).filter(|n| ok_asset(n)).collect()).unwrap_or_default();
            if assets.is_empty() {
                continue;
            }
            for a in &assets {
                let says = spt_in(a);
                named_any.extend(says.clone());
                if says.iter().any(|x| x == mm) {
                    return Ok((i, Some(a.clone())));
                }
            }
            let says: Vec<String> = [spt_in(&s(r, "tag_name")), spt_in(&s(r, "name"))].concat();
            named_any.extend(says.clone());
            if says.iter().any(|x| x == mm) {
                return Ok((i, None));
            }
        }
    }
    if named_any.is_empty() {
        if let Some(i) = rels.iter().position(|r| !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) && !r.get("prerelease").and_then(|d| d.as_bool()).unwrap_or(false)) {
            return Ok((i, None));
        }
    }
    named_any.sort();
    named_any.dedup();
    Err(format!("没有适用于 SPT {mm} 的发布{}", if named_any.is_empty() { String::new() } else { format!("（现有的发布适用于 SPT {}）", named_any.join("、")) }))
}

impl ModSource for GitHub {
    fn id(&self) -> &'static str {
        "github"
    }
    fn handles(&self, url: &str) -> bool {
        parse_github(url).is_some()
    }
    fn release(&self, url: &str) -> Res<Release> {
        let (o, r, tag, asset) = parse_github(url).ok_or("不是 GitHub 仓库链接")?;
        let v = self.get_release(&o, &r, tag.as_deref())?;
        self.to_release(&o, &r, &v, asset.as_deref())
    }
    fn latest(&self, url: &str) -> Res<Release> {
        let (o, r, _, _) = parse_github(url).ok_or("不是 GitHub 仓库链接")?;
        let v = self.get_release(&o, &r, None)?;
        self.to_release(&o, &r, &v, None)
    }
    fn for_spt(&self, url: &str, mm: &str) -> Res<Release> {
        let (o, r, _, _) = parse_github(url).ok_or("不是 GitHub 仓库链接")?;
        let rels = self.releases(&o, &r)?;
        let (i, asset) = pick_for_spt(&rels, mm).map_err(|e| format!("{o}/{r}：{e}"))?;
        self.to_release(&o, &r, &rels[i], asset.as_deref())
    }
}

/// SPT's mod site: recognised, not readable yet (no public API to list a mod's files).
pub struct SpMod;

impl ModSource for SpMod {
    fn id(&self) -> &'static str {
        "sp-mod"
    }
    fn handles(&self, url: &str) -> bool {
        let l = url.to_ascii_lowercase();
        l.contains("sp-mod.com") || l.contains("forge.sp-tarkov.com") || l.contains("hub.sp-tarkov.com")
    }
    fn release(&self, _url: &str) -> Res<Release> {
        err("暂不支持直接从 SPT Mod 站点下载：请在浏览器下载压缩包后拖入，或粘贴该 Mod 的 GitHub Releases 链接")
    }
    fn latest(&self, url: &str) -> Res<Release> {
        self.release(url)
    }
}

pub fn sources() -> Vec<Box<dyn ModSource>> {
    vec![Box::new(GitHub::default()), Box::new(SpMod)]
}

pub fn source_for(url: &str) -> Res<Box<dyn ModSource>> {
    sources().into_iter().find(|s| s.handles(url)).ok_or_else(|| "不认识的链接：支持 GitHub Releases（github.com/作者/仓库）".to_string())
}

/// Downloads a release's asset into `dir`; its path.
pub fn download(rel: &Release, dir: &Path) -> Res<PathBuf> {
    std::fs::create_dir_all(dir).at("创建下载目录", dir)?;
    let name: String = rel.asset_name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect();
    let to = dir.join(name);
    let resp = agent().get(&rel.asset_url).call().map_err(|e| format!("下载 {} 失败：{e}", rel.asset_name))?;
    let mut data = Vec::new();
    resp.into_reader().take(2 << 30).read_to_end(&mut data).map_err(|e| format!("下载 {} 失败：{e}", rel.asset_name))?;
    std::fs::write(&to, &data).at("保存下载", &to)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_links() {
        assert_eq!(parse_github("https://github.com/Solarint/SAIN"), Some(("Solarint".into(), "SAIN".into(), None, None)));
        assert_eq!(parse_github("https://github.com/Solarint/SAIN/releases/latest"), Some(("Solarint".into(), "SAIN".into(), None, None)));
        assert_eq!(parse_github("https://github.com/a/b/releases/tag/v4.2.1"), Some(("a".into(), "b".into(), Some("v4.2.1".into()), None)));
        assert_eq!(
            parse_github("https://github.com/a/b/releases/download/v1/b-1.zip"),
            Some(("a".into(), "b".into(), Some("v1".into()), Some("b-1.zip".into())))
        );
        assert_eq!(parse_github("https://example.com/a/b"), None);
        assert!(source_for("https://sp-mod.com/mods/1").unwrap().release("x").is_err());
        assert!(source_for("https://example.com").is_err());
    }

    #[test]
    fn releases_for_an_spt() {
        assert_eq!(spt_in("UnityToolkit-v2.0.2-SPT4.1.5.zip"), vec!["4.1"]);
        assert_eq!(spt_in("UnityToolkit 2.0.2 for SPT 4.1.5"), vec!["4.1"]);
        assert_eq!(spt_in("SAIN-SPT-v4.0.x.7z"), vec!["4.0"]);
        assert!(spt_in("SAIN 4.2.1").is_empty());
        let rel = |tag: &str, asset: &str, pre: bool| serde_json::json!({"tag_name": tag, "name": tag, "prerelease": pre, "draft": false, "assets": [{"name": asset, "browser_download_url": format!("https://x/{asset}")}, {"name": "SHA256SUMS.txt"}]});
        let list = vec![
            rel("v3.0.0", "UnityToolkit-v3.0.0-SPT4.2.0.zip", false),
            rel("v2.0.2", "UnityToolkit-v2.0.2-SPT4.1.5.zip", false),
            rel("v1.0.0", "UnityToolkit-v1.0.0-SPT4.0.1.zip", false),
        ];
        assert_eq!(pick_for_spt(&list, "4.1").unwrap(), (1, Some("UnityToolkit-v2.0.2-SPT4.1.5.zip".into())));
        assert_eq!(pick_for_spt(&list, "4.0").unwrap().0, 2);
        let e = pick_for_spt(&list, "3.11").unwrap_err();
        assert!(e.contains("4.0、4.1、4.2"), "{e}");
        // none says its SPT: the newest full release
        let plain = vec![rel("v2", "a-2.zip", true), rel("v1", "a-1.zip", false)];
        assert_eq!(pick_for_spt(&plain, "4.1").unwrap(), (1, None));
    }
}
