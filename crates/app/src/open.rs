//! Hands a web page or a folder to whatever the system opens it with.

/// The project's page, its guide and where problems are reported.
pub const HOME: &str = "https://github.com/winnerspiros/djsugar/tree/rust";
pub const ISSUES: &str = "https://github.com/winnerspiros/djsugar/issues/new";

/// Opens `what`, a link or a path; false when the system has nothing to open it with.
#[cfg(not(target_os = "android"))]
pub fn open(what: &str) -> bool {
    let tool = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(tool).arg(what).spawn().is_ok()
}

/// Opens the link `what` in the phone's browser.
#[cfg(target_os = "android")]
#[allow(unsafe_code, reason = "JNI hands us raw VM and activity pointers")]
pub fn open(what: &str) -> bool {
    use jni::objects::{JObject, JValue};
    use jni::{jni_sig, jni_str};
    let Some(app) = crate::android::app() else { return false };
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        let link = env.new_string(what)?;
        let uri = env
            .call_static_method(
                jni_str!("android/net/Uri"),
                jni_str!("parse"),
                jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                &[JValue::Object(&link)],
            )?
            .l()?;
        let action = env.new_string("android.intent.action.VIEW")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
            &[JValue::Object(&action), JValue::Object(&uri)],
        )?;
        let r = env.call_method(
            &activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        );
        if r.is_err() {
            env.exception_clear();
        }
        r.map(|_| ())
    })
    .is_ok()
}

/// Hands the log to whatever shows it: the file itself on a desk.
#[cfg(not(target_os = "android"))]
pub fn send(_text: &str) -> bool {
    crate::log::path().is_some_and(|p| open(&p.to_string_lossy()))
}

/// Hands `text` to the app the DJ picks from the phone's share sheet.
#[cfg(target_os = "android")]
#[allow(unsafe_code, reason = "JNI hands us raw VM and activity pointers")]
pub fn send(text: &str) -> bool {
    use jni::objects::{JObject, JValue};
    use jni::{jni_sig, jni_str};
    let Some(app) = crate::android::app() else { return false };
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        let action = env.new_string("android.intent.action.SEND")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&action)],
        )?;
        let kind = env.new_string("text/plain")?;
        env.call_method(
            &intent,
            jni_str!("setType"),
            jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&kind)],
        )?;
        let (key, body) = (env.new_string("android.intent.extra.TEXT")?, env.new_string(text)?);
        env.call_method(
            &intent,
            jni_str!("putExtra"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&key), JValue::Object(&body)],
        )?;
        let title = env.new_string("DJ Sugar log")?;
        let chooser = env
            .call_static_method(
                jni_str!("android/content/Intent"),
                jni_str!("createChooser"),
                jni_sig!("(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;"),
                &[JValue::Object(&intent), JValue::Object(&title)],
            )?
            .l()?;
        let r = env.call_method(
            &activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&chooser)],
        );
        if r.is_err() {
            env.exception_clear();
        }
        r.map(|_| ())
    })
    .is_ok()
}
