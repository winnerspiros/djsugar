use djsugar_js::*;
#[test]
fn micro() {
    let e = Engine::new();
    let cases = [
        ("empty loop", "for (var i = 0; i < N; i++) {}"),
        ("local add", "var x = 0; for (var i = 0; i < N; i++) { x = x + i; }"),
        ("call empty fn", "function f() {} for (var i = 0; i < N; i++) f();"),
        ("call fn 3 args", "function f(a, b, c) { return a; } for (var i = 0; i < N; i++) f(i, 2, 3);"),
        ("method call", "var o = { m: function () { return 1; } }; for (var i = 0; i < N; i++) o.m();"),
        ("proto method", "function C() {} C.prototype.m = function () { return 1; }; var o = new C(); for (var i = 0; i < N; i++) o.m();"),
        ("this.x read", "function C() { this.x = 1; } C.prototype.m = function () { return this.x + this.x + this.x + this.x; }; var o = new C(); for (var i = 0; i < N; i++) o.m();"),
        ("prop write", "var o = { x: 0 }; for (var i = 0; i < N; i++) o.x = i;"),
        ("fn.call", "function f(a) { return a; } for (var i = 0; i < N; i++) f.call(null, i);"),
        ("array idx", "var a = [1, 2, 3, 4]; var s = 0; for (var i = 0; i < N; i++) s += a[i & 3];"),
        ("global read", "var g = 1; function f() { return g + g + g + g; } for (var i = 0; i < N; i++) f();"),
    ];
    let only = std::env::var("MICRO").ok();
    for (name, code) in cases {
        if only.as_deref().is_some_and(|o| !name.starts_with(o)) {
            continue;
        }
        let wrapped = format!("(function () {{ var N = 10000000; {code} }})");
        let f = e.eval(&wrapped).unwrap();
        let t = std::time::Instant::now();
        e.call(&f, Value::Undefined, &[]).unwrap();
        eprintln!("{name:16} {:6.1} ns/iter", t.elapsed().as_secs_f64() * 1e9 / 1e7);
    }
}
#[test]
fn sizes() {
    eprintln!(
        "Value {} R<Value> {} Exc {}",
        size_of::<Value>(),
        size_of::<Result<Value, djsugar_js::value::Exc>>(),
        size_of::<djsugar_js::value::Exc>()
    );
}
