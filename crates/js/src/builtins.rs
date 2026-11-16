//! The standard objects mapping scripts use.

use std::cell::RefCell;
use std::rc::Rc;

use crate::number_to_string;
use crate::run::pow;
use crate::value::*;

type NativeBox = Box<dyn Fn(&Rt, &Value, &[Value]) -> R<Value>>;
type MathFn = (&'static str, fn(f64) -> f64);

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

fn items(rt: &Rt, this: &Value) -> R<Rc<Obj>> {
    match this {
        Value::Obj(o) if matches!(o.kind, Kind::Array(_)) => Ok(o.clone()),
        _ => rt.type_error("not an array"),
    }
}

fn array_of(o: &Rc<Obj>) -> &RefCell<Vec<Value>> {
    match &o.kind {
        Kind::Array(v) => v,
        _ => unreachable!(),
    }
}

fn this_str(rt: &Rt, this: &Value) -> R<Rc<str>> {
    if this.is_nullish() {
        return rt.type_error("string method called on undefined or null");
    }
    rt.to_str(this)
}

/// A relative index (negative from the end) clamped to `0..=len`.
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

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn from16(u: &[u16]) -> Value {
    Value::Str(String::from_utf16_lossy(u).into())
}

fn callback(rt: &Rt, f: &Value) -> R<()> {
    if f.as_obj().is_some_and(|o| o.is_callable()) {
        Ok(())
    } else {
        rt.type_error("callback is not a function")
    }
}

pub fn realm() -> Rt {
    let object_proto = Obj::new(None, Kind::Plain);
    let function_proto = Obj::new(Some(object_proto.clone()), Kind::Plain);
    let p = |k| Obj::new(Some(object_proto.clone()), k);
    let error_proto = p(Kind::Plain);
    let sub = || Obj::new(Some(error_proto.clone()), Kind::Plain);
    let rt = Rt {
        global: p(Kind::Plain),
        array_proto: p(Kind::Array(RefCell::new(Vec::new()))),
        string_proto: p(Kind::Plain),
        regexp_proto: p(Kind::Plain),
        generator_proto: p(Kind::Plain),
        number_proto: p(Kind::Plain),
        boolean_proto: p(Kind::Plain),
        type_error_proto: sub(),
        range_error_proto: sub(),
        reference_error_proto: sub(),
        error_proto,
        object_proto,
        function_proto,
        length: "length".into(),
        prototype: "prototype".into(),
        constructor: "constructor".into(),
        log: RefCell::new(Box::new(|s| eprintln!("{s}"))),
    };
    let g = rt.global.clone();
    let put = |name: &str, v: Value| g.props.borrow_mut().set(&name.into(), Prop::Method(v));
    put("globalThis", Value::Obj(g.clone()));
    put("NaN", Value::Num(f64::NAN));
    put("Infinity", Value::Num(f64::INFINITY));

    let ctor = |name: &str, proto: &Rc<Obj>, f: NativeBox| {
        let c = rt.native(name, f);
        let co = c.as_obj().unwrap();
        co.props
            .borrow_mut()
            .set(&rt.prototype, Prop::Method(Value::Obj(proto.clone())));
        proto
            .props
            .borrow_mut()
            .set(&rt.constructor, Prop::Method(c.clone()));
        put(name, c.clone());
        c
    };

    // Object
    let object = ctor(
        "Object",
        &rt.object_proto,
        Box::new(|rt, _, a| {
            Ok(match arg(a, 0) {
                v @ Value::Obj(_) => v,
                _ => Value::Obj(rt.object()),
            })
        }),
    );
    let oc = object.as_obj().unwrap();
    method(&rt, oc, "keys", |rt, _, a| match arg(a, 0) {
        Value::Obj(o) => Ok(rt.array(rt.own_keys(&o).into_iter().map(Value::Str).collect())),
        Value::Str(s) => Ok(rt.array(
            (0..str_len(&s))
                .map(|i| Value::str(&i.to_string()))
                .collect(),
        )),
        _ => Ok(rt.array(Vec::new())),
    });
    method(&rt, oc, "values", |rt, _, a| {
        let v = arg(a, 0);
        let Value::Obj(o) = &v else {
            return Ok(rt.array(Vec::new()));
        };
        let vals = rt
            .own_keys(o)
            .iter()
            .map(|k| rt.get(&v, k))
            .collect::<R<_>>()?;
        Ok(rt.array(vals))
    });
    method(&rt, oc, "entries", |rt, _, a| {
        let v = arg(a, 0);
        let Value::Obj(o) = &v else {
            return Ok(rt.array(Vec::new()));
        };
        let mut out = Vec::new();
        for k in rt.own_keys(o) {
            let x = rt.get(&v, &k)?;
            out.push(rt.array(vec![Value::Str(k), x]));
        }
        Ok(rt.array(out))
    });
    method(&rt, oc, "assign", |rt, _, a| {
        let t = arg(a, 0);
        for s in a.iter().skip(1) {
            if let Value::Obj(o) = s {
                for k in rt.own_keys(o) {
                    rt.set(&t, &k, rt.get(s, &k)?)?;
                }
            }
        }
        Ok(t)
    });
    for n in ["freeze", "seal", "preventExtensions"] {
        method(&rt, oc, n, |_, _, a| Ok(arg(a, 0)));
    }
    for n in ["isFrozen", "isSealed"] {
        method(&rt, oc, n, |_, _, _| Ok(Value::Bool(false)));
    }
    method(&rt, oc, "create", |rt, _, a| {
        let proto = match arg(a, 0) {
            Value::Obj(o) => Some(o),
            Value::Null => None,
            _ => return rt.type_error("Object.create needs an object or null"),
        };
        let o = Obj::new(proto, Kind::Plain);
        if let Value::Obj(props) = arg(a, 1) {
            define_properties(rt, &o, &props)?;
        }
        Ok(Value::Obj(o))
    });
    method(&rt, oc, "getPrototypeOf", |rt, _, a| {
        Ok(rt.proto_of(&arg(a, 0)).map_or(Value::Null, Value::Obj))
    });
    method(&rt, oc, "setPrototypeOf", |_, _, a| {
        let o = arg(a, 0);
        if let Value::Obj(x) = &o {
            *x.proto.borrow_mut() = arg(a, 1).as_obj().cloned();
        }
        Ok(o)
    });
    method(&rt, oc, "defineProperty", |rt, _, a| {
        let o = arg(a, 0);
        let Value::Obj(x) = &o else {
            return rt.type_error("Object.defineProperty on a non-object");
        };
        let k = rt.to_key(&arg(a, 1))?;
        let Value::Obj(d) = arg(a, 2) else {
            return rt.type_error("property description must be an object");
        };
        define_property(rt, x, &k, &d)?;
        Ok(o)
    });
    method(&rt, oc, "defineProperties", |rt, _, a| {
        let o = arg(a, 0);
        if let (Value::Obj(x), Value::Obj(d)) = (&o, arg(a, 1)) {
            define_properties(rt, x, &d)?;
        }
        Ok(o)
    });
    method(&rt, oc, "getOwnPropertyNames", |rt, _, a| match arg(a, 0) {
        Value::Obj(o) => {
            let mut ks = rt.own_keys(&o);
            for (k, p) in o.props.borrow().entries() {
                if matches!(p, Prop::Method(_)) {
                    ks.push(k.clone());
                }
            }
            Ok(rt.array(ks.into_iter().map(Value::Str).collect()))
        }
        _ => Ok(rt.array(Vec::new())),
    });
    method(&rt, oc, "is", |_, _, a| {
        let (x, y) = (arg(a, 0), arg(a, 1));
        Ok(Value::Bool(match (&x, &y) {
            (Value::Num(p), Value::Num(q)) => {
                (p.is_nan() && q.is_nan())
                    || (p == q && p.is_sign_negative() == q.is_sign_negative())
            }
            _ => x.strict_eq(&y),
        }))
    });
    let op = rt.object_proto.clone();
    method(&rt, &op, "hasOwnProperty", |rt, this, a| {
        let k = rt.to_key(&arg(a, 0))?;
        Ok(Value::Bool(match this {
            Value::Obj(o) => {
                o.props.borrow().get(&k).is_some()
                    || matches!(&o.kind, Kind::Array(v) if &*k == "length" || index_of_name(&k).is_some_and(|i| i < v.borrow().len()))
            }
            Value::Str(s) => &*k == "length" || index_of_name(&k).is_some_and(|i| i < str_len(s)),
            _ => false,
        }))
    });
    method(&rt, &op, "isPrototypeOf", |_, this, a| {
        let (Value::Obj(p), Value::Obj(o)) = (this, arg(a, 0)) else {
            return Ok(Value::Bool(false));
        };
        let mut x = o.proto.borrow().clone();
        while let Some(y) = x {
            if Rc::ptr_eq(&y, p) {
                return Ok(Value::Bool(true));
            }
            x = y.proto.borrow().clone();
        }
        Ok(Value::Bool(false))
    });
    method(&rt, &op, "propertyIsEnumerable", |rt, this, a| {
        let k = rt.to_key(&arg(a, 0))?;
        Ok(Value::Bool(
            this.as_obj().is_some_and(|o| rt.own_keys(o).contains(&k)),
        ))
    });
    method(&rt, &op, "toString", |_, this, _| {
        Ok(Value::str(match this {
            Value::Undefined => "[object Undefined]",
            Value::Null => "[object Null]",
            Value::Obj(o) if matches!(o.kind, Kind::Array(_)) => "[object Array]",
            Value::Obj(o) if o.is_callable() => "[object Function]",
            _ => "[object Object]",
        }))
    });
    method(&rt, &op, "valueOf", |_, this, _| Ok(this.clone()));

    // Function
    ctor(
        "Function",
        &rt.function_proto,
        Box::new(|rt, _, a| {
            let body = match a.last() {
                Some(b) => rt.to_str(b)?,
                None => "".into(),
            };
            let params = a[..a.len().saturating_sub(1)]
                .iter()
                .map(|p| rt.to_str(p).map(|p| p.to_string()))
                .collect::<R<Vec<_>>>()?;
            crate::run::compile_function(rt, &params.join(","), &body)
        }),
    );
    crate::date::install(&rt);
    crate::coll::install(&rt);
    iteration(&rt, &put);
    put(
        "Proxy",
        rt.native("Proxy", |rt, this, a| {
            let (Value::Obj(_), Value::Obj(_)) = (arg(a, 0), arg(a, 1)) else {
                return rt.type_error("Proxy needs a target and a handler object");
            };
            if !matches!(this, Value::Obj(o) if !Rc::ptr_eq(o, &rt.global)) {
                return rt.type_error("Proxy needs `new`");
            }
            let proto = arg(a, 0).as_obj().unwrap().proto.borrow().clone();
            Ok(Value::Obj(Obj::new(
                proto,
                Kind::Proxy(arg(a, 0), arg(a, 1)),
            )))
        }),
    );
    put(
        "eval",
        rt.native("eval", |rt, _, a| match a.first() {
            Some(Value::Str(src)) => crate::run::eval_global(rt, src),
            Some(v) => Ok(v.clone()),
            None => Ok(Value::Undefined),
        }),
    );
    let fp = rt.function_proto.clone();
    method(&rt, &fp, "call", |rt, this, a| {
        rt.call(this, arg(a, 0), a.get(1..).unwrap_or(&[]))
    });
    method(&rt, &fp, "apply", |rt, this, a| {
        let args = match arg(a, 1) {
            v if v.is_nullish() => Vec::new(),
            v => rt.list(&v)?,
        };
        rt.call(this, arg(a, 0), &args)
    });
    method(&rt, &fp, "bind", |rt, this, a| {
        if !this.as_obj().is_some_and(|o| o.is_callable()) {
            return rt.type_error("bind on a non-function");
        }
        Ok(Value::Obj(Obj::new(
            Some(rt.function_proto.clone()),
            Kind::Bound(this.clone(), arg(a, 0), a.get(1..).unwrap_or(&[]).to_vec()),
        )))
    });
    method(&rt, &fp, "toString", |_, _, _| {
        Ok(Value::str("function () { [code] }"))
    });

    // Errors
    for (name, proto) in [
        ("Error", rt.error_proto.clone()),
        ("TypeError", rt.type_error_proto.clone()),
        ("RangeError", rt.range_error_proto.clone()),
        ("ReferenceError", rt.reference_error_proto.clone()),
        (
            "SyntaxError",
            Obj::new(Some(rt.error_proto.clone()), Kind::Plain),
        ),
    ] {
        let pc = proto.clone();
        ctor(
            name,
            &proto,
            Box::new(move |rt, this, a| {
                let o = match this {
                    Value::Obj(o) if !Rc::ptr_eq(o, &rt.global) => o.clone(),
                    _ => Obj::new(Some(pc.clone()), Kind::Plain),
                };
                if let Some(m) = a.first().filter(|m| !matches!(m, Value::Undefined)) {
                    o.props
                        .borrow_mut()
                        .set(&"message".into(), Prop::Method(Value::Str(rt.to_str(m)?)));
                }
                Ok(Value::Obj(o))
            }),
        );
        proto
            .props
            .borrow_mut()
            .set(&"name".into(), Prop::Method(Value::str(name)));
        proto
            .props
            .borrow_mut()
            .set(&"message".into(), Prop::Method(Value::str("")));
    }
    let ep = rt.error_proto.clone();
    method(&rt, &ep, "toString", |rt, this, _| {
        let n = rt.to_str(&rt.get(this, "name")?)?;
        let m = rt.to_str(&rt.get(this, "message")?)?;
        Ok(Value::Str(if m.is_empty() {
            n
        } else {
            format!("{n}: {m}").into()
        }))
    });

    array(&rt, &ctor);
    string(&rt, &ctor);
    regexp(&rt, &ctor);
    number(&rt, &ctor);
    ctor(
        "Boolean",
        &rt.boolean_proto,
        Box::new(|_, _, a| Ok(Value::Bool(arg(a, 0).truthy()))),
    );
    let bp = rt.boolean_proto.clone();
    method(&rt, &bp, "toString", |rt, this, _| {
        rt.to_str(this).map(Value::Str)
    });
    method(&rt, &bp, "valueOf", |_, this, _| Ok(this.clone()));
    math(&rt, &put);
    json(&rt, &put);

    let console = rt.object();
    for n in ["log", "info", "warn", "error", "debug"] {
        method(&rt, &console, n, |rt, _, a| {
            let parts: Vec<String> = a
                .iter()
                .map(|v| rt.to_str(v).map(|s| s.to_string()))
                .collect::<R<_>>()?;
            (rt.log.borrow_mut())(&parts.join(" "));
            Ok(Value::Undefined)
        });
    }
    put("console", Value::Obj(console));
    put(
        "print",
        rt.native("print", |rt, _, a| {
            let s = rt.to_str(&arg(a, 0))?;
            (rt.log.borrow_mut())(&s);
            Ok(Value::Undefined)
        }),
    );
    put(
        "parseInt",
        rt.native("parseInt", |rt, _, a| {
            let s = rt.to_str(&arg(a, 0))?;
            let s = s.trim();
            let (neg, s) = match s.as_bytes().first() {
                Some(b'-') => (true, &s[1..]),
                Some(b'+') => (false, &s[1..]),
                _ => (false, s),
            };
            let mut radix = match arg(a, 1) {
                Value::Undefined => 0,
                r => to_int32(rt.to_num(&r)?) as u32,
            };
            let mut s = s;
            if (radix == 0 || radix == 16) && (s.starts_with("0x") || s.starts_with("0X")) {
                s = &s[2..];
                radix = 16;
            }
            if radix == 0 {
                radix = 10;
            }
            if !(2..=36).contains(&radix) {
                return Ok(Value::Num(f64::NAN));
            }
            let digits: Vec<u32> = s.chars().map_while(|c| c.to_digit(radix)).collect();
            if digits.is_empty() {
                return Ok(Value::Num(f64::NAN));
            }
            let v = digits
                .iter()
                .fold(0.0, |acc, &d| acc * radix as f64 + d as f64);
            Ok(Value::Num(if neg { -v } else { v }))
        }),
    );
    put(
        "parseFloat",
        rt.native("parseFloat", |rt, _, a| {
            let s = rt.to_str(&arg(a, 0))?;
            let t = s.trim_start();
            for inf in ["Infinity", "+Infinity", "-Infinity"] {
                if t.starts_with(inf) {
                    return Ok(Value::Num(if inf.starts_with('-') {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    }));
                }
            }
            let b = t.as_bytes();
            let mut end = 0;
            let mut i = 0;
            if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
                i += 1;
            }
            let mut seen_dot = false;
            let mut seen_digit = false;
            while i < b.len() {
                match b[i] {
                    b'0'..=b'9' => {
                        seen_digit = true;
                        end = i + 1;
                    }
                    b'.' if !seen_dot => seen_dot = true,
                    b'e' | b'E' if seen_digit => {
                        let mut j = i + 1;
                        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                            j += 1;
                        }
                        let k = j;
                        while j < b.len() && b[j].is_ascii_digit() {
                            j += 1;
                        }
                        if j > k {
                            end = j;
                        }
                        break;
                    }
                    _ => break,
                }
                i += 1;
            }
            Ok(Value::Num(if seen_digit {
                t[..end].parse().unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }))
        }),
    );
    put(
        "isNaN",
        rt.native("isNaN", |rt, _, a| {
            Ok(Value::Bool(rt.to_num(&arg(a, 0))?.is_nan()))
        }),
    );
    put(
        "isFinite",
        rt.native("isFinite", |rt, _, a| {
            Ok(Value::Bool(rt.to_num(&arg(a, 0))?.is_finite()))
        }),
    );
    rt
}

