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
    assert_eq!(out("var out = [Math.clz32(1), Math.imul(0xffffffff, 5), eval('1 + 2'), ArrayBuffer.isView(new Uint8Array(1))].join();"), "31,-5,3,true");
    assert_eq!(out("function* r(n) { for (let i = 0; i < n; i++) yield i; } function* m(f, it) { for (const x of it) yield f(x); } var out = [...m(x => x * 2, r(4))].join() + [...r(0)].length;"), "0,2,4,60");
    assert_eq!(out("class C { constructor() { this.a = 1; this.b = 2; } *[Symbol.iterator]() { for (const k in this) yield this[k]; yield* [9]; } } var out = 0; for (const v of new C()) out += v;"), "12");
    assert_eq!(out("var it = { [Symbol.iterator]() { let i = 0; return { next: () => ({ value: i, done: i++ > 2 }) }; } }; var out = Array.from(it).join() + [...new Set([3, 3, 4])].join();"), "0,1,23,4");
    assert_eq!(out("var p = new Proxy([1, 2], { get: (t, k) => k === '0' ? 'x' : t[k] }); var out = p[0] + p[1] + p.length + Array.isArray(p) + ('1' in p);"), "x22truetrue");
    assert_eq!(out("var a = []; a[2] = 'c'; a[1] = 'b'; var k = ''; for (var i in a) k += i; delete a[1]; var out = k + a.length + (1 in a) + a.hasOwnProperty(2) + a[0] + [,1].length + Object.keys(new Array(3)).length;"), "123falsetrueundefined20");
    assert_eq!(out("var o = { valueOf() { return 2; } }; var out = 1 + 2 + '3' + 4 + o + null + [5, 6] + 1 + 1;"), "3342null5,611");
    assert_eq!(
        out("var o = { valueOf() { return 2; } }; var out = String(1 + o + 3) + (o + o + '');"),
        "64"
    );
    assert_eq!(out("var g = (function* () { yield 1; yield 2; })(); var a = g.next(), b = g.next(), c = g.next(); var out = [a.value, b.value, c.done, typeof Symbol.iterator].join();"), "1,2,true,string");
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

/// Every bundled mapping, loaded like the app does: the prelude, then the
/// script files its XML lists, then each `init` call.
#[test]
fn bundled_mappings() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/controllers");
    let prelude =
        std::fs::read_to_string(dir.join("../../crates/controllers/src/prelude.js")).unwrap();
    let mut xmls: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "xml"))
        .collect();
    xmls.sort();
    let (mut ok, mut total, mut fails) = (0, 0, Vec::new());
    for x in &xmls {
        let xml = std::fs::read_to_string(x).unwrap();
        let mut files = vec!["common-controller-scripts.js".to_string()];
        let mut prefixes = Vec::new();
        for tag in xml.split("<file ").skip(1) {
            let attr = |n: &str| {
                tag.split(&format!("{n}=\""))
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .map(str::to_string)
            };
            if let Some(f) = attr("filename") {
                if !files.contains(&f) {
                    files.push(f);
                }
            }
            if let Some(p) = attr("functionprefix").filter(|p| !p.is_empty()) {
                prefixes.push(p);
            }
        }
        if files.len() == 1 {
            continue;
        }
        total += 1;
        let e = Engine::new();
        e.set_log(|_| {});
        let hid = xml.contains("<hid")
            || xml.contains("HIDController")
            || x.to_string_lossy().contains("hid");
        let setup = prelude.clone();
        let r = e.eval(NATIVE).and_then(|_| e.eval(&setup)).and_then(|f| {
            let g = Value::Obj(e.global().clone());
            let native = e.get(&g, "native0")?;
            e.call(
                &f,
                Value::Undefined,
                &[native, settings(&e, &xml), Value::Bool(hid)],
            )
        });
        let mut err = r.err().map(|e| format!("prelude: {e}"));
        for f in &files {
            if err.is_some() {
                break;
            }
            match std::fs::read_to_string(dir.join(f)) {
                Ok(src) => err = e.eval(&src).err().map(|e| format!("{f}: {e}")),
                Err(_) => err = Some(format!("{f}: missing")),
            }
        }
        for p in &prefixes {
            if err.is_some() {
                break;
            }
            err = e.eval(&format!("if (typeof {p} !== 'undefined' && typeof {p}.init === 'function') {p}.init('test', false);")).err().map(|e| format!("{p}.init: {e}"));
        }
        match err {
            None => ok += 1,
            Some(m) => fails.push(format!("{}: {m}", x.file_name().unwrap().to_string_lossy())),
        }
    }
    eprintln!(
        "{ok} of {total} mappings load and init\n{}",
        fails.join("\n")
    );
    assert!(ok > 0);
}

