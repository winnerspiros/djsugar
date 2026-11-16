//! Values, objects and property access.

use std::cell::{Cell, RefCell};
use std::collections::HashMap as StdMap;

pub type HashMap<K, V> = StdMap<K, V, std::hash::BuildHasherDefault<FastHash>>;

/// FxHash: property names are short, and untrusted input never picks them
/// at scale, so a fast non-keyed hash beats SipHash here.
#[derive(Default, Clone, Copy)]
pub struct FastHash(u64);

impl std::hash::Hasher for FastHash {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        let mut h = self.0;
        let (words, rest) = bytes.as_chunks::<8>();
        for w in words {
            h = (h.rotate_left(5) ^ u64::from_le_bytes(*w)).wrapping_mul(K);
        }
        for &b in rest {
            h = (h.rotate_left(5) ^ b as u64).wrapping_mul(K);
        }
        self.0 = h;
    }
    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}
use std::rc::Rc;

use crate::number_to_string;
use crate::run::Closure;

pub use crate::ast::Name;

#[derive(Clone, Default)]
pub enum Value {
    #[default]
    Undefined,
    Null,
    Bool(bool),
    Num(f64),
    Str(Name),
    Obj(Rc<Obj>),
    /// An array slot never assigned. Only ever stored in an array's items;
    /// reads see `undefined`.
    Hole,
}

/// What unwinds the stack: a thrown value, or a `?.` chain stopping short.
pub enum Exc {
    Throw(Value),
    Short,
}

pub type R<T> = Result<T, Exc>;

pub type NativeFn = dyn Fn(&Rt, &Value, &[Value]) -> R<Value>;

#[derive(Clone)]
pub enum Prop {
    Data(Value),
    /// Data left out of enumeration (methods, built-ins).
    Method(Value),
    Accessor(Option<Value>, Option<Value>),
}

/// Interned names: every property key is the one shared allocation for its
/// text, so a key matches by address. Entries nothing else holds are purged
/// as the table grows.
#[derive(Default)]
struct Interner {
    by_text: StdMap<Name, (), std::hash::BuildHasherDefault<FastHash>>,
    addrs: StdMap<usize, (), std::hash::BuildHasherDefault<FastHash>>,
    purge_at: usize,
}

thread_local! {
    static INTERNER: RefCell<Interner> = RefCell::new(Interner { purge_at: 4096, ..Default::default() });
}

#[inline]
fn addr(s: &str) -> usize {
    s.as_ptr() as usize
}

/// Whether `s` is an interned name (checked by address only).
#[inline]
pub fn is_interned(s: &str) -> bool {
    INTERNER.with(|i| i.borrow().addrs.contains_key(&addr(s)))
}

/// The interned name for `s`, if one exists.
pub fn interned(s: &str) -> Option<Name> {
    INTERNER.with(|i| i.borrow().by_text.get_key_value(s).map(|(k, _)| k.clone()))
}

/// The shared name for `s`, created if new.
pub fn intern(s: &str) -> Name {
    INTERNER.with(|i| {
        let mut i = i.borrow_mut();
        if let Some((k, _)) = i.by_text.get_key_value(s) {
            return k.clone();
        }
        if i.by_text.len() >= i.purge_at {
            let Interner { by_text, addrs, .. } = &mut *i;
            by_text.retain(|k, _| {
                let keep = Name::strong_count(k) > 1;
                if !keep {
                    addrs.remove(&addr(k));
                }
                keep
            });
            i.purge_at = (i.by_text.len() * 2).max(4096);
        }
        let n: Name = Name::from(s);
        i.addrs.insert(addr(&n), ());
        i.by_text.insert(n.clone(), ());
        n
    })
}

/// `intern`, reusing `n` itself when it is new.
pub fn intern_rc(n: &Name) -> Name {
    if is_interned(n) {
        return n.clone();
    }
    intern(n)
}