fn define_properties(rt: &Rt, o: &Rc<Obj>, props: &Rc<Obj>) -> R<()> {
    let pv = Value::Obj(props.clone());
    for k in rt.own_keys(props) {
        if let Value::Obj(d) = rt.get(&pv, &k)? {
            define_property(rt, o, &k, &d)?;
        }
    }
    Ok(())
}

fn define_property(rt: &Rt, o: &Rc<Obj>, k: &Name, d: &Rc<Obj>) -> R<()> {
    let dv = Value::Obj(d.clone());
    let g = rt.get(&dv, "get")?;
    let s = rt.get(&dv, "set")?;
    if !g.is_nullish() || !s.is_nullish() {
        let g = (!g.is_nullish()).then_some(g);
        let s = (!s.is_nullish()).then_some(s);
        o.props.borrow_mut().set(k, Prop::Accessor(g, s));
        return Ok(());
    }
    let v = rt.get(&dv, "value")?;
    if let (Kind::Array(_), Some(_)) = (&o.kind, index_of_name(k)) {
        return rt.set(&Value::Obj(o.clone()), k, v);
    }
    let enumerable = rt.get(&dv, "enumerable")?.truthy();
    // A new property defaults to not enumerable; an existing one keeps its say.
    let existing = o.props.borrow().get(k).is_some();
    let p = if enumerable || (existing && !d.props.borrow().get("enumerable").is_some()) {
        Prop::Data(v)
    } else {
        Prop::Method(v)
    };
    o.props.borrow_mut().set(k, p);
    Ok(())
}

