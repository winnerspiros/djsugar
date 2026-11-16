//! Map, Set, WeakMap, WeakSet, ArrayBuffer, the typed arrays and DataView.

use std::cell::RefCell;
use std::rc::Rc;

use crate::value::*;

#[derive(Hash, PartialEq, Eq)]
enum Key {
    Undefined,
    Null,
    Bool(bool),
    Num(u64),
    Str(Name),
    Obj(*const Obj),
}

fn key(v: &Value) -> Key {
    match v {
        Value::Undefined | Value::Hole => Key::Undefined,
        Value::Null => Key::Null,
        Value::Bool(b) => Key::Bool(*b),
        // SameValueZero: -0 is 0 and every NaN is one key.
        Value::Num(n) if *n == 0.0 => Key::Num(0),
        Value::Num(n) if n.is_nan() => Key::Num(f64::NAN.to_bits()),
        Value::Num(n) => Key::Num(n.to_bits()),
        Value::Str(s) => Key::Str(s.clone()),
        Value::Obj(o) => Key::Obj(Rc::as_ptr(o)),
    }
}

/// Insertion-ordered entries; removed ones leave a hole so that iteration
/// in progress keeps its place.
#[derive(Default)]
pub struct Coll {
    entries: Vec<Option<(Value, Value)>>,
    index: HashMap<Key, usize>,
    live: usize,
}

