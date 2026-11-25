//! Unlocks YouTube stream links that carry a scrambled signature or a throttling
//! `n` value. The player script is run as it is, in a JavaScript engine written in
//! Rust; its own URL builder, the function that marks a link `alr=yes`, rewrites
//! both. Nothing in the script is pattern-matched beyond finding that function.

use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;

use boa_engine::{Context, JsValue, Source, js_string};

use crate::http::{agent, read};
use crate::{Error, Result};

const TIMEOUT: Duration = Duration::from_secs(30);
const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";

/// What the player script expects of a browser, and no more.
const PRELUDE: &str = r#"
var globalThis = this;
if (typeof globalThis.XMLHttpRequest === "undefined") { globalThis.XMLHttpRequest = { prototype: {} }; }
globalThis.location = { hash: "", host: "www.youtube.com", hostname: "www.youtube.com",
  href: "https://www.youtube.com/watch?v=dQw4w9WgXcQ", origin: "https://www.youtube.com", password: "",
  pathname: "/watch", port: "", protocol: "https:", search: "?v=dQw4w9WgXcQ", username: "" };
if (typeof globalThis.document === "undefined") { globalThis.document = Object.create(null); }
if (typeof globalThis.navigator === "undefined") { globalThis.navigator = Object.create(null); }
if (typeof globalThis.self === "undefined") { globalThis.self = globalThis; }
if (typeof globalThis.window === "undefined") { globalThis.window = globalThis; }
"#;

/// A loaded player: its version and the signature timestamp the API wants with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: String,
    pub sts: u32,
}

enum Ask {
    Load(String, Sender<Result<Player>>),
    Solve(Option<String>, String, Sender<Result<(String, String)>>),
}

/// The engine lives on one thread, loaded once per player version.
fn worker() -> &'static Mutex<Sender<Ask>> {
    static W: OnceLock<Mutex<Sender<Ask>>> = OnceLock::new();
    W.get_or_init(|| {
        let (tx, rx) = channel::<Ask>();
        std::thread::Builder::new()
            .name("cipher".into())
            .stack_size(64 << 20)
            .spawn(move || {
                let mut loaded: Option<(Player, Context)> = None;
                for ask in rx {
                    match ask {
                        Ask::Load(id, back) => {
                            if let Some((p, _)) = loaded.as_ref().filter(|(p, _)| p.id == id) {
                                let _ = back.send(Ok(p.clone()));
                                continue;
                            }
                            let out = fetch(&id).and_then(|code| prepare(&id, &code));
                            let _ = back.send(out.map(|(p, ctx)| {
                                loaded = Some((p.clone(), ctx));
                                p
                            }));
                        }
                        Ask::Solve(sig, n, back) => {
                            let out = match loaded.as_mut() {
                                Some((_, ctx)) => unlock(ctx, sig.as_deref(), &n),
                                None => Err(Error::Refused("No player loaded".into())),
                            };
                            let _ = back.send(out);
                        }
                    }
                }
            })
            .ok();
        Mutex::new(tx)
    })
}

fn ask<T>(make: impl FnOnce(Sender<Result<T>>) -> Ask) -> Result<T> {
    let (tx, rx) = channel();
    worker().lock().unwrap_or_else(PoisonError::into_inner).send(make(tx)).map_err(|_| gone())?;
    rx.recv().map_err(|_| gone())?
}

fn gone() -> Error {
    Error::Refused("The player script engine stopped".into())
}

/// Loads the current player: its version comes from the iframe API.
pub fn load() -> Result<Player> {
    let (_, page) = read(get("https://www.youtube.com/iframe_api"))?;
    let id = player_id(&page).ok_or(Error::Parse("no player version"))?;
    ask(|tx| Ask::Load(id, tx))
}

/// Unlocks one link's signature (when it has one) and `n` value with the loaded player.
pub fn solve(sig: Option<&str>, n: &str) -> Result<(String, String)> {
    ask(|tx| Ask::Solve(sig.map(str::to_string), n.to_string(), tx))
}

fn get(url: &str) -> std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    agent().get(url).header("User-Agent", AGENT).config().timeout_global(Some(TIMEOUT)).build().call()
}

fn fetch(id: &str) -> Result<String> {
    let url = format!("https://www.youtube.com/s/player/{id}/tv-player-ias.vflset/tv-player-ias.js");
    match read(get(&url))? {
        (200, code) => Ok(code),
        (s, _) => Err(Error::Status(s)),
    }
}

/// The eight hex digit player version in the iframe API's script.
pub fn player_id(page: &str) -> Option<String> {
    let at = page.find("player\\/").map(|i| i + 8).or_else(|| page.find("player/").map(|i| i + 7))?;
    let id = page.get(at..at + 8)?;
    id.bytes().all(|b| b.is_ascii_hexdigit()).then(|| id.to_string())
}

/// The signature timestamp written in the player.
pub fn sts(code: &str) -> Option<u32> {
    let at = code.find("signatureTimestamp:")? + "signatureTimestamp:".len();
    code[at..].bytes().take_while(u8::is_ascii_digit).fold(None, |a, d| Some(a.unwrap_or(0) * 10 + u32::from(d - b'0')))
}

