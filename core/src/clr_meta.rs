//! A .NET assembly's own metadata (ECMA-335, partition II), read on any OS without a runtime: the PE's CLI header,
//! the metadata root and its streams, and from the `#~` tables the assembly's name and version (`Assembly`, 0x20),
//! the assemblies it references with their versions (`AssemblyRef`, 0x23), and the BepInEx attributes on its types
//! (`[BepInPlugin(guid, name, version)]`, `[BepInDependency(guid, …)]`), found through `CustomAttribute` (0x0C) →
//! `MemberRef` (0x0A) → `TypeRef` (0x01).

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AsmRef {
    pub name: String,
    /// `major.minor.build.revision`
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plugin {
    pub guid: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dependency {
    pub guid: String,
    /// `[BepInDependency(guid, DependencyFlags.SoftDependency)]`: loads without it
    pub soft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Meta {
    pub name: String,
    pub version: String,
    pub refs: Vec<AsmRef>,
    pub plugins: Vec<Plugin>,
    pub deps: Vec<Dependency>,
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o.checked_add(2)?).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o.checked_add(4)?).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// A .NET assembly's metadata from its file; None for a native library, or anything else.
pub fn read(path: &Path) -> Option<Meta> {
    let len = std::fs::metadata(path).ok()?.len();
    if len > 256 << 20 {
        return None;
    }
    parse(&std::fs::read(path).ok()?)
}

/// The file offset of a relative virtual address, through the section table.
fn rva_to_off(secs: &[(u32, u32, u32, u32)], rva: u32) -> Option<usize> {
    for &(va, vsize, raw, raw_size) in secs {
        if rva >= va && rva < va.checked_add(vsize.max(raw_size))? {
            return Some((raw + (rva - va)) as usize);
        }
    }
    None
}

pub fn parse(b: &[u8]) -> Option<Meta> {
    if b.get(0..2)? != b"MZ" {
        return None;
    }
    let pe = u32_at(b, 0x3c)? as usize;
    if b.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let nsec = u16_at(b, pe + 6)? as usize;
    let opt_size = u16_at(b, pe + 20)? as usize;
    let opt = pe + 24;
    let dd = match u16_at(b, opt)? {
        0x10b => opt + 96,
        0x20b => opt + 112,
        _ => return None,
    };
    // data directory 14: the CLI header (COM descriptor)
    let cli_rva = u32_at(b, dd + 14 * 8)?;
    if cli_rva == 0 {
        return None;
    }
    let st = opt + opt_size;
    let mut secs = Vec::with_capacity(nsec);
    for i in 0..nsec {
        let s = st + i * 40;
        secs.push((u32_at(b, s + 12)?, u32_at(b, s + 8)?, u32_at(b, s + 20)?, u32_at(b, s + 16)?));
    }
    let cli = rva_to_off(&secs, cli_rva)?;
    let md = rva_to_off(&secs, u32_at(b, cli + 8)?)?;
    let md_size = u32_at(b, cli + 12)? as usize;
    let root = b.get(md..md.checked_add(md_size)?)?;
    parse_metadata(root)
}

struct Streams<'a> {
    tables: &'a [u8],
    strings: &'a [u8],
    blob: &'a [u8],
}

fn streams(root: &[u8]) -> Option<Streams<'_>> {
    if u32_at(root, 0)? != 0x424A_5342 {
        return None;
    }
    let vlen = u32_at(root, 12)? as usize;
    let mut o = 16 + vlen;
    let n = u16_at(root, o + 2)? as usize;
    o += 4;
    let (mut tables, mut strings, mut blob): (&[u8], &[u8], &[u8]) = (&[], &[], &[]);
    for _ in 0..n {
        let off = u32_at(root, o)? as usize;
        let size = u32_at(root, o + 4)? as usize;
        let name_at = o + 8;
        let end = root.get(name_at..)?.iter().position(|&c| c == 0)?;
        let name = &root[name_at..name_at + end];
        o = name_at + ((end + 4) & !3);
        let data = root.get(off..off.checked_add(size)?)?;
        match name {
            b"#~" | b"#-" => tables = data,
            b"#Strings" => strings = data,
            b"#Blob" => blob = data,
            _ => {}
        }
    }
    (!tables.is_empty()).then_some(Streams { tables, strings, blob })
}

