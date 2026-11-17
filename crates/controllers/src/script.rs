//! Runs a mapping's JavaScript with the API Mixxx gives scripts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use djsugar_js::{Engine, Value};

use crate::host::Host;
use crate::mapping::SettingValue;

const PRELUDE: &str = include_str!("prelude.js");

pub(crate) struct Script {
    functions: RefCell<HashMap<String, Option<Value>>>,
    fire: Value,
    timer: Value,
    input: Value,
    engine: Engine,
    host: Rc<RefCell<Host>>,
}

fn num(v: f64) -> Value {
    Value::Num(v)
}

impl Script {
    pub(crate) fn new(
        host: Rc<RefCell<Host>>,
        settings: &[(String, SettingValue)],
        hid: bool,
    ) -> Result<Self, String> {
        let engine = Engine::new();
        let err = |e: djsugar_js::Error| e.to_string();
        let native = natives(&engine, &host);
        let settings_obj = engine.object();
        for (k, v) in settings {
            let v = match v {
                SettingValue::Bool(b) => Value::Bool(*b),
                SettingValue::Number(n) => num(*n),
                SettingValue::Text(t) => Value::Str(t.as_str().into()),
            };
            engine.set(&settings_obj, k, v).map_err(err)?;
        }
        let install = engine
            .eval(PRELUDE)
            .map_err(|e| format!("prelude.js: {e}"))?;
        let hooks = engine
            .call(
                &install,
                Value::Undefined,
                &[native, settings_obj, Value::Bool(hid)],
            )
            .map_err(|e| format!("prelude.js: {e}"))?;
        let fire = engine.get(&hooks, "fire").map_err(err)?;
        let timer = engine.get(&hooks, "timer").map_err(err)?;
        let input = engine.get(&hooks, "input").map_err(err)?;
        Ok(Self {
            functions: RefCell::new(HashMap::new()),
            fire,
            timer,
            input,
            engine,
            host,
        })
    }

    fn log(&self, message: &str) {
        self.host.borrow_mut().error(message);
    }

    /// Evaluates a script file in the global scope.
    pub(crate) fn eval_file(&self, name: &str, source: &str) -> Result<(), String> {
        self.engine
            .eval(source)
            .map(drop)
            .map_err(|e| format!("{name}: {e}"))
    }

    /// Evaluates an expression, logging errors. Used for `X.init`.
    pub(crate) fn eval(&self, code: &str) {
        if let Err(e) = self.engine.eval(code) {
            self.log(&format!("{code}: {e}"));
        }
    }

    pub(crate) fn has_function(&self, expr: &str) -> bool {
        self.engine
            .eval(&format!("typeof ({expr})"))
            .is_ok_and(|v| matches!(v, Value::Str(s) if &*s == "function"))
    }

    /// Calls the function a `<key>` names, e.g. `XoneK2.decks[1].play.input`,
    /// with `this` bound as the expression implies.
    pub(crate) fn call_binding(&self, key: &str, args: (f64, f64, f64, f64, &str)) {
        let mut cache = self.functions.borrow_mut();
        let entry = cache.entry(key.to_string()).or_insert_with(|| {
            let code =
                format!("(function (a1, a2, a3, a4, a5) {{ ({key})(a1, a2, a3, a4, a5); }})");
            match self.engine.eval(&code) {
                Ok(f) => Some(f),
                Err(e) => {
                    self.log(&format!("invalid script binding {key}: {e}"));
                    None
                }
            }
        });
        let Some(f) = entry.clone() else { return };
        drop(cache);
        let a = [
            num(args.0),
            num(args.1),
            num(args.2),
            num(args.3),
            Value::Str(args.4.into()),
        ];
        if let Err(e) = self.engine.call(&f, Value::Undefined, &a) {
            self.log(&format!("{key}: {e}"));
        }
    }

    fn incoming(&self, prefix: &str, data: Value, len: usize) {
        let r = (|| {
            let o = self.engine.eval(prefix)?;
            let f = self.engine.get(&o, "incomingData")?;
            self.engine.call(&f, o, &[data, num(len as f64)])
        })();
        if let Err(e) = r {
            self.log(&format!("{prefix}.incomingData: {e}"));
        }
    }

    /// Calls `prefix.incomingData(data, length)` for SysEx and other long messages.
    pub(crate) fn call_incoming_data(&self, prefix: &str, data: &[u8]) {
        let a = self
            .engine
            .array(data.iter().map(|&b| num(b as f64)).collect());
        self.incoming(prefix, a, data.len());
    }

