//! Mints YouTube PO tokens on Android with no Java of our own: a hidden web view on
//! the main thread runs the attestation script and Rust reads results from its title.

use std::sync::mpsc::sync_channel;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use jni::objects::{JObject, JString, JValue};
use jni::refs::Global;
use jni::{Env, JavaVM, jni_sig, jni_str};
use sugar_net::botguard;
use sugar_net::youtube::PoTokens;

type View = Global<JObject<'static>>;

/// Minting state: when the integrity token runs out, the page is set up again.
struct Mint {
    expires: Option<Instant>,
    seq: u32,
}

static VIEW: OnceLock<View> = OnceLock::new();
static MINT: Mutex<Mint> = Mutex::new(Mint { expires: None, seq: 0 });
const CHUNK: usize = 1000;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The token source handed to the YouTube client.
pub fn mint(video: &str, visitor: &str) -> Option<PoTokens> {
    let started = Instant::now();
    let out = mint_inner(video, visitor);
    match &out {
        Ok(_) => crate::diag(&format!("token mint ok in {} ms", started.elapsed().as_millis())),
        Err(e) => crate::diag(&format!("token mint failed: {e}")),
    }
    out.ok()
}

fn mint_inner(video: &str, visitor: &str) -> Result<PoTokens, String> {
    let mut m = lock(&MINT);
    if m.expires.is_none_or(|t| Instant::now() >= t) {
        m.expires = None;
        setup(&mut m)?;
    }
    let visitor = if visitor.is_empty() { video } else { visitor };
    let streaming = token(&mut m, visitor)?;
    let player = token(&mut m, video)?;
    Ok(PoTokens { player, streaming, visitor: visitor.to_owned() })
}

