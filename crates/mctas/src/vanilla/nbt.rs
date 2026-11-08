//! Minimal reader for Minecraft's binary NBT (big endian), enough to read saved chunks.

use anyhow::{Result, bail};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<Tag>),
    Compound(BTreeMap<String, Tag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(m) => m.get(key),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_list(&self) -> &[Tag] {
        match self {
            Tag::List(l) => l,
            _ => &[],
        }
    }
    pub fn as_compound(&self) -> Option<&BTreeMap<String, Tag>> {
        match self {
            Tag::Compound(m) => Some(m),
            _ => None,
        }
    }
    /// Any integer tag as i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Tag::Byte(v) => Some(*v as i64),
            Tag::Short(v) => Some(*v as i64),
            Tag::Int(v) => Some(*v as i64),
            Tag::Long(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Tag::Float(v) => Some(*v as f64),
            Tag::Double(v) => Some(*v),
            _ => self.as_i64().map(|v| v as f64),
        }
    }
    pub fn as_long_array(&self) -> &[i64] {
        match self {
            Tag::LongArray(l) => l,
            _ => &[],
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.i + n > self.b.len() {
            bail!("NBT truncated at byte {}", self.i);
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into()?))
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into()?))
    }
    fn len(&mut self) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 {
            bail!("negative NBT length {n}");
        }
        Ok(n as usize)
    }
    fn string(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        // Java's modified UTF-8; block and key names are plain ASCII.
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn payload(&mut self, kind: u8, depth: u32) -> Result<Tag> {
        if depth > 512 {
            bail!("NBT nested too deep");
        }
        Ok(match kind {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(self.i16()?),
            3 => Tag::Int(self.i32()?),
            4 => Tag::Long(self.i64()?),
            5 => Tag::Float(f32::from_bits(self.i32()? as u32)),
            6 => Tag::Double(f64::from_bits(self.i64()? as u64)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.iter().map(|&b| b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.payload(elem, depth + 1)?);
                }
                Tag::List(v)
            }
            10 => {
                let mut m = BTreeMap::new();
                loop {
                    let k = self.u8()?;
                    if k == 0 {
                        break;
                    }
                    let name = self.string()?;
                    m.insert(name, self.payload(k, depth + 1)?);
                }
                Tag::Compound(m)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.i32()?);
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(self.i64()?);
                }
                Tag::LongArray(v)
            }
            k => bail!("unknown NBT tag type {k} at byte {}", self.i),
        })
    }
}

/// Parse a named root compound (the on-disk form: type byte, name, payload).
pub fn parse_named(bytes: &[u8]) -> Result<Tag> {
    let mut r = Reader { b: bytes, i: 0 };
    let kind = r.u8()?;
    if kind != 10 {
        bail!("NBT root is tag type {kind}, expected a compound");
    }
    let _name = r.string()?;
    r.payload(10, 0)
}

/// A tag as SNBT (the `/data get` text form): keys sorted, numbers with their type suffix,
/// strings and keys quoted when they need it.
pub fn to_snbt(t: &Tag) -> String {
    let mut out = String::new();
    write_snbt(t, &mut out);
    out
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_snbt(t: &Tag, out: &mut String) {
    use std::fmt::Write;
    match t {
        Tag::Byte(v) => {
            let _ = write!(out, "{v}b");
        }
        Tag::Short(v) => {
            let _ = write!(out, "{v}s");
        }
        Tag::Int(v) => {
            let _ = write!(out, "{v}");
        }
        Tag::Long(v) => {
            let _ = write!(out, "{v}L");
        }
        Tag::Float(v) => {
            let _ = write!(out, "{v:?}f");
        }
        Tag::Double(v) => {
            let _ = write!(out, "{v:?}d");
        }
        Tag::String(s) => quote(s, out),
        Tag::ByteArray(a) => {
            out.push_str("[B;");
            out.push_str(&a.iter().map(|v| format!("{v}B")).collect::<Vec<_>>().join(","));
            out.push(']');
        }
        Tag::IntArray(a) => {
            out.push_str("[I;");
            out.push_str(&a.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","));
            out.push(']');
        }
        Tag::LongArray(a) => {
            out.push_str("[L;");
            out.push_str(&a.iter().map(|v| format!("{v}L")).collect::<Vec<_>>().join(","));
            out.push(']');
        }
        Tag::List(l) => {
            out.push('[');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_snbt(v, out);
            }
            out.push(']');
        }
        Tag::Compound(m) => {
            out.push('{');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')) {
                    out.push_str(k);
                } else {
                    quote(k, out);
                }
                out.push(':');
                write_snbt(v, out);
            }
            out.push('}');
        }
    }
}

/// A block state from a chunk palette entry, in the game's string form (`minecraft:x[a=b,c=d]`,
/// properties sorted by name like `StateDefinition` does). 26.x writes a block's DEFAULT state
/// as a bare name string (`"minecraft:redstone_wire"`), which is returned as is: the caller
/// expands it to the full default state (the simulator knows the defaults). Other states are
/// `{id, properties}` (26.x) or `{Name, Properties}` (older).
pub fn block_state_string(entry: &Tag) -> String {
    if let Tag::String(name) = entry {
        return name.clone();
    }
    // A list mixing strings and compounds wraps each string as {"": value}.
    if let Some(Tag::String(name)) = entry.get("") {
        return name.clone();
    }
    let name = entry.get("id").or_else(|| entry.get("Name")).and_then(|t| t.as_str()).unwrap_or("?");
    match entry.get("properties").or_else(|| entry.get("Properties")).and_then(|t| t.as_compound()) {
        Some(props) if !props.is_empty() => {
            let kv: Vec<String> = props.iter().map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("?"))).collect();
            format!("{name}[{}]", kv.join(","))
        }
        _ => name.to_string(),
    }
}