/// A column of a table's row.
#[derive(Clone, Copy)]
enum Col {
    U16,
    U32,
    Str,
    Guid,
    Blob,
    /// an index into one table
    T(usize),
    /// a coded index: its tag's bits and the tables it can name
    C(u32, &'static [usize]),
}

use Col::*;

const TYPE_DEF_OR_REF: Col = C(2, &[0x02, 0x01, 0x1B]);
const HAS_CONSTANT: Col = C(2, &[0x04, 0x08, 0x17]);
const HAS_CUSTOM_ATTRIBUTE: Col = C(5, &[0x06, 0x04, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x00, 0x0E, 0x17, 0x14, 0x11, 0x1A, 0x1B, 0x20, 0x23, 0x26, 0x27, 0x28, 0x2A, 0x2C, 0x2B]);
const HAS_FIELD_MARSHAL: Col = C(1, &[0x04, 0x08]);
const HAS_DECL_SECURITY: Col = C(2, &[0x02, 0x06, 0x20]);
const MEMBER_REF_PARENT: Col = C(3, &[0x02, 0x01, 0x1A, 0x06, 0x1B]);
const HAS_SEMANTICS: Col = C(1, &[0x14, 0x17]);
const METHOD_DEF_OR_REF: Col = C(1, &[0x06, 0x0A]);
const MEMBER_FORWARDED: Col = C(1, &[0x04, 0x06]);
const IMPLEMENTATION: Col = C(2, &[0x26, 0x23, 0x27]);
// (tags 0, 1 and 4 are unused; 2 MethodDef, 3 MemberRef)
const CUSTOM_ATTRIBUTE_TYPE: Col = C(3, &[usize::MAX, usize::MAX, 0x06, 0x0A, usize::MAX]);
const RESOLUTION_SCOPE: Col = C(2, &[0x00, 0x1A, 0x23, 0x01]);
const TYPE_OR_METHOD_DEF: Col = C(1, &[0x02, 0x06]);

/// Every table's columns (II.22), by number: what sizes its rows, so the tables after it are found.
fn schema(t: usize) -> &'static [Col] {
    match t {
        0x00 => &[U16, Str, Guid, Guid, Guid],
        0x01 => &[RESOLUTION_SCOPE, Str, Str],
        0x02 => &[U32, Str, Str, TYPE_DEF_OR_REF, T(0x04), T(0x06)],
        0x03 => &[T(0x04)],
        0x04 => &[U16, Str, Blob],
        0x05 => &[T(0x06)],
        0x06 => &[U32, U16, U16, Str, Blob, T(0x08)],
        0x07 => &[T(0x08)],
        0x08 => &[U16, U16, Str],
        0x09 => &[T(0x02), TYPE_DEF_OR_REF],
        0x0A => &[MEMBER_REF_PARENT, Str, Blob],
        0x0B => &[U16, HAS_CONSTANT, Blob],
        0x0C => &[HAS_CUSTOM_ATTRIBUTE, CUSTOM_ATTRIBUTE_TYPE, Blob],
        0x0D => &[HAS_FIELD_MARSHAL, Blob],
        0x0E => &[U16, HAS_DECL_SECURITY, Blob],
        0x0F => &[U16, U32, T(0x02)],
        0x10 => &[U32, T(0x04)],
        0x11 => &[Blob],
        0x12 => &[T(0x02), T(0x14)],
        0x13 => &[T(0x14)],
        0x14 => &[U16, Str, TYPE_DEF_OR_REF],
        0x15 => &[T(0x02), T(0x17)],
        0x16 => &[T(0x17)],
        0x17 => &[U16, Str, Blob],
        0x18 => &[U16, T(0x06), HAS_SEMANTICS],
        0x19 => &[T(0x02), METHOD_DEF_OR_REF, METHOD_DEF_OR_REF],
        0x1A => &[Str],
        0x1B => &[Blob],
        0x1C => &[U16, MEMBER_FORWARDED, Str, T(0x1A)],
        0x1D => &[U32, T(0x04)],
        0x1E => &[U32, U32],
        0x1F => &[U32],
        0x20 => &[U32, U16, U16, U16, U16, U32, Blob, Str, Str],
        0x21 => &[U32],
        0x22 => &[U32, U32, U32],
        0x23 => &[U16, U16, U16, U16, U32, Blob, Str, Str, Blob],
        0x24 => &[U32, T(0x23)],
        0x25 => &[U32, U32, U32, T(0x23)],
        0x26 => &[U32, Str, Blob],
        0x27 => &[U32, U32, Str, Str, IMPLEMENTATION],
        0x28 => &[U32, U32, Str, IMPLEMENTATION],
        0x29 => &[T(0x02), T(0x02)],
        0x2A => &[U16, U16, TYPE_OR_METHOD_DEF, Str],
        0x2B => &[METHOD_DEF_OR_REF, Blob],
        0x2C => &[T(0x2A), TYPE_DEF_OR_REF],
        _ => &[],
    }
}

