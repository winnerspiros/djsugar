//! Reads TSAF, the object archive djay Pro keeps each record of its library
//! in. Only reading: DJ Sugar never writes djay's library.
//!
//! The layout follows djay-pro-autohotcue's `tsaf.py` (MIT, Copyright (c)
//! 2026 payne0420), which round-trips every blob of a real library byte for
//! byte, and tsaf-parser's notes (MIT, Copyright (c) 2026 Arlo O'Keeffe).
//!
//! A blob is a 20-byte header (`TSAF`, two u16 versions, a u64 count of
//! containers, a u32 count of strings) and then one value. A value starts
//! with a tag byte; numbers wider than a byte sit on their natural boundary
//! from the start of the blob, with zero bytes before them. An object is
//! `0x2B`, its class name, then pairs of value and (optional) field name,
//! then `0x00`. Strings are written once and later named by their place in
//! the order they were first written.

/// One value in a blob.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Obj(Obj),
    Arr(Vec<Value>),
    Str(String),
    Url(String),
    Int(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Data(Vec<u8>),
    /// A container written earlier in the same blob, by the order
    /// containers finished. Not followed: nothing DJ Sugar reads uses it.
    Ref(usize),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::F32(v) => Some(*v as f64),
            Value::F64(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Url(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&Obj> {
        match self {
            Value::Obj(o) => Some(o),
            _ => None,
        }
    }

    pub fn items(&self) -> &[Value] {
        match self {
            Value::Arr(items) => items,
            _ => &[],
        }
    }
}

/// An object: its class (`ADCCuePoint`) and fields in the order written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obj {
    pub class: String,
    pub fields: Vec<(Option<String>, Value)>,
}

impl Obj {
    /// The first field called `name`.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(n, _)| n.as_deref() == Some(name))
            .map(|(_, v)| v)
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name)?.as_f64()
    }

    pub fn text(&self, name: &str) -> String {
        self.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}

const END: u8 = 0x00;
const OBJ_REF: u8 = 0x02;
const STR_REF: u8 = 0x05;
const STR: u8 = 0x08;
const ARRAYS: [u8; 3] = [0x0A, 0x0B, 0x0C];
const TRUE: u8 = 0x0D;
const FALSE: u8 = 0x0E;
const INT8: u8 = 0x0F;
const INT16: u8 = 0x10;
const INT32: u8 = 0x11;
/// Not seen in Mac libraries; read in case the Windows app writes it.
const INT64: u8 = 0x12;
const F32: u8 = 0x13;
/// Not seen in Mac libraries; read in case the Windows app writes it.
const F64_WIDE: u8 = 0x14;
const DATA: u8 = 0x15;
const SETS: [u8; 2] = [0x19, 0x1A];
const URL: u8 = 0x21;
const OBJ: u8 = 0x2B;
/// djay's 1 and 0, with no bytes after the tag. (Checked on a real
/// library: cue pads, numbered from 1, start with 0x2D; key index 0 is
/// 0x2E.)
const ONE: u8 = 0x2D;
const ZERO: u8 = 0x2E;
const F64: u8 = 0x30;

/// Reads one blob's value.
pub fn parse(buf: &[u8]) -> Result<Value, String> {
    if buf.len() < 20 || &buf[..4] != b"TSAF" {
        return Err("not a TSAF record".into());
    }
    let mut r = Reader {
        buf,
        pos: 20,
        strings: Vec::new(),
        containers: 0,
        depth: 0,
    };
    let tag = r.u8()?;
    r.value(tag)
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    strings: Vec<String>,
    containers: usize,
    depth: usize,
}

