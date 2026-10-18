//! A small reader/writer for Minecraft Java Edition NBT (big-endian, "modified UTF-8" strings).
//! Values keep their tag type and compounds keep their key order, so a file can be read, edited
//! and written back unchanged.

use std::io::{Read, Write};

use anyhow::{anyhow, bail, Result};
use flate2::read::{MultiGzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};

pub const END: u8 = 0;
pub const BYTE: u8 = 1;
pub const SHORT: u8 = 2;
pub const INT: u8 = 3;
pub const LONG: u8 = 4;
pub const FLOAT: u8 = 5;
pub const DOUBLE: u8 = 6;
pub const BYTE_ARRAY: u8 = 7;
pub const STRING: u8 = 8;
pub const LIST: u8 = 9;
pub const COMPOUND: u8 = 10;
pub const INT_ARRAY: u8 = 11;
pub const LONG_ARRAY: u8 = 12;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// Element type id and items. An empty list is written with type `END`.
    List(u8, Vec<Tag>),
    Compound(Compound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

/// An ordered map: setting an existing key keeps its position, so files are rewritten in their original order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Compound(Vec<(String, Tag)>);

impl Compound {
    pub fn new() -> Self {
        Compound(Vec::new())
    }
    pub fn get(&self, key: &str) -> Option<&Tag> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Tag> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn set(&mut self, key: impl Into<String>, value: Tag) {
        let key = key.into();
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.0.push((key, value)),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &(String, Tag)> {
        self.0.iter()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Tag {
    pub fn type_id(&self) -> u8 {
        match self {
            Tag::Byte(_) => BYTE,
            Tag::Short(_) => SHORT,
            Tag::Int(_) => INT,
            Tag::Long(_) => LONG,
            Tag::Float(_) => FLOAT,
            Tag::Double(_) => DOUBLE,
            Tag::ByteArray(_) => BYTE_ARRAY,
            Tag::String(_) => STRING,
            Tag::List(..) => LIST,
            Tag::Compound(_) => COMPOUND,
            Tag::IntArray(_) => INT_ARRAY,
            Tag::LongArray(_) => LONG_ARRAY,
        }
    }

    /// A list of `of`; empty lists are stored as lists of END, as the game does.
    pub fn list(of: u8, items: Vec<Tag>) -> Tag {
        Tag::List(if items.is_empty() { END } else { of }, items)
    }
}

fn type_name(t: u8) -> &'static str {
    match t {
        END => "end",
        BYTE => "byte",
        SHORT => "short",
        INT => "int",
        LONG => "long",
        FLOAT => "float",
        DOUBLE => "double",
        BYTE_ARRAY => "byteArray",
        STRING => "string",
        LIST => "list",
        COMPOUND => "compound",
        INT_ARRAY => "intArray",
        LONG_ARRAY => "longArray",
        _ => "unknown",
    }
}

// ---------- accessors (None-tolerant, like the TS helpers) ----------

pub fn compound(tag: Option<&Tag>) -> Option<&Compound> {
    match tag {
        Some(Tag::Compound(c)) => Some(c),
        _ => None,
    }
}

pub fn str(tag: Option<&Tag>) -> Option<&str> {
    match tag {
        Some(Tag::String(s)) => Some(s),
        _ => None,
    }
}

/// Any numeric tag as f64 (longs are converted, which is fine for timestamps).
pub fn num(tag: Option<&Tag>) -> Option<f64> {
    Some(match tag? {
        Tag::Byte(v) => *v as f64,
        Tag::Short(v) => *v as f64,
        Tag::Int(v) => *v as f64,
        Tag::Long(v) => *v as f64,
        Tag::Float(v) => *v as f64,
        Tag::Double(v) => *v,
        _ => return None,
    })
}

pub fn long(tag: Option<&Tag>) -> Option<i64> {
    match tag {
        Some(Tag::Long(v)) => Some(*v),
        Some(Tag::Int(v)) => Some(*v as i64),
        _ => None,
    }
}

pub fn list(tag: Option<&Tag>) -> &[Tag] {
    match tag {
        Some(Tag::List(_, items)) => items,
        _ => &[],
    }
}

// ---------- modified UTF-8 ----------

fn decode_mutf8(buf: &[u8]) -> String {
    let at = |i: usize| buf.get(i).copied().unwrap_or(0) as u16;
    let mut units: Vec<u16> = Vec::with_capacity(buf.len());
    let mut i = 0;
    while i < buf.len() {
        let a = buf[i] as u16;
        i += 1;
        if a < 0x80 {
            units.push(a);
        } else if a & 0xe0 == 0xc0 {
            units.push(((a & 0x1f) << 6) | (at(i) & 0x3f));
            i += 1;
        } else {
            units.push(((a & 0x0f) << 12) | ((at(i) & 0x3f) << 6) | (at(i + 1) & 0x3f));
            i += 2;
        }
    }
    // Surrogate pairs arrive as two 3-byte sequences, exactly like Java writes them.
    String::from_utf16_lossy(&units)
}

fn encode_mutf8(s: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(s.len());
    for c in s.encode_utf16() {
        if (1..0x80).contains(&c) {
            bytes.push(c as u8);
        } else if c < 0x800 {
            bytes.push(0xc0 | (c >> 6) as u8);
            bytes.push(0x80 | (c & 0x3f) as u8);
        } else {
            bytes.push(0xe0 | (c >> 12) as u8);
            bytes.push(0x80 | ((c >> 6) & 0x3f) as u8);
            bytes.push(0x80 | (c & 0x3f) as u8);
        }
    }
    if bytes.len() > 0xffff {
        bail!("NBT string too long");
    }
    Ok(bytes)
}

// ---------- reading ----------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos.checked_add(n).map_or(true, |end| end > self.buf.len()) {
            bail!("NBT data ends unexpectedly");
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.arr()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.arr()?))
    }
    fn string(&mut self) -> Result<String> {
        let len = u16::from_be_bytes(self.arr()?) as usize;
        Ok(decode_mutf8(self.take(len)?))
    }
    fn count(&mut self) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 {
            bail!("Corrupt NBT array length");
        }
        Ok(n as usize)
    }
    fn need(&self, n: usize) -> Result<()> {
        if self.pos.checked_add(n).map_or(true, |end| end > self.buf.len()) {
            bail!("NBT data ends unexpectedly");
        }
        Ok(())
    }

    fn payload(&mut self, t: u8, depth: usize) -> Result<Tag> {
        if depth > 512 {
            bail!("NBT nested too deeply");
        }
        Ok(match t {
            BYTE => Tag::Byte(self.u8()? as i8),
            SHORT => Tag::Short(i16::from_be_bytes(self.arr()?)),
            INT => Tag::Int(self.i32()?),
            LONG => Tag::Long(self.i64()?),
            FLOAT => Tag::Float(f32::from_be_bytes(self.arr()?)),
            DOUBLE => Tag::Double(f64::from_be_bytes(self.arr()?)),
            BYTE_ARRAY => {
                let n = self.count()?;
                Tag::ByteArray(self.take(n)?.iter().map(|b| *b as i8).collect())
            }
            STRING => Tag::String(self.string()?),
            LIST => {
                let of = self.u8()?;
                if of > LONG_ARRAY {
                    bail!("Unknown NBT tag type in list");
                }
                let n = self.count()?;
                let mut items = Vec::new();
                if of != END {
                    // Every element is at least one byte, so a huge count can't over-allocate.
                    self.need(n)?;
                    items.reserve(n.min(4096));
                    for _ in 0..n {
                        items.push(self.payload(of, depth + 1)?);
                    }
                }
                Tag::List(of, items)
            }
            COMPOUND => {
                let mut value = Compound::new();
                loop {
                    let t = self.u8()?;
                    if t > LONG_ARRAY {
                        bail!("Unknown NBT tag type");
                    }
                    if t == END {
                        break;
                    }
                    let name = self.string()?;
                    let v = self.payload(t, depth + 1)?;
                    value.set(name, v);
                }
                Tag::Compound(value)
            }
            INT_ARRAY => {
                let n = self.count()?;
                let bytes = self.take(n.checked_mul(4).ok_or_else(|| anyhow!("NBT data ends unexpectedly"))?)?;
                Tag::IntArray(bytes.chunks_exact(4).map(|c| i32::from_be_bytes(c.try_into().unwrap())).collect())
            }
            LONG_ARRAY => {
                let n = self.count()?;
                let bytes = self.take(n.checked_mul(8).ok_or_else(|| anyhow!("NBT data ends unexpectedly"))?)?;
                Tag::LongArray(bytes.chunks_exact(8).map(|c| i64::from_be_bytes(c.try_into().unwrap())).collect())
            }
            other => bail!("Unexpected NBT tag {}", type_name(other)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Zlib,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NbtFile {
    pub name: String,
    pub root: Compound,
    /// How the file was stored, so it can be written back the same way.
    pub compression: Compression,
}

/// Parses an NBT file, detecting gzip / zlib / uncompressed data.
pub fn parse(data: &[u8]) -> Result<NbtFile> {
    let mut compression = Compression::None;
    let mut inflated = Vec::new();
    let data = if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        compression = Compression::Gzip;
        MultiGzDecoder::new(data).read_to_end(&mut inflated)?;
        &inflated[..]
    } else if data.len() >= 2 && data[0] == 0x78 && (data[0] as u32 * 256 + data[1] as u32) % 31 == 0 {
        compression = Compression::Zlib;
        ZlibDecoder::new(data).read_to_end(&mut inflated)?;
        &inflated[..]
    } else {
        data
    };
    let mut r = Reader { buf: data, pos: 0 };
    if r.u8()? != COMPOUND {
        bail!("Not an NBT file (root is not a compound)");
    }
    let name = r.string()?;
    let Tag::Compound(root) = r.payload(COMPOUND, 0)? else { unreachable!() };
    Ok(NbtFile { name, root, compression })
}

// ---------- writing ----------

fn write_string(out: &mut Vec<u8>, s: &str) -> Result<()> {
    let enc = encode_mutf8(s)?;
    out.extend_from_slice(&(enc.len() as u16).to_be_bytes());
    out.extend_from_slice(&enc);
    Ok(())
}

fn write_len(out: &mut Vec<u8>, n: usize) -> Result<()> {
    let n = i32::try_from(n).map_err(|_| anyhow!("NBT array too long"))?;
    out.extend_from_slice(&n.to_be_bytes());
    Ok(())
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) -> Result<()> {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::ByteArray(v) => {
            write_len(out, v.len())?;
            out.extend(v.iter().map(|b| *b as u8));
        }
        Tag::String(s) => write_string(out, s)?,
        Tag::List(of, items) => {
            for it in items {
                if it.type_id() != *of {
                    bail!("NBT list of {} contains a {}", type_name(*of), type_name(it.type_id()));
                }
            }
            out.push(if items.is_empty() { END } else { *of });
            write_len(out, items.len())?;
            for it in items {
                write_payload(out, it)?;
            }
        }
        Tag::Compound(c) => {
            for (name, t) in c.iter() {
                out.push(t.type_id());
                write_string(out, name)?;
                write_payload(out, t)?;
            }
            out.push(END);
        }
        Tag::IntArray(v) => {
            write_len(out, v.len())?;
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            write_len(out, v.len())?;
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
    Ok(())
}

/// Serialises a file, compressed the same way it was read.
pub fn write(file: &NbtFile) -> Result<Vec<u8>> {
    let mut raw = vec![COMPOUND];
    write_string(&mut raw, &file.name)?;
    for (name, t) in file.root.iter() {
        raw.push(t.type_id());
        write_string(&mut raw, name)?;
        write_payload(&mut raw, t)?;
    }
    raw.push(END);
    Ok(match file.compression {
        Compression::Gzip => {
            let mut e = GzEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(&raw)?;
            e.finish()?
        }
        Compression::Zlib => {
            let mut e = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(&raw)?;
            e.finish()?
        }
        Compression::None => raw,
    })
}