type Ctor<'a> = dyn Fn(&str, &Rc<Obj>, NativeBox) -> Value + 'a;

fn array(rt: &Rt, ctor: &Ctor) {
    let a = ctor(
        "Array",
        &rt.array_proto,
        Box::new(|rt, _, a| {
            if let [Value::Num(n)] = a {
                let Some(n) = as_index(*n) else {
                    let p = rt.range_error_proto.clone();
                    return rt.throw(&p, "invalid array length");
                };
                return Ok(rt.array(vec![Value::Hole; n]));
            }
            Ok(rt.array(a.to_vec()))
        }),
    );
    let ac = a.as_obj().unwrap();
    method(rt, ac, "isArray", |_, _, a| {
        Ok(Value::Bool(is_array(&arg(a, 0))))
    });
    method(rt, ac, "of", |rt, _, a| Ok(rt.array(a.to_vec())));
    method(rt, ac, "from", |rt, _, a| {
        let src = arg(a, 0);
        let list = if src.is_nullish() {
            return rt.type_error("Array.from of undefined or null");
        } else {
            rt.list(&src)?
        };
        let f = arg(a, 1);
        if f.is_nullish() {
            return Ok(rt.array(list));
        }
        let out = list
            .into_iter()
            .enumerate()
            .map(|(i, v)| rt.call(&f, Value::Undefined, &[v, Value::Num(i as f64)]))
            .collect::<R<_>>()?;
        Ok(rt.array(out))
    });
    let p = rt.array_proto.clone();
    method(rt, &p, "push", |rt, this, a| {
        let o = items(rt, this)?;
        let mut v = array_of(&o).borrow_mut();
        v.extend_from_slice(a);
        Ok(Value::Num(v.len() as f64))
    });
    method(rt, &p, "pop", |rt, this, _| {
        Ok(array_of(&items(rt, this)?)
            .borrow_mut()
            .pop()
            .map(|v| v.unhole())
            .unwrap_or_default())
    });
    method(rt, &p, "shift", |rt, this, _| {
        let o = items(rt, this)?;
        let mut v = array_of(&o).borrow_mut();
        Ok(if v.is_empty() {
            Value::Undefined
        } else {
            v.remove(0).unhole()
        })
    });
    method(rt, &p, "unshift", |rt, this, a| {
        let o = items(rt, this)?;
        let mut v = array_of(&o).borrow_mut();
        v.splice(0..0, a.iter().cloned());
        Ok(Value::Num(v.len() as f64))
    });
    method(rt, &p, "slice", |rt, this, a| {
        let v = rt.list(this)?;
        let s = rel(rt, &arg(a, 0), v.len(), 0)?;
        let e = rel(rt, &arg(a, 1), v.len(), v.len())?;
        Ok(rt.array(if s < e { v[s..e].to_vec() } else { Vec::new() }))
    });
    method(rt, &p, "splice", |rt, this, a| {
        let o = items(rt, this)?;
        let len = array_of(&o).borrow().len();
        let s = rel(rt, &arg(a, 0), len, 0)?;
        let n = match a.len() {
            0 => 0,
            1 => len - s,
            _ => (rt.to_num(&a[1])?.max(0.0) as usize).min(len - s),
        };
        let removed: Vec<Value> = array_of(&o)
            .borrow_mut()
            .splice(s..s + n, a.iter().skip(2).cloned())
            .map(|v| v.unhole())
            .collect();
        Ok(rt.array(removed))
    });
    method(rt, &p, "concat", |rt, this, a| {
        let mut v = rt.list(this)?;
        for x in a {
            match x {
                Value::Obj(o) if matches!(o.kind, Kind::Array(_)) => {
                    v.extend(array_of(o).borrow().iter().map(Value::unhole))
                }
                x => v.push(x.clone()),
            }
        }
        Ok(rt.array(v))
    });
    method(rt, &p, "join", |rt, this, a| {
        let sep = match arg(a, 0) {
            Value::Undefined => ",".into(),
            s => rt.to_str(&s)?,
        };
        let v = rt.list(this)?;
        let mut out = String::new();
        for (i, x) in v.iter().enumerate() {
            if i > 0 {
                out += &sep;
            }
            if !x.is_nullish() {
                out += &rt.to_str(x)?;
            }
        }
        Ok(Value::Str(out.into()))
    });
    method(rt, &p, "toString", |rt, this, _| {
        let j = rt.get(this, "join")?;
        rt.call(&j, this.clone(), &[])
    });
    method(rt, &p, "reverse", |rt, this, _| {
        array_of(&items(rt, this)?).borrow_mut().reverse();
        Ok(this.clone())
    });
    method(rt, &p, "indexOf", |rt, this, a| {
        let v = rt.list(this)?;
        let x = arg(a, 0);
        let from = rel(rt, &arg(a, 1), v.len(), 0)?;
        Ok(Value::Num(
            v.iter()
                .skip(from)
                .position(|y| y.strict_eq(&x))
                .map_or(-1.0, |i| (i + from) as f64),
        ))
    });
    method(rt, &p, "lastIndexOf", |rt, this, a| {
        let v = rt.list(this)?;
        let x = arg(a, 0);
        Ok(Value::Num(
            v.iter()
                .rposition(|y| y.strict_eq(&x))
                .map_or(-1.0, |i| i as f64),
        ))
    });
    method(rt, &p, "includes", |rt, this, a| {
        let x = arg(a, 0);
        Ok(Value::Bool(
            rt.list(this)?.iter().any(|y| y.same_value_zero(&x)),
        ))
    });
    method(rt, &p, "fill", |rt, this, a| {
        let o = items(rt, this)?;
        let len = array_of(&o).borrow().len();
        let s = rel(rt, &arg(a, 1), len, 0)?;
        let e = rel(rt, &arg(a, 2), len, len)?;
        for x in array_of(&o).borrow_mut().iter_mut().take(e).skip(s) {
            *x = arg(a, 0);
        }
        Ok(this.clone())
    });
    // Iteration methods: each sees the array as it was when called.
    fn each(
        rt: &Rt,
        this: &Value,
        a: &[Value],
        mut f: impl FnMut(Value, usize, Value) -> R<bool>,
    ) -> R<()> {
        let cb = arg(a, 0);
        callback(rt, &cb)?;
        let v = rt.list(this)?;
        for (i, x) in v.into_iter().enumerate() {
            let r = rt.call(
                &cb,
                arg(a, 1),
                &[x.clone(), Value::Num(i as f64), this.clone()],
            )?;
            if !f(x, i, r)? {
                break;
            }
        }
        Ok(())
    }
    method(rt, &p, "forEach", |rt, this, a| {
        each(rt, this, a, |_, _, _| Ok(true))?;
        Ok(Value::Undefined)
    });
    method(rt, &p, "map", |rt, this, a| {
        let mut out = Vec::new();
        each(rt, this, a, |_, _, r| {
            out.push(r);
            Ok(true)
        })?;
        Ok(rt.array(out))
    });
    method(rt, &p, "filter", |rt, this, a| {
        let mut out = Vec::new();
        each(rt, this, a, |x, _, r| {
            if r.truthy() {
                out.push(x);
            }
            Ok(true)
        })?;
        Ok(rt.array(out))
    });
    method(rt, &p, "some", |rt, this, a| {
        let mut any = false;
        each(rt, this, a, |_, _, r| {
            any = r.truthy();
            Ok(!any)
        })?;
        Ok(Value::Bool(any))
    });
    method(rt, &p, "every", |rt, this, a| {
        let mut all = true;
        each(rt, this, a, |_, _, r| {
            all = r.truthy();
            Ok(all)
        })?;
        Ok(Value::Bool(all))
    });
    method(rt, &p, "find", |rt, this, a| {
        let mut found = Value::Undefined;
        each(rt, this, a, |x, _, r| {
            if r.truthy() {
                found = x;
                return Ok(false);
            }
            Ok(true)
        })?;
        Ok(found)
    });
    method(rt, &p, "findIndex", |rt, this, a| {
        let mut found = -1.0;
        each(rt, this, a, |_, i, r| {
            if r.truthy() {
                found = i as f64;
                return Ok(false);
            }
            Ok(true)
        })?;
        Ok(Value::Num(found))
    });
    for right in [false, true] {
        method(
            rt,
            &p,
            if right { "reduceRight" } else { "reduce" },
            move |rt, this, a| {
                let cb = arg(a, 0);
                callback(rt, &cb)?;
                let mut v: Vec<(usize, Value)> = rt.list(this)?.into_iter().enumerate().collect();
                if right {
                    v.reverse();
                }
                let mut it = v.into_iter();
                let mut acc = match a.get(1) {
                    Some(x) => x.clone(),
                    None => match it.next() {
                        Some((_, x)) => x,
                        None => {
                            return rt.type_error("reduce of an empty array with no initial value")
                        }
                    },
                };
                for (i, x) in it {
                    acc = rt.call(
                        &cb,
                        Value::Undefined,
                        &[acc, x, Value::Num(i as f64), this.clone()],
                    )?;
                }
                Ok(acc)
            },
        );
    }
    method(rt, &p, "sort", |rt, this, a| {
        let o = items(rt, this)?;
        let mut v: Vec<Value> = array_of(&o).borrow().iter().map(Value::unhole).collect();
        let cmp = arg(a, 0);
        let mut err = None;
        // Merge sort: stable, and safe with comparators that are inconsistent.
        let mut keyed: Vec<(Value, Option<Rc<str>>)> = Vec::with_capacity(v.len());
        for x in v.drain(..) {
            let k = if cmp.is_nullish() && !matches!(x, Value::Undefined) {
                Some(rt.to_str(&x)?)
            } else {
                None
            };
            keyed.push((x, k));
        }
        let mut less = |x: &(Value, Option<Rc<str>>), y: &(Value, Option<Rc<str>>)| -> bool {
            if err.is_some() {
                return false;
            }
            match (&x.0, &y.0) {
                (Value::Undefined, _) => return false,
                (_, Value::Undefined) => return true,
                _ => {}
            }
            if cmp.is_nullish() {
                return x.1.as_ref().unwrap().encode_utf16().lt(y
                    .1
                    .as_ref()
                    .unwrap()
                    .encode_utf16());
            }
            match rt
                .call(&cmp, Value::Undefined, &[x.0.clone(), y.0.clone()])
                .and_then(|r| rt.to_num(&r))
            {
                Ok(n) => n < 0.0,
                Err(e) => {
                    err = Some(e);
                    false
                }
            }
        };
        keyed = merge_sort(keyed, &mut less);
        if let Some(e) = err {
            return Err(e);
        }
        *array_of(&o).borrow_mut() = keyed.into_iter().map(|(x, _)| x).collect();
        Ok(this.clone())
    });
}