impl Reader<'_> {
    fn eof(&self) -> String {
        format!("TSAF record ends early at {}", self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.pos.checked_add(n).ok_or_else(|| self.eof())?;
        let bytes = self.buf.get(self.pos..end).ok_or_else(|| self.eof())?;
        self.pos = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn align(&mut self, n: usize) {
        while !self.pos.is_multiple_of(n) && self.pos < self.buf.len() {
            self.pos += 1;
        }
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.align(N);
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn cstr(&mut self) -> Result<String, String> {
        let rest = &self.buf[self.pos..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| self.eof())?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        self.strings.push(s.clone());
        Ok(s)
    }

    fn string_ref(&mut self) -> Result<String, String> {
        let i = self.u8()? as usize;
        self.strings
            .get(i)
            .cloned()
            .ok_or_else(|| format!("TSAF string {i} not written yet"))
    }

    fn string_token(&mut self) -> Result<String, String> {
        match self.u8()? {
            STR => self.cstr(),
            STR_REF => self.string_ref(),
            t => Err(format!("TSAF: expected a string, found tag {t:#04x}")),
        }
    }

    fn value(&mut self, tag: u8) -> Result<Value, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("TSAF record nests too deep".into());
        }
        let v = self.inner(tag);
        self.depth -= 1;
        let v = v?;
        if matches!(
            v,
            Value::Obj(_) | Value::Arr(_) | Value::Data(_) | Value::F64(_)
        ) {
            self.containers += 1;
        }
        Ok(v)
    }

    fn inner(&mut self, tag: u8) -> Result<Value, String> {
        Ok(match tag {
            OBJ => {
                let mut obj = Obj {
                    class: self.string_token()?,
                    fields: Vec::new(),
                };
                loop {
                    let t = self.u8()?;
                    if t == END {
                        break;
                    }
                    let v = self.value(t)?;
                    let name = match self.buf.get(self.pos) {
                        Some(&STR) | Some(&STR_REF) => Some(self.string_token()?),
                        _ => None,
                    };
                    obj.fields.push((name, v));
                }
                Value::Obj(obj)
            }
            t if ARRAYS.contains(&t) || SETS.contains(&t) => {
                let n = self.u32()? as usize;
                // Every item takes at least a byte.
                if n > self.buf.len() - self.pos {
                    return Err(self.eof());
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    let t = self.u8()?;
                    items.push(self.value(t)?);
                }
                Value::Arr(items)
            }
            OBJ_REF => {
                let i = self.u8()? as usize;
                if i >= self.containers {
                    return Err(format!("TSAF container {i} not written yet"));
                }
                Value::Ref(i)
            }
            STR => Value::Str(self.cstr()?),
            STR_REF => Value::Str(self.string_ref()?),
            URL => {
                let s = self.string_token()?;
                if self.u8()? != END {
                    return Err("TSAF URL not closed".into());
                }
                Value::Url(s)
            }
            TRUE => Value::Bool(true),
            FALSE => Value::Bool(false),
            ZERO => Value::Int(0),
            ONE => Value::Int(1),
            INT8 => Value::Int(self.u8()? as i64),
            INT16 => Value::Int(u16::from_le_bytes(self.fixed()?) as i64),
            INT32 => Value::Int(self.u32()? as i64),
            INT64 => Value::Int(i64::from_le_bytes(self.fixed()?)),
            F32 => Value::F32(f32::from_le_bytes(self.fixed()?)),
            F64 | F64_WIDE => Value::F64(f64::from_le_bytes(self.fixed()?)),
            DATA => {
                let n = self.u32()? as usize;
                let bytes = self.take(n)?.to_vec();
                self.align(4);
                Value::Data(bytes)
            }
            t => return Err(format!("TSAF: unknown tag {t:#04x} at {}", self.pos - 1)),
        })
    }
}

/// Writes blobs the way djay does, for tests.
#[cfg(test)]
pub mod write {
    use super::*;

    #[derive(Default)]
    pub struct Writer {
        pub out: Vec<u8>,
        strings: Vec<String>,
    }

    impl Writer {
        pub fn blob(root: &Value) -> Vec<u8> {
            let mut w = Writer::default();
            w.out.extend_from_slice(b"TSAF\x03\x00\x03\x00");
            w.out.extend_from_slice(&[0; 12]);
            w.value(root);
            w.out
        }

