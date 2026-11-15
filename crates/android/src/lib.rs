#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    let _ = djsugar::ANDROID_APP.set(app);
    if let Err(e) = djsugar::run() {
        eprintln!("DJ Sugar stopped: {e}");
    }
}
