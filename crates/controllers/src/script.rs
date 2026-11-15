//! Runs a mapping's JavaScript in QuickJS with the API Mixxx gives scripts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::context::EvalOptions;
use rquickjs::{CatchResultExt, Context, Ctx, Function, Object, Persistent, Runtime, Value};

use crate::host::Host;
use crate::mapping::SettingValue;

const PRELUDE: &str = include_str!("prelude.js");

pub(crate) struct Script {
    // Field order matters: functions must be dropped before the context and runtime.
    functions: RefCell<HashMap<String, Option<Persistent<Function<'static>>>>>,
    fire: Persistent<Function<'static>>,
    timer: Persistent<Function<'static>>,
    input: Persistent<Function<'static>>,
    context: Context,
    _runtime: Runtime,
    host: Rc<RefCell<Host>>,
}

fn options(file: &str) -> EvalOptions {
    let mut o = EvalOptions::default();
    // Mapping scripts are written for sloppy mode; most opt into strict themselves.
    o.strict = false;
    o.filename = Some(file.to_string());
    o
}

fn error_text<'js>(ctx: &Ctx<'js>, e: rquickjs::Error) -> String {
    match rquickjs::CaughtError::from_error(ctx, e) {
        rquickjs::CaughtError::Exception(ex) => {
            let msg = ex.message().unwrap_or_default();
            match ex.stack() {
                Some(stack) if !stack.is_empty() => format!("{msg}\n{stack}"),
                _ => msg,
            }
        }
        other => other.to_string(),
    }
}

