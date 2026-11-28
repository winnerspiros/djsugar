//! The network and encoding half of minting YouTube PO tokens. A web view on the
//! device runs the attestation script; this module talks to the attestation service
//! and turns its replies into script calls and back.

use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{agent, read};
use crate::{Error, Result};

const SERVICE: &str = "https://jnn-pa.googleapis.com/$rpc/google.internal.waa.v1.Waa/";
const API_KEY: &str = "AIzaSyDyT5W0Jh49F30Pqqtyfdf7pDLFKLJoAnw";
const REQUEST_KEY: &str = "O43z0dpjhgX20SCx4KAo";
/// The script expects a desktop browser, so the page and the service both see one.
pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.3";

/// The page the web view loads. `report` and `fail` put results in the title,
/// the only channel a native host can read without a host object.
pub const PAGE: &str = r#"<!DOCTYPE html><html><head><title></title><script>
var R = '';
function report(tag, v) { R = String(v); document.title = tag + ':' + R.length; }
function fail(tag, e) { document.title = '!' + tag + ':' + e + ' ' + ((e && e.stack) || ''); }
function chunk(tag, at, n) { document.title = tag + ':' + R.substr(at, n); }
function runBotGuard(c) {
  var code = c.interpreterJavascript.privateDoNotAccessOrElseSafeScriptWrappedValue;
  if (!code) throw new Error('no interpreter');
  new Function(code)();
  var vm = window[c.globalName];
  if (!vm || !vm.a) throw new Error('vm missing');
  var fns = null;
  var out = [];
  vm.a(c.program, function (snap, shut, pass, cam) { fns = { snap: snap }; }, true, undefined, function () {}, [[], []]);
  return new Promise(function (ok, no) {
    var tries = 0;
    var id = setInterval(function () {
      if (fns && fns.snap) {
        clearInterval(id);
        fns.snap(function (r) { ok({ out: out, response: r }); }, [undefined, undefined, out, undefined]);
      } else if (++tries > 10000) {
        clearInterval(id);
        no('snapshot never became ready');
      }
    }, 1);
  });
}
function mint(signals, integrity, id) {
  var minter = signals[0];
  if (!minter) throw new Error('no minter');
  var cb = minter(integrity);
  if (!(cb instanceof Function)) throw new Error('minter is not callable');
  var t = cb(id);
  if (!(t instanceof Uint8Array)) throw new Error('mint returned no bytes');
  return t;
}
document.title = 'ready';
</script></head><body></body></html>"#;

/// The challenge from the service, as the object `runBotGuard` takes.
pub fn challenge() -> Result<String> {
    let raw = call("Create", &json!([REQUEST_KEY]))?;
    challenge_from(&raw)
}

/// Trades the script's answer for an integrity token: (JS byte array, lifetime).
pub fn integrity(response: &str) -> Result<(String, Duration)> {
    let raw = call("GenerateIT", &json!([REQUEST_KEY, response]))?;
    let v: Value = serde_json::from_str(&raw).map_err(|_| Error::Parse("integrity token"))?;
    let token = v[0].as_str().ok_or(Error::Parse("integrity token"))?;
    let ttl = v[1].as_u64().or_else(|| v[1].as_str()?.parse().ok()).ok_or(Error::Parse("integrity lifetime"))?;
    Ok((js_bytes(&b64_decode(token).ok_or(Error::Parse("integrity base64"))?), Duration::from_secs(ttl)))
}

fn call(method: &str, body: &Value) -> Result<String> {
    let req = agent()
        .post(format!("{SERVICE}{method}"))
        .config()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json+protobuf")
        .header("x-goog-api-key", API_KEY)
        .header("x-user-agent", "grpc-web-javascript/0.1");
    match read(req.send(body.to_string()))? {
        (200..=299, text) => Ok(text),
        (s, _) => Err(Error::Status(s)),
    }
}

fn challenge_from(raw: &str) -> Result<String> {
    let bad = || Error::Parse("attestation challenge");
    let outer: Value = serde_json::from_str(raw).map_err(|_| bad())?;
    let c = match outer[1].as_str() {
        Some(s) => serde_json::from_str(&descramble(s).ok_or_else(bad)?).map_err(|_| bad())?,
        None => outer[0].clone(),
    };
    let first = |i: usize| c[i].as_array().and_then(|a| a.iter().find_map(Value::as_str)).map(str::to_owned);
    let text = |i: usize| c[i].as_str().map(str::to_owned).ok_or_else(bad);
    Ok(json!({
        "messageId": text(0)?,
        "interpreterJavascript": {
            "privateDoNotAccessOrElseSafeScriptWrappedValue": first(1),
            "privateDoNotAccessOrElseTrustedResourceUrlWrappedValue": first(2),
        },
        "interpreterHash": text(3)?,
        "program": text(4)?,
        "globalName": text(5)?,
        "clientExperimentsStateBlob": text(7)?,
    })
    .to_string())
}

fn descramble(s: &str) -> Option<String> {
    String::from_utf8(b64_decode(s)?.into_iter().map(|b| b.wrapping_add(97)).collect()).ok()
}

/// `new Uint8Array([..])` holding the bytes, for handing data to the page.
pub fn js_bytes(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 4 + 18);
    s.push_str("new Uint8Array([");
    for (i, v) in b.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&v.to_string());
    }
    s.push_str("])");
    s
}

/// The page's comma separated bytes as the URL-safe base64 the player wants.
pub fn token_from_csv(csv: &str) -> Option<String> {
    let bytes: Vec<u8> = csv.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    Some(b64_url(&bytes))
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_url(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..=c.len() {
            s.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
        }
    }
    s
}

/// Decodes either base64 alphabet; '.' stands in for padding in the service's replies.
pub(crate) fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'.' => break,
            b'\n' | b'\r' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for n in 0..40u8 {
            let b: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
            assert_eq!(b64_decode(&b64_url(&b)).unwrap(), b);
        }
        assert_eq!(b64_url(b"hello?>"), "aGVsbG8_Pg");
        assert_eq!(b64_decode("aGVsbG8/Pg==").unwrap(), b"hello?>");
        assert_eq!(token_from_csv("104, 105").unwrap(), "aGk");
        assert_eq!(js_bytes(&[1, 255]), "new Uint8Array([1,255])");
    }

    #[test]
    fn scrambled_challenge() {
        let inner = r#"["m",["s"],[null,"u"],"h","p","g",0,"x"]"#;
        let scrambled: Vec<u8> = inner.bytes().map(|b| b.wrapping_sub(97)).collect();
        let raw = json!([null, b64_url(&scrambled)]).to_string();
        let c: Value = serde_json::from_str(&challenge_from(&raw).unwrap()).unwrap();
        assert_eq!(c["globalName"], "g");
        assert_eq!(c["interpreterJavascript"]["privateDoNotAccessOrElseTrustedResourceUrlWrappedValue"], "u");
        assert_eq!(c["clientExperimentsStateBlob"], "x");
    }

    #[test]
    #[ignore = "network: talks to the live attestation service"]
    fn live_challenge() {
        let c: Value = serde_json::from_str(&challenge().unwrap()).unwrap();
        assert!(c["program"].as_str().is_some_and(|p| p.len() > 100));
    }
}