struct Tables<'a> {
    s: Streams<'a>,
    rows: [u32; 64],
    /// each table's first row's offset in the `#~` stream, and its row's size
    at: [(usize, usize); 64],
    str4: bool,
    guid4: bool,
    blob4: bool,
}

impl<'a> Tables<'a> {
    fn width(&self, c: Col) -> usize {
        match c {
            U16 => 2,
            U32 => 4,
            Str => 2 + 2 * self.str4 as usize,
            Guid => 2 + 2 * self.guid4 as usize,
            Blob => 2 + 2 * self.blob4 as usize,
            T(t) => 2 + 2 * (self.rows[t] > 0xffff) as usize,
            C(bits, ts) => {
                let most = ts.iter().map(|&t| if t < 64 { self.rows[t] } else { 0 }).max().unwrap_or(0);
                2 + 2 * (most >= (1u32 << (16 - bits))) as usize
            }
        }
    }

    /// Column `col` of row `row` (1-based) of table `t`, as a number.
    fn get(&self, t: usize, row: u32, col: usize) -> Option<u32> {
        if row == 0 || row > self.rows[t] {
            return None;
        }
        let (base, size) = self.at[t];
        let cols = schema(t);
        let mut o = base + (row as usize - 1) * size;
        for c in &cols[..col] {
            o += self.width(*c);
        }
        match self.width(cols[col]) {
            2 => u16_at(self.s.tables, o).map(u32::from),
            _ => u32_at(self.s.tables, o),
        }
    }

    fn string(&self, i: u32) -> String {
        let s = self.s.strings.get(i as usize..).unwrap_or(&[]);
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf8_lossy(&s[..end]).into_owned()
    }

    fn blob(&self, i: u32) -> &'a [u8] {
        let b = self.s.blob.get(i as usize..).unwrap_or(&[]);
        match compressed(b) {
            Some((n, used)) => b.get(used..used + n as usize).unwrap_or(&[]),
            None => &[],
        }
    }
}

/// A compressed unsigned integer (II.23.2): its value and how many bytes it took.
fn compressed(b: &[u8]) -> Option<(u32, usize)> {
    let x = *b.first()?;
    if x & 0x80 == 0 {
        Some((x as u32, 1))
    } else if x & 0xC0 == 0x80 {
        Some(((((x & 0x3f) as u32) << 8) | *b.get(1)? as u32, 2))
    } else if x & 0xE0 == 0xC0 {
        Some(((((x & 0x1f) as u32) << 24) | ((*b.get(1)? as u32) << 16) | ((*b.get(2)? as u32) << 8) | *b.get(3)? as u32, 4))
    } else {
        None
    }
}

