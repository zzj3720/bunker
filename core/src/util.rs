//! Small helpers shared by the core: errors, time, paths, file walking and moving.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The core's error: a message for people (Chinese, shown by the UI as is).
pub type Res<T> = Result<T, String>;

pub fn err<T>(msg: impl Into<String>) -> Res<T> {
    Err(msg.into())
}

/// io errors said with the path they happened at.
pub trait IoCtx<T> {
    fn at(self, what: &str, p: &Path) -> Res<T>;
}

impl<T> IoCtx<T> for io::Result<T> {
    fn at(self, what: &str, p: &Path) -> Res<T> {
        self.map_err(|e| format!("{what}失败：{}（{e}）", p.display()))
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Minutes east of UTC of the local time zone now (0 where it cannot be known).
pub fn utc_offset_min() -> i64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
        let mut tz: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        let r = unsafe { GetTimeZoneInformation(&mut tz) };
        let mut bias = tz.Bias as i64;
        if r == 2 {
            bias += tz.DaylightBias as i64;
        } else if r == 1 {
            bias += tz.StandardBias as i64;
        }
        -bias
    }
    #[cfg(not(windows))]
    {
        std::env::var("BUNKER_UTC_OFFSET_MIN").ok().and_then(|s| s.parse().ok()).unwrap_or(0)
    }
}

/// `HH:MM:SS` of a moment, local time.
pub fn clock_text(ms: i64) -> String {
    let s = ms / 1000 + utc_offset_min() * 60;
    let d = s.rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", d / 3600, d / 60 % 60, d % 60)
}

/// `YYYYMMDD-HHMMSS-mmm` (UTC) of a moment: a sortable id.
pub fn stamp_id(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (y, mo, d) = civil_from_days(secs.div_euclid(86400));
    let t = secs.rem_euclid(86400);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}",
        y,
        mo,
        d,
        t / 3600,
        t / 60 % 60,
        t % 60,
        ms.rem_euclid(1000)
    )
}

// (Howard Hinnant's days-to-civil)
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A path relative to `base`, with `/` between its parts (how manifests and the UI speak of files).
pub fn rel(base: &Path, p: &Path) -> String {
    let r = p.strip_prefix(base).unwrap_or(p);
    r.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `base` joined with a `/`-separated relative path that may not climb out of it (`..`, roots, drive letters).
pub fn safe_join(base: &Path, relp: &str) -> Res<PathBuf> {
    let mut out = base.to_path_buf();
    for part in relp.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return err(format!("路径越界：{relp}")),
            p if p.contains(':') => return err(format!("路径越界：{relp}")),
            p => out.push(p),
        }
    }
    Ok(out)
}

/// Every file under `dir` (not directories), sorted, following no links.
pub fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// The files of `p`: itself if it is a file, those under it if a directory.
pub fn files_of(p: &Path) -> Vec<PathBuf> {
    if p.is_file() {
        vec![p.to_path_buf()]
    } else {
        walk_files(p)
    }
}

/// Entries of a directory, sorted by name (none where it cannot be read).
pub fn list_dir(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

pub fn name_of(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Moves a file or directory, making its new parent; across volumes by copying then removing.
pub fn move_path(from: &Path, to: &Path) -> Res<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).at("创建目录", parent)?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_path(from, to)?;
    remove_path(from)
}

pub fn copy_path(from: &Path, to: &Path) -> Res<()> {
    if from.is_dir() {
        fs::create_dir_all(to).at("创建目录", to)?;
        for f in walk_files(from) {
            let dst = to.join(f.strip_prefix(from).unwrap());
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).at("创建目录", parent)?;
            }
            fs::copy(&f, &dst).at("复制文件", &f)?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).at("创建目录", parent)?;
        }
        fs::copy(from, to).at("复制文件", from).map(|_| ())
    }
}