/// The URL builder: the function one of whose own statements is `x.set("alr","yes")`.
/// Returns its name as the player's scope knows it.
pub fn builder(code: &str) -> Option<&str> {
    let mut from = 0;
    while let Some(i) = code[from..].find(".set(\"alr\",\"yes\")").map(|i| i + from) {
        from = i + 1;
        // A statement of its own: `;x.set(...);` or `{x.set(...);`.
        let head = code[..i].trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        if !matches!(head.chars().last(), Some(';' | '{')) || !code[i..].starts_with(".set(\"alr\",\"yes\");") {
            continue;
        }
        let start = code[..i].rfind("function ")?;
        let name = &code[start + 9..];
        let name = &name[..name.find('(')?];
        if !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$') {
            // The function must still be open where the statement sits.
            let body = &code[start..i];
            let depth = body.bytes().fold(0i32, |d, b| d + i32::from(b == b'{') - i32::from(b == b'}'));
            if depth == 1 {
                return Some(name);
            }
        }
    }
    None
}

/// Runs the player with the unlock function handed out of its scope.
fn prepare(id: &str, code: &str) -> Result<(Player, Context)> {
    let name = builder(code).ok_or(Error::Parse("no URL builder in the player"))?;
    let sts = sts(code).ok_or(Error::Parse("no signature timestamp in the player"))?;
    let end = code.rfind("})").ok_or(Error::Parse("player is not one function"))?;
    let hand_out = format!(
        r#";globalThis.__unlock = function (sig, n) {{
  var u = {name}("https://youtube.com/watch?v=dQw4w9WgXcQ", "s", sig ? encodeURIComponent(sig) : undefined);
  u.set("n", n);
  var p = Object.getPrototypeOf(u);
  var ks = Object.keys(p).concat(Object.getOwnPropertyNames(p));
  for (var i = 0; i < ks.length; i++) {{
    if (["constructor", "set", "get", "clone"].indexOf(ks[i]) < 0) {{ u[ks[i]](); break; }}
  }}
  var s = u.get("s");
  return [s ? decodeURIComponent(s) : "", u.get("n") || ""];
}};"#
    );
    let script = format!("{PRELUDE}{}{hand_out}{}", &code[..end], &code[end..]);
    let mut ctx = Context::default();
    ctx.eval(Source::from_bytes(script.as_bytes())).map_err(|e| Error::Refused(format!("player script: {e}")))?;
    Ok((Player { id: id.to_string(), sts }, ctx))
}

fn unlock(ctx: &mut Context, sig: Option<&str>, n: &str) -> Result<(String, String)> {
    let f = ctx.global_object().get(js_string!("__unlock"), ctx).map_err(|e| Error::Refused(e.to_string()))?;
    let f = f.as_callable().ok_or(Error::Parse("no unlock function"))?.clone();
    let sig = sig.map_or(JsValue::undefined(), |s| JsValue::from(js_string!(s)));
    let out = f
        .call(&JsValue::undefined(), &[sig, JsValue::from(js_string!(n))], ctx)
        .map_err(|e| Error::Refused(format!("unlock: {e}")))?;
    let field = |i: u32, ctx: &mut Context| -> Result<String> {
        let o = out.as_object().ok_or(Error::Parse("unlock gave no pair"))?;
        let v = o.get(i, ctx).map_err(|e| Error::Refused(e.to_string()))?;
        Ok(v.as_string().map(|s| s.to_std_string_escaped()).unwrap_or_default())
    };
    Ok((field(0, ctx)?, field(1, ctx)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_player_version_and_builder() {
        assert_eq!(
            player_id(r#"var u="https:\/\/www.youtube.com\/s\/player\/b485e332\/www-widgetapi""#).as_deref(),
            Some("b485e332")
        );
        let code = "x;function Ab(a){return a.get(\"alr\")||a.set(\"alr\",\"yes\")}\nfunction lS(a,b){b=b||\"\";a=new P(a);a.set(\"alr\",\"yes\");return a}var q={signatureTimestamp:20779}";
        assert_eq!(builder(code), Some("lS"));
        assert_eq!(sts(code), Some(20779));
    }

    /// The answers the reference solver gives for player b485e332.
    #[test]
    #[ignore = "network: downloads a 2.5 MB player script"]
    fn unlocks_like_the_reference() {
        let code = fetch("b485e332").unwrap();
        let t = std::time::Instant::now();
        let (p, mut ctx) = prepare("b485e332", &code).unwrap();
        eprintln!("prepared in {:?}", t.elapsed());
        assert_eq!(p.sts, 20779);
        let t = std::time::Instant::now();
        assert_eq!(unlock(&mut ctx, None, "abcdEFGHijklMN12").unwrap().1, "BOclaP_AI5tVz");
        assert_eq!(unlock(&mut ctx, None, "Zx9_-QwErTyUiOp0").unwrap().1, "GLrwDyCyqmVoA");
        let sig =
            "AAAAAAAAAAabcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_abcdefghijklmnopqrstuvwxyzAB";
        let want =
            "AAAAAAAAtabcdefghijklmnopqrsWuvwxyzABCDAFGHIJKLMNOPQRSTUVEXYZ0123456789-_abcdefghijklmnopqrstuvwxyzAB";
        assert_eq!(
            unlock(&mut ctx, Some(sig), "abcdEFGHijklMN12").unwrap(),
            (want.to_string(), "BOclaP_AI5tVz".to_string())
        );
        eprintln!("solved in {:?}", t.elapsed());
    }
}
