use djsugar_js::{Engine, Value};

fn ev(src: &str) -> String {
    let e = Engine::new();
    let v = e.eval(src).unwrap_or_else(|err| panic!("{src}\n{err}"));
    e.to_string(&v).unwrap()
}

fn out(src: &str) -> String {
    let e = Engine::new();
    e.eval(src).unwrap_or_else(|err| panic!("{src}\n{err}"));
    let v = e.get(&Value::Obj(e.global().clone()), "out").unwrap();
    e.to_string(&v).unwrap()
}

#[test]
fn arithmetic_and_strings() {
    assert_eq!(out("var out = 1 + 2 * 3 - 4 / 2 + 7 % 3 + 2 ** 3"), "14");
    assert_eq!(out("var out = '[Channel' + (1 + 1) + ']'"), "[Channel2]");
    assert_eq!(out("var out = (0x7f & 0x0f) | (1 << 4) ^ 3 >>> 0"), "31");
    assert_eq!(out("var out = -1 >>> 28"), "15");
    assert_eq!(out("var out = 0.1 + 0.2"), "0.30000000000000004");
    assert_eq!(out("var out = '5' * '2' + ('5' + 2)"), "1052");
    assert_eq!(
        out("var out = [1 == '1', 1 === '1', null == undefined, NaN == NaN, 'b' > 'a'].join()"),
        "true,false,true,false,true"
    );
    assert_eq!(out("var out = typeof x + typeof 1 + typeof '' + typeof {} + typeof null + typeof function(){}"), "undefinednumberstringobjectobjectfunction");
    assert_eq!(out("var out = `a${1 + 1}b${'c'}`"), "a2bc");
    assert_eq!(out("var out = (255).toString(16) + (3.14159).toFixed(2) + (1.005).toFixed(2) + (0.5).toFixed(0)"), "ff3.141.001");
}

#[test]
fn functions_closures_and_this() {
    assert_eq!(out("function mk(n) { var c = n; return function () { return ++c; }; } var f = mk(5); f(); var out = f();"), "7");
    assert_eq!(
        out("var o = { n: 2, get: function () { return this.n; } }; var out = o.get();"),
        "2"
    );
    assert_eq!(out("function F(x) { this.x = x; } F.prototype.y = function () { return this.x * 2; }; var out = new F(4).y();"), "8");
    assert_eq!(
        out("var o = { v: 3, f() { return [1].map(() => this.v)[0]; } }; var out = o.f();"),
        "3"
    );
    assert_eq!(out("function f(a, b = a + 1, ...r) { return a + b + r.length; } var out = f(1) + f(1, 1, 9, 9);"), "7");
    assert_eq!(
        out("function f() { return arguments.length; } var out = f(1, 2, 3);"),
        "3"
    );
    assert_eq!(out("var b = function (x) { return this.k + x; }.bind({ k: 1 }, 2); var out = b() + Math.max.apply(null, [3, 9]);"), "12");
    assert_eq!(
        out("var fact = function f(n) { return n < 2 ? 1 : n * f(n - 1); }; var out = fact(10);"),
        "3628800"
    );
}

#[test]
fn control_flow() {
    assert_eq!(out("var out = 0; for (var i = 0; i < 10; i++) { if (i % 2) continue; if (i > 6) break; out += i; }"), "12");
    assert_eq!(out("var out = ''; outer: for (var i = 0; i < 3; i++) { for (var j = 0; j < 3; j++) { if (j == 1) continue outer; if (i == 2) break outer; out += i + '' + j; } }"), "0010");
    assert_eq!(out("function f(x) { switch (x) { case 1: return 'a'; case 2: case 3: return 'b'; default: return 'c'; } } var out = f(1) + f(3) + f(9);"), "abc");
    assert_eq!(out("var out = ''; try { throw new TypeError('bad'); } catch (e) { out = e.name + ':' + e.message + (e instanceof Error); } finally { out += '!'; }"), "TypeError:badtrue!");
    assert_eq!(out("var out = ''; for (var k in { a: 1, b: 2 }) out += k; for (const v of [3, 4]) out += v;"), "ab34");
    assert_eq!(
        out("var i = 0, out = 0; do { out += i; } while (++i < 4); while (i--) out++;"),
        "10"
    );
    assert_eq!(
        out("var out; try { undefinedThing(); } catch (e) { out = e instanceof ReferenceError; }"),
        "true"
    );
}