    /// Calls `prefix.incomingData(data, length)` with an HID input report,
    /// as a `Uint8Array` (its first byte is the report ID, for devices that
    /// number their reports).
    pub(crate) fn call_incoming_report(&self, prefix: &str, data: &[u8]) {
        match self.engine.bytes(data) {
            Ok(b) => self.incoming(prefix, b, data.len()),
            Err(e) => self.log(&format!("{prefix}.incomingData: {e}")),
        }
    }

    pub(crate) fn fire_connection(&self, id: u32, value: f64) {
        self.call_hook(&self.fire, id, value, "connection callback");
    }

    /// Runs the `midi.makeInputHandler` callbacks for a message.
    pub(crate) fn fire_input(&self, status: u8, control: u8, value: u8) {
        let a = [num(status as f64), num(control as f64), num(value as f64)];
        if let Err(e) = self.engine.call(&self.input, Value::Undefined, &a) {
            self.log(&format!("input handler: {e}"));
        }
    }

    pub(crate) fn fire_timer(&self, id: u32) {
        self.call_hook(&self.timer, id, 0.0, "timer callback");
    }

    fn call_hook(&self, hook: &Value, id: u32, value: f64, what: &str) {
        if let Err(e) = self
            .engine
            .call(hook, Value::Undefined, &[num(id as f64), num(value)])
        {
            self.log(&format!("{what}: {e}"));
        }
    }
}

/// Script values to native argument types, leniently as scripts expect.
trait Arg: Sized {
    fn arg(e: &Engine, v: &Value) -> Result<Self, String>;
}

impl Arg for String {
    fn arg(e: &Engine, v: &Value) -> Result<Self, String> {
        e.to_string(v).map_err(|x| x.to_string())
    }
}

impl Arg for f64 {
    fn arg(e: &Engine, v: &Value) -> Result<Self, String> {
        e.to_number(v).map_err(|x| x.to_string())
    }
}

impl Arg for bool {
    fn arg(_: &Engine, v: &Value) -> Result<Self, String> {
        Ok(v.truthy())
    }
}

macro_rules! int_arg {
    ($($t:ty),*) => {$(
        impl Arg for $t {
            fn arg(e: &Engine, v: &Value) -> Result<Self, String> {
                let n = f64::arg(e, v)?;
                Ok(if n.is_finite() { n as i64 as $t } else { 0 })
            }
        }
    )*};
}
int_arg!(u8, u32, i32);

impl Arg for Vec<f64> {
    fn arg(e: &Engine, v: &Value) -> Result<Self, String> {
        if v.is_nullish() {
            return Ok(Vec::new());
        }
        let items = e.list(v).map_err(|x| x.to_string())?;
        items.iter().map(|x| f64::arg(e, x)).collect()
    }
}

/// Native results back to script values.
trait Ret {
    fn ret(self, e: &Engine) -> Value;
}

impl Ret for () {
    fn ret(self, _: &Engine) -> Value {
        Value::Undefined
    }
}

impl Ret for bool {
    fn ret(self, _: &Engine) -> Value {
        Value::Bool(self)
    }
}

impl Ret for f64 {
    fn ret(self, _: &Engine) -> Value {
        Value::Num(self)
    }
}

macro_rules! int_ret {
    ($($t:ty),*) => {$(
        impl Ret for $t {
            fn ret(self, _: &Engine) -> Value {
                Value::Num(self as f64)
            }
        }
    )*};
}
int_ret!(u8, u32, i32, u64, usize);

impl Ret for Vec<u8> {
    fn ret(self, e: &Engine) -> Value {
        e.array(self.into_iter().map(|b| Value::Num(b as f64)).collect())
    }
}

