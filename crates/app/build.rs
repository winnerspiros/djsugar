fn main() {
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