/// Loads the page, runs the attestation script and stores the integrity token in it.
fn setup(m: &mut Mint) -> Result<(), String> {
    on_web(|env, view| {
        let (base, page) = (env.new_string("https://www.youtube.com")?, env.new_string(botguard::PAGE)?);
        let (mime, enc) = (env.new_string("text/html")?, env.new_string("utf-8")?);
        env.call_method(
            view,
            jni_str!("loadDataWithBaseURL"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
            &[
                JValue::Object(&base),
                JValue::Object(&page),
                JValue::Object(&mime),
                JValue::Object(&enc),
                JValue::Object(&JObject::null()),
            ],
        )?;
        Ok(())
    })?;
    wait_title(|t| t == "ready", Duration::from_secs(20)).map_err(|e| format!("page: {e}"))?;
    let challenge = botguard::challenge().map_err(|e| format!("challenge: {e}"))?;
    let response = run(
        m,
        &format!(
            "runBotGuard({challenge}).then(function (r) {{ signals = r.out; report(TAG, r.response); }}, function (e) {{ fail(TAG, e); }});"
        ),
        Duration::from_secs(30),
    )?;
    let (integrity, ttl) = botguard::integrity(&response).map_err(|e| format!("integrity: {e}"))?;
    eval(format!("integrity = {integrity};"))?;
    m.expires = Some(Instant::now() + ttl.saturating_sub(Duration::from_secs(600)));
    Ok(())
}

fn token(m: &mut Mint, id: &str) -> Result<String, String> {
    let bytes = botguard::js_bytes(id.as_bytes());
    let csv = run(
        m,
        &format!("report(TAG, Array.from(mint(signals, integrity, {bytes})).join(','));"),
        Duration::from_secs(10),
    )?;
    botguard::token_from_csv(&csv).ok_or_else(|| "token bytes unreadable".into())
}

/// Runs `body` in the page with `TAG` bound to a fresh tag and returns what it
/// reports, read back through the title in pieces.
fn run(m: &mut Mint, body: &str, limit: Duration) -> Result<String, String> {
    m.seq = m.seq.wrapping_add(1);
    let tag = format!("r{}", m.seq);
    eval(format!("(function (TAG) {{ try {{ {body} }} catch (e) {{ fail(TAG, e); }} }})('{tag}');"))?;
    let (head, fail) = (format!("{tag}:"), format!("!{tag}:"));
    let title = wait_title(|t| t.starts_with(&head) || t.starts_with(&fail), limit)?;
    if let Some(why) = title.strip_prefix(&fail) {
        return Err(why.chars().take(300).collect());
    }
    let len: usize = title[head.len()..].parse().map_err(|_| "bad length")?;
    let mut out = String::with_capacity(len);
    let mut at = 0;
    while at < len {
        let piece = format!("{tag}.{at}:");
        eval(format!("chunk('{}', {at}, {CHUNK});", &piece[..piece.len() - 1]))?;
        let t = wait_title(|t| t.starts_with(&piece), Duration::from_secs(5))?;
        let part = &t[piece.len()..];
        if part.is_empty() {
            return Err("result cut short".into());
        }
        at += part.encode_utf16().count();
        out.push_str(part);
    }
    Ok(out)
}

fn eval(js: String) -> Result<(), String> {
    on_web(move |env, view| {
        let js = env.new_string(js)?;
        env.call_method(
            view,
            jni_str!("evaluateJavascript"),
            jni_sig!("(Ljava/lang/String;Landroid/webkit/ValueCallback;)V"),
            &[JValue::Object(&js), JValue::Object(&JObject::null())],
        )?;
        Ok(())
    })
}

fn wait_title(done: impl Fn(&str) -> bool, limit: Duration) -> Result<String, String> {
    let until = Instant::now() + limit;
    loop {
        let t = on_web(|env, view| {
            let t = env.call_method(view, jni_str!("getTitle"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
            if t.is_null() {
                return Ok(String::new());
            }
            env.cast_local::<JString>(t)?.try_to_string(env)
        })?;
        if done(&t) {
            return Ok(t);
        }
        if Instant::now() >= until {
            return Err(format!("timed out, title {:?}", t.chars().take(80).collect::<String>()));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Runs `f` on the main thread, where the web view lives, and waits for the answer.
/// The engine binds to the thread that makes the first view, and the system
/// talks to it from the main thread on rotation, so no other thread will do.
fn on_web<T: Send + 'static>(
    f: impl FnOnce(&mut Env<'_>, &JObject<'_>) -> jni::errors::Result<T> + Send + 'static,
) -> Result<T, String> {
    let app = crate::android::app().ok_or("no activity")?;
    let (vm, activity) = (app.vm_as_ptr() as usize, app.activity_as_ptr() as usize);
    let (tx, rx) = sync_channel(1);
    app.run_on_java_main_thread(Box::new(move || {
        #[allow(unsafe_code, reason = "the pointer is the process VM, valid for its lifetime")]
        let vm = unsafe { JavaVM::from_raw(vm as *mut _) };
        let r = vm.attach_current_thread(|env| -> jni::errors::Result<T> {
            let view = match VIEW.get() {
                Some(v) => v,
                None => {
                    let v = make_view(env, activity)?;
                    VIEW.get_or_init(|| v)
                }
            };
            let r = f(env, view);
            if r.is_err() {
                env.exception_clear();
            }
            r
        });
        let _ = tx.send(r.map_err(|e| e.to_string()));
    }));
    rx.recv_timeout(Duration::from_secs(20)).map_err(|_| "main thread did not answer".to_owned())?
}

fn make_view(env: &mut Env<'_>, activity: usize) -> jni::errors::Result<View> {
    #[allow(unsafe_code, reason = "the activity outlives every main thread task")]
    let activity = unsafe { JObject::from_raw(env, activity as *mut _) };
    let view = env.new_object(
        jni_str!("android/webkit/WebView"),
        jni_sig!("(Landroid/content/Context;)V"),
        &[JValue::Object(&activity)],
    )?;
    let settings =
        env.call_method(&view, jni_str!("getSettings"), jni_sig!("()Landroid/webkit/WebSettings;"), &[])?.l()?;
    env.call_method(&settings, jni_str!("setJavaScriptEnabled"), jni_sig!("(Z)V"), &[JValue::Bool(true)])?;
    // The page needs nothing from the network; the service calls go through Rust.
    env.call_method(&settings, jni_str!("setBlockNetworkLoads"), jni_sig!("(Z)V"), &[JValue::Bool(true)])?;
    let ua = env.new_string(botguard::USER_AGENT)?;
    env.call_method(
        &settings,
        jni_str!("setUserAgentString"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(&ua)],
    )?;
    env.new_global_ref(view)
}