const NATIVE: &str = r#"
function Proxy0() {}
var native0 = new Proxy0();
var watchId = 0;
["defaultParameter", "defaultValue", "get", "getParameter", "hidGetFeature", "hidGetInput", "hidOutput", "hidSendFeature",
 "inputHandlerAllowed", "inputHandlers", "isScratching", "log", "parameterForValue", "ramp", "rampActive", "reset", "routeNote",
 "scratchDisable", "scratchEnable", "scratchTick", "send", "set", "setParameter", "softTakeover", "softTakeoverIgnoreNextValue",
 "softTakeoverWillIgnore", "timerStart", "timerStop", "trigger", "unwatch"].forEach(function (n) { native0[n] = function () { return 0; }; });
native0.watch = function () { return ++watchId; };
native0.timerStart = function () { return ++watchId; };
native0.inputHandlerAllowed = function () { return true; };
native0.hidGetInput = function () { return new Uint8Array(256); };
native0.hidGetFeature = function () { return []; };
native0.get = function (g, k) { return /^num_/.test(k) ? 3 : 0; };
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
    let mut best = f64::MAX;
    for _ in 0..5 {
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
        best = best.min(t.elapsed().as_secs_f64() * 1e6 / n as f64);
    }
    eprintln!("djsugar-js {best:.2} us/call (best of 5)");
}