/// The native functions behind the prelude's `engine` and `midi` objects.
/// Each borrows the host only for its own duration and never calls back into JS.
fn natives(engine: &Engine, host: &Rc<RefCell<Host>>) -> Value {
    let o = engine.object();
    macro_rules! native {
        ($name:literal, |$h:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let host = host.clone();
            let f = engine.function($name, move |e, _, args| {
                let mut _i = 0;
                $(
                    let $arg: $ty = <$ty as Arg>::arg(e, args.get(_i).unwrap_or(&Value::Undefined))?;
                    _i += 1;
                )*
                #[allow(unused_mut)]
                let mut $h = host.borrow_mut();
                let r = $body;
                drop($h);
                Ok(Ret::ret(r, e))
            });
            engine.set(&o, $name, f).expect("plain object");
        }};
    }
    native!("get", |h, g: String, k: String| h.resolve(&g, &k).get());
    native!("set", |h, g: String, k: String, v: f64| h
        .set_value(&g, &k, v));
    native!("getParameter", |h, g: String, k: String| h
        .resolve(&g, &k)
        .parameter());
    native!("setParameter", |h, g: String, k: String, p: f64| h
        .set_parameter(&g, &k, p));
    native!("parameterForValue", |h, g: String, k: String, v: f64| h
        .resolve(&g, &k)
        .parameter_for_value(v));
    native!("reset", |h, g: String, k: String| {
        let t = h.resolve(&g, &k);
        h.record(&k, &t, t.default_value());
        t.set(t.default_value());
    });
    native!("defaultValue", |h, g: String, k: String| h
        .resolve(&g, &k)
        .default_value());
    native!("defaultParameter", |h, g: String, k: String| {
        let t = h.resolve(&g, &k);
        t.parameter_for_value(t.default_value())
    });
    native!("watch", |h, g: String, k: String| h.watch(&g, &k));
    native!("unwatch", |h, id: u32| h.unwatch(id));
    native!("trigger", |h, g: String, k: String| h.trigger(&g, &k));
    native!("inputHandlerAllowed", |h, status: u8, control: u8| {
        !h.script_bound.contains(&(status, control))
    });
    native!("inputHandlers", |h, status: u8, control: u8, delta: i32| {
        h.add_input_handlers(status, control, delta)
    });
    native!("send", |h, bytes: Vec<f64>| {
        let msg: Vec<u8> = bytes.iter().map(|b| (*b as i64 & 0xFF) as u8).collect();
        if !msg.is_empty() {
            h.sent.push(msg);
        }
    });
    // HID: `controller.sendOutputReport` and friends (see `hid`).
    let bytes =
        |data: &[f64]| -> Vec<u8> { data.iter().map(|b| (*b as i64 & 0xFF) as u8).collect() };
    native!("hidOutput", |h, id: u8, data: Vec<f64>, fifo: bool| {
        match &mut h.hid {
            Some(hid) => hid.out.push(id, &bytes(&data), fifo),
            None => h.error("controller.sendOutputReport: not an HID controller"),
        }
    });
    native!("hidSendFeature", |h, id: u8, data: Vec<f64>| {
        let mut report = vec![id];
        report.extend(bytes(&data));
        let result = match &h.hid {
            Some(hid) => hid.io.send_feature(&report),
            None => Err("not an HID controller".into()),
        };
        if let Err(e) = result {
            h.error(&format!("controller.sendFeatureReport({id}): {e}"));
        }
    });
    native!("hidGetInput", |h, id: u8| {
        let got = h.hid.as_ref().and_then(|hid| hid.io.get_input(id));
        if got.is_none() {
            h.log(&format!(
                "controller.getInputReport({id}): the device didn't answer"
            ));
        }
        got.unwrap_or_default()
    });
    native!("hidGetFeature", |h, id: u8| {
        let got = h.hid.as_ref().and_then(|hid| hid.io.get_feature(id));
        if got.is_none() {
            h.log(&format!(
                "controller.getFeatureReport({id}): the device didn't answer"
            ));
        }
        got.unwrap_or_default()
    });
    native!("routeNote", |h, channel: u8, from: u8, to: u8| h
        .route_note(channel, from, to));
    native!("log", |h, m: String| h.log(&m));
    native!("timerStart", |h, ms: f64, one_shot: bool| h
        .timer_start(ms, one_shot));
    native!("timerStop", |h, id: u32| h.timer_stop(id));
    native!("softTakeover", |h, g: String, k: String, on: bool| h
        .soft_takeover(&g, &k, on));
    native!("softTakeoverIgnoreNextValue", |h, g: String, k: String| h
        .soft_takeover_ignore_next(&g, &k));
    native!(
        "softTakeoverWillIgnore",
        |h, g: String, k: String, p: f64| h.soft_takeover_will_ignore(&g, &k, p)
    );
    native!("scratchEnable", |h,
                              deck: f64,
                              ipr: f64,
                              rpm: f64,
                              alpha: f64,
                              beta: f64,
                              ramp: bool| h
        .scratch_enable(deck, ipr, rpm, alpha, beta, ramp));
    native!("scratchTick", |h, deck: f64, n: f64| h
        .scratch_tick(deck, n));
    native!("scratchDisable", |h, deck: f64, ramp: bool| h
        .scratch_disable(deck, ramp));
    native!("isScratching", |h, deck: f64| h.is_scratching(deck));
    native!("ramp", |h,
                     deck: f64,
                     kind: String,
                     on: bool,
                     factor: f64,
                     rate: f64| h
        .ramp(deck, &kind, on, factor, rate));
    native!("rampActive", |h, deck: f64, kind: String| h
        .ramp_active(deck, &kind));
    o
}