/// A custom attribute's `SerString` at `o`: the string (None for a null one) and where what follows starts.
fn ser_string(b: &[u8], o: usize) -> Option<(Option<String>, usize)> {
    if *b.get(o)? == 0xFF {
        return Some((None, o + 1));
    }
    let (n, used) = compressed(b.get(o..)?)?;
    let s = b.get(o + used..o + used + n as usize)?;
    Some((Some(String::from_utf8_lossy(s).into_owned()), o + used + n as usize))
}

fn parse_metadata(root: &[u8]) -> Option<Meta> {
    let s = streams(root)?;
    let t = s.tables;
    let heaps = *t.get(6)?;
    let valid = u64::from_le_bytes(t.get(8..16)?.try_into().ok()?);
    let mut rows = [0u32; 64];
    let mut o = 24;
    for (i, r) in rows.iter_mut().enumerate() {
        if valid >> i & 1 == 1 {
            *r = u32_at(t, o)?;
            o += 4;
        }
    }
    // (an uncompressed `#-` stream's extra data flag: four bytes more)
    if heaps & 0x40 != 0 {
        o += 4;
    }
    let mut tb = Tables { s, rows, at: [(0, 0); 64], str4: heaps & 1 != 0, guid4: heaps & 2 != 0, blob4: heaps & 4 != 0 };
    for i in 0..64 {
        if tb.rows[i] == 0 {
            continue;
        }
        let cols = schema(i);
        if cols.is_empty() {
            // a table this reader does not know the rows of: nothing after it can be found
            break;
        }
        let size: usize = cols.iter().map(|c| tb.width(*c)).sum();
        tb.at[i] = (o, size);
        o = o.checked_add(size.checked_mul(tb.rows[i] as usize)?)?;
    }
    if o > t.len() {
        return None;
    }
    let ver = |tb: &Tables, table: usize, row: u32, first: usize| -> String {
        let p: Vec<String> = (0..4).map(|k| tb.get(table, row, first + k).unwrap_or(0).to_string()).collect();
        p.join(".")
    };
    let mut m = Meta::default();
    if tb.rows[0x20] > 0 {
        m.name = tb.string(tb.get(0x20, 1, 7)?);
        m.version = ver(&tb, 0x20, 1, 1);
    }
    for r in 1..=tb.rows[0x23] {
        m.refs.push(AsmRef { name: tb.string(tb.get(0x23, r, 6)?), version: ver(&tb, 0x23, r, 0) });
    }
    // the BepInEx attributes: a CustomAttribute whose constructor is a MemberRef on a TypeRef named so
    for r in 1..=tb.rows[0x0C] {
        let ty = tb.get(0x0C, r, 1)?;
        if ty & 7 != 3 {
            continue;
        }
        let parent = tb.get(0x0A, ty >> 3, 0).unwrap_or(0);
        if parent & 7 != 1 {
            continue;
        }
        let name = tb.string(tb.get(0x01, parent >> 3, 1).unwrap_or(0));
        if name != "BepInPlugin" && name != "BepInDependency" {
            continue;
        }
        let v = tb.blob(tb.get(0x0C, r, 2)?);
        if v.get(0..2) != Some(&[1, 0]) {
            continue;
        }
        let Some((Some(first), next)) = ser_string(v, 2) else { continue };
        if name == "BepInPlugin" {
            let (pname, n2) = ser_string(v, next).unwrap_or((None, next));
            let (pver, _) = ser_string(v, n2).unwrap_or((None, n2));
            m.plugins.push(Plugin { guid: first, name: pname.unwrap_or_default(), version: pver.unwrap_or_default() });
        } else {
            // (guid, DependencyFlags): an int32 after it, 2 is SoftDependency; (guid, minimumVersion): a string,
            // which is a hard dependency. Which overload it is shows in the constructor's signature: its second
            // parameter's type, ELEMENT_TYPE_STRING (0x0e) or a value type (the enum)
            let sig = tb.blob(tb.get(0x0A, ty >> 3, 2).unwrap_or(0));
            let second = sig.get(4).copied();
            let soft = second != Some(0x0e) && v.len() >= next + 4 && u32_at(v, next) == Some(2);
            m.deps.push(Dependency { guid: first, soft });
        }
    }
    Some(m)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A small metadata writer: just the tables the reader looks at (and Module, TypeRef, MemberRef, CustomAttribute,
    /// Assembly, AssemblyRef), wrapped in a PE32 with one section, so the reader is tested on bytes laid out as the
    /// C# compiler lays them out.
    #[derive(Default)]
    pub struct Dll {
        pub name: String,
        pub version: [u16; 4],
        pub refs: Vec<(String, [u16; 4])>,
        pub plugin: Option<(String, String, String)>,
        /// (guid, Some(flags)) for the flags overload, (guid, None) for the minimum-version one
        pub deps: Vec<(String, Option<u32>)>,
        /// rows of padding in TypeRef: past 0x3fff of them, coded indexes into it are 4 bytes wide
        pub many_typerefs: u32,
    }

    struct Heap {
        strings: Vec<u8>,
        blob: Vec<u8>,
    }

    impl Heap {
        fn s(&mut self, s: &str) -> u32 {
            if s.is_empty() {
                return 0;
            }
            let at = self.strings.len() as u32;
            self.strings.extend_from_slice(s.as_bytes());
            self.strings.push(0);
            at
        }
        fn b(&mut self, data: &[u8]) -> u32 {
            let at = self.blob.len() as u32;
            assert!(data.len() < 0x80);
            self.blob.push(data.len() as u8);
            self.blob.extend_from_slice(data);
            at
        }
    }

    fn ser(out: &mut Vec<u8>, s: &str) {
        out.push(s.len() as u8);
        out.extend_from_slice(s.as_bytes());
    }

    impl Dll {
        pub fn bytes(&self) -> Vec<u8> {
            let mut h = Heap { strings: vec![0], blob: vec![0] };
                        fn w(v: u32, wide: bool, t: &mut Vec<u8>) {
                if wide {
                    t.extend_from_slice(&v.to_le_bytes())
                } else {
                    t.extend_from_slice(&(v as u16).to_le_bytes())
                }
            }
            // TypeRef rows: BepInPlugin, BepInDependency, then padding
            let mut typerefs: Vec<(u32, u32, u32)> = Vec::new();
            let bep = h.s("BepInEx");
            typerefs.push((0, h.s("BepInPlugin"), bep));
            typerefs.push((0, h.s("BepInDependency"), bep));
            let pad = h.s("Pad");
            for _ in 0..self.many_typerefs {
                typerefs.push((0, pad, 0));
            }
            let wide_tr = typerefs.len() as u32 >= 1 << 13;
            // MemberRef rows: .ctor of each (signatures: hasthis, n params, void, params)
            let ctor = h.s(".ctor");
            let sig_plugin = h.b(&[0x20, 3, 0x01, 0x0e, 0x0e, 0x0e]);
            let sig_dep_flags = h.b(&[0x20, 2, 0x01, 0x0e, 0x11, 0x08]);
            let sig_dep_ver = h.b(&[0x20, 2, 0x01, 0x0e, 0x0e]);
            let memberrefs = [(1u32, sig_plugin), (2, sig_dep_flags), (2, sig_dep_ver)];
            // CustomAttribute rows: (MemberRef row, value)
            let mut cas: Vec<(u32, u32)> = Vec::new();
            if let Some((g, n, v)) = &self.plugin {
                let mut val = vec![1, 0];
                ser(&mut val, g);
                ser(&mut val, n);
                ser(&mut val, v);
                val.extend_from_slice(&[0, 0]);
                cas.push((1, h.b(&val)));
            }
            for (g, flags) in &self.deps {
                let mut val = vec![1, 0];
                ser(&mut val, g);
                match flags {
                    Some(f) => {
                        val.extend_from_slice(&f.to_le_bytes());
                        cas.push((2, 0));
                    }
                    None => {
                        ser(&mut val, "1.0.0");
                        cas.push((3, 0));
                    }
                }
                val.extend_from_slice(&[0, 0]);
                cas.last_mut().unwrap().1 = h.b(&val);
            }
            let asm_name = h.s(&self.name);
            let refs: Vec<(u32, [u16; 4])> = self.refs.iter().map(|(n, v)| (h.s(n), *v)).collect();
            let module_name = h.s(&format!("{}.dll", self.name));
            // #~
            let mut t = Vec::new();
            t.extend_from_slice(&0u32.to_le_bytes());
            t.extend_from_slice(&[2, 0, 0, 1]);
            let present = [0x00usize, 0x01, 0x0A, 0x0C, 0x20, 0x23];
            let valid: u64 = present.iter().map(|&i| 1u64 << i).sum();
            t.extend_from_slice(&valid.to_le_bytes());
            t.extend_from_slice(&0u64.to_le_bytes());
            let counts = [1u32, typerefs.len() as u32, memberrefs.len() as u32, cas.len() as u32, 1, refs.len() as u32];
            for c in counts {
                t.extend_from_slice(&c.to_le_bytes());
            }
            // Module
            t.extend_from_slice(&0u16.to_le_bytes());
            w(module_name, false, &mut t);
            t.extend_from_slice(&[0; 6]);
            // TypeRef: ResolutionScope (coded, 2 bits over Module, ModuleRef, AssemblyRef, TypeRef)
            let rs_wide = typerefs.len() as u32 >= 1 << 14;
            for (scope, n, ns) in &typerefs {
                w(*scope, rs_wide, &mut t);
                w(*n, false, &mut t);
                w(*ns, false, &mut t);
            }
            // MemberRef: Class (MemberRefParent, 3 bits; TypeRef is tag 1)
            for (tr, sig) in memberrefs {
                w(tr << 3 | 1, wide_tr, &mut t);
                w(ctor, false, &mut t);
                w(sig, false, &mut t);
            }
            // CustomAttribute: Parent (HasCustomAttribute, 5 bits: the Assembly, tag 14), Type (3: MemberRef)
            let hca_wide = typerefs.len() as u32 >= 1 << 11;
            for (mr, val) in &cas {
                w(1 << 5 | 14, hca_wide, &mut t);
                w(mr << 3 | 3, false, &mut t);
                w(*val, false, &mut t);
            }
            // Assembly
            t.extend_from_slice(&0x8004u32.to_le_bytes());
            for x in self.version {
                t.extend_from_slice(&x.to_le_bytes());
            }
            t.extend_from_slice(&0u32.to_le_bytes());
            w(0, false, &mut t);
            w(asm_name, false, &mut t);
            w(0, false, &mut t);
            // AssemblyRef
            for (n, v) in &refs {
                for x in v {
                    t.extend_from_slice(&x.to_le_bytes());
                }
                t.extend_from_slice(&0u32.to_le_bytes());
                w(0, false, &mut t);
                w(*n, false, &mut t);
                w(0, false, &mut t);
                w(0, false, &mut t);
            }
            while t.len() % 4 != 0 {
                t.push(0);
            }
            while h.strings.len() % 4 != 0 {
                h.strings.push(0);
            }
            while h.blob.len() % 4 != 0 {
                h.blob.push(0);
            }
            // the metadata root
            let ver = b"v4.0.30319\0\0";
            let names: [(&[u8], &Vec<u8>); 3] = [(b"#~\0\0", &t), (b"#Strings\0\0\0\0", &h.strings), (b"#Blob\0\0\0", &h.blob)];
            let head = 16 + ver.len() + 4 + names.iter().map(|(n, _)| 8 + n.len()).sum::<usize>();
            let mut md = Vec::new();
            md.extend_from_slice(&0x424A_5342u32.to_le_bytes());
            md.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0, 0]);
            md.extend_from_slice(&(ver.len() as u32).to_le_bytes());
            md.extend_from_slice(ver);
            md.extend_from_slice(&[0, 0, 3, 0]);
            let mut off = head;
            for (n, data) in &names {
                md.extend_from_slice(&(off as u32).to_le_bytes());
                md.extend_from_slice(&(data.len() as u32).to_le_bytes());
                md.extend_from_slice(n);
                off += data.len();
            }
            for (_, data) in &names {
                md.extend_from_slice(data);
            }
            // the PE: DOS header, PE32 headers, one section `.text` at RVA 0x2000 holding the CLI header and metadata
            let file_align = 0x200usize;
            let sec_raw = 0x200usize;
            let sec_rva = 0x2000u32;
            let mut text = vec![0u8; 72];
            text[0..4].copy_from_slice(&72u32.to_le_bytes());
            text[4..8].copy_from_slice(&[2, 0, 5, 0]);
            text[8..12].copy_from_slice(&(sec_rva + 72).to_le_bytes());
            text[12..16].copy_from_slice(&(md.len() as u32).to_le_bytes());
            text.extend_from_slice(&md);
            let raw_size = text.len().div_ceil(file_align) * file_align;
            let mut f = vec![0u8; sec_raw];
            f[0..2].copy_from_slice(b"MZ");
            f[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
            let pe = 0x80;
            f[pe..pe + 4].copy_from_slice(b"PE\0\0");
            f[pe + 4..pe + 6].copy_from_slice(&0x14cu16.to_le_bytes());
            f[pe + 6..pe + 8].copy_from_slice(&1u16.to_le_bytes());
            f[pe + 20..pe + 22].copy_from_slice(&224u16.to_le_bytes());
            let opt = pe + 24;
            f[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes());
            f[opt + 92..opt + 96].copy_from_slice(&16u32.to_le_bytes());
            let dd = opt + 96;
            f[dd + 14 * 8..dd + 14 * 8 + 4].copy_from_slice(&sec_rva.to_le_bytes());
            f[dd + 14 * 8 + 4..dd + 14 * 8 + 8].copy_from_slice(&72u32.to_le_bytes());
            let s = opt + 224;
            f[s..s + 5].copy_from_slice(b".text");
            f[s + 8..s + 12].copy_from_slice(&(text.len() as u32).to_le_bytes());
            f[s + 12..s + 16].copy_from_slice(&sec_rva.to_le_bytes());
            f[s + 16..s + 20].copy_from_slice(&(raw_size as u32).to_le_bytes());
            f[s + 20..s + 24].copy_from_slice(&(sec_raw as u32).to_le_bytes());
            text.resize(raw_size, 0);
            f.extend_from_slice(&text);
            f
        }
    }

    /// UnityToolkit 2.0.2 as its metadata says (a plugin built against spt-reflection 4.1.5).
    pub fn unity_toolkit(spt: [u16; 4]) -> Dll {
        Dll {
            name: "UnityToolkit".into(),
            version: [2, 0, 2, 0],
            refs: vec![("netstandard".into(), [2, 1, 0, 0]), ("BepInEx".into(), [5, 4, 23, 0]), ("spt-reflection".into(), spt)],
            plugin: Some(("com.tylevo.unitytoolkit".into(), "UnityToolkit".into(), "2.0.2".into())),
            ..Default::default()
        }
    }

    #[test]
    fn reads_assembly_and_refs() {
        let m = parse(&unity_toolkit([4, 0, 1, 0]).bytes()).expect("metadata");
        assert_eq!(m.name, "UnityToolkit");
        assert_eq!(m.version, "2.0.2.0");
        assert_eq!(m.refs.len(), 3);
        assert_eq!(m.refs[2], AsmRef { name: "spt-reflection".into(), version: "4.0.1.0".into() });
        assert_eq!(m.plugins, vec![Plugin { guid: "com.tylevo.unitytoolkit".into(), name: "UnityToolkit".into(), version: "2.0.2".into() }]);
        assert!(m.deps.is_empty());
    }

    #[test]
    fn reads_dependencies_both_overloads() {
        let d = Dll {
            name: "ManimalIcebreakerClient".into(),
            version: [1, 2, 4, 0],
            refs: vec![("spt-reflection".into(), [4, 1, 6, 0]), ("DrakiaXYZ-BigBrain".into(), [1, 5, 0, 0])],
            plugin: Some(("com.manimal.icebreaker".into(), "Icebreaker".into(), "1.2.4".into())),
            deps: vec![("com.tylevo.unitytoolkit".into(), Some(1)), ("xyz.drakia.bigbrain".into(), None), ("com.soft.thing".into(), Some(2))],
            ..Default::default()
        };
        let m = parse(&d.bytes()).unwrap();
        assert_eq!(m.refs[1], AsmRef { name: "DrakiaXYZ-BigBrain".into(), version: "1.5.0.0".into() });
        assert_eq!(
            m.deps,
            vec![
                Dependency { guid: "com.tylevo.unitytoolkit".into(), soft: false },
                Dependency { guid: "xyz.drakia.bigbrain".into(), soft: false },
                Dependency { guid: "com.soft.thing".into(), soft: true },
            ]
        );
    }

    #[test]
    fn wide_coded_indexes() {
        // past 2^14 TypeRefs, ResolutionScope, MemberRefParent and HasCustomAttribute are 4 bytes wide
        let mut d = unity_toolkit([4, 1, 5, 0]);
        d.many_typerefs = 20000;
        d.deps = vec![("a.b".into(), Some(1))];
        let m = parse(&d.bytes()).unwrap();
        assert_eq!(m.refs[2].version, "4.1.5.0");
        assert_eq!(m.plugins.len(), 1);
        assert_eq!(m.deps.len(), 1);
    }

    #[test]
    fn not_dotnet() {
        assert_eq!(parse(b"MZ"), None);
        assert_eq!(parse(b"hello"), None);
        // a PE without a CLI header (a native library)
        let mut b = unity_toolkit([4, 1, 6, 0]).bytes();
        let dd = 0x80 + 24 + 96 + 14 * 8;
        b[dd..dd + 8].fill(0);
        assert_eq!(parse(&b), None);
        // cut short: no panic, nothing
        let full = unity_toolkit([4, 1, 6, 0]).bytes();
        for n in [64, 300, 600, full.len() - 40] {
            let _ = parse(&full[..n]);
        }
    }

    /// Real assemblies named by BUNKER_REAL_DLLS (`;`-separated), printed: a check on real files, run by hand.
    #[test]
    fn real_dlls_if_named() {
        let Ok(list) = std::env::var("BUNKER_REAL_DLLS") else { return };
        for p in list.split(';').filter(|p| !p.is_empty()) {
            let m = read(Path::new(p));
            let spt = std::env::var("BUNKER_REAL_SPT").unwrap_or_else(|_| "4.1.6".into());
            if let Some(x) = &m {
                let (built, status) = crate::clientcheck::judge(x, &spt);
                let refs: Vec<String> = x.refs.iter().map(|r| format!("{}={}", r.name, r.version)).collect();
                println!("REAL {p}\n  assembly {} {}\n  built_for {built} on SPT {spt}: {status}\n  refs {}\n  plugins {:?}\n  deps {:?}", x.name, x.version, refs.join(", "), x.plugins, x.deps);
            }
            assert!(m.is_some(), "{p} has no .NET metadata");
        }
    }
}