#[test]
fn objects_arrays_and_destructuring() {
    assert_eq!(out("var a = [3, 1, 2]; a.sort(); a.push(5); var out = a.join('-') + a.length + a.indexOf(2);"), "1-2-3-541");
    assert_eq!(out("var out = [1, 2, 3, 4].filter(x => x % 2).map(x => x * 10).reduce((s, x) => s + x, 0);"), "40");
    assert_eq!(out("var { a, b: [c, d = 5], ...rest } = { a: 1, b: [2], e: 6, f: 7 }; var out = a + c + d + Object.keys(rest).join('');"), "8ef");
    assert_eq!(
        out("var o = { x: 1, ['y' + 1]: 2, ...{ z: 3 } }; var out = JSON.stringify(o);"),
        "{\"x\":1,\"y1\":2,\"z\":3}"
    );
    assert_eq!(out("var o = { _v: 1, get v() { return this._v * 2; }, set v(x) { this._v = x; } }; o.v = 5; var out = o.v;"), "10");
    assert_eq!(out("var a = []; a[3] = 1; var out = a.length + ',' + [, 1].length + ',' + [...'ab', ...[1]].join('');"), "4,2,ab1");
    assert_eq!(out("var o = JSON.parse('{\"a\":[1,2,{\"b\":\"\\\\u0041\"}]}'); var out = o.a[2].b + o.a.length;"), "A3");
    assert_eq!(out("var o = { a: 1 }; delete o.a; var out = ('a' in o) + ',' + o.hasOwnProperty('a') + ',' + (o?.b?.c ?? 'none');"), "false,false,none");
    assert_eq!(
        out("var x = 1, y = 2; [x, y] = [y, x]; var out = x * 10 + y;"),
        "21"
    );
}

