/// Writes `theme_props.rs`: every property of the Theme global with a
/// getter and setter by name, so mods can set any of them (see mods.rs)
/// and new ones are moddable as soon as they are added.
fn theme_props() {
    let src = std::fs::read_to_string("ui/theme.slint").expect("read theme.slint");
    let start = src.find("export global Theme {").expect("Theme global");
    let end = start + src[start..].find("\n}\n").expect("end of Theme");
    let mut props = Vec::new();
    for line in src[start..end].lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("in-out property <") else {
            continue;
        };
        let Some((ty, rest)) = rest.split_once('>') else {
            continue;
        };
        let name: String = rest
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let kind = match ty {
            "color" => "Color",
            "length" => "Length",
            "bool" => "Bool",
            "string" => "Text",
            _ => continue,
        };
        props.push((name, kind));
    }
    let mut out = String::from("pub const PROPS: &[(&str, Kind)] = &[\n");
    for (n, k) in &props {
        out += &format!("    (\"{n}\", Kind::{k}),\n");
    }
    out += "];\n\npub fn get(t: &crate::Theme<'_>, name: &str) -> Option<Prop> {\n    Some(match name {\n";
    for (n, k) in &props {
        let f = n.replace('-', "_");
        out += &format!("        \"{n}\" => Prop::{k}(t.get_{f}()),\n");
    }
    out += "        _ => return None,\n    })\n}\n\npub fn set(t: &crate::Theme<'_>, name: &str, p: &Prop) -> bool {\n    match (name, p) {\n";
    for (n, k) in &props {
        let f = n.replace('-', "_");
        let v = if *k == "Text" { "v.clone()" } else { "*v" };
        out += &format!("        (\"{n}\", Prop::{k}(v)) => t.set_{f}({v}),\n");
    }
    out += "        _ => return false,\n    }\n    true\n}\n";
    let dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(std::path::Path::new(&dir).join("theme_props.rs"), out)
        .expect("write theme_props.rs");
    println!("cargo:rerun-if-changed=ui/theme.slint");
}

fn main() {
    theme_props();
    // The default skin is compiled in; user skins will load at runtime through slint-interpreter.
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
    )
    .expect("compile default skin");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../res/windows/djsugar.ico")
            .set("ProductName", "DJ Sugar")
            .set("FileDescription", "DJ Sugar");
        res.compile().expect("embed the Windows icon");
    }
}