impl Coll {
    fn get(&self, k: &Value) -> Option<&Value> {
        self.index
            .get(&key(k))
            .and_then(|&i| self.entries[i].as_ref())
            .map(|(_, v)| v)
    }
    fn set(&mut self, k: Value, v: Value) {
        let k = if matches!(k, Value::Num(n) if n == 0.0) {
            Value::Num(0.0)
        } else {
            k
        };
        match self.index.get(&key(&k)) {
            Some(&i) => self.entries[i] = Some((k, v)),
            None => {
                self.index.insert(key(&k), self.entries.len());
                self.entries.push(Some((k, v)));
                self.live += 1;
            }
        }
    }
    fn delete(&mut self, k: &Value) -> bool {
        match self.index.remove(&key(k)) {
            Some(i) => {
                self.entries[i] = None;
                self.live -= 1;
                if self.entries.len() > 32 && self.live * 2 < self.entries.len() {
                    self.entries.retain(Option::is_some);
                    self.index = self
                        .entries
                        .iter()
                        .enumerate()
                        .map(|(i, e)| (key(&e.as_ref().unwrap().0), i))
                        .collect();
                }
                true
            }
            None => false,
        }
    }
    pub fn items(&self) -> impl Iterator<Item = &(Value, Value)> {
        self.entries.iter().flatten()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Elem {
    U8,
    I8,
    U8C,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
}

impl Elem {
    pub fn size(self) -> usize {
        match self {
            Elem::U8 | Elem::I8 | Elem::U8C => 1,
            Elem::U16 | Elem::I16 => 2,
            Elem::U32 | Elem::I32 | Elem::F32 => 4,
            Elem::F64 => 8,
        }
    }

    pub fn read(self, b: &[u8], little: bool) -> f64 {
        macro_rules! rd {
            ($t:ty, $n:expr) => {{
                let a: [u8; $n] = b[..$n].try_into().unwrap();
                (if little {
                    <$t>::from_le_bytes(a)
                } else {
                    <$t>::from_be_bytes(a)
                }) as f64
            }};
        }
        match self {
            Elem::U8 | Elem::U8C => b[0] as f64,
            Elem::I8 => b[0] as i8 as f64,
            Elem::U16 => rd!(u16, 2),
            Elem::I16 => rd!(i16, 2),
            Elem::U32 => rd!(u32, 4),
            Elem::I32 => rd!(i32, 4),
            Elem::F32 => rd!(f32, 4),
            Elem::F64 => rd!(f64, 8),
        }
    }

    pub fn write(self, b: &mut [u8], v: f64, little: bool) {
        macro_rules! wr {
            ($x:expr) => {{
                let x = $x;
                let a = if little {
                    x.to_le_bytes()
                } else {
                    x.to_be_bytes()
                };
                b[..a.len()].copy_from_slice(&a);
            }};
        }
        let int = to_int32(v);
        match self {
            Elem::U8 | Elem::I8 => b[0] = int as u8,
            Elem::U8C => {
                b[0] = if v.is_nan() {
                    0
                } else {
                    v.clamp(0.0, 255.0).round_ties_even() as u8
                }
            }
            Elem::U16 | Elem::I16 => wr!(int as u16),
            Elem::U32 | Elem::I32 => wr!(int as u32),
            Elem::F32 => wr!(v as f32),
            Elem::F64 => wr!(v),
        }
    }
}

pub type Bytes = Rc<RefCell<Vec<u8>>>;

pub struct View {
    pub buf: Bytes,
    pub off: usize,
    pub len: usize,
    pub elem: Elem,
}

impl View {
    pub fn get(&self, i: usize) -> Option<f64> {
        (i < self.len).then(|| {
            let s = self.elem.size();
            let b = self.buf.borrow();
            let st = self.off + i * s;
            b.get(st..st + s)
                .map_or(f64::NAN, |x| self.elem.read(x, true))
        })
    }
    pub fn set(&self, i: usize, v: f64) {
        if i < self.len {
            let s = self.elem.size();
            let st = self.off + i * s;
            if let Some(x) = self.buf.borrow_mut().get_mut(st..st + s) {
                self.elem.write(x, v, true);
            }
        }
    }
}

fn arg(a: &[Value], i: usize) -> Value {
    a.get(i).cloned().unwrap_or_default()
}

fn method(
    rt: &Rt,
    o: &Rc<Obj>,
    name: &str,
    f: impl Fn(&Rt, &Value, &[Value]) -> R<Value> + 'static,
) {
    let v = rt.native(name, f);
    o.props.borrow_mut().set(&name.into(), Prop::Method(v));
}

fn getter(
    rt: &Rt,
    o: &Rc<Obj>,
    name: &str,
    f: impl Fn(&Rt, &Value, &[Value]) -> R<Value> + 'static,
) {
    let v = rt.native(name, f);
    o.props
        .borrow_mut()
        .set(&name.into(), Prop::Accessor(Some(v), None));
}

fn class(rt: &Rt, name: &str, f: impl Fn(&Rt, &Value, &[Value]) -> R<Value> + 'static) -> Rc<Obj> {
    let proto = rt.object();
    let c = rt.native(name, f);
    let co = c.as_obj().unwrap();
    co.props
        .borrow_mut()
        .set(&rt.prototype, Prop::Method(Value::Obj(proto.clone())));
    proto
        .props
        .borrow_mut()
        .set(&rt.constructor, Prop::Method(c.clone()));
    rt.global
        .props
        .borrow_mut()
        .set(&name.into(), Prop::Method(c));
    proto
}

/// The object `new` made, given its new kind (natives get a plain one).
fn rekind(rt: &Rt, this: &Value, kind: Kind, proto: &Rc<Obj>) -> R<Value> {
    let p = match this {
        Value::Obj(o) if !Rc::ptr_eq(o, &rt.global) => o.proto.borrow().clone(),
        _ => return rt.type_error("constructor needs `new`"),
    };
    Ok(Value::Obj(Obj::new(
        p.or_else(|| Some(proto.clone())),
        kind,
    )))
}

fn coll<'a>(rt: &Rt, this: &'a Value, set: bool) -> R<&'a RefCell<Coll>> {
    match this.as_obj().map(|o| &o.kind) {
        Some(Kind::Map(c)) if !set => Ok(c),
        Some(Kind::Set(c)) if set => Ok(c),
        _ => rt.type_error(if set { "not a Set" } else { "not a Map" }),
    }
}