/// Properties in insertion order, indexed by key address once there are many.
#[derive(Default)]
pub struct Props {
    list: Vec<(Name, Prop)>,
    index: Option<HashMap<usize, usize>>,
    /// One bit per key address bucket: a clear bit proves a key is absent.
    bloom: u64,
}

#[inline]
fn bloom_bit(a: usize) -> u64 {
    1 << ((a >> 3 ^ a >> 9) & 63)
}

const INDEX_FROM: usize = 8;

impl Props {
    #[inline]
    fn find_addr(&self, a: usize) -> Option<usize> {
        match &self.index {
            Some(ix) => ix.get(&a).copied(),
            None => self.list.iter().position(|(n, _)| addr(n) == a),
        }
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        if self.index.is_none() {
            let a = addr(name);
            return self
                .list
                .iter()
                .position(|(n, _)| addr(n) == a && n.len() == name.len() || **n == *name);
        }
        if let Some(i) = self.find_atom(name) {
            return Some(i);
        }
        if is_interned(name) {
            return None;
        }
        let canon = interned(name)?;
        self.find_atom(&canon)
    }

    /// `find` for a name known to be interned: by address alone.
    #[inline]
    pub fn find_atom(&self, name: &str) -> Option<usize> {
        if self.bloom & bloom_bit(addr(name)) == 0 {
            return None;
        }
        self.find_addr(addr(name))
            .filter(|&i| self.list[i].0.len() == name.len())
    }

    /// `find_atom`, trying the index this site found last time first.
    #[inline]
    pub fn find_atom_hinted(&self, name: &str, hint: &Cell<u32>) -> Option<usize> {
        if let Some((n, _)) = self.list.get(hint.get() as usize) {
            if addr(n) == addr(name) && n.len() == name.len() {
                return Some(hint.get() as usize);
            }
        }
        let i = self.find_atom(name)?;
        hint.set(i as u32);
        Some(i)
    }

    pub fn get(&self, name: &str) -> Option<&Prop> {
        self.find(name).map(|i| &self.list[i].1)
    }

    pub fn set(&mut self, name: &Name, prop: Prop) {
        match self.find(name) {
            Some(i) => {
                let slot = &mut self.list[i].1;
                *slot = match (&*slot, prop) {
                    (Prop::Method(_), Prop::Data(v)) => Prop::Method(v),
                    (_, p) => p,
                };
            }
            None => {
                let name = intern_rc(name);
                if let Some(ix) = &mut self.index {
                    ix.insert(addr(&name), self.list.len());
                } else if self.list.len() >= INDEX_FROM {
                    let mut ix: HashMap<usize, usize> = self
                        .list
                        .iter()
                        .enumerate()
                        .map(|(i, (n, _))| (addr(n), i))
                        .collect();
                    ix.insert(addr(&name), self.list.len());
                    self.index = Some(ix);
                }
                self.bloom |= bloom_bit(addr(&name));
                self.list.push((name, prop));
            }
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let Some(i) = self.find(name) else {
            return false;
        };
        self.list.remove(i);
        if self.index.is_some() {
            self.index = Some(
                self.list
                    .iter()
                    .enumerate()
                    .map(|(i, (n, _))| (addr(n), i))
                    .collect(),
            );
        }
        true
    }

    pub fn keys(&self) -> impl Iterator<Item = &Name> {
        self.list
            .iter()
            .filter(|(n, p)| !n.starts_with('\0') && !matches!(p, Prop::Method(_)))
            .map(|(n, _)| n)
    }

    pub fn entries(&self) -> &[(Name, Prop)] {
        &self.list
    }
}

pub enum Kind {
    Plain,
    Array(RefCell<Vec<Value>>),
    Closure(Closure),
    Native(Name, Rc<NativeFn>),
    Bound(Value, Value, Vec<Value>),
    Regex(Box<crate::regex::Regex>),
    Map(RefCell<crate::coll::Coll>),
    Set(RefCell<crate::coll::Coll>),
    Buffer(crate::coll::Bytes),
    Typed(crate::coll::View),
    /// A generator's values not yet handed out.
    Generator(RefCell<std::vec::IntoIter<Value>>),
    /// A target and the handler whose traps stand in for it.
    Proxy(Value, Value),
}

/// The well-known `Symbol.iterator` key.
pub const ITERATOR: &str = "\0@@iterator";

pub struct Obj {
    pub props: RefCell<Props>,
    pub proto: RefCell<Option<Rc<Obj>>>,
    pub kind: Kind,
}

impl Obj {
    pub fn new(proto: Option<Rc<Obj>>, kind: Kind) -> Rc<Obj> {
        Rc::new(Obj {
            props: RefCell::new(Props::default()),
            proto: RefCell::new(proto),
            kind,
        })
    }

