//! A JavaScript interpreter for controller mapping scripts.

pub mod ast;
mod builtins;
mod coll;
mod date;
pub mod lexer;
pub mod parser;
mod regex;
mod run;
pub mod str;
pub mod value;

use std::rc::Rc;

use value::{Exc, Rt};
pub use value::{Obj, Value};

#[derive(Debug, Clone)]
pub enum Error {
    /// A syntax error, with its line and column.
    Syntax(String, usize, usize),
    /// Code this interpreter does not handle yet.
    Unsupported(String),
    /// An uncaught exception, as text.
    Thrown(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Syntax(m, l, c) => write!(f, "syntax error at {l}:{c}: {m}"),
            Error::Unsupported(m) => f.write_str(m),
            Error::Thrown(m) => write!(f, "uncaught {m}"),
        }
    }
}

impl std::error::Error for Error {}

#[repr(transparent)]
pub struct Engine {
    rt: Rt,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            rt: builtins::realm(),
        }
    }

    pub fn global(&self) -> &Rc<Obj> {
        &self.rt.global
    }

    /// Where `console.log` and `print` write.
    pub fn set_log(&self, f: impl FnMut(&str) + 'static) {
        *self.rt.log.borrow_mut() = Box::new(f);
    }

    /// Runs a script; its global declarations become globals.
    pub fn eval(&self, src: &str) -> Result<Value, Error> {
        let prog = parser::parse(src).map_err(|e| {
            let (l, c) = lexer::Lexer::line_col(src, e.pos);
            Error::Syntax(e.message, l, c)
        })?;
        let (body, nslots, vars) = run::Compiler::new(src)
            .program(&prog)
            .map_err(|e| Error::Unsupported(e.0))?;
        run::run_program(&self.rt, &body, nslots, &vars).map_err(|e| self.error(e))
    }

    pub fn call(&self, f: &Value, this: Value, args: &[Value]) -> Result<Value, Error> {
        self.rt.call(f, this, args).map_err(|e| self.error(e))
    }

    pub fn get(&self, v: &Value, name: &str) -> Result<Value, Error> {
        self.rt.get(v, name).map_err(|e| self.error(e))
    }

    pub fn set(&self, v: &Value, name: &str, x: Value) -> Result<(), Error> {
        self.rt.set(v, &name.into(), x).map_err(|e| self.error(e))
    }

    pub fn to_string(&self, v: &Value) -> Result<String, Error> {
        self.rt
            .to_str(v)
            .map(|s| s.to_string())
            .map_err(|e| self.error(e))
    }

    pub fn function(
        &self,
        name: &str,
        f: impl Fn(&Engine, &Value, &[Value]) -> Result<Value, String> + 'static,
    ) -> Value {
        self.rt.native(name, move |rt, this, args| {
            // SAFETY: `Engine` is a transparent wrapper around `Rt`.
            let engine = unsafe { &*(rt as *const Rt as *const Engine) };
            f(engine, this, args).or_else(|m| rt.throw(&rt.error_proto.clone(), &m))
        })
    }

    pub fn to_number(&self, v: &Value) -> Result<f64, Error> {
        self.rt.to_num(v).map_err(|e| self.error(e))
    }

    /// The items of an array or other iterable.
    pub fn list(&self, v: &Value) -> Result<Vec<Value>, Error> {
        self.rt.list(v).map_err(|e| self.error(e))
    }

    /// A new `Uint8Array` holding `data`.
    pub fn bytes(&self, data: &[u8]) -> Result<Value, Error> {
        let ctor = self.get(&Value::Obj(self.rt.global.clone()), "Uint8Array")?;
        let items = data.iter().map(|&b| Value::Num(b as f64)).collect();
        let arr = self.rt.array(items);
        self.rt
            .construct(&ctor, &[arr], &ctor)
            .map_err(|e| self.error(e))
    }

    pub fn object(&self) -> Value {
        Value::Obj(self.rt.object())
    }

    pub fn array(&self, items: Vec<Value>) -> Value {
        self.rt.array(items)
    }

    fn error(&self, e: Exc) -> Error {
        match e {
            Exc::Short => Error::Thrown("optional chain escaped".into()),
            Exc::Throw(v) => {
                let text = match &v {
                    Value::Obj(_) => self
                        .rt
                        .to_str(&v)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| "an exception".into()),
                    v => self.rt.to_str(v).map(|s| s.to_string()).unwrap_or_default(),
                };
                Error::Thrown(text)
            }
        }
    }
}

/// The shortest text that reads back as the same number, as JavaScript
/// writes numbers.
/// Appends `v` as JavaScript prints it.
#[inline]
pub fn push_number(out: &mut String, v: f64) {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        let mut n = v.abs() as u64;
        let mut buf = [0u8; 20];
        let mut i = buf.len();
        loop {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        if v < 0.0 {
            out.push('-');
        }
        // SAFETY: only ASCII digits were written.
        out.push_str(unsafe { std::str::from_utf8_unchecked(&buf[i..]) });
    } else {
        out.push_str(&number_to_string(v));
    }
}

pub fn number_to_string(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        let mut s = String::with_capacity(16);
        push_number(&mut s, v);
        return s;
    }
    if v.is_nan() {
        return "NaN".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let neg = v < 0.0;
    let a = v.abs();
    // Rust's `{:e}` gives the shortest round-tripping digits.
    let e = format!("{a:e}");
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    if k <= n && n <= 21 {
        s += &digits;
        s.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        s += &digits[..n as usize];
        s.push('.');
        s += &digits[n as usize..];
    } else if -6 < n && n <= 0 {
        s += "0.";
        s.extend(std::iter::repeat_n('0', (-n) as usize));
        s += &digits;
    } else {
        s += &digits[..1];
        if k > 1 {
            s.push('.');
            s += &digits[1..];
        }
        s.push('e');
        s.push(if n > 0 { '+' } else { '-' });
        s += &(n - 1).abs().to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn numbers_print_like_javascript() {
        for (v, s) in [
            (1.0, "1"),
            (0.1, "0.1"),
            (1e21, "1e+21"),
            (1e-7, "1e-7"),
            (123.456, "123.456"),
            (-0.000001, "-0.000001"),
            (1.5e300, "1.5e+300"),
            (100.0, "100"),
        ] {
            assert_eq!(super::number_to_string(v), s);
        }
    }
}