impl Script {
    pub(crate) fn new(
        host: Rc<RefCell<Host>>,
        settings: &[(String, SettingValue)],
        hid: bool,
    ) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_max_stack_size(4 * 1024 * 1024);
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let (fire, timer, input) = context.with(|ctx| -> Result<_, String> {
            let native = natives(&ctx, &host).map_err(|e| error_text(&ctx, e))?;
            let settings_obj = Object::new(ctx.clone()).map_err(|e| e.to_string())?;
            for (k, v) in settings {
                let r = match v {
                    SettingValue::Bool(b) => settings_obj.set(k.as_str(), *b),
                    SettingValue::Number(n) => settings_obj.set(k.as_str(), *n),
                    SettingValue::Text(t) => settings_obj.set(k.as_str(), t.as_str()),
                };
                r.map_err(|e| e.to_string())?;
            }
            let install: Function = ctx
                .eval_with_options(PRELUDE, options("prelude.js"))
                .map_err(|e| error_text(&ctx, e))?;
            let hooks: Object = install
                .call((native, settings_obj, hid))
                .map_err(|e| error_text(&ctx, e))?;
            let fire: Function = hooks.get("fire").map_err(|e| e.to_string())?;
            let timer: Function = hooks.get("timer").map_err(|e| e.to_string())?;
            let input: Function = hooks.get("input").map_err(|e| e.to_string())?;
            Ok((
                Persistent::save(&ctx, fire),
                Persistent::save(&ctx, timer),
                Persistent::save(&ctx, input),
            ))
        })?;
        Ok(Self {
            functions: RefCell::new(HashMap::new()),
            fire,
            timer,
            input,
            context,
            _runtime: runtime,
            host,
        })
    }

    fn log(&self, message: &str) {
        self.host.borrow_mut().error(message);
    }

    /// Evaluates a script file in the global scope.
    pub(crate) fn eval_file(&self, name: &str, source: &str) -> Result<(), String> {
        let source = self.allow_shadowing(source);
        self.context.with(|ctx| {
            ctx.eval_with_options::<(), _>(source.as_ref(), options(name))
                .map_err(|e| format!("{name}: {}", error_text(&ctx, e)))
        })
    }

    /// Qt's engine lets a script declare a `class`, `const` or `let` with
    /// the name of a global an earlier file made with `var`: the Traktor
    /// Kontrol S4 MK3's `class Deck` and `common-controller-scripts.js`'s
    /// `var Deck`. QuickJS, following the standard, refuses. Such a
    /// declaration at the top of a file becomes a `var`, which replaces the
    /// earlier global as Qt's shadowing does for the file's own code.
    fn allow_shadowing<'a>(&self, source: &'a str) -> std::borrow::Cow<'a, str> {
        let clash = |line: &str| -> Option<(usize, String)> {
            let (keyword, rest) = ["class ", "const ", "let "]
                .iter()
                .find_map(|k| line.strip_prefix(k).map(|r| (*k, r)))?;
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            let is_var_global = !name.is_empty()
                && self.context.with(|ctx| {
                    ctx.eval_with_options::<bool, _>(
                        format!(
                            "Object.prototype.hasOwnProperty.call(globalThis, {name:?}) && \
                             !Object.getOwnPropertyDescriptor(globalThis, {name:?}).configurable"
                        ),
                        options("<eval>"),
                    )
                    .unwrap_or(false)
                });
            is_var_global.then_some((keyword.len(), name))
        };
        if !source.lines().any(|l| clash(l).is_some()) {
            return std::borrow::Cow::Borrowed(source);
        }
        let mut out = String::with_capacity(source.len() + 64);
        for line in source.split_inclusive('\n') {
            match clash(line) {
                Some((len, name)) if line.starts_with("class ") => {
                    out.push_str(&format!("var {name} = class "));
                    out.push_str(&line[len..]);
                }
                Some((len, _)) => {
                    out.push_str("var ");
                    out.push_str(&line[len..]);
                }
                None => out.push_str(line),
            }
        }
        std::borrow::Cow::Owned(out)
    }

    /// Evaluates an expression, logging errors. Used for `X.init`.
    pub(crate) fn eval(&self, code: &str) {
        self.context.with(|ctx| {
            if let Err(e) = ctx.eval_with_options::<Value, _>(code, options("<eval>")) {
                let msg = error_text(&ctx, e);
                self.log(&format!("{code}: {msg}"));
            }
        });
    }

    pub(crate) fn has_function(&self, expr: &str) -> bool {
        self.context.with(|ctx| {
            ctx.eval_with_options::<Value, _>(format!("typeof ({expr})"), options("<eval>"))
                .ok()
                .and_then(|v| v.as_string().and_then(|s| s.to_string().ok()))
                .is_some_and(|t| t == "function")
        })
    }

    /// Calls the function a `<key>` names, e.g. `XoneK2.decks[1].play.input`,
    /// with `this` bound as the expression implies.
    pub(crate) fn call_binding(&self, key: &str, args: (f64, f64, f64, f64, &str)) {
        let mut cache = self.functions.borrow_mut();
        let entry = cache.entry(key.to_string()).or_insert_with(|| {
            self.context.with(|ctx| {
                let code =
                    format!("(function (a1, a2, a3, a4, a5) {{ ({key})(a1, a2, a3, a4, a5); }})");
                match ctx.eval_with_options::<Function, _>(code, options("<binding>")) {
                    Ok(f) => Some(Persistent::save(&ctx, f)),
                    Err(e) => {
                        let msg = error_text(&ctx, e);
                        self.log(&format!("invalid script binding {key}: {msg}"));
                        None
                    }
                }
            })
        });
        let Some(f) = entry.clone() else { return };
        drop(cache);
        self.context.with(|ctx| {
            let result = f
                .restore(&ctx)
                .and_then(|f| f.call::<_, ()>(args))
                .catch(&ctx);
            if let Err(e) = result {
                self.log(&format!("{key}: {e}"));
            }
        });
    }

    /// Calls `prefix.incomingData(data, length)` for SysEx and other long messages.
    pub(crate) fn call_incoming_data(&self, prefix: &str, data: &[u8]) {
        self.context.with(|ctx| {
            let result = (|| {
                let f: Function = ctx.eval_with_options(
                    format!("{prefix}.incomingData.bind({prefix})"),
                    options("<incomingData>"),
                )?;
                f.call::<_, ()>((data.to_vec(), data.len()))
            })()
            .catch(&ctx);
            if let Err(e) = result {
                self.log(&format!("{prefix}.incomingData: {e}"));
            }
        });
    }

    /// Calls `prefix.incomingData(data, length)` with an HID input report,
    /// as a `Uint8Array` (its first byte is the report ID, for devices that
    /// number their reports).
    pub(crate) fn call_incoming_report(&self, prefix: &str, data: &[u8]) {
        self.context.with(|ctx| {
            let result = (|| {
                let f: Function = ctx.eval_with_options(
                    format!("{prefix}.incomingData.bind({prefix})"),
                    options("<incomingData>"),
                )?;
                let bytes = rquickjs::TypedArray::<u8>::new_copy(ctx.clone(), data)?;
                f.call::<_, ()>((bytes, data.len()))
            })()
            .catch(&ctx);
            if let Err(e) = result {
                self.log(&format!("{prefix}.incomingData: {e}"));
            }
        });
    }

    pub(crate) fn fire_connection(&self, id: u32, value: f64) {
        self.call_hook(&self.fire, id, value, "connection callback");
    }

    /// Runs the `midi.makeInputHandler` callbacks for a message.
    pub(crate) fn fire_input(&self, status: u8, control: u8, value: u8) {
        self.context.with(|ctx| {
            let result = self
                .input
                .clone()
                .restore(&ctx)
                .and_then(|f| f.call::<_, ()>((status, control, value)))
                .catch(&ctx);
            if let Err(e) = result {
                self.log(&format!("input handler: {e}"));
            }
        });
    }

    pub(crate) fn fire_timer(&self, id: u32) {
        self.call_hook(&self.timer, id, 0.0, "timer callback");
    }

    fn call_hook(&self, hook: &Persistent<Function<'static>>, id: u32, value: f64, what: &str) {
        self.context.with(|ctx| {
            let result = hook
                .clone()
                .restore(&ctx)
                .and_then(|f| f.call::<_, ()>((id, value)))
                .catch(&ctx);
            if let Err(e) = result {
                self.log(&format!("{what}: {e}"));
            }
        });
    }
}

/// The native functions behind the prelude's `engine` and `midi` objects.
/// Each borrows the host only for its own duration and never calls back into JS.
fn natives<'js>(ctx: &Ctx<'js>, host: &Rc<RefCell<Host>>) -> rquickjs::Result<Object<'js>> {
    let o = Object::new(ctx.clone())?;
    macro_rules! native {
        ($name:literal, |$h:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let host = host.clone();
            o.set(
                $name,
                Function::new(ctx.clone(), move |$($arg: $ty),*| {
                    #[allow(unused_mut)]
                    let mut $h = host.borrow_mut();
                    $body
                })?,
            )?;
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
    Ok(o)
}
