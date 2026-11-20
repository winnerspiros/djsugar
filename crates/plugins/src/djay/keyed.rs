//! Reads the records djay Pro wrote before it moved to TSAF: Apple's
//! keyed archives (`NSKeyedArchiver`, a binary property list starting
//! `bplist00`), turned into the same [`Value`]s the TSAF reader gives, so
//! the importer reads both alike.
//!
//! A keyed archive is a list of `$objects` and a `$top` naming the root.
//! Objects point at each other by index (a UID); index 0 is `$null`. An
//! object's `$class` names its class; arrays and sets keep their items in
//! `NS.objects`, dictionaries in `NS.keys` and `NS.objects`, a URL is
//! `NS.base` and `NS.relative`, a date `NS.time` in seconds since 2001.

use plist::Value as P;

use super::tsaf::{Obj, Value};

/// Reads one keyed archive.
pub fn parse(data: &[u8]) -> Result<Value, String> {
    let root = P::from_reader(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
    let archive = root.as_dictionary().ok_or("not a keyed archive")?;
    let objects = archive
        .get("$objects")
        .and_then(P::as_array)
        .ok_or("not a keyed archive")?;
    let top = archive
        .get("$top")
        .and_then(P::as_dictionary)
        .and_then(|t| t.get("root").or_else(|| t.values().next()))
        .ok_or("keyed archive without a root")?;
    Reader { objects }
        .value(top, 0)
        .ok_or_else(|| "empty keyed archive".into())
}

struct Reader<'a> {
    objects: &'a [P],
}

impl Reader<'_> {
    /// `v` as a value, following UIDs. `None` for nil.
    fn value(&self, v: &P, depth: usize) -> Option<Value> {
        if depth > 32 {
            return None;
        }
        Some(match v {
            P::Uid(u) => {
                let target = self.objects.get(u.get() as usize)?;
                if matches!(target, P::String(s) if s == "$null") {
                    return None;
                }
                if matches!(target, P::Uid(_)) {
                    return None;
                }
                return self.value(target, depth + 1);
            }
            P::String(s) => Value::Str(s.clone()),
            P::Integer(i) => Value::Int(i.as_signed().unwrap_or(i64::MAX)),
            P::Real(r) => Value::F64(*r),
            P::Boolean(b) => Value::Bool(*b),
            P::Data(d) => Value::Data(d.clone()),
            P::Date(d) => {
                let t: std::time::SystemTime = (*d).into();
                let unix = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0.0, |d| d.as_secs_f64());
                Value::F64(unix - APPLE_EPOCH)
            }
            P::Array(items) => Value::Arr(
                items
                    .iter()
                    .filter_map(|i| self.value(i, depth + 1))
                    .collect(),
            ),
            P::Dictionary(d) => {
                let class = d
                    .get("$class")
                    .and_then(|c| match c {
                        P::Uid(u) => self.objects.get(u.get() as usize),
                        _ => None,
                    })
                    .and_then(P::as_dictionary)
                    .and_then(|c| c.get("$classname"))
                    .and_then(P::as_string)
                    .unwrap_or_default();
                let field = |name: &str| d.get(name).and_then(|v| self.value(v, depth + 1));
                match class {
                    "NSArray"
                    | "NSMutableArray"
                    | "NSSet"
                    | "NSMutableSet"
                    | "NSOrderedSet"
                    | "NSMutableOrderedSet" => {
                        let items = d.get("NS.objects").and_then(P::as_array);
                        Value::Arr(
                            items
                                .into_iter()
                                .flatten()
                                .filter_map(|i| self.value(i, depth + 1))
                                .collect(),
                        )
                    }
                    "NSDictionary" | "NSMutableDictionary" => {
                        let keys = d.get("NS.keys").and_then(P::as_array);
                        let values = d.get("NS.objects").and_then(P::as_array);
                        let mut obj = Obj {
                            class: class.into(),
                            fields: Vec::new(),
                        };
                        for (k, v) in keys.into_iter().flatten().zip(values.into_iter().flatten()) {
                            let key = match self.value(k, depth + 1) {
                                Some(Value::Str(s)) => Some(s),
                                Some(Value::Int(i)) => Some(i.to_string()),
                                _ => None,
                            };
                            if let Some(v) = self.value(v, depth + 1) {
                                obj.fields.push((key, v));
                            }
                        }
                        Value::Obj(obj)
                    }
                    "NSURL" => {
                        let relative = field("NS.relative")?;
                        let relative = relative.as_str()?.to_string();
                        match field("NS.base").as_ref().and_then(Value::as_str) {
                            Some(base) if !relative.contains("://") => {
                                let base = base.trim_end_matches('/');
                                Value::Url(format!("{base}/{}", relative.trim_start_matches('/')))
                            }
                            _ => Value::Url(relative),
                        }
                    }
                    "NSDate" => Value::F64(d.get("NS.time").and_then(P::as_real)?),
                    "NSString" | "NSMutableString" => field("NS.string")?,
                    "NSData" | "NSMutableData" => field("NS.data")?,
                    "NSUUID" => {
                        let b = d.get("NS.uuidbytes").and_then(P::as_data)?;
                        Value::Str(uuid_text(b)?)
                    }
                    _ => Value::Obj(Obj {
                        class: class.into(),
                        fields: d
                            .iter()
                            .filter(|(k, _)| !k.starts_with('$'))
                            .filter_map(|(k, v)| Some((Some(k.clone()), self.value(v, depth + 1)?)))
                            .collect(),
                    }),
                }
            }
            _ => return None,
        })
    }
}

/// Seconds from 1970 to 2001, where Apple's dates count from.
pub const APPLE_EPOCH: f64 = 978_307_200.0;