pub fn remove_path(p: &Path) -> Res<()> {
    if p.is_dir() {
        fs::remove_dir_all(p).at("删除目录", p)
    } else if p.exists() {
        fs::remove_file(p).at("删除文件", p)
    } else {
        Ok(())
    }
}

/// Removes the empty directories from `p` up to (not including) `stop`.
pub fn prune_empty(mut p: PathBuf, stop: &Path) {
    while p.starts_with(stop) && p != stop {
        if fs::remove_dir(&p).is_err() {
            break;
        }
        if !p.pop() {
            break;
        }
    }
}

/// Writes a file through a temporary beside it, so a reader never sees half of it.
pub fn write_atomic(p: &Path, data: &[u8]) -> Res<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).at("创建目录", parent)?;
    }
    let tmp = p.with_file_name(format!(".{}.bunker-tmp", name_of(p)));
    fs::write(&tmp, data).at("写入文件", &tmp)?;
    fs::rename(&tmp, p).at("写入文件", p)
}

pub fn read_json(p: &Path) -> Res<serde_json::Value> {
    let text = fs::read(p).at("读取文件", p)?;
    serde_json::from_slice(strip_bom(&text)).map_err(|e| format!("JSON 解析失败：{}（{e}）", p.display()))
}

pub fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

/// A name made into an id: lowercase letters, digits and `-`.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let t = out.trim_matches('-').to_string();
    if t.is_empty() {
        "mod".into()
    } else {
        t
    }
}

/// Compares dotted versions (`v1.2.10` > `1.2.9`); non-numeric parts compare as text.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<&str> = a.trim_start_matches(['v', 'V']).split(['.', '-', '+']).collect();
    let pb: Vec<&str> = b.trim_start_matches(['v', 'V']).split(['.', '-', '+']).collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or("0");
        let y = pb.get(i).copied().unwrap_or("0");
        let o = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(m), Ok(n)) => m.cmp(&n),
            _ => x.cmp(y),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    std::cmp::Ordering::Equal
}

/// The `1.2.3`-like version with the most parts in a text (an archive's or release's name), without a leading `v`.
pub fn version_in(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut best: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'v' || b[i - 1] == b'V') {
            let start = i;
            let mut dots = 0;
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || (b[j] == b'.' && j + 1 < b.len() && b[j + 1].is_ascii_digit())) {
                if b[j] == b'.' {
                    dots += 1;
                }
                j += 1;
            }
            let glued = j < b.len() && b[j].is_ascii_alphabetic();
            if dots >= 1 && !glued && best.map_or(true, |(bs, be)| s[bs..be].matches('.').count() < dots) {
                best = Some((start, j));
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    best.map(|(s0, e)| s[s0..e].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_and_versions() {
        assert_eq!(stamp_id(0), "19700101-000000-000");
        assert_eq!(stamp_id(1_795_160_361_838), "20261120-073921-838");
        assert_eq!(version_in("Tyfon-UIFixes-6.0.4.zip").as_deref(), Some("6.0.4"));
        assert_eq!(version_in("SAIN-v4.2.1-SPT4.1.7z").as_deref(), Some("4.2.1"));
        assert_eq!(version_in("SAIN v4.2.1").as_deref(), Some("4.2.1"));
        assert_eq!(version_in("nothing"), None);
        assert_eq!(version_cmp("v4.2.1", "4.2.0"), std::cmp::Ordering::Greater);
        assert_eq!(version_cmp("1.10", "1.9"), std::cmp::Ordering::Greater);
        assert_eq!(slug("Tyfon UIFixes!"), "tyfon-uifixes");
    }

    #[test]
    fn safe_join_refuses_climbing() {
        let b = Path::new("/base");
        assert!(safe_join(b, "a/../../x").is_err());
        assert!(safe_join(b, "C:/Windows").is_err());
        assert_eq!(safe_join(b, "a/b").unwrap(), Path::new("/base/a/b"));
    }
}