fn merge_sort<T: Clone>(v: Vec<T>, less: &mut impl FnMut(&T, &T) -> bool) -> Vec<T> {
    if v.len() <= 1 {
        return v;
    }
    let mut right = v;
    let left = right.drain(..right.len() / 2).collect();
    let left = merge_sort(left, less);
    let right = merge_sort(right, less);
    let mut out = Vec::with_capacity(left.len() + right.len());
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if less(&right[j], &left[i]) {
            out.push(right[j].clone());
            j += 1;
        } else {
            out.push(left[i].clone());
            i += 1;
        }
    }
    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    out
}

fn string(rt: &Rt, ctor: &Ctor) {
    let s = ctor(
        "String",
        &rt.string_proto,
        Box::new(|rt, _, a| {
            Ok(Value::Str(if a.is_empty() {
                "".into()
            } else {
                rt.to_str(&a[0])?
            }))
        }),
    );
    let sc = s.as_obj().unwrap();
    method(rt, sc, "fromCharCode", |rt, _, a| {
        let u: Vec<u16> = a
            .iter()
            .map(|v| rt.to_num(v).map(|n| to_int32(n) as u16))
            .collect::<R<_>>()?;
        Ok(from16(&u))
    });
    let p = rt.string_proto.clone();
    method(rt, &p, "toString", |rt, this, _| {
        this_str(rt, this).map(Value::Str)
    });
    method(rt, &p, "valueOf", |rt, this, _| {
        this_str(rt, this).map(Value::Str)
    });
    method(rt, &p, "charAt", |rt, this, a| {
        let s = this_str(rt, this)?;
        let i = rt.to_num(&arg(a, 0))?;
        let i = if i.is_nan() { 0.0 } else { i };
        Ok(Value::Str(if i < 0.0 {
            "".into()
        } else {
            char_at(&s, i as usize).unwrap_or_else(|| "".into())
        }))
    });
    method(rt, &p, "charCodeAt", |rt, this, a| {
        let s = this_str(rt, this)?;
        let i = rt.to_num(&arg(a, 0))?;
        let i = if i.is_nan() { 0.0 } else { i };
        if i < 0.0 {
            return Ok(Value::Num(f64::NAN));
        }
        let i = i as usize;
        Ok(Value::Num(if s.is_ascii() {
            s.as_bytes().get(i).map_or(f64::NAN, |&b| b as f64)
        } else {
            s.encode_utf16().nth(i).map_or(f64::NAN, |u| u as f64)
        }))
    });
    method(rt, &p, "indexOf", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let n = utf16(&rt.to_str(&arg(a, 0))?);
        let from = rel(rt, &arg(a, 1), s.len(), 0)?;
        Ok(Value::Num(find16(&s, &n, from).map_or(-1.0, |i| i as f64)))
    });
    method(rt, &p, "lastIndexOf", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let n = utf16(&rt.to_str(&arg(a, 0))?);
        Ok(Value::Num(if n.len() > s.len() {
            -1.0
        } else {
            (0..=s.len() - n.len())
                .rev()
                .find(|&i| s[i..].starts_with(&n))
                .map_or(-1.0, |i| i as f64)
        }))
    });
    method(rt, &p, "includes", |rt, this, a| {
        let s = this_str(rt, this)?;
        Ok(Value::Bool(s.contains(&*rt.to_str(&arg(a, 0))?)))
    });
    method(rt, &p, "startsWith", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let from = rel(rt, &arg(a, 1), s.len(), 0)?;
        Ok(Value::Bool(
            s[from..].starts_with(&utf16(&rt.to_str(&arg(a, 0))?)),
        ))
    });
    method(rt, &p, "endsWith", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let end = rel(rt, &arg(a, 1), s.len(), s.len())?;
        Ok(Value::Bool(
            s[..end].ends_with(&utf16(&rt.to_str(&arg(a, 0))?)),
        ))
    });
    method(rt, &p, "slice", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let b = rel(rt, &arg(a, 0), s.len(), 0)?;
        let e = rel(rt, &arg(a, 1), s.len(), s.len())?;
        Ok(from16(if b < e { &s[b..e] } else { &[] }))
    });
    method(rt, &p, "substring", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let clamp = |v: &Value, d: usize| -> R<usize> {
            if matches!(v, Value::Undefined) {
                return Ok(d);
            }
            let n = rt.to_num(v)?;
            Ok(if n.is_nan() {
                0
            } else {
                n.max(0.0).min(s.len() as f64) as usize
            })
        };
        let (x, y) = (clamp(&arg(a, 0), 0)?, clamp(&arg(a, 1), s.len())?);
        Ok(from16(&s[x.min(y)..x.max(y)]))
    });
    method(rt, &p, "substr", |rt, this, a| {
        let s = utf16(&this_str(rt, this)?);
        let b = rel(rt, &arg(a, 0), s.len(), 0)?;
        let n = match arg(a, 1) {
            Value::Undefined => s.len() - b,
            v => rt.to_num(&v)?.max(0.0).min((s.len() - b) as f64) as usize,
        };
        Ok(from16(&s[b..b + n]))
    });
    method(rt, &p, "toUpperCase", |rt, this, _| {
        Ok(Value::Str(this_str(rt, this)?.to_uppercase().into()))
    });
    method(rt, &p, "toLowerCase", |rt, this, _| {
        Ok(Value::Str(this_str(rt, this)?.to_lowercase().into()))
    });
    method(rt, &p, "trim", |rt, this, _| {
        Ok(Value::str(this_str(rt, this)?.trim()))
    });
    method(rt, &p, "trimStart", |rt, this, _| {
        Ok(Value::str(this_str(rt, this)?.trim_start()))
    });
    method(rt, &p, "trimEnd", |rt, this, _| {
        Ok(Value::str(this_str(rt, this)?.trim_end()))
    });
    method(rt, &p, "concat", |rt, this, a| {
        let mut s = this_str(rt, this)?.to_string();
        for x in a {
            s += &rt.to_str(x)?;
        }
        Ok(Value::Str(s.into()))
    });
    method(rt, &p, "repeat", |rt, this, a| {
        let s = this_str(rt, this)?;
        let n = rt.to_num(&arg(a, 0))?;
        if !(0.0..1e9).contains(&n) {
            let p = rt.range_error_proto.clone();
            return rt.throw(&p, "invalid repeat count");
        }
        Ok(Value::Str(s.repeat(n as usize).into()))
    });
    for start in [true, false] {
        method(
            rt,
            &p,
            if start { "padStart" } else { "padEnd" },
            move |rt, this, a| {
                let s = this_str(rt, this)?;
                let target = rt.to_num(&arg(a, 0))?.max(0.0) as usize;
                let fill: Vec<u16> = match arg(a, 1) {
                    Value::Undefined => vec![b' ' as u16],
                    f => utf16(&rt.to_str(&f)?),
                };
                let mut u = utf16(&s);
                if u.len() >= target || fill.is_empty() {
                    return Ok(Value::Str(s));
                }
                let pad: Vec<u16> = fill
                    .iter()
                    .cycle()
                    .take(target - u.len())
                    .copied()
                    .collect();
                if start {
                    let mut p = pad;
                    p.extend(u);
                    u = p;
                } else {
                    u.extend(pad);
                }
                Ok(from16(&u))
            },
        );
    }
    method(rt, &p, "split", |rt, this, a| {
        let s = this_str(rt, this)?;
        let limit = match arg(a, 1) {
            Value::Undefined => usize::MAX,
            l => rt.to_num(&l)? as u32 as usize,
        };
        let parts: Vec<Value> = match arg(a, 0) {
            Value::Undefined => vec![Value::Str(s)],
            sep => {
                let sep = rt.to_str(&sep)?;
                if sep.is_empty() {
                    utf16(&s).iter().map(|u| from16(&[*u])).collect()
                } else {
                    s.split(&*sep).map(Value::str).collect()
                }
            }
        };
        Ok(rt.array(parts.into_iter().take(limit).collect()))
    });
    for all in [false, true] {
        method(
            rt,
            &p,
            if all { "replaceAll" } else { "replace" },
            move |rt, this, a| {
                let s = this_str(rt, this)?;
                let pat = rt.to_str(&arg(a, 0))?;
                let with = arg(a, 1);
                let callable = with.as_obj().is_some_and(|o| o.is_callable());
                let mut out = String::new();
                let mut rest: &str = &s;
                let mut offset = 0;
                while let Some(i) = rest.find(&*pat) {
                    out += &rest[..i];
                    let pos = offset + i;
                    if callable {
                        let r = rt.call(
                            &with,
                            Value::Undefined,
                            &[
                                Value::Str(pat.clone()),
                                Value::Num(str_len(&s[..pos]) as f64),
                                Value::Str(s.clone()),
                            ],
                        )?;
                        out += &rt.to_str(&r)?;
                    } else {
                        let w = rt.to_str(&with)?;
                        out += &w.replace("$&", &pat).replace("$$", "$");
                    }
                    let skip = i + pat.len().max(if pat.is_empty() {
                        rest[i..].chars().next().map_or(1, char::len_utf8)
                    } else {
                        0
                    });
                    if pat.is_empty() && i < rest.len() {
                        out += &rest[i..skip];
                    }
                    offset += skip;
                    rest = &rest[skip.min(rest.len())..];
                    if !all || rest.is_empty() {
                        break;
                    }
                }
                out += rest;
                Ok(Value::Str(out.into()))
            },
        );
    }
    method(rt, &p, "localeCompare", |rt, this, a| {
        let s = this_str(rt, this)?;
        let o = rt.to_str(&arg(a, 0))?;
        Ok(Value::Num(match s.cmp(&o) {
            std::cmp::Ordering::Less => -1.0,
            std::cmp::Ordering::Equal => 0.0,
            std::cmp::Ordering::Greater => 1.0,
        }))
    });
}