#[test]
fn regular_expressions() {
    assert_eq!(
        out(
            r#"var m = /\[Channel(\d)\]/.exec("[Channel3]"); var out = m[1] + m.index + /x/i.test("X");"#
        ),
        "30true"
    );
    assert_eq!(
        out(r#"var out = "a1b22c333".replace(/\d+/g, n => "<" + n.length + ">");"#),
        "a<1>b<2>c<3>"
    );
    assert_eq!(
        out(r#"var out = "2024-01-05".replace(/(\d+)-(\d+)-(\d+)/, "$3/$2/$1");"#),
        "05/01/2024"
    );
    assert_eq!(
        out(r#"var out = "a, b,c".split(/\s*,\s*/).join("|") + "x".split(/(x)/).length;"#),
        "a|b|c3"
    );
    assert_eq!(
        out(
            r#"var out = "one two three".match(/\w+/g).length + "abc".search(/c/) + String(/a\/b/g);"#
        ),
        "5/a\\/b/g"
    );
    assert_eq!(
        out(
            r#"var r = /o/g, n = 0; while (r.exec("foo boo")) n++; var out = n + new RegExp("^[a-z]+$").test("abc");"#
        ),
        "5"
    );
    assert_eq!(
        out(
            r#"var out = "John Smith".replace(/(?<first>\w+) (?<last>\w+)/, "$<last>, $<first>");"#
        ),
        "Smith, John"
    );
}

#[test]
fn collections_and_bytes() {
    assert_eq!(out("var m = new Map([[1, 'a']]); m.set('k', 2).set(NaN, 3); m.delete(1); var out = m.size + ',' + m.get(NaN) + ',' + [...m.keys()].join('/');"), "2,3,k/NaN");
    assert_eq!(out("var s = new Set([1, 1, 2]); s.add(3); var t = 0; s.forEach(v => t += v); var out = s.size + ',' + t + s.has(2);"), "3,6true");
    assert_eq!(out("var b = new Uint8Array([1, 2, 300]); b[0] = -1; var out = b.length + ',' + Array.from(b).join('/') + ',' + b.subarray(1).length;"), "3,255/2/44,2");
    assert_eq!(out("var d = new DataView(new ArrayBuffer(4)); d.setUint16(0, 0x1234); d.setInt8(2, -2); var out = d.getUint8(0) + ',' + d.getUint16(0, true) + ',' + d.getUint8(2);"), "18,13330,254");
    assert_eq!(out("var f = new Function('a', 'b', 'return a * b + this.x'); var x = 1; var out = f(3, 4) + Function('return this')().x;"), "14");
}

/// A components-style handler: prototype methods, `this` fields, closures.
#[test]
fn component_handler_speed() {
    let e = Engine::new();
    e.eval(r#"
var engine = { values: {}, setValue: function (g, k, v) { this.values[k] = v; }, getValue: function (g, k) { return this.values[k] || 0; } };
function Component(o) { this.group = o.group; this.key = o.key; this.max = 127; this.shift = false; }
Component.prototype.inValueScale = function (v) { return v / this.max; };
Component.prototype.input = function (ch, ctrl, value, status) {
    var v = this.inValueScale(value);
    if (this.shift) { v = 1 - v; }
    engine.setValue(this.group, this.key, v);
    this.output(engine.getValue(this.group, this.key));
};
Component.prototype.output = function (v) { this.last = v > 0.5 ? 0x7f : 0; };
function Pot(o) { Component.call(this, o); }
Pot.prototype = Object.create(Component.prototype);
Pot.prototype.inValueScale = function (v) { return Component.prototype.inValueScale.call(this, v) * 2; };
var pots = [];
for (var i = 0; i < 8; i++) { pots.push(new Pot({ group: "[Channel1]", key: "k" + i })); }
function handle(ch, ctrl, value, status) { for (var i = 0; i < pots.length; i++) { pots[i].input(ch, ctrl, value + i, status); } }
"#).unwrap();
    let f = e.get(&Value::Obj(e.global().clone()), "handle").unwrap();
    let n = 20_000;
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = std::time::Instant::now();
        for i in 0..n {
            let a = [
                Value::Num(0.0),
                Value::Num(7.0),
                Value::Num((i % 128) as f64),
                Value::Num(176.0),
            ];
            e.call(&f, Value::Undefined, &a).unwrap();
        }
        best = best.min(t.elapsed().as_secs_f64() * 1e6 / n as f64);
    }
    eprintln!("djsugar-js components {best:.2} us/call (best of 5)");
}

#[test]
fn direct_calls() {
    assert_eq!(
        ev("function f(a, b) { return [a, b].join(); } f(1) + '|' + f(1, 2, 3)"),
        "1,|1,2"
    );
    assert_eq!(
        ev("function f(a) { return arguments.length; } f(1, 2, 3)"),
        "3"
    );
    assert_eq!(
        ev("function f(n) { return n ? 1 + f(n - 1) : 0; } f(100)"),
        "100"
    );
    assert_eq!(ev("function f(n) { return f(n + 1); } var r; try { f(0) } catch (e) { r = e instanceof RangeError } r"), "true");
    assert_eq!(
        ev("function f(n) { return f(n + 1); } try { f(0) } catch (e) {} f.length"),
        "1"
    );
    assert_eq!(ev("function g() { throw 5; } function f(a) { return a; } var r; try { f(g()) } catch (e) { r = e } r"), "5");
    assert_eq!(
        ev("var o = { k: 2, m: function (x) { return this.k * x; } }; o.m(4)"),
        "8"
    );
}

#[test]
fn natives_survive_engine_moves() {
    let e = Engine::new();
    let f = e.function("twice", |e, _, a| {
        Ok(Value::Num(
            e.to_number(&a[0]).map_err(|x| x.to_string())? * 2.0,
        ))
    });
    let moved = Box::new(e);
    let g = moved.global().clone();
    moved.set(&Value::Obj(g), "twice", f).unwrap();
    let v = moved.eval("twice(21)").unwrap();
    assert_eq!(moved.to_string(&v).unwrap(), "42");
}

/// A mapping's setting defaults, as the controller runner passes them.
fn settings(e: &Engine, xml: &str) -> Value {
    let o = e.object();
    for opt in xml.split("<option").skip(1) {
        let opt = &opt[..opt
            .find("</option>")
            .or_else(|| opt.find("/>"))
            .unwrap_or(opt.len())];
        let head = &opt[..opt.find('>').unwrap_or(opt.len())];
        let attr = |src: &str, k: &str| {
            src.split(&format!("{k}=\""))
                .nth(1)
                .and_then(|r| r.split('"').next())
                .map(str::to_string)
        };
        let (Some(var), ty) = (
            attr(head, "variable"),
            attr(head, "type").unwrap_or_default(),
        ) else {
            continue;
        };
        let text = match attr(head, "default") {
            Some(d) => d,
            None => opt
                .split("<value")
                .skip(1)
                .find(|v| v[..v.find('>').unwrap_or(0)].contains("default=\"true\""))
                .or_else(|| opt.split("<value").nth(1))
                .and_then(|v| v.split('>').nth(1))
                .and_then(|v| v.split('<').next())
                .unwrap_or("")
                .trim()
                .to_string(),
        };
        let v = match ty.as_str() {
            "boolean" => Value::Bool(text == "true"),
            "integer" | "real" => Value::Num(text.parse().unwrap_or(0.0)),
            _ => Value::Str(text.as_str().into()),
        };
        e.set(&o, &var, v).unwrap();
    }
    o
}

#[test]
fn loop_bindings_per_iteration() {
    assert_eq!(ev("var fs = []; for (let i = 0; i < 3; i++) { fs.push(function () { return i; }); } fs.map(function (f) { return f(); }).join()"), "0,1,2");
    assert_eq!(ev("var fs = []; for (const o of [0, 1]) { for (let i = 0; i < 2; i++) { fs.push(() => o + ':' + i); } } fs.map(f => f()).join()"), "0:0,0:1,1:0,1:1");
    assert_eq!(ev("var fs = []; for (const k in { a: 1, b: 2 }) { fs.push(() => k); } fs.map(f => f()).join()"), "a,b");
    assert_eq!(
        ev("var s = 0; for (let i = 0; i < 4; i++) { if (i === 1) { continue; } s += i; } s"),
        "5"
    );
    assert_eq!(ev("var fs = []; for (let i = 0; i < 3; i++) { fs.push(() => i); i++; } fs.map(f => f()).join()"), "1,3");
    assert_eq!(
        ev("var x = 1; for (let i = 0; i < 2; i++) { var g = () => x + i; } g()"),
        "2"
    );
}

#[test]
fn exceptions_through_finally() {
    assert_eq!(ev("var r; try { try { throw 1; } finally { try { throw 2; } catch (e) {} } } catch (e) { r = e; } r"), "1");
    assert_eq!(ev("var r; try { throw new TypeError('x'); } catch (e) { r = e.message + (e instanceof TypeError); } r"), "xtrue");
    assert_eq!(
        ev("function f() { try { return 1; } finally { r = 2; } } var r; f() + r"),
        "3"
    );
    assert_eq!(
        ev("var r; try { [3, 1, 2].sort(function () { throw 7; }); } catch (e) { r = e; } r"),
        "7"
    );
    assert_eq!(ev("var o = null; o?.x.y === undefined"), "true");
}

#[test]
fn return_through_finally() {
    assert_eq!(
        ev("function g() { return 2; } function f() { try { return 1; } finally { g(); } } f()"),
        "1"
    );
    assert_eq!(
        ev("function f() { try { return 1; } finally { return 3; } } f()"),
        "3"
    );
    assert_eq!(ev("function f() { for (const x of [1, 2]) { try { return x; } finally { [5].map(y => y); } } } f()"), "1");
}
