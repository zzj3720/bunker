//! A Windows program's or library's version, read from its version resource (VS_VERSIONINFO) on any OS:
//! the PE headers, the resource directory down to RT_VERSION, then its fixed numbers and its strings.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeVersion {
    /// VS_FIXEDFILEINFO's file version, `a.b.c.d`.
    pub file: String,
    /// The `ProductVersion` string (`4.1.6-RELEASE+731d7a2…`, `0.16.9.5-40743-f137e819`), else the file version.
    pub product: String,
    /// The `FileVersion` string, else the fixed one.
    pub file_text: String,
}

impl PeVersion {
    /// The version people know it by: the product string up to its build metadata (`4.1.6`, `0.16.9.5`).
    pub fn short(&self) -> String {
        let p = if self.product.is_empty() { &self.file } else { &self.product };
        let cut = p.find(['+', '-', ' ']).map(|i| &p[..i]).unwrap_or(p);
        cut.to_string()
    }
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_at(f: &mut File, off: u64, len: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut v = vec![0u8; len];
    f.read_exact(&mut v).ok()?;
    Some(v)
}

pub fn read(path: &Path) -> Option<PeVersion> {
    let mut f = File::open(path).ok()?;
    let dos = read_at(&mut f, 0, 64)?;
    if &dos[0..2] != b"MZ" {
        return None;
    }
    let pe = u32_at(&dos, 0x3c)? as u64;
    let hdr = read_at(&mut f, pe, 24)?;
    if &hdr[0..4] != b"PE\0\0" {
        return None;
    }
    let nsec = u16_at(&hdr, 6)? as usize;
    let opt_size = u16_at(&hdr, 20)? as usize;
    let opt = read_at(&mut f, pe + 24, opt_size)?;
    let magic = u16_at(&opt, 0)?;
    let dd = match magic {
        0x10b => 96,
        0x20b => 112,
        _ => return None,
    };
    // data directory 2: resources
    let rsrc_rva = u32_at(&opt, dd + 2 * 8)?;
    if rsrc_rva == 0 {
        return None;
    }
    let secs = read_at(&mut f, pe + 24 + opt_size as u64, nsec * 40)?;
    let mut rsrc_sec = None;
    for i in 0..nsec {
        let s = &secs[i * 40..i * 40 + 40];
        let vsize = u32_at(s, 8)?;
        let va = u32_at(s, 12)?;
        let raw_size = u32_at(s, 16)?;
        let raw = u32_at(s, 20)?;
        if rsrc_rva >= va && rsrc_rva < va + vsize.max(raw_size) {
            rsrc_sec = Some((va, raw, raw_size));
        }
    }
    let (va, raw, raw_size) = rsrc_sec?;
    let base_off = raw as u64 + (rsrc_rva - va) as u64;
    let avail = (raw_size - (rsrc_rva - va)) as usize;
    let rs = read_at(&mut f, base_off, avail.min(64 << 20))?;
    // RT_VERSION (16), then its first name, then its first language
    let lvl1 = find_entry(&rs, 0, Some(16))?;
    let lvl2 = find_entry(&rs, lvl1, None)?;
    let leaf = find_entry_leaf(&rs, lvl2)?;
    let data_rva = u32_at(&rs, leaf)?;
    let data_size = u32_at(&rs, leaf + 4)? as usize;
    let start = (data_rva - rsrc_rva) as usize;
    let vi = rs.get(start..start + data_size)?;
    Some(parse_version_info(vi))
}

/// A directory's entry (by id, or its first): the offset of the subdirectory it points to.
fn find_entry(rs: &[u8], dir: usize, id: Option<u32>) -> Option<usize> {
    let named = u16_at(rs, dir + 12)? as usize;
    let ids = u16_at(rs, dir + 14)? as usize;
    for i in 0..named + ids {
        let e = dir + 16 + i * 8;
        let name = u32_at(rs, e)?;
        let off = u32_at(rs, e + 4)?;
        if id.map_or(true, |want| name == want) && off & 0x8000_0000 != 0 {
            return Some((off & 0x7fff_ffff) as usize);
        }
    }
    None
}

fn find_entry_leaf(rs: &[u8], dir: usize) -> Option<usize> {
    let named = u16_at(rs, dir + 12)? as usize;
    let ids = u16_at(rs, dir + 14)? as usize;
    if named + ids == 0 {
        return None;
    }
    let off = u32_at(rs, dir + 16 + 4)?;
    if off & 0x8000_0000 != 0 {
        return None;
    }
    Some(off as usize)
}

fn utf16_le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

/// A string value of the StringFileInfo: the key's UTF-16 text and its NUL, padding to 4, then the value.
fn string_value(vi: &[u8], key: &str) -> Option<String> {
    let mut pat = utf16_le(key);
    pat.extend_from_slice(&[0, 0]);
    let pos = vi.windows(pat.len()).position(|w| w == pat.as_slice())?;
    // the String's header (wLength, wValueLength, wType) is the 6 bytes before its key
    if pos < 6 {
        return None;
    }
    let value_len = u16_at(vi, pos - 4)? as usize;
    let mut v = pos + pat.len();
    v = (v + 3) & !3;
    let mut units = Vec::new();
    for i in 0..value_len {
        let u = u16_at(vi, v + i * 2)?;
        if u == 0 {
            break;
        }
        units.push(u);
    }
    let s = String::from_utf16_lossy(&units).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn parse_version_info(vi: &[u8]) -> PeVersion {
    let mut out = PeVersion::default();
    let sig = 0xFEEF04BDu32.to_le_bytes();
    if let Some(p) = vi.windows(4).position(|w| w == sig) {
        if let (Some(ms), Some(ls)) = (u32_at(vi, p + 8), u32_at(vi, p + 12)) {
            out.file = format!("{}.{}.{}.{}", ms >> 16, ms & 0xffff, ls >> 16, ls & 0xffff);
        }
    }
    out.product = string_value(vi, "ProductVersion").unwrap_or_else(|| out.file.clone());
    out.file_text = string_value(vi, "FileVersion").unwrap_or_else(|| out.file.clone());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_versions() {
        let v = PeVersion { file: "4.1.6.0".into(), product: "4.1.6-RELEASE+731d7a2".into(), file_text: "4.1.6".into() };
        assert_eq!(v.short(), "4.1.6");
        let e = PeVersion { file: "0.16.9.40743".into(), product: "0.16.9.5-40743-f137e819".into(), file_text: String::new() };
        assert_eq!(e.short(), "0.16.9.5");
    }

    #[test]
    fn not_a_program() {
        let p = std::env::temp_dir().join(format!("bunker-pever-{}.txt", std::process::id()));
        std::fs::write(&p, b"hello").unwrap();
        assert_eq!(read(&p), None);
        let _ = std::fs::remove_file(&p);
    }

    #[cfg(windows)]
    #[test]
    fn reads_a_system_dll() {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let v = read(&Path::new(&root).join("System32").join("kernel32.dll")).expect("kernel32 has a version");
        assert!(v.file.starts_with("10.0."), "{v:?}");
        assert!(!v.product.is_empty());
    }
}