    pub fn is_callable(&self) -> bool {
        matches!(
            self.kind,
            Kind::Closure(_) | Kind::Native(..) | Kind::Bound(..)
        )
    }

    pub fn put(&self, name: &str, v: Value) {
        self.props.borrow_mut().set(&name.into(), Prop::Data(v));
    }

    pub fn put_name(&self, name: &Name, v: Value) {
        self.props.borrow_mut().set(name, Prop::Data(v));
    }

    /// The property found along the prototype chain.
    /// An own data property, `name` interned, read through a site's cache.
    #[inline]
    pub fn own_data_hinted(&self, name: &str, hint: &Cell<u32>) -> Option<Value> {
        let props = self.props.borrow();
        match &props.list[props.find_atom_hinted(name, hint)?].1 {
            Prop::Data(v) | Prop::Method(v) => Some(v.clone()),
            Prop::Accessor(..) => None,
        }
    }

    /// A data property found along the prototype chain, `name` interned.
    /// None means the full lookup must decide (accessors, proxies, misses,
    /// array lengths).
    #[inline]
    pub fn get_atom(&self, name: &str, hint: &Cell<u32>, length: bool) -> Option<Value> {
        if matches!(self.kind, Kind::Proxy(..)) || length && matches!(self.kind, Kind::Array(_)) {
            return None;
        }
        {
            let props = self.props.borrow();
            if let Some(i) = props.find_atom_hinted(name, hint) {
                return match &props.list[i].1 {
                    Prop::Data(v) | Prop::Method(v) => Some(v.clone()),
                    Prop::Accessor(..) => None,
                };
            }
        }
        let mut cur: *const Obj = self
            .proto
            .borrow()
            .as_ref()
            .map_or(std::ptr::null(), Rc::as_ptr);
        // SAFETY: no script runs during the walk, so every object on the
        // chain stays alive (each is held by the one before it).
        while let Some(o) = unsafe { cur.as_ref() } {
            if matches!(o.kind, Kind::Proxy(..)) {
                return None;
            }
            let props = o.props.borrow();
            if let Some(i) = props.find_atom(name) {
                return match &props.list[i].1 {
                    Prop::Data(v) | Prop::Method(v) => Some(v.clone()),
                    Prop::Accessor(..) => None,
                };
            }
            cur = o
                .proto
                .borrow()
                .as_ref()
                .map_or(std::ptr::null(), Rc::as_ptr);
        }
        None
    }

    pub fn lookup(self: &Rc<Obj>, name: &str) -> Option<Prop> {
        let mut o = self.clone();
        loop {
            if let Some(p) = o.props.borrow().get(name) {
                return Some(p.clone());
            }
            let next = o.proto.borrow().clone()?;
            o = next;
        }
    }
}

/// The realm: the global object and the built-in prototypes.
pub type Logger = Box<dyn FnMut(&str)>;

pub struct Rt {
    /// `Function.prototype.call`, recognised at call sites.
    pub fn_call: std::cell::OnceCell<Rc<Obj>>,
    /// Script call depth, bounded to keep the native stack safe.
    pub depth: Cell<u32>,
    pub generator_proto: Rc<Obj>,
    pub regexp_proto: Rc<Obj>,
    pub global: Rc<Obj>,
    pub object_proto: Rc<Obj>,
    pub function_proto: Rc<Obj>,
    pub array_proto: Rc<Obj>,
    pub string_proto: Rc<Obj>,
    pub number_proto: Rc<Obj>,
    pub boolean_proto: Rc<Obj>,
    pub error_proto: Rc<Obj>,
    pub type_error_proto: Rc<Obj>,
    pub range_error_proto: Rc<Obj>,
    pub reference_error_proto: Rc<Obj>,
    pub length: Name,
    pub prototype: Name,
    pub constructor: Name,
    pub log: RefCell<Logger>,
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Undefined | Value::Hole => f.write_str("undefined"),
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(n) => f.write_str(&number_to_string(*n)),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Obj(o) if o.is_callable() => f.write_str("[function]"),
            Value::Obj(_) => f.write_str("[object]"),
        }
    }
}