fn find16(s: &[u16], n: &[u16], from: usize) -> Option<usize> {
    if n.is_empty() {
        return Some(from.min(s.len()));
    }
    (from..=s.len().checked_sub(n.len())?).find(|&i| s[i..].starts_with(n))
}

fn number(rt: &Rt, ctor: &Ctor) {
    let n = ctor(
        "Number",
        &rt.number_proto,
        Box::new(|rt, _, a| {
            Ok(Value::Num(if a.is_empty() {
                0.0
            } else {
                rt.to_num(&a[0])?
            }))
        }),
    );
    let nc = n.as_obj().unwrap();
    for (k, v) in [
        ("MAX_SAFE_INTEGER", 9007199254740991.0),
        ("MIN_SAFE_INTEGER", -9007199254740991.0),
        ("MAX_VALUE", f64::MAX),
        ("MIN_VALUE", 5e-324),
        ("EPSILON", f64::EPSILON),
        ("POSITIVE_INFINITY", f64::INFINITY),
        ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
        ("NaN", f64::NAN),
    ] {
        nc.props
            .borrow_mut()
            .set(&k.into(), Prop::Method(Value::Num(v)));
    }
    method(rt, nc, "isInteger", |_, _, a| {
        Ok(Value::Bool(
            matches!(arg(a, 0), Value::Num(n) if n.is_finite() && n.trunc() == n),
        ))
    });
    method(rt, nc, "isSafeInteger", |_, _, a| {
        Ok(Value::Bool(
            matches!(arg(a, 0), Value::Num(n) if n.trunc() == n && n.abs() <= 9007199254740991.0),
        ))
    });
    method(rt, nc, "isFinite", |_, _, a| {
        Ok(Value::Bool(
            matches!(arg(a, 0), Value::Num(n) if n.is_finite()),
        ))
    });
    method(rt, nc, "isNaN", |_, _, a| {
        Ok(Value::Bool(
            matches!(arg(a, 0), Value::Num(n) if n.is_nan()),
        ))
    });
    for k in ["parseInt", "parseFloat"] {
        let f = rt.global.props.borrow().get(k).cloned();
        if let Some(f) = f {
            nc.props.borrow_mut().set(&k.into(), f);
        }
    }
    let p = rt.number_proto.clone();
    let this_num = |rt: &Rt, this: &Value| -> R<f64> {
        match this {
            Value::Num(n) => Ok(*n),
            _ => rt.type_error("not a number"),
        }
    };
    method(rt, &p, "valueOf", move |rt, this, _| {
        this_num(rt, this).map(Value::Num)
    });
    method(rt, &p, "toString", move |rt, this, a| {
        let n = this_num(rt, this)?;
        let radix = match arg(a, 0) {
            Value::Undefined => 10,
            r => rt.to_num(&r)? as u32,
        };
        if radix == 10 {
            return Ok(Value::Str(number_to_string(n).into()));
        }
        if !(2..=36).contains(&radix) {
            let p = rt.range_error_proto.clone();
            return rt.throw(&p, "radix must be between 2 and 36");
        }
        Ok(Value::Str(radix_string(n, radix).into()))
    });
    method(rt, &p, "toFixed", move |rt, this, a| {
        let n = this_num(rt, this)?;
        let d = rt.to_num(&arg(a, 0))?;
        let d = if d.is_nan() { 0 } else { d as usize };
        if d > 100 {
            let p = rt.range_error_proto.clone();
            return rt.throw(&p, "toFixed digits out of range");
        }
        if !n.is_finite() || n.abs() >= 1e21 {
            return Ok(Value::Str(number_to_string(n).into()));
        }
        Ok(Value::Str(fixed(n, d).into()))
    });
    method(rt, &p, "toPrecision", move |rt, this, a| {
        let n = this_num(rt, this)?;
        if matches!(arg(a, 0), Value::Undefined) || !n.is_finite() {
            return Ok(Value::Str(number_to_string(n).into()));
        }
        let p = rt.to_num(&arg(a, 0))? as i32;
        if n == 0.0 {
            return Ok(Value::Str(fixed(0.0, (p - 1).max(0) as usize).into()));
        }
        let e = n.abs().log10().floor() as i32;
        let s = if e < -6 || e >= p {
            format!("{:.*e}", (p - 1).max(0) as usize, n)
                .replace("e", "e+")
                .replace("e+-", "e-")
        } else {
            fixed(n, (p - 1 - e).max(0) as usize)
        };
        Ok(Value::Str(s.into()))
    });
}

/// `toFixed`: rounds half away from zero on the decimal value printed.
fn fixed(n: f64, d: usize) -> String {
    let s = format!("{:.*}", d + 3, n);
    let neg = s.starts_with('-');
    let digits: Vec<u8> = s.bytes().filter(u8::is_ascii_digit).collect();
    let int_len = s.trim_start_matches('-').find('.').unwrap_or(s.len());
    let keep = int_len + d;
    let mut v: Vec<u8> = digits[..keep].to_vec();
    // Exact digits beyond what's kept decide the rounding.
    let exact = format!("{:.60}", n.abs());
    let exact: Vec<u8> = exact.bytes().filter(u8::is_ascii_digit).collect();
    if exact.get(keep).is_some_and(|&b| b >= b'5') {
        let mut i = v.len();
        loop {
            if i == 0 {
                v.insert(0, b'1');
                break;
            }
            i -= 1;
            if v[i] == b'9' {
                v[i] = b'0';
            } else {
                v[i] += 1;
                break;
            }
        }
    }
    let il = v.len() - d;
    let mut out = String::new();
    let all_zero = v.iter().all(|&b| b == b'0');
    if neg && !all_zero {
        out.push('-');
    }
    out += std::str::from_utf8(&v[..il]).unwrap();
    if d > 0 {
        out.push('.');
        out += std::str::from_utf8(&v[il..]).unwrap();
    }
    out
}

fn radix_string(n: f64, radix: u32) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let neg = n < 0.0;
    let mut i = n.abs().trunc();
    let mut frac = n.abs() - i;
    let mut digits = Vec::new();
    if i == 0.0 {
        digits.push(b'0');
    }
    while i >= 1.0 {
        let d = (i % radix as f64) as u32;
        digits.push(std::char::from_digit(d, radix).unwrap() as u8);
        i = (i / radix as f64).trunc();
    }
    digits.reverse();
    let mut s = String::from_utf8(digits).unwrap();
    if frac > 0.0 {
        s.push('.');
        for _ in 0..52 {
            frac *= radix as f64;
            let d = frac.trunc() as u32;
            s.push(std::char::from_digit(d, radix).unwrap());
            frac -= d as f64;
            if frac == 0.0 {
                break;
            }
        }
    }
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

