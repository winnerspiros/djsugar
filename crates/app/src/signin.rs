//! A service's sign-in on its own page, shown over the app; the page's cookies come back in a file.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Whether there is such a page here.
pub const HERE: bool = cfg!(target_os = "android");

/// A sign-in page: where it starts, the host whose cookies are wanted, the cookie that tells it is done, and
/// whether it needs the desktop site.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(target_os = "android"), allow(dead_code, reason = "only a phone has the page"))]
pub struct Form {
    pub url: &'static str,
    pub host: &'static str,
    pub cookie: &'static str,
    pub wide: bool,
}

/// Waits for the page's answer in `out`: its cookies, nothing when it was closed, `None` when `stop` is set.
pub fn wait(out: &Path, stop: &AtomicBool) -> Option<String> {
    while !stop.load(Ordering::Acquire) {
        if let Ok(text) = std::fs::read_to_string(out) {
            let _ = std::fs::remove_file(out);
            return Some(text);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    None
}

/// Opens the page; false where there is none.
#[cfg(not(target_os = "android"))]
pub fn open(_: Form, _: &Path) -> bool {
    false
}

/// Opens the page over the app; it writes its answer to `out`.
#[cfg(target_os = "android")]
#[allow(unsafe_code, reason = "JNI hands us raw VM and activity pointers")]
pub fn open(form: Form, out: &Path) -> bool {
    use jni::objects::{JObject, JValue};
    use jni::{jni_sig, jni_str};
    let Some(app) = crate::android::app() else { return false };
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let out = out.to_string_lossy().into_owned();
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        let intent = env.new_object(jni_str!("android/content/Intent"), jni_sig!("()V"), &[])?;
        let class = env.new_string("app.djsugar.SignIn")?;
        env.call_method(
            &intent,
            jni_str!("setClassName"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&activity), JValue::Object(&class)],
        )?;
        let wide = if form.wide { "1" } else { "" };
        for (key, value) in
            [("url", form.url), ("host", form.host), ("cookie", form.cookie), ("wide", wide), ("out", out.as_str())]
        {
            let (key, value) = (env.new_string(key)?, env.new_string(value)?);
            env.call_method(
                &intent,
                jni_str!("putExtra"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
                &[JValue::Object(&key), JValue::Object(&value)],
            )?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_is_read_once_and_a_stop_ends_the_wait() {
        let out = std::env::temp_dir().join(format!("djsugar-signin-{}", std::process::id()));
        std::fs::write(&out, "A=1; B=2").unwrap();
        assert_eq!(wait(&out, &AtomicBool::new(false)).as_deref(), Some("A=1; B=2"));
        assert!(!out.exists());
        assert_eq!(wait(&out, &AtomicBool::new(true)), None);
    }
}
