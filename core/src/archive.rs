//! Unpacking a mod's archive (`.zip`, `.7z`) into a folder, refusing entries that would land outside it.

use crate::util::*;
use std::fs;
use std::io::Read;
use std::path::Path;

pub fn kind_of(name: &str, head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        return Some("zip");
    }
    if head.starts_with(&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some("7z");
    }
    let n = name.to_ascii_lowercase();
    if n.ends_with(".zip") {
        Some("zip")
    } else if n.ends_with(".7z") {
        Some("7z")
    } else {
        None
    }
}

/// Unpacks `archive` into `dest` (made empty first); returns how many files it held.
pub fn extract(archive: &Path, dest: &Path) -> Res<usize> {
    let mut head = [0u8; 8];
    let n = fs::File::open(archive).and_then(|mut f| f.read(&mut head)).at("读取压缩包", archive)?;
    let kind = kind_of(&name_of(archive), &head[..n]).ok_or_else(|| format!("不支持的压缩格式：{}（支持 .zip / .7z）", name_of(archive)))?;
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest).at("创建临时目录", dest)?;
    match kind {
        "zip" => extract_zip(archive, dest),
        _ => extract_7z(archive, dest),
    }
}

fn entry_name_ok(name: &str) -> bool {
    !(name.starts_with('/') || name.starts_with('\\') || name.split(['/', '\\']).any(|p| p == ".." || p.contains(':')))
}

fn extract_zip(archive: &Path, dest: &Path) -> Res<usize> {
    let f = fs::File::open(archive).at("打开压缩包", archive)?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("zip 解析失败：{e}"))?;
    let mut count = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| format!("zip 解析失败：{e}"))?;
        let name = e.name().replace('\\', "/");
        if !entry_name_ok(&name) {
            return err(format!("压缩包含有越界路径，已拒绝：{name}"));
        }
        let out = safe_join(dest, &name)?;
        if e.is_dir() {
            fs::create_dir_all(&out).at("创建目录", &out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p).at("创建目录", p)?;
        }
        let mut w = fs::File::create(&out).at("写入文件", &out)?;
        std::io::copy(&mut e, &mut w).at("解压文件", &out)?;
        count += 1;
    }
    Ok(count)
}

fn extract_7z(archive: &Path, dest: &Path) -> Res<usize> {
    let mut bad: Option<String> = None;
    let mut count = 0;
    let r = sevenz_rust::decompress_file_with_extract_fn(archive, dest, |entry, reader, _default_dest| {
        let name = entry.name().replace('\\', "/");
        if !entry_name_ok(&name) {
            bad = Some(name);
            return Ok(false);
        }
        let out = match safe_join(dest, &name) {
            Ok(p) => p,
            Err(e) => {
                bad = Some(e);
                return Ok(false);
            }
        };
        if entry.is_directory() {
            fs::create_dir_all(&out)?;
            return Ok(true);
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p)?;
        }
        let mut w = fs::File::create(&out)?;
        std::io::copy(reader, &mut w)?;
        count += 1;
        Ok(true)
    });
    if let Some(b) = bad {
        return err(format!("压缩包含有越界路径，已拒绝：{b}"));
    }
    r.map_err(|e| format!("7z 解压失败：{e}"))?;
    Ok(count)
}