fn math(rt: &Rt, put: &dyn Fn(&str, Value)) {
    let m = rt.object();
    for (k, v) in [
        ("PI", std::f64::consts::PI),
        ("E", std::f64::consts::E),
        ("LN2", std::f64::consts::LN_2),
        ("LN10", std::f64::consts::LN_10),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("SQRT2", std::f64::consts::SQRT_2),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
    ] {
        m.props
            .borrow_mut()
            .set(&k.into(), Prop::Method(Value::Num(v)));
    }
    let one: [MathFn; 22] = [
        ("abs", f64::abs),
        ("floor", f64::floor),
        ("ceil", f64::ceil),
        ("round", |x| {
            if x.fract() == -0.5 {
                x.ceil()
            } else {
                (x + 0.5).floor()
            }
        }),
        ("trunc", f64::trunc),
        ("sign", |x| {
            if x.is_nan() || x == 0.0 {
                x
            } else {
                x.signum()
            }
        }),
        ("sqrt", f64::sqrt),
        ("cbrt", f64::cbrt),
        ("exp", f64::exp),
        ("expm1", f64::exp_m1),
        ("log", f64::ln),
        ("log2", f64::log2),
        ("log10", f64::log10),
        ("log1p", f64::ln_1p),
        ("sin", f64::sin),
        ("cos", f64::cos),
        ("tan", f64::tan),
        ("asin", f64::asin),
        ("acos", f64::acos),
        ("atan", f64::atan),
        ("tanh", f64::tanh),
        ("fround", |x| x as f32 as f64),
    ];
    for (k, f) in one {
        method(rt, &m, k, move |rt, _, a| {
            Ok(Value::Num(f(match a.first() {
                Some(Value::Num(n)) => *n,
                Some(v) => rt.to_num(v)?,
                None => f64::NAN,
            })))
        });
    }
    method(rt, &m, "clz32", |rt, _, a| {
        Ok(Value::Num(
            (to_int32(rt.to_num(&arg(a, 0))?) as u32).leading_zeros() as f64,
        ))
    });
    method(rt, &m, "imul", |rt, _, a| {
        Ok(Value::Num(
            to_int32(rt.to_num(&arg(a, 0))?).wrapping_mul(to_int32(rt.to_num(&arg(a, 1))?)) as f64,
        ))
    });
    method(rt, &m, "atan2", |rt, _, a| {
        Ok(Value::Num(
            rt.to_num(&arg(a, 0))?.atan2(rt.to_num(&arg(a, 1))?),
        ))
    });
    method(rt, &m, "pow", |rt, _, a| {
        Ok(Value::Num(pow(
            rt.to_num(&arg(a, 0))?,
            rt.to_num(&arg(a, 1))?,
        )))
    });
    method(rt, &m, "hypot", |rt, _, a| {
        Ok(Value::Num(
            a.iter()
                .map(|v| rt.to_num(v))
                .collect::<R<Vec<_>>>()?
                .iter()
                .map(|x| x * x)
                .sum::<f64>()
                .sqrt(),
        ))
    });
    method(rt, &m, "max", |rt, _, a| {
        let mut r = f64::NEG_INFINITY;
        for v in a {
            let x = rt.to_num(v)?;
            if x.is_nan() {
                return Ok(Value::Num(f64::NAN));
            }
            if x > r || (x == 0.0 && r == 0.0 && r.is_sign_negative()) {
                r = x;
            }
        }
        Ok(Value::Num(r))
    });
    method(rt, &m, "min", |rt, _, a| {
        let mut r = f64::INFINITY;
        for v in a {
            let x = rt.to_num(v)?;
            if x.is_nan() {
                return Ok(Value::Num(f64::NAN));
            }
            if x < r || (x == 0.0 && r == 0.0 && x.is_sign_negative()) {
                r = x;
            }
        }
        Ok(Value::Num(r))
    });
    let seed = std::cell::Cell::new(
        0x9E3779B97F4A7C15u64
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64),
    );
    method(rt, &m, "random", move |_, _, _| {
        // xorshift64*
        let mut x = seed.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        seed.set(x);
        Ok(Value::Num(
            (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64,
        ))
    });
    put("Math", Value::Obj(m));
}

fn json(rt: &Rt, put: &dyn Fn(&str, Value)) {
    let j = rt.object();
    method(rt, &j, "stringify", |rt, _, a| {
        let indent = match arg(a, 2) {
            Value::Num(n) => " ".repeat(n.clamp(0.0, 10.0) as usize),
            Value::Str(s) => s.chars().take(10).collect(),
            _ => String::new(),
        };
        let mut out = String::new();
        let mut seen = Vec::new();
        Ok(
            if stringify(rt, &arg(a, 0), &indent, 0, &mut out, &mut seen)? {
                Value::Str(out.into())
            } else {
                Value::Undefined
            },
        )
    });
    method(rt, &j, "parse", |rt, _, a| {
        let s = rt.to_str(&arg(a, 0))?;
        let mut p = JsonParser {
            s: s.as_bytes(),
            i: 0,
            rt,
        };
        let v = p.value()?;
        p.ws();
        if p.i != p.s.len() {
            return p.fail();
        }
        Ok(v)
    });
    put("JSON", Value::Obj(j));
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Whether anything was written (undefined and functions write nothing).
fn stringify(
    rt: &Rt,
    v: &Value,
    indent: &str,
    depth: usize,
    out: &mut String,
    seen: &mut Vec<*const Obj>,
) -> R<bool> {
    let v = match v {
        Value::Obj(_) => {
            let tj = rt.get(v, "toJSON")?;
            if tj.as_obj().is_some_and(|o| o.is_callable()) {
                rt.call(&tj, v.clone(), &[])?
            } else {
                v.clone()
            }
        }
        v => v.clone(),
    };
    match &v {
        Value::Undefined | Value::Hole => return Ok(false),
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Num(n) => out.push_str(&if n.is_finite() {
            number_to_string(*n)
        } else {
            "null".into()
        }),
        Value::Str(s) => quote(s, out),
        Value::Obj(o) if o.is_callable() => return Ok(false),
        Value::Obj(o) => {
            let p = Rc::as_ptr(o);
            if seen.contains(&p) {
                return rt.type_error("cyclic object in JSON.stringify");
            }
            seen.push(p);
            let nl = |out: &mut String, d: usize| {
                if !indent.is_empty() {
                    out.push('\n');
                    for _ in 0..d {
                        out.push_str(indent);
                    }
                }
            };
            if let Kind::Array(items) = &o.kind {
                let items: Vec<Value> = items.borrow().iter().map(Value::unhole).collect();
                out.push('[');
                for (i, x) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    nl(out, depth + 1);
                    if !stringify(rt, x, indent, depth + 1, out, seen)? {
                        out.push_str("null");
                    }
                }
                if !items.is_empty() {
                    nl(out, depth);
                }
                out.push(']');
            } else {
                out.push('{');
                let mut first = true;
                for k in rt.own_keys(o) {
                    let x = rt.get(&v, &k)?;
                    let mark = out.len();
                    if !first {
                        out.push(',');
                    }
                    nl(out, depth + 1);
                    quote(&k, out);
                    out.push(':');
                    if !indent.is_empty() {
                        out.push(' ');
                    }
                    if stringify(rt, &x, indent, depth + 1, out, seen)? {
                        first = false;
                    } else {
                        out.truncate(mark);
                    }
                }
                if !first {
                    nl(out, depth);
                }
                out.push('}');
            }
            seen.pop();
        }
    }
    Ok(true)
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
    rt: &'a Rt,
}

impl JsonParser<'_> {
    fn fail<T>(&self) -> R<T> {
        let p = self.rt.global.lookup("SyntaxError");
        let msg = format!("bad JSON at {}", self.i);
        match p {
            Some(Prop::Method(f)) => {
                let e = self.rt.construct(&f, &[Value::Str(msg.into())], &f)?;
                Err(Exc::Throw(e))
            }
            _ => self.rt.type_error(&msg),
        }
    }

    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn eat(&mut self, w: &[u8]) -> bool {
        if self.s[self.i..].starts_with(w) {
            self.i += w.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> R<Value> {
        self.ws();
        let Some(&c) = self.s.get(self.i) else {
            return self.fail();
        };
        Ok(match c {
            b'{' => {
                self.i += 1;
                let o = self.rt.object();
                self.ws();
                if !self.eat(b"}") {
                    loop {
                        self.ws();
                        let Value::Str(k) = self.string()? else {
                            unreachable!()
                        };
                        self.ws();
                        if !self.eat(b":") {
                            return self.fail();
                        }
                        let v = self.value()?;
                        o.put_name(&k, v);
                        self.ws();
                        if self.eat(b"}") {
                            break;
                        }
                        if !self.eat(b",") {
                            return self.fail();
                        }
                    }
                }
                Value::Obj(o)
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if !self.eat(b"]") {
                    loop {
                        items.push(self.value()?);
                        self.ws();
                        if self.eat(b"]") {
                            break;
                        }
                        if !self.eat(b",") {
                            return self.fail();
                        }
                    }
                }
                self.rt.array(items)
            }
            b'"' => self.string()?,
            b't' if self.eat(b"true") => Value::Bool(true),
            b'f' if self.eat(b"false") => Value::Bool(false),
            b'n' if self.eat(b"null") => Value::Null,
            b'-' | b'0'..=b'9' => {
                let st = self.i;
                self.i += 1;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
                    )
                {
                    self.i += 1;
                }
                match std::str::from_utf8(&self.s[st..self.i])
                    .ok()
                    .and_then(|t| t.parse().ok())
                {
                    Some(n) => Value::Num(n),
                    None => return self.fail(),
                }
            }
            _ => return self.fail(),
        })
    }

    fn string(&mut self) -> R<Value> {
        if !self.eat(b"\"") {
            return self.fail();
        }
        let mut u: Vec<u16> = Vec::new();
        loop {
            let Some(&c) = self.s.get(self.i) else {
                return self.fail();
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.s.get(self.i) else {
                        return self.fail();
                    };
                    self.i += 1;
                    u.push(match e {
                        b'n' => 10,
                        b't' => 9,
                        b'r' => 13,
                        b'b' => 8,
                        b'f' => 12,
                        b'u' => {
                            let h = self
                                .s
                                .get(self.i..self.i + 4)
                                .and_then(|h| std::str::from_utf8(h).ok())
                                .and_then(|h| u16::from_str_radix(h, 16).ok());
                            let Some(h) = h else { return self.fail() };
                            self.i += 4;
                            h
                        }
                        b'"' | b'\\' | b'/' => e as u16,
                        _ => return self.fail(),
                    });
                }
                c if c < 0x20 => return self.fail(),
                c if c < 0x80 => u.push(c as u16),
                _ => {
                    let st = self.i - 1;
                    let len = match c {
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    let Some(ch) = self
                        .s
                        .get(st..st + len)
                        .and_then(|b| std::str::from_utf8(b).ok())
                    else {
                        return self.fail();
                    };
                    u.extend(ch.encode_utf16());
                    self.i = st + len;
                }
            }
        }
        Ok(from16(&u))
    }
}