fn uuid_text(b: &[u8]) -> Option<String> {
    if b.len() != 16 {
        return None;
    }
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}

/// Writes a keyed archive of `root`, for tests: objects as djay's are, with
/// `$null` first and every value but numbers behind a UID.
#[cfg(test)]
pub fn write(root: &Value) -> Vec<u8> {
    use plist::{Dictionary, Uid};

    struct W {
        objects: Vec<P>,
        classes: Vec<(String, u64)>,
    }
    impl W {
        fn push(&mut self, v: P) -> P {
            self.objects.push(v);
            P::Uid(Uid::new(self.objects.len() as u64 - 1))
        }
        fn class(&mut self, name: &str) -> P {
            if let Some((_, i)) = self.classes.iter().find(|(n, _)| n == name) {
                return P::Uid(Uid::new(*i));
            }
            let mut c = Dictionary::new();
            c.insert("$classname".into(), P::String(name.into()));
            c.insert(
                "$classes".into(),
                P::Array(vec![P::String(name.into()), P::String("NSObject".into())]),
            );
            let uid = self.push(P::Dictionary(c));
            if let P::Uid(u) = &uid {
                self.classes.push((name.into(), u.get()));
            }
            uid
        }
        fn value(&mut self, v: &Value) -> P {
            match v {
                Value::Int(i) => P::Integer((*i).into()),
                Value::F32(f) => P::Real(*f as f64),
                Value::F64(f) => P::Real(*f),
                Value::Bool(b) => P::Boolean(*b),
                Value::Str(s) => self.push(P::String(s.clone())),
                Value::Data(d) => self.push(P::Data(d.clone())),
                Value::Url(u) => {
                    let rel = self.push(P::String(u.clone()));
                    let mut d = Dictionary::new();
                    d.insert("NS.base".into(), P::Uid(Uid::new(0)));
                    d.insert("NS.relative".into(), rel);
                    d.insert("$class".into(), self.class("NSURL"));
                    self.push(P::Dictionary(d))
                }
                Value::Arr(items) => {
                    let items: Vec<P> = items.iter().map(|i| self.value(i)).collect();
                    let mut d = Dictionary::new();
                    d.insert("NS.objects".into(), P::Array(items));
                    d.insert("$class".into(), self.class("NSMutableArray"));
                    self.push(P::Dictionary(d))
                }
                Value::Obj(o) => {
                    let mut d = Dictionary::new();
                    for (k, v) in &o.fields {
                        let v = self.value(v);
                        d.insert(k.clone().unwrap_or_default(), v);
                    }
                    d.insert("$class".into(), self.class(&o.class));
                    self.push(P::Dictionary(d))
                }
                Value::Ref(_) => P::Uid(Uid::new(0)),
            }
        }
    }
    let mut w = W {
        objects: vec![P::String("$null".into())],
        classes: Vec::new(),
    };
    let top = w.value(root);
    let mut t = Dictionary::new();
    t.insert("root".into(), top);
    let mut a = Dictionary::new();
    a.insert("$version".into(), P::Integer(100_000.into()));
    a.insert("$archiver".into(), P::String("NSKeyedArchiver".into()));
    a.insert("$top".into(), P::Dictionary(t));
    a.insert("$objects".into(), P::Array(w.objects));
    let mut out = Vec::new();
    P::Dictionary(a).to_writer_binary(&mut out).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::super::tsaf::write::obj;
    use super::*;

    #[test]
    fn reads_a_keyed_archive() {
        let root = obj(
            "ADCMediaItemUserData",
            vec![
                ("uuid", Value::Str("abc".into())),
                (
                    "cuePoints",
                    Value::Arr(vec![obj(
                        "ADCCuePoint",
                        vec![
                            ("number", Value::Int(2)),
                            ("time", Value::F64(12.5)),
                            ("comment", Value::Str("Drop".into())),
                        ],
                    )]),
                ),
                ("playCount", Value::Int(3)),
                ("where", Value::Url("file:///Music/a%20b.mp3".into())),
            ],
        );
        let blob = write(&root);
        assert!(blob.starts_with(b"bplist00"));
        let Value::Obj(o) = parse(&blob).unwrap() else {
            panic!()
        };
        assert_eq!(o.class, "ADCMediaItemUserData");
        assert_eq!(o.text("uuid"), "abc");
        assert_eq!(o.number("playCount"), Some(3.0));
        let cue = o.get("cuePoints").unwrap().items()[0].as_obj().unwrap();
        assert_eq!(
            (cue.number("number"), cue.text("comment")),
            (Some(2.0), "Drop".into())
        );
        assert_eq!(o.text("where"), "file:///Music/a%20b.mp3");
        assert!(parse(b"bplist00junk").is_err());
    }

    #[test]
    fn broken_archives_are_turned_down() {
        let root = obj(
            "ADCMediaItemUserData",
            vec![
                ("uuid", Value::Str("abc".into())),
                ("playCount", Value::Int(3)),
                ("where", Value::Url("file:///Music/a.mp3".into())),
            ],
        );
        crate::garble::check(&[write(&root)], |b| {
            let _ = parse(b);
        });
    }

    #[test]
    fn uuids() {
        assert_eq!(
            uuid_text(&[
                0x3C, 0x77, 0x30, 0xA7, 0xF1, 0xDD, 0x41, 0x4E, 0xBB, 0xF3, 0xE2, 0x76, 0xB5, 0xAC,
                0x8D, 0xB2
            ])
            .as_deref(),
            Some("3C7730A7-F1DD-414E-BBF3-E276B5AC8DB2")
        );
    }
}