pub fn install(rt: &Rt) {
    for (name, set, weak) in [
        ("Map", false, false),
        ("WeakMap", false, true),
        ("Set", true, false),
        ("WeakSet", true, true),
    ] {
        let proto = Rc::new(RefCell::new(None::<Rc<Obj>>));
        let pr = proto.clone();
        let p = class(rt, name, move |rt, this, a| {
            let kind = if set {
                Kind::Set(RefCell::new(Coll::default()))
            } else {
                Kind::Map(RefCell::new(Coll::default()))
            };
            let o = rekind(rt, this, kind, pr.borrow().as_ref().unwrap())?;
            let init = arg(a, 0);
            if !init.is_nullish() {
                let c = coll(rt, &o, set)?;
                for item in rt.list(&init)? {
                    if set {
                        c.borrow_mut().set(item.clone(), item);
                    } else {
                        let k = rt.get_index(&item, &Value::Num(0.0))?;
                        let v = rt.get_index(&item, &Value::Num(1.0))?;
                        c.borrow_mut().set(k, v);
                    }
                }
            }
            Ok(o)
        });
        *proto.borrow_mut() = Some(p.clone());
        method(rt, &p, "has", move |rt, this, a| {
            Ok(Value::Bool(
                coll(rt, this, set)?.borrow().get(&arg(a, 0)).is_some(),
            ))
        });
        method(rt, &p, "delete", move |rt, this, a| {
            Ok(Value::Bool(
                coll(rt, this, set)?.borrow_mut().delete(&arg(a, 0)),
            ))
        });
        if set {
            method(rt, &p, "add", move |rt, this, a| {
                coll(rt, this, true)?.borrow_mut().set(arg(a, 0), arg(a, 0));
                Ok(this.clone())
            });
        } else {
            method(rt, &p, "get", move |rt, this, a| {
                Ok(coll(rt, this, false)?
                    .borrow()
                    .get(&arg(a, 0))
                    .cloned()
                    .unwrap_or_default())
            });
            method(rt, &p, "set", move |rt, this, a| {
                coll(rt, this, false)?
                    .borrow_mut()
                    .set(arg(a, 0), arg(a, 1));
                Ok(this.clone())
            });
        }
        if weak {
            continue;
        }
        method(rt, &p, "clear", move |rt, this, _| {
            *coll(rt, this, set)?.borrow_mut() = Coll::default();
            Ok(Value::Undefined)
        });
        getter(rt, &p, "size", move |rt, this, _| {
            Ok(Value::Num(coll(rt, this, set)?.borrow().live as f64))
        });
        method(rt, &p, "forEach", move |rt, this, a| {
            let f = arg(a, 0);
            let c = coll(rt, this, set)?;
            // Entries added during iteration are visited too.
            let mut i = 0;
            loop {
                let e = c.borrow().entries.get(i).cloned();
                let Some(e) = e else { break };
                if let Some((k, v)) = e {
                    rt.call(&f, arg(a, 1), &[v, k, this.clone()])?;
                }
                i += 1;
            }
            Ok(Value::Undefined)
        });
        for (m, what) in [("keys", 0), ("values", 1), ("entries", 2)] {
            method(rt, &p, m, move |rt, this, _| {
                let c = coll(rt, this, set)?;
                let items: Vec<Value> = c
                    .borrow()
                    .items()
                    .map(|(k, v)| match what {
                        0 => k.clone(),
                        1 => v.clone(),
                        _ => rt.array(vec![k.clone(), v.clone()]),
                    })
                    .collect();
                Ok(rt.array(items))
            });
        }
    }

    let abp = Rc::new(RefCell::new(None::<Rc<Obj>>));
    let pr = abp.clone();
    let ab = class(rt, "ArrayBuffer", move |rt, this, a| {
        let n = rt.to_num(&arg(a, 0))?;
        let Some(n) = as_index(if n.is_nan() { 0.0 } else { n }) else {
            let p = rt.range_error_proto.clone();
            return rt.throw(&p, "invalid ArrayBuffer length");
        };
        rekind(
            rt,
            this,
            Kind::Buffer(Rc::new(RefCell::new(vec![0; n]))),
            pr.borrow().as_ref().unwrap(),
        )
    });
    *abp.borrow_mut() = Some(ab.clone());
    if let Some(Prop::Method(c)) = ab.props.borrow().get("constructor").cloned() {
        method(rt, c.as_obj().unwrap(), "isView", |_, _, a| {
            Ok(Value::Bool(matches!(
                arg(a, 0).as_obj().map(|o| &o.kind),
                Some(Kind::Typed(_))
            )))
        });
    }
    getter(rt, &ab, "byteLength", |rt, this, _| {
        match this.as_obj().map(|o| &o.kind) {
            Some(Kind::Buffer(b)) => Ok(Value::Num(b.borrow().len() as f64)),
            _ => rt.type_error("not an ArrayBuffer"),
        }
    });
    method(rt, &ab, "slice", |rt, this, a| {
        let Some(Kind::Buffer(b)) = this.as_obj().map(|o| &o.kind) else {
            return rt.type_error("not an ArrayBuffer");
        };
        let len = b.borrow().len();
        let s = rel(rt, &arg(a, 0), len, 0)?;
        let e = rel(rt, &arg(a, 1), len, len)?;
        let bytes = if s < e {
            b.borrow()[s..e].to_vec()
        } else {
            Vec::new()
        };
        let o = Obj::new(
            this.as_obj().unwrap().proto.borrow().clone(),
            Kind::Buffer(Rc::new(RefCell::new(bytes))),
        );
        Ok(Value::Obj(o))
    });
    let abp2 = abp.clone();

    for (name, elem) in [
        ("Uint8Array", Elem::U8),
        ("Int8Array", Elem::I8),
        ("Uint8ClampedArray", Elem::U8C),
        ("Uint16Array", Elem::U16),
        ("Int16Array", Elem::I16),
        ("Uint32Array", Elem::U32),
        ("Int32Array", Elem::I32),
        ("Float32Array", Elem::F32),
        ("Float64Array", Elem::F64),
    ] {
        let tp = Rc::new(RefCell::new(None::<Rc<Obj>>));
        let pr = tp.clone();
        let p = class(rt, name, move |rt, this, a| {
            let s = elem.size();
            let view = match arg(a, 0) {
                Value::Obj(o) if matches!(o.kind, Kind::Buffer(_)) => {
                    let Kind::Buffer(b) = &o.kind else {
                        unreachable!()
                    };
                    let total = b.borrow().len();
                    let off = match arg(a, 1) {
                        Value::Undefined => 0,
                        v => rt.to_num(&v)? as usize,
                    };
                    let len = match arg(a, 2) {
                        Value::Undefined => (total.saturating_sub(off)) / s,
                        v => rt.to_num(&v)? as usize,
                    };
                    if off % s != 0 || off + len * s > total {
                        let p = rt.range_error_proto.clone();
                        return rt.throw(&p, "typed array out of the buffer's bounds");
                    }
                    View {
                        buf: b.clone(),
                        off,
                        len,
                        elem,
                    }
                }
                Value::Obj(_) => {
                    let items = rt.list(&arg(a, 0))?;
                    let v = View {
                        buf: Rc::new(RefCell::new(vec![0; items.len() * s])),
                        off: 0,
                        len: items.len(),
                        elem,
                    };
                    for (i, x) in items.iter().enumerate() {
                        v.set(i, rt.to_num(x)?);
                    }
                    v
                }
                n => {
                    let n = if matches!(n, Value::Undefined) {
                        0.0
                    } else {
                        rt.to_num(&n)?
                    };
                    let Some(n) = as_index(n) else {
                        let p = rt.range_error_proto.clone();
                        return rt.throw(&p, "invalid typed array length");
                    };
                    View {
                        buf: Rc::new(RefCell::new(vec![0; n * s])),
                        off: 0,
                        len: n,
                        elem,
                    }
                }
            };
            rekind(rt, this, Kind::Typed(view), pr.borrow().as_ref().unwrap())
        });
        *tp.borrow_mut() = Some(p.clone());
        p.props.borrow_mut().set(
            &"BYTES_PER_ELEMENT".into(),
            Prop::Method(Value::Num(elem.size() as f64)),
        );
        let tpc = tp.clone();
        let make = move |rt: &Rt, this: &Value, vals: Vec<f64>| -> Value {
            let v = View {
                buf: Rc::new(RefCell::new(vec![0; vals.len() * elem.size()])),
                off: 0,
                len: vals.len(),
                elem,
            };
            for (i, x) in vals.into_iter().enumerate() {
                v.set(i, x);
            }
            let proto = this
                .as_obj()
                .and_then(|o| o.proto.borrow().clone())
                .or_else(|| tpc.borrow().clone());
            let _ = rt;
            Value::Obj(Obj::new(proto, Kind::Typed(v)))
        };
        let make = Rc::new(make);
        typed_methods(rt, &p, make, abp2.clone());
    }

    let dvp = Rc::new(RefCell::new(None::<Rc<Obj>>));
    let pr = dvp.clone();
    let dv = class(rt, "DataView", move |rt, this, a| {
        let src = arg(a, 0);
        let Some(Kind::Buffer(b)) = src.as_obj().map(|o| &o.kind) else {
            return rt.type_error("DataView needs an ArrayBuffer");
        };
        let total = b.borrow().len();
        let off = match arg(a, 1) {
            Value::Undefined => 0,
            v => rt.to_num(&v)? as usize,
        };
        let len = match arg(a, 2) {
            Value::Undefined => total.saturating_sub(off),
            v => rt.to_num(&v)? as usize,
        };
        if off + len > total {
            let p = rt.range_error_proto.clone();
            return rt.throw(&p, "DataView out of the buffer's bounds");
        }
        rekind(
            rt,
            this,
            Kind::Typed(View {
                buf: b.clone(),
                off,
                len,
                elem: Elem::U8,
            }),
            pr.borrow().as_ref().unwrap(),
        )
    });
    *dvp.borrow_mut() = Some(dv.clone());
    for (suffix, elem) in [
        ("Uint8", Elem::U8),
        ("Int8", Elem::I8),
        ("Uint16", Elem::U16),
        ("Int16", Elem::I16),
        ("Uint32", Elem::U32),
        ("Int32", Elem::I32),
        ("Float32", Elem::F32),
        ("Float64", Elem::F64),
    ] {
        let at = move |rt: &Rt, this: &Value, a: &[Value]| -> R<(Bytes, usize)> {
            let Some(Kind::Typed(v)) = this.as_obj().map(|o| &o.kind) else {
                return rt.type_error("not a DataView");
            };
            let i = rt.to_num(&arg(a, 0))?;
            if !(0.0..=(v.len.saturating_sub(elem.size())) as f64).contains(&i)
                || v.len < elem.size()
            {
                let p = rt.range_error_proto.clone();
                return rt.throw(&p, "offset is outside the bounds of the DataView");
            }
            Ok((v.buf.clone(), v.off + i as usize))
        };
        method(rt, &dv, &format!("get{suffix}"), move |rt, this, a| {
            let (b, st) = at(rt, this, a)?;
            let little = arg(a, 1).truthy();
            let x = elem.read(&b.borrow()[st..st + elem.size()], little);
            Ok(Value::Num(x))
        });
        method(rt, &dv, &format!("set{suffix}"), move |rt, this, a| {
            let (b, st) = at(rt, this, a)?;
            let v = rt.to_num(&arg(a, 1))?;
            let little = arg(a, 2).truthy();
            elem.write(&mut b.borrow_mut()[st..st + elem.size()], v, little);
            Ok(Value::Undefined)
        });
    }
    getter(rt, &dv, "byteLength", |rt, this, _| {
        match this.as_obj().map(|o| &o.kind) {
            Some(Kind::Typed(v)) => Ok(Value::Num(v.len as f64)),
            _ => rt.type_error("not a DataView"),
        }
    });
}