fn re_of(v: &Value) -> Option<&crate::regex::Regex> {
    match v {
        Value::Obj(o) => match &o.kind {
            Kind::Regex(r) => Some(r),
            _ => None,
        },
        _ => None,
    }
}

fn last_index(rt: &Rt, re: &Value) -> R<usize> {
    let n = rt.to_num(&rt.get(re, "lastIndex")?)?;
    Ok(if n.is_nan() || n < 0.0 { 0 } else { n as usize })
}

/// The match array `exec` returns, updating `lastIndex` for g/y.
fn exec(rt: &Rt, re: &Value, s: &Rc<str>) -> R<Value> {
    let r = re_of(re).unwrap();
    let u = utf16(s);
    let sticky = r.global || r.sticky;
    let start = if sticky { last_index(rt, re)? } else { 0 };
    let found = if start > u.len() {
        None
    } else {
        r.exec(&u, start)
    };
    let Some(caps) = found else {
        if sticky {
            rt.set(re, &"lastIndex".into(), Value::Num(0.0))?;
        }
        return Ok(Value::Null);
    };
    let (a, b) = caps[0].unwrap();
    if sticky {
        rt.set(re, &"lastIndex".into(), Value::Num(b as f64))?;
    }
    let items: Vec<Value> = caps
        .iter()
        .map(|c| c.map_or(Value::Undefined, |(x, y)| from16(&u[x..y])))
        .collect();
    let arr = rt.array(items.clone());
    rt.set(&arr, &"index".into(), Value::Num(a as f64))?;
    rt.set(&arr, &"input".into(), Value::Str(s.clone()))?;
    let groups = if r.names.is_empty() {
        Value::Undefined
    } else {
        let g = rt.object();
        for (n, i) in &r.names {
            g.put_name(&n.as_str().into(), items[*i].clone());
        }
        Value::Obj(g)
    };
    rt.set(&arr, &"groups".into(), groups)?;
    Ok(arr)
}

/// `$&`, `$1`, `$<name>` and friends in a replacement string.
fn expand(with: &str, m: &[Value], groups: &Value, pos: usize, s: &[u16], rt: &Rt) -> R<String> {
    let mut out = String::new();
    let w: Vec<char> = with.chars().collect();
    let mut i = 0;
    let whole = rt.to_str(&m[0])?;
    while i < w.len() {
        if w[i] != '$' || i + 1 == w.len() {
            out.push(w[i]);
            i += 1;
            continue;
        }
        let n = w[i + 1];
        i += 2;
        match n {
            '$' => out.push('$'),
            '&' => out += &whole,
            '`' => out += &String::from_utf16_lossy(&s[..pos]),
            '\'' => {
                out += &String::from_utf16_lossy(&s[(pos + utf16(&whole).len()).min(s.len())..])
            }
            '<' if !matches!(groups, Value::Undefined) => {
                let end = w[i..].iter().position(|&c| c == '>').map(|p| i + p);
                match end {
                    Some(e) => {
                        let name: String = w[i..e].iter().collect();
                        let v = rt.get(groups, &name)?;
                        if !matches!(v, Value::Undefined) {
                            out += &rt.to_str(&v)?;
                        }
                        i = e + 1;
                    }
                    None => out.push_str("$<"),
                }
            }
            d if d.is_ascii_digit() => {
                let mut g = d as usize - '0' as usize;
                if i < w.len()
                    && w[i].is_ascii_digit()
                    && g * 10 + (w[i] as usize - '0' as usize) < m.len()
                {
                    g = g * 10 + (w[i] as usize - '0' as usize);
                    i += 1;
                }
                if g >= 1 && g < m.len() {
                    if !matches!(m[g], Value::Undefined) {
                        out += &rt.to_str(&m[g])?;
                    }
                } else {
                    out.push('$');
                    out.push(d);
                }
            }
            c => {
                out.push('$');
                out.push(c);
            }
        }
    }
    Ok(out)
}

fn regex_replace(rt: &Rt, re: &Value, s: &Rc<str>, with: &Value, all_required: bool) -> R<Value> {
    let r = re_of(re).unwrap();
    if all_required && !r.global {
        return rt.type_error("replaceAll needs a global regular expression");
    }
    let u = utf16(s);
    let callable = with.as_obj().is_some_and(|o| o.is_callable());
    let with_s = if callable {
        "".into()
    } else {
        rt.to_str(with)?
    };
    let mut out: Vec<u16> = Vec::new();
    let mut at = if r.sticky && !r.global {
        last_index(rt, re)?
    } else {
        0
    };
    let mut copied = 0;
    while at <= u.len() {
        let Some(caps) = r.exec(&u, at) else { break };
        if r.sticky && caps[0].unwrap().0 != at {
            break;
        }
        let (a, b) = caps[0].unwrap();
        out.extend_from_slice(&u[copied..a]);
        let m: Vec<Value> = caps
            .iter()
            .map(|c| c.map_or(Value::Undefined, |(x, y)| from16(&u[x..y])))
            .collect();
        let groups = if r.names.is_empty() {
            Value::Undefined
        } else {
            let g = rt.object();
            for (n, i) in &r.names {
                g.put_name(&n.as_str().into(), m[*i].clone());
            }
            Value::Obj(g)
        };
        let rep = if callable {
            let mut args = m.clone();
            args.push(Value::Num(a as f64));
            args.push(Value::Str(s.clone()));
            if !matches!(groups, Value::Undefined) {
                args.push(groups);
            }
            rt.to_str(&rt.call(with, Value::Undefined, &args)?)?
                .to_string()
        } else {
            expand(&with_s, &m, &groups, a, &u, rt)?
        };
        out.extend(rep.encode_utf16());
        copied = b;
        if !r.global {
            break;
        }
        at = if b == a { b + 1 } else { b };
    }
    if r.global || r.sticky {
        rt.set(re, &"lastIndex".into(), Value::Num(0.0))?;
    }
    out.extend_from_slice(&u[copied.min(u.len())..]);
    Ok(from16(&out))
}