#[test]
fn classes() {
    assert_eq!(
        out(r#"
        class A { #p = 1; static count = 0; constructor(n) { this.n = n; A.count++; }
          get p() { return this.#p; } inc() { this.#p++; return this; } static make() { return new this(9); } }
        class B extends A { constructor() { super(2); this.b = true; } inc() { super.inc(); return super.inc(); } }
        const b = new B(); b.inc();
        var out = [b.n, b.p, b.b, A.count, A.make().n, b instanceof A, typeof B].join();
    "#),
        "2,3,true,1,9,true,function"
    );
    assert_eq!(
        out("class P { x = 1; } class Q extends P { y = this.x + 1; } var out = new Q().y;"),
        "2"
    );
    assert_eq!(out("class E extends Error { constructor(m) { super(m); this.name = 'E'; } } var out = String(new E('boom'));"), "E: boom");
}

#[test]
fn strings_methods() {
    assert_eq!(out("var out = ' Ab,c '.trim().toLowerCase().split(',').concat('x'.padStart(3, '.')).join('|');"), "ab|c|..x");
    assert_eq!(out("var out = 'hello'.slice(-3) + 'hello'.substring(1, 3) + 'hello'.charCodeAt(1) + 'a-b-c'.replace('-', '+') + 'a-b'.replaceAll('-', '');"), "lloel101a+b-cab");
    assert_eq!(out("var out = 'ab'.repeat(2) + 'abc'.indexOf('c') + 'abc'.includes('d') + String.fromCharCode(72, 105);"), "abab2falseHi");
    assert_eq!(out("var out = parseInt('0x1f') + parseInt('12px') + parseFloat('3.5e1x') + Number('') + Number('a');"), "NaN");
    assert_eq!(
        out("var out = parseInt('0x1f') + parseInt('12px') + parseFloat('3.5e1x');"),
        "78"
    );
}

#[test]
fn errors_surface() {
    let e = Engine::new();
    assert!(e.eval("null.x").is_err());
    assert!(
        e.eval("(function f() { f(); })()").is_err(),
        "deep recursion is an error, not a crash"
    );
    assert!(e.eval("var 1").is_err());
    assert!(e
        .eval("throw new Error('x')")
        .unwrap_err()
        .to_string()
        .contains("Error: x"));
}

#[test]
fn host_functions() {
    let e = Engine::new();
    let g = Value::Obj(e.global().clone());
    let engine = e.object();
    e.set(
        &engine,
        "getValue",
        e.function("getValue", |_, _, a| match a.first() {
            Some(Value::Str(s)) if &**s == "[Master]" => Ok(Value::Num(0.5)),
            _ => Ok(Value::Num(0.0)),
        }),
    )
    .unwrap();
    e.set(&g, "engine", engine).unwrap();
    e.eval("var out = engine.getValue('[Master]') * 4").unwrap();
    assert_eq!(e.to_string(&e.get(&g, "out").unwrap()).unwrap(), "2");
    let _ = ev("1");
}

/// Every bundled mapping script: how many compile and run their top level
/// against a stand-in host.
#[test]
fn bundled_scripts() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/controllers");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .collect();
    files.sort();
    let mut fails = Vec::new();
    for p in &files {
        let src = std::fs::read_to_string(p).unwrap();
        let e = Engine::new();
        e.set_log(|_| {});
        e.eval(HOST).unwrap();
        for lib in [
            "lodash.mixxx.js",
            "common-controller-scripts.js",
            "midi-components-0.0.js",
        ] {
            if p.file_name().unwrap() != lib {
                let l = std::fs::read_to_string(dir.join(lib)).unwrap();
                let _ = e.eval(&l);
            }
        }
        if let Err(err) = e.eval(&src) {
            fails.push(format!(
                "{}: {err}",
                p.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    let ok = files.len() - fails.len();
    eprintln!("{ok} of {} scripts run\n{}", files.len(), fails.join("\n"));
    assert!(ok > 0);
}

const HOST: &str = r#"
var engine = new Proxy0();
function Proxy0() {}
["getValue", "setValue", "getParameter", "setParameter", "makeConnection", "connectControl", "trigger",
 "beginTimer", "stopTimer", "scratchEnable", "scratchTick", "scratchDisable", "isScratching", "brake",
 "spinback", "softTakeover", "softTakeoverIgnoreNextValue", "log", "getSetting", "makeUnbufferedConnection",
 "softStart", "getParameterForValue", "reset", "getDefaultValue", "getDefaultParameter"].forEach(function (n) {
    Proxy0.prototype[n] = function () { return n === "makeConnection" ? { disconnect: function () {}, trigger: function () {} } : 0; };
});
var midi = { sendShortMsg: function () {}, sendSysexMsg: function () {}, send: function () {}, makeInputHandler: function () {} };
var controller = { send: function () {}, sendOutputReport: function () {}, sendFeatureReport: function () {}, getInputReport: function () {}, getFeatureReport: function () { return []; } };
var script = {};
var svg = {};
var color = {};
"#;

#[test]
fn jog_handler_speed() {
    let e = Engine::new();
    e.eval(r#"
var state = { decks: [0,0,0,0], shift: false, leds: {} };
function setValue(g, k, v) { state.leds[g + k] = v; }
function wheelTurn(channel, control, value, status, group) {
  var deck = channel + 1; var delta = value - 64;
  if (state.shift) { delta *= 4; }
  state.decks[channel] += delta;
  setValue("[Channel" + deck + "]", "jog", delta / 3);
  for (var i = 0; i < 8; i++) { setValue("[Channel" + deck + "]", "hotcue_" + i + "_enabled", (state.decks[channel] >> i) & 1); }
}
"#).unwrap();
    let f = e.get(&Value::Obj(e.global().clone()), "wheelTurn").unwrap();
    let n = 20_000;
    let t = std::time::Instant::now();
    for i in 0..n {
        let a = [
            Value::Num((i % 4) as f64),
            Value::Num(32.0),
            Value::Num((60 + i % 8) as f64),
            Value::Num(176.0),
            Value::str("[Channel1]"),
        ];
        e.call(&f, Value::Undefined, &a).unwrap();
    }
    eprintln!(
        "djsugar-js {:.2} us/call",
        t.elapsed().as_secs_f64() * 1e6 / n as f64
    );
}