fn rel(rt: &Rt, v: &Value, len: usize, default: usize) -> R<usize> {
    if matches!(v, Value::Undefined) {
        return Ok(default);
    }
    let n = rt.to_num(v)?;
    let n = if n.is_nan() { 0.0 } else { n.trunc() };
    Ok(if n < 0.0 {
        (len as f64 + n).max(0.0) as usize
    } else {
        n.min(len as f64) as usize
    })
}

type Make = Rc<dyn Fn(&Rt, &Value, Vec<f64>) -> Value>;

fn view(rt: &Rt, this: &Value) -> R<Rc<Obj>> {
    match this {
        Value::Obj(o) if matches!(o.kind, Kind::Typed(_)) => Ok(o.clone()),
        _ => rt.type_error("not a typed array"),
    }
}

fn values(o: &Rc<Obj>) -> Vec<f64> {
    let Kind::Typed(v) = &o.kind else {
        unreachable!()
    };
    (0..v.len).map(|i| v.get(i).unwrap()).collect()
}

fn typed_methods(rt: &Rt, p: &Rc<Obj>, make: Make, abp: Rc<RefCell<Option<Rc<Obj>>>>) {
    getter(rt, p, "length", |rt, this, _| match &view(rt, this)?.kind {
        Kind::Typed(v) => Ok(Value::Num(v.len as f64)),
        _ => unreachable!(),
    });
    getter(rt, p, "byteLength", |rt, this, _| {
        match &view(rt, this)?.kind {
            Kind::Typed(v) => Ok(Value::Num((v.len * v.elem.size()) as f64)),
            _ => unreachable!(),
        }
    });
    getter(rt, p, "byteOffset", |rt, this, _| {
        match &view(rt, this)?.kind {
            Kind::Typed(v) => Ok(Value::Num(v.off as f64)),
            _ => unreachable!(),
        }
    });
    getter(rt, p, "buffer", move |rt, this, _| {
        match &view(rt, this)?.kind {
            Kind::Typed(v) => Ok(Value::Obj(Obj::new(
                abp.borrow().clone(),
                Kind::Buffer(v.buf.clone()),
            ))),
            _ => unreachable!(),
        }
    });
    method(rt, p, "set", |rt, this, a| {
        let o = view(rt, this)?;
        let Kind::Typed(v) = &o.kind else {
            unreachable!()
        };
        let src = rt.list(&arg(a, 0))?;
        let off = match arg(a, 1) {
            Value::Undefined => 0,
            x => rt.to_num(&x)? as usize,
        };
        if off + src.len() > v.len {
            let p = rt.range_error_proto.clone();
            return rt.throw(&p, "source is too large");
        }
        let nums = src.iter().map(|x| rt.to_num(x)).collect::<R<Vec<_>>>()?;
        for (i, x) in nums.into_iter().enumerate() {
            v.set(off + i, x);
        }
        Ok(Value::Undefined)
    });
    method(rt, p, "subarray", |rt, this, a| {
        let o = view(rt, this)?;
        let Kind::Typed(v) = &o.kind else {
            unreachable!()
        };
        let s = rel(rt, &arg(a, 0), v.len, 0)?;
        let e = rel(rt, &arg(a, 1), v.len, v.len)?.max(s);
        let nv = View {
            buf: v.buf.clone(),
            off: v.off + s * v.elem.size(),
            len: e - s,
            elem: v.elem,
        };
        let proto = o.proto.borrow().clone();
        Ok(Value::Obj(Obj::new(proto, Kind::Typed(nv))))
    });
    let mk = make.clone();
    method(rt, p, "slice", move |rt, this, a| {
        let o = view(rt, this)?;
        let vals = values(&o);
        let s = rel(rt, &arg(a, 0), vals.len(), 0)?;
        let e = rel(rt, &arg(a, 1), vals.len(), vals.len())?.max(s);
        Ok(mk(rt, this, vals[s..e].to_vec()))
    });
    method(rt, p, "fill", |rt, this, a| {
        let o = view(rt, this)?;
        let Kind::Typed(v) = &o.kind else {
            unreachable!()
        };
        let x = rt.to_num(&arg(a, 0))?;
        let s = rel(rt, &arg(a, 1), v.len, 0)?;
        let e = rel(rt, &arg(a, 2), v.len, v.len)?;
        for i in s..e {
            v.set(i, x);
        }
        Ok(this.clone())
    });
    let mk = make.clone();
    method(rt, p, "map", move |rt, this, a| {
        let o = view(rt, this)?;
        let f = arg(a, 0);
        let mut out = Vec::new();
        for (i, x) in values(&o).into_iter().enumerate() {
            out.push(rt.to_num(&rt.call(
                &f,
                arg(a, 1),
                &[Value::Num(x), Value::Num(i as f64), this.clone()],
            )?)?);
        }
        Ok(mk(rt, this, out))
    });
    // The rest behave as on a plain array of the values.
    for m in [
        "forEach",
        "indexOf",
        "includes",
        "join",
        "reduce",
        "some",
        "every",
        "find",
        "findIndex",
        "lastIndexOf",
        "toString",
    ] {
        method(rt, p, m, move |rt, this, a| {
            let o = view(rt, this)?;
            let arr = rt.array(values(&o).into_iter().map(Value::Num).collect());
            let f = rt.get(&arr, m)?;
            rt.call(&f, arr, a)
        });
    }
    let mk = make;
    method(rt, p, "filter", move |rt, this, a| {
        let o = view(rt, this)?;
        let f = arg(a, 0);
        let mut out = Vec::new();
        for (i, x) in values(&o).into_iter().enumerate() {
            if rt
                .call(
                    &f,
                    arg(a, 1),
                    &[Value::Num(x), Value::Num(i as f64), this.clone()],
                )?
                .truthy()
            {
                out.push(x);
            }
        }
        Ok(mk(rt, this, out))
    });
    method(rt, p, "reverse", |rt, this, _| {
        let o = view(rt, this)?;
        let Kind::Typed(v) = &o.kind else {
            unreachable!()
        };
        let vals = values(&o);
        for (i, x) in vals.into_iter().rev().enumerate() {
            v.set(i, x);
        }
        Ok(this.clone())
    });
}