        fn align(&mut self, n: usize) {
            while !self.out.len().is_multiple_of(n) {
                self.out.push(0);
            }
        }

        fn string(&mut self, s: &str) {
            if let Some(i) = self.strings.iter().position(|x| x == s) {
                self.out.extend_from_slice(&[STR_REF, i as u8]);
            } else {
                self.strings.push(s.to_string());
                self.out.push(STR);
                self.out.extend_from_slice(s.as_bytes());
                self.out.push(0);
            }
        }

        fn value(&mut self, v: &Value) {
            match v {
                Value::Obj(o) => {
                    self.out.push(OBJ);
                    self.string(&o.class);
                    for (name, v) in &o.fields {
                        self.value(v);
                        if let Some(n) = name {
                            self.string(n);
                        }
                    }
                    self.out.push(END);
                }
                Value::Arr(items) => {
                    self.out.push(0x0B);
                    self.align(4);
                    self.out
                        .extend_from_slice(&(items.len() as u32).to_le_bytes());
                    for i in items {
                        self.value(i);
                    }
                }
                Value::Str(s) => self.string(s),
                Value::Url(s) => {
                    self.out.push(URL);
                    self.string(s);
                    self.out.push(END);
                }
                Value::Int(0) => self.out.push(ZERO),
                Value::Int(1) => self.out.push(ONE),
                Value::Int(n) => self.out.extend_from_slice(&[INT8, *n as u8]),
                Value::F32(f) => {
                    self.out.push(F32);
                    self.align(4);
                    self.out.extend_from_slice(&f.to_le_bytes());
                }
                Value::F64(f) => {
                    self.out.push(F64);
                    self.align(8);
                    self.out.extend_from_slice(&f.to_le_bytes());
                }
                Value::Bool(b) => self.out.push(if *b { TRUE } else { FALSE }),
                Value::Data(d) => {
                    self.out.push(DATA);
                    self.align(4);
                    self.out.extend_from_slice(&(d.len() as u32).to_le_bytes());
                    self.out.extend_from_slice(d);
                    self.align(4);
                }
                Value::Ref(i) => self.out.extend_from_slice(&[OBJ_REF, *i as u8]),
            }
        }
    }

    pub fn obj(class: &str, fields: Vec<(&str, Value)>) -> Value {
        Value::Obj(Obj {
            class: class.into(),
            fields: fields
                .into_iter()
                .map(|(n, v)| ((!n.is_empty()).then(|| n.to_string()), v))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::write::*;
    use super::*;

    #[test]
    fn reads_what_it_writes() {
        let cue = |t: f32, n: i64| {
            obj(
                "ADCCuePoint",
                vec![
                    ("time", Value::F32(t)),
                    ("endTime", Value::F32(-1.0)),
                    ("number", Value::Int(n)),
                ],
            )
        };
        let root = obj(
            "ADCMediaItemUserData",
            vec![
                ("uuid", Value::Str("abc".into())),
                (
                    "cuePoints",
                    Value::Arr(vec![cue(1.5, 0), cue(2.5, 1), cue(9.0, 5)]),
                ),
                ("playCount", Value::Int(12)),
                ("", Value::Data(vec![1, 2, 3])),
                ("added", Value::F64(7.25)),
                ("url", Value::Url("file:///a.mp3".into())),
                ("flag", Value::Bool(false)),
            ],
        );
        let blob = Writer::blob(&root);
        assert_eq!(parse(&blob).unwrap(), root);
    }

    #[test]
    fn rejects_damage() {
        assert!(parse(b"TSAX").is_err());
        let blob = Writer::blob(&obj("A", vec![("x", Value::F32(1.0))]));
        for cut in 20..blob.len() {
            assert!(parse(&blob[..cut]).is_err(), "cut at {cut}");
        }
    }
}