impl Value {
    #[inline]
    pub fn unhole(&self) -> Value {
        match self {
            Value::Hole => Value::Undefined,
            v => v.clone(),
        }
    }

    pub fn str(s: &str) -> Value {
        Value::Str(s.into())
    }

    pub fn truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Hole | Value::Null => false,
            Value::Bool(b) => *b,
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Str(s) => !s.is_empty(),
            Value::Obj(_) => true,
        }
    }

    pub fn is_nullish(&self) -> bool {
        matches!(self, Value::Undefined | Value::Null)
    }

    pub fn as_obj(&self) -> Option<&Rc<Obj>> {
        match self {
            Value::Obj(o) => Some(o),
            _ => None,
        }
    }

    pub fn type_of(&self) -> &'static str {
        match self {
            Value::Undefined | Value::Hole => "undefined",
            Value::Null => "object",
            Value::Bool(_) => "boolean",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Obj(o) if o.is_callable() => "function",
            Value::Obj(_) => "object",
        }
    }

    pub fn strict_eq(&self, o: &Value) -> bool {
        match (self, o) {
            (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Obj(a), Value::Obj(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub fn same_value_zero(&self, o: &Value) -> bool {
        match (self, o) {
            (Value::Num(a), Value::Num(b)) if a.is_nan() && b.is_nan() => true,
            _ => self.strict_eq(o),
        }
    }
}

pub fn str_to_num(s: &str) -> f64 {
    let t = s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if t.is_empty() {
        return 0.0;
    }
    let radix = match t.get(..2) {
        Some("0x" | "0X") => 16,
        Some("0o" | "0O") => 8,
        Some("0b" | "0B") => 2,
        _ => 10,
    };
    if radix != 10 {
        let d = &t[2..];
        if d.is_empty() || !d.chars().all(|c| c.is_digit(radix)) {
            return f64::NAN;
        }
        return d.chars().fold(0.0, |a, c| {
            a * radix as f64 + c.to_digit(radix).unwrap() as f64
        });
    }
    match t {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ if t
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-')) =>
        {
            t.parse().unwrap_or(f64::NAN)
        }
        _ => f64::NAN,
    }
}

pub fn to_int32(n: f64) -> i32 {
    if n.is_finite() {
        (n.trunc() as i64 as u64 as u32) as i32
    } else {
        0
    }
}

/// `n` as an array index, if it is one.
#[inline]
pub fn as_index(n: f64) -> Option<usize> {
    let i = n as usize;
    (i as f64 == n && n >= 0.0 && i < u32::MAX as usize).then_some(i)
}

pub fn index_of_name(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.is_empty()
        || b.len() > 10
        || !b.iter().all(u8::is_ascii_digit)
        || (b[0] == b'0' && b.len() > 1)
    {
        return None;
    }
    s.parse::<u64>()
        .ok()
        .filter(|&v| v < u32::MAX as u64)
        .map(|v| v as usize)
}

impl Rt {
    pub fn throw<T>(&self, proto: &Rc<Obj>, msg: &str) -> R<T> {
        let e = Obj::new(Some(proto.clone()), Kind::Plain);
        e.put("message", Value::str(msg));
        Err(Exc::Throw(Value::Obj(e)))
    }

    pub fn type_error<T>(&self, msg: &str) -> R<T> {
        let p = self.type_error_proto.clone();
        self.throw(&p, msg)
    }

    pub fn object(&self) -> Rc<Obj> {
        Obj::new(Some(self.object_proto.clone()), Kind::Plain)
    }

    pub fn array(&self, items: Vec<Value>) -> Value {
        Value::Obj(Obj::new(
            Some(self.array_proto.clone()),
            Kind::Array(RefCell::new(items)),
        ))
    }

    pub fn native(
        &self,
        name: &str,
        f: impl Fn(&Rt, &Value, &[Value]) -> R<Value> + 'static,
    ) -> Value {
        Value::Obj(Obj::new(
            Some(self.function_proto.clone()),
            Kind::Native(name.into(), Rc::new(f)),
        ))
    }

    pub fn proto_of(&self, v: &Value) -> Option<Rc<Obj>> {
        match v {
            Value::Obj(o) => o.proto.borrow().clone(),
            Value::Str(_) => Some(self.string_proto.clone()),
            Value::Num(_) => Some(self.number_proto.clone()),
            Value::Bool(_) => Some(self.boolean_proto.clone()),
            _ => None,
        }
    }

    pub fn to_num(&self, v: &Value) -> R<f64> {
        Ok(match v {
            Value::Undefined | Value::Hole => f64::NAN,
            Value::Null => 0.0,
            Value::Bool(b) => *b as u8 as f64,
            Value::Num(n) => *n,
            Value::Str(s) => str_to_num(s),
            Value::Obj(_) => {
                let p = self.to_primitive(v, false)?;
                return self.to_num(&p);
            }
        })
    }

    pub fn to_str(&self, v: &Value) -> R<Name> {
        Ok(match v {
            Value::Undefined | Value::Hole => "undefined".into(),
            Value::Null => "null".into(),
            Value::Bool(b) => if *b { "true" } else { "false" }.into(),
            Value::Num(n) => number_to_string(*n).into(),
            Value::Str(s) => s.clone(),
            Value::Obj(_) => {
                let p = self.to_primitive(v, true)?;
                return self.to_str(&p);
            }
        })
    }

    /// Objects to primitives through their `valueOf`/`toString`.
    pub fn to_primitive(&self, v: &Value, string_first: bool) -> R<Value> {
        let Value::Obj(_) = v else {
            return Ok(v.clone());
        };
        let order = if string_first {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };
        for m in order {
            let f = self.get(v, m)?;
            if f.as_obj().is_some_and(|o| o.is_callable()) {
                let r = self.call(&f, v.clone(), &[])?;
                if !matches!(r, Value::Obj(_)) {
                    return Ok(r);
                }
            }
        }
        self.type_error("cannot convert object to a primitive value")
    }

    pub fn get(&self, v: &Value, name: &str) -> R<Value> {
        match v {
            Value::Obj(o) => {
                if let Kind::Proxy(target, handler) = &o.kind {
                    let trap = self.get(handler, "get")?;
                    if trap.is_nullish() {
                        return self.get(target, name);
                    }
                    return self.call(
                        &trap,
                        handler.clone(),
                        &[target.clone(), Value::str(name), v.clone()],
                    );
                }
                if let Kind::Array(items) = &o.kind {
                    if name == "length" {
                        return Ok(Value::Num(items.borrow().len() as f64));
                    }
                    if let Some(i) = index_of_name(name) {
                        if let Some(x) = items.borrow().get(i) {
                            return Ok(x.unhole());
                        }
                    }
                } else if let (Kind::Typed(t), Some(i)) = (&o.kind, index_of_name(name)) {
                    return Ok(t.get(i).map_or(Value::Undefined, Value::Num));
                }
                self.get_from(o, v, name)
            }
            Value::Str(s) => {
                if name == "length" {
                    return Ok(Value::Num(str_len(s) as f64));
                }
                if let Some(i) = index_of_name(name) {
                    return Ok(char_at(s, i).map_or(Value::Undefined, Value::Str));
                }
                self.get_from(&self.string_proto, v, name)
            }
            Value::Num(_) => self.get_from(&self.number_proto, v, name),
            Value::Bool(_) => self.get_from(&self.boolean_proto, v, name),
            _ => self.type_error(&format!(
                "cannot read property '{name}' of {}",
                self.to_str(v)?
            )),
        }
    }

    fn get_from(&self, o: &Rc<Obj>, receiver: &Value, name: &str) -> R<Value> {
        match o.lookup(name) {
            None => Ok(match (&o.kind, name) {
                (Kind::Closure(c), "length") => Value::Num(c.code.length as f64),
                (Kind::Closure(c), "name") => Value::Str(c.code.name.clone().unwrap_or_default()),
                (Kind::Native(n, _), "name") => Value::Str(n.clone()),
                _ => Value::Undefined,
            }),
            Some(Prop::Data(v) | Prop::Method(v)) => Ok(v),
            Some(Prop::Accessor(Some(g), _)) => self.call(&g, receiver.clone(), &[]),
            Some(Prop::Accessor(None, _)) => Ok(Value::Undefined),
        }
    }

    pub fn get_index(&self, v: &Value, key: &Value) -> R<Value> {
        if let Value::Num(n) = key {
            if let Some(i) = as_index(*n) {
                match v {
                    Value::Obj(o) => match &o.kind {
                        Kind::Array(items) => {
                            if let Some(x) = items.borrow().get(i) {
                                return Ok(x.unhole());
                            }
                        }
                        Kind::Typed(t) => return Ok(t.get(i).map_or(Value::Undefined, Value::Num)),
                        _ => {}
                    },
                    Value::Str(s) => return Ok(char_at(s, i).map_or(Value::Undefined, Value::Str)),
                    _ => {}
                }
            }
        }
        let k = self.to_key(key)?;
        self.get(v, &k)
    }

    pub fn to_key(&self, key: &Value) -> R<Name> {
        match key {
            Value::Str(s) => Ok(s.clone()),
            _ => self.to_str(key),
        }
    }

    pub fn set(&self, target: &Value, name: &Name, value: Value) -> R<()> {
        let Value::Obj(o) = target else {
            if target.is_nullish() {
                return self.type_error(&format!(
                    "cannot set property '{name}' of {}",
                    self.to_str(target)?
                ));
            }
            return Ok(());
        };
        if let Kind::Array(items) = &o.kind {
            if let Some(i) = index_of_name(name) {
                set_item(items, i, value);
                return Ok(());
            }
            if &**name == "length" {
                let n = self.to_num(&value)?;
                let Some(n) = as_index(n) else {
                    return self.throw(&self.range_error_proto.clone(), "invalid array length");
                };
                items.borrow_mut().resize(n, Value::Hole);
                return Ok(());
            }
        }
        if let Kind::Proxy(t, handler) = &o.kind {
            let trap = self.get(handler, "set")?;
            if trap.is_nullish() {
                return self.set(t, name, value);
            }
            self.call(
                &trap,
                handler.clone(),
                &[t.clone(), Value::Str(name.clone()), value, target.clone()],
            )?;
            return Ok(());
        }
        if let (Kind::Typed(t), Some(i)) = (&o.kind, index_of_name(name)) {
            let n = self.to_num(&value)?;
            t.set(i, n);
            return Ok(());
        }
        let own_data = matches!(
            o.props.borrow().get(name),
            Some(Prop::Data(_) | Prop::Method(_))
        );
        if !own_data {
            if let Some(Prop::Accessor(_, setter)) = o.lookup(name) {
                if let Some(s) = setter {
                    self.call(&s, target.clone(), &[value])?;
                }
                return Ok(());
            }
        }
        o.props.borrow_mut().set(name, Prop::Data(value));
        Ok(())
    }

    pub fn set_index(&self, target: &Value, key: &Value, value: Value) -> R<()> {
        if let (Value::Obj(o), Value::Num(n)) = (target, key) {
            match (&o.kind, as_index(*n)) {
                (Kind::Array(items), Some(i)) => {
                    set_item(items, i, value);
                    return Ok(());
                }
                (Kind::Typed(t), Some(i)) => {
                    let x = self.to_num(&value)?;
                    t.set(i, x);
                    return Ok(());
                }
                _ => {}
            }
        }
        let k = self.to_key(key)?;
        self.set(target, &k, value)
    }

    pub fn has(&self, o: &Rc<Obj>, name: &str) -> bool {
        if let Kind::Proxy(t, handler) = &o.kind {
            let trap = self.get(handler, "has").unwrap_or_default();
            if !trap.is_nullish() {
                return self
                    .call(&trap, handler.clone(), &[t.clone(), Value::str(name)])
                    .is_ok_and(|r| r.truthy());
            }
            return t.as_obj().is_some_and(|t| self.has(t, name));
        }
        if let Kind::Array(items) = &o.kind {
            if name == "length"
                || index_of_name(name).is_some_and(|i| {
                    items
                        .borrow()
                        .get(i)
                        .is_some_and(|v| !matches!(v, Value::Hole))
                })
            {
                return true;
            }
        }
        o.lookup(name).is_some()
    }

    /// Own enumerable keys, array indices first.
    pub fn own_keys(&self, o: &Rc<Obj>) -> Vec<Name> {
        let mut out: Vec<Name> = Vec::new();
        match &o.kind {
            Kind::Proxy(Value::Obj(t), _) => return self.own_keys(t),
            Kind::Array(items) => out.extend(
                items
                    .borrow()
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| !matches!(v, Value::Hole))
                    .map(|(i, _)| Name::from(i.to_string())),
            ),
            Kind::Typed(t) => out.extend((0..t.len).map(|i| Name::from(i.to_string()))),
            _ => {}
        }
        out.extend(o.props.borrow().keys().cloned());
        out
    }

    pub fn equals(&self, a: &Value, b: &Value) -> R<bool> {
        Ok(match (a, b) {
            (Value::Undefined | Value::Null, Value::Undefined | Value::Null) => true,
            (Value::Undefined | Value::Null, _) | (_, Value::Undefined | Value::Null) => false,
            (Value::Num(_), Value::Str(_)) | (Value::Str(_), Value::Num(_)) => {
                self.to_num(a)? == self.to_num(b)?
            }
            (Value::Bool(_), _) => return self.equals(&Value::Num(self.to_num(a)?), b),
            (_, Value::Bool(_)) => return self.equals(a, &Value::Num(self.to_num(b)?)),
            (Value::Obj(_), Value::Obj(_)) => a.strict_eq(b),
            (Value::Obj(_), _) => return self.equals(&self.to_primitive(a, false)?, b),
            (_, Value::Obj(_)) => return self.equals(a, &self.to_primitive(b, false)?),
            _ => a.strict_eq(b),
        })
    }

    pub fn add(&self, a: &Value, b: &Value) -> R<Value> {
        if let (Value::Num(x), Value::Num(y)) = (a, b) {
            return Ok(Value::Num(x + y));
        }
        let (a, b) = match (a, b) {
            (Value::Str(_) | Value::Num(_), Value::Str(_) | Value::Num(_)) => {
                (a.clone(), b.clone())
            }
            _ => (self.to_primitive(a, false)?, self.to_primitive(b, false)?),
        };
        if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) {
            let mut s = String::with_capacity(24);
            for v in [&a, &b] {
                match v {
                    Value::Str(x) => s.push_str(x),
                    Value::Num(n) => crate::push_number(&mut s, *n),
                    v => s.push_str(&self.to_str(v)?),
                }
            }
            return Ok(Value::Str(Name::from(s.as_str())));
        }
        Ok(Value::Num(self.to_num(&a)? + self.to_num(&b)?))
    }

    /// `a < b`; None where either is NaN.
    pub fn less(&self, a: &Value, b: &Value) -> R<Option<bool>> {
        if let (Value::Num(x), Value::Num(y)) = (a, b) {
            return Ok(x.partial_cmp(y).map(|o| o.is_lt()));
        }
        let a = self.to_primitive(a, false)?;
        let b = self.to_primitive(b, false)?;
        if let (Value::Str(x), Value::Str(y)) = (&a, &b) {
            return Ok(Some(x.encode_utf16().lt(y.encode_utf16())));
        }
        let (x, y) = (self.to_num(&a)?, self.to_num(&b)?);
        Ok(x.partial_cmp(&y).map(|o| o.is_lt()))
    }

    pub fn instance_of(&self, v: &Value, f: &Value) -> R<bool> {
        let Some(fo) = f.as_obj().filter(|o| o.is_callable()) else {
            return self.type_error("right side of `instanceof` is not callable");
        };
        if let Kind::Bound(t, ..) = &fo.kind {
            return self.instance_of(v, t);
        }
        let Value::Obj(proto) = self.get(f, "prototype")? else {
            return Ok(false);
        };
        let Value::Obj(o) = v else { return Ok(false) };
        let mut p = o.proto.borrow().clone();
        while let Some(x) = p {
            if Rc::ptr_eq(&x, &proto) {
                return Ok(true);
            }
            p = x.proto.borrow().clone();
        }
        Ok(false)
    }

    /// The items of an array, string, or other iterable-by-index value.
    pub fn list(&self, v: &Value) -> R<Vec<Value>> {
        match v {
            Value::Obj(o) => {
                match &o.kind {
                    Kind::Array(items) => {
                        return Ok(items.borrow().iter().map(Value::unhole).collect())
                    }
                    Kind::Typed(t) => {
                        return Ok((0..t.len).map(|i| Value::Num(t.get(i).unwrap())).collect())
                    }
                    Kind::Set(c) => {
                        return Ok(c.borrow().items().map(|(k, _)| k.clone()).collect())
                    }
                    Kind::Map(c) => {
                        return Ok(c
                            .borrow()
                            .items()
                            .map(|(k, v)| self.array(vec![k.clone(), v.clone()]))
                            .collect());
                    }
                    _ => {}
                }
                let it = self.get(v, ITERATOR)?;
                if it.as_obj().is_some_and(|o| o.is_callable()) {
                    let iter = self.call(&it, v.clone(), &[])?;
                    if let Some(Kind::Generator(rest)) = iter.as_obj().map(|o| &o.kind) {
                        return Ok(rest.borrow_mut().by_ref().collect());
                    }
                    let next = self.get(&iter, "next")?;
                    let mut out = Vec::new();
                    loop {
                        let r = self.call(&next, iter.clone(), &[])?;
                        if self.get(&r, "done")?.truthy() {
                            return Ok(out);
                        }
                        out.push(self.get(&r, "value")?);
                    }
                }
                let n = self.to_num(&self.get(v, "length")?)?;
                let n = as_index(n).unwrap_or(0);
                (0..n)
                    .map(|i| self.get_index(v, &Value::Num(i as f64)))
                    .collect()
            }
            Value::Str(s) => Ok(s
                .chars()
                .map(|c| Value::Str(c.to_string().into()))
                .collect()),
            _ => self.type_error(&format!("{} is not iterable", self.to_str(v)?)),
        }
    }
}

fn set_item(items: &RefCell<Vec<Value>>, i: usize, value: Value) {
    let mut v = items.borrow_mut();
    if i < v.len() {
        v[i] = value;
    } else {
        v.resize(i, Value::Hole);
        v.push(value);
    }
}

/// Length in UTF-16 units, as JavaScript counts.
pub fn str_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.encode_utf16().count()
    }
}

pub fn char_at(s: &str, i: usize) -> Option<Name> {
    if s.is_ascii() {
        return s.get(i..i + 1).map(Into::into);
    }
    let u = s.encode_utf16().nth(i)?;
    Some(String::from_utf16_lossy(&[u]).into())
}