fn regexp(rt: &Rt, ctor: &Ctor) {
    ctor(
        "RegExp",
        &rt.regexp_proto,
        Box::new(|rt, _, a| {
            let (src, flags) = match (re_of(&arg(a, 0)), arg(a, 1)) {
                (Some(r), Value::Undefined) => (r.source.clone(), r.flags.clone()),
                (Some(r), f) => (r.source.clone(), rt.to_str(&f)?.to_string()),
                (None, f) => (
                    match arg(a, 0) {
                        Value::Undefined => "(?:)".to_string(),
                        p => rt.to_str(&p)?.to_string(),
                    },
                    match f {
                        Value::Undefined => String::new(),
                        f => rt.to_str(&f)?.to_string(),
                    },
                ),
            };
            rt.regexp(&src, &flags)
        }),
    );
    let p = rt.regexp_proto.clone();
    method(rt, &p, "exec", |rt, this, a| {
        if re_of(this).is_none() {
            return rt.type_error("exec on a non-RegExp");
        }
        let s = rt.to_str(&arg(a, 0))?;
        exec(rt, this, &s)
    });
    method(rt, &p, "test", |rt, this, a| {
        if re_of(this).is_none() {
            return rt.type_error("test on a non-RegExp");
        }
        let s = rt.to_str(&arg(a, 0))?;
        Ok(Value::Bool(!matches!(exec(rt, this, &s)?, Value::Null)))
    });
    method(rt, &p, "toString", |rt, this, _| {
        let src = rt.to_str(&rt.get(this, "source")?)?;
        let fl = rt.to_str(&rt.get(this, "flags")?)?;
        Ok(Value::Str(format!("/{src}/{fl}").into()))
    });

    let sp = rt.string_proto.clone();
    let old = |n: &str| {
        sp.props
            .borrow()
            .get(n)
            .cloned()
            .map(|p| match p {
                Prop::Data(v) | Prop::Method(v) => v,
                _ => Value::Undefined,
            })
            .unwrap()
    };
    for (name, all) in [("replace", false), ("replaceAll", true)] {
        let prev = old(name);
        method(rt, &sp, name, move |rt, this, a| {
            if re_of(&arg(a, 0)).is_some() {
                let s = this_str(rt, this)?;
                return regex_replace(rt, &arg(a, 0), &s, &arg(a, 1), all);
            }
            rt.call(&prev, this.clone(), a)
        });
    }
    let prev = old("split");
    method(rt, &sp, "split", move |rt, this, a| {
        let sep = arg(a, 0);
        let Some(r) = re_of(&sep) else {
            return rt.call(&prev, this.clone(), a);
        };
        let s = this_str(rt, this)?;
        let u = utf16(&s);
        let limit = match arg(a, 1) {
            Value::Undefined => usize::MAX,
            l => rt.to_num(&l)? as u32 as usize,
        };
        let mut out = Vec::new();
        if u.is_empty() {
            if r.exec(&u, 0).is_none() {
                out.push(Value::Str(s));
            }
            return Ok(rt.array(out));
        }
        let (mut p, mut q) = (0, 0);
        while q < u.len() {
            match r.exec(&u, q) {
                Some(caps) => {
                    let (a, e) = caps[0].unwrap();
                    if a >= u.len() {
                        break;
                    }
                    if e == p || (e == a && a == q && a == p) {
                        q = a + 1;
                        continue;
                    }
                    if e == a && a < q {
                        q += 1;
                        continue;
                    }
                    out.push(from16(&u[p..a]));
                    for c in &caps[1..] {
                        out.push(c.map_or(Value::Undefined, |(x, y)| from16(&u[x..y])));
                    }
                    p = e;
                    q = if e == a { e + 1 } else { e };
                }
                None => break,
            }
        }
        out.push(from16(&u[p..]));
        out.truncate(limit);
        Ok(rt.array(out))
    });
    method(rt, &sp, "match", |rt, this, a| {
        let s = this_str(rt, this)?;
        let re = match arg(a, 0) {
            v if re_of(&v).is_some() => v,
            Value::Undefined => rt.regexp("(?:)", "")?,
            v => rt.regexp(&rt.to_str(&v)?, "")?,
        };
        if !re_of(&re).unwrap().global {
            return exec(rt, &re, &s);
        }
        rt.set(&re, &"lastIndex".into(), Value::Num(0.0))?;
        let mut all = Vec::new();
        loop {
            let m = exec(rt, &re, &s)?;
            if matches!(m, Value::Null) {
                break;
            }
            let whole = rt.get(&m, "0")?;
            if rt.to_str(&whole)?.is_empty() {
                let li = last_index(rt, &re)?;
                rt.set(&re, &"lastIndex".into(), Value::Num((li + 1) as f64))?;
            }
            all.push(whole);
        }
        Ok(if all.is_empty() {
            Value::Null
        } else {
            rt.array(all)
        })
    });
    method(rt, &sp, "search", |rt, this, a| {
        let s = this_str(rt, this)?;
        let re = match arg(a, 0) {
            v if re_of(&v).is_some() => v,
            v => rt.regexp(&rt.to_str(&v)?, "")?,
        };
        let r = re_of(&re).unwrap();
        Ok(Value::Num(
            r.exec(&utf16(&s), 0)
                .map_or(-1.0, |c| c[0].unwrap().0 as f64),
        ))
    });
}

impl Rt {
    pub fn regexp(&self, src: &str, flags: &str) -> R<Value> {
        let r = match crate::regex::Regex::new(src, flags) {
            Ok(r) => r,
            Err(e) => {
                let p = self.global.lookup("SyntaxError");
                return match p {
                    Some(Prop::Method(f)) => {
                        Err(Exc::Throw(self.construct(&f, &[Value::str(&e)], &f)?))
                    }
                    _ => self.type_error(&e),
                };
            }
        };
        let flags_sorted: String = "dgimsuvy".chars().filter(|c| flags.contains(*c)).collect();
        let props = [
            (
                "source",
                Value::str(if src.is_empty() { "(?:)" } else { src }),
            ),
            ("flags", Value::str(&flags_sorted)),
            ("global", Value::Bool(r.global)),
            ("ignoreCase", Value::Bool(r.ignore_case)),
            ("multiline", Value::Bool(r.multiline)),
            ("sticky", Value::Bool(r.sticky)),
            ("unicode", Value::Bool(r.unicode)),
            ("dotAll", Value::Bool(r.dot_all)),
        ];
        let o = Obj::new(Some(self.regexp_proto.clone()), Kind::Regex(Box::new(r)));
        o.props
            .borrow_mut()
            .set(&"lastIndex".into(), Prop::Method(Value::Num(0.0)));
        for (k, v) in props {
            o.props.borrow_mut().set(&k.into(), Prop::Method(v));
        }
        Ok(Value::Obj(o))
    }
}

fn iteration(rt: &Rt, put: &dyn Fn(&str, Value)) {
    let symbol = rt.native("Symbol", |rt, _, a| {
        thread_local! {
            static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
        }
        let n = NEXT.with(|c| {
            c.set(c.get() + 1);
            c.get()
        });
        let desc = match a.first() {
            Some(Value::Undefined) | None => "".into(),
            Some(d) => rt.to_str(d)?,
        };
        Ok(Value::Str(format!("\0@{n}:{desc}").into()))
    });
    let so = symbol.as_obj().unwrap();
    for name in [
        "iterator",
        "asyncIterator",
        "hasInstance",
        "toPrimitive",
        "toStringTag",
        "species",
        "match",
        "replace",
        "search",
        "split",
        "unscopables",
        "isConcatSpreadable",
    ] {
        so.props.borrow_mut().set(
            &name.into(),
            Prop::Method(Value::Str(format!("\0@@{name}").into())),
        );
    }
    method(rt, so, "for", |rt, _, a| {
        Ok(Value::Str(
            format!("\0@for:{}", rt.to_str(&arg(a, 0))?).into(),
        ))
    });
    put("Symbol", symbol);

    let self_iter = |rt: &Rt, o: &Rc<Obj>| {
        let f = rt.native("[Symbol.iterator]", |_, this, _| Ok(this.clone()));
        o.props.borrow_mut().set(&ITERATOR.into(), Prop::Method(f));
    };
    let gp = rt.generator_proto.clone();
    self_iter(rt, &gp);
    method(rt, &gp, "next", |rt, this, _| {
        let Some(Kind::Generator(rest)) = this.as_obj().map(|o| &o.kind) else {
            return rt.type_error("next on a non-generator");
        };
        let v = rest.borrow_mut().next();
        Ok(rt.iter_result(v))
    });
    method(rt, &gp, "return", |rt, this, a| {
        if let Some(Kind::Generator(rest)) = this.as_obj().map(|o| &o.kind) {
            rest.borrow_mut().by_ref().for_each(drop);
        }
        let r = rt.iter_result(None);
        rt.set(&r, &"value".into(), arg(a, 0))?;
        Ok(r)
    });
    // Arrays, strings, maps and sets: iterate over a snapshot of their items.
    let list_iter = rt.native("[Symbol.iterator]", |rt, this, _| {
        Ok(rt.generator(rt.list(this)?))
    });
    for p in [&rt.array_proto, &rt.string_proto] {
        p.props
            .borrow_mut()
            .set(&ITERATOR.into(), Prop::Method(list_iter.clone()));
    }
    for n in [
        "Map",
        "Set",
        "Uint8Array",
        "Int8Array",
        "Uint8ClampedArray",
        "Uint16Array",
        "Int16Array",
        "Uint32Array",
        "Int32Array",
        "Float32Array",
        "Float64Array",
    ] {
        let c = rt.global.lookup(n);
        if let Some(Prop::Method(c)) = c {
            if let Value::Obj(p) = rt.get(&c, "prototype").unwrap_or_default() {
                p.props
                    .borrow_mut()
                    .set(&ITERATOR.into(), Prop::Method(list_iter.clone()));
            }
        }
    }
    for (proto, name) in [
        (&rt.array_proto, "keys"),
        (&rt.array_proto, "values"),
        (&rt.array_proto, "entries"),
    ] {
        let which = name;
        method(rt, proto, name, move |rt, this, _| {
            let items = rt.list(this)?;
            Ok(rt.generator(match which {
                "keys" => (0..items.len()).map(|i| Value::Num(i as f64)).collect(),
                "values" => items,
                _ => items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| rt.array(vec![Value::Num(i as f64), v]))
                    .collect(),
            }))
        });
    }
}

impl Rt {
    pub fn generator(&self, items: Vec<Value>) -> Value {
        Value::Obj(Obj::new(
            Some(self.generator_proto.clone()),
            Kind::Generator(RefCell::new(items.into_iter())),
        ))
    }

    pub fn iter_result(&self, v: Option<Value>) -> Value {
        let o = self.object();
        o.put_name(&"value".into(), v.clone().unwrap_or_default());
        o.put_name(&"done".into(), Value::Bool(v.is_none()));
        Value::Obj(o)
    }
}

fn is_array(v: &Value) -> bool {
    match v.as_obj().map(|o| &o.kind) {
        Some(Kind::Array(_)) => true,
        Some(Kind::Proxy(t, _)) => is_array(t),
        _ => false,
    }
}
