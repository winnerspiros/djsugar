//! The server end to end, with a real socket and WebSocket client.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

use super::*;

fn start() -> Server {
    // Port 0: the system picks a free one, so tests can run side by side.
    Server::start(0, Vec::new()).unwrap()
}

fn get(port: u16, request: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

type Ws = WebSocket<TcpStream>;

fn connect(port: u16, origin: Option<&str>) -> Result<Ws, tungstenite::Error> {
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut req = format!("ws://127.0.0.1:{port}/ws")
        .into_client_request()
        .unwrap();
    if let Some(o) = origin {
        req.headers_mut().insert("Origin", o.parse().unwrap());
    }
    tungstenite::client(req, stream)
        .map(|(ws, _)| ws)
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => e,
            tungstenite::HandshakeError::Interrupted(_) => panic!("interrupted"),
        })
}

fn send(ws: &mut Ws, msg: Value) {
    ws.send(Message::text(msg.to_string())).unwrap();
}

fn recv(ws: &mut Ws) -> Value {
    loop {
        match ws.read().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Close(_) => return Value::Null,
            _ => {}
        }
    }
}

/// Waits for the app side to see events.
fn events(server: &Server, n: usize) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut all = Vec::new();
    while all.len() < n && Instant::now() < deadline {
        all.extend(server.poll());
        std::thread::sleep(Duration::from_millis(10));
    }
    all
}

fn pair(server: &Server, ws: &mut Ws) -> (String, String) {
    let (code, _) = server.pair_code();
    send(
        ws,
        json!({"id": 1, "method": "pair", "params": {"code": code, "name": "Test phone"}}),
    );
    let r = recv(ws);
    assert_eq!(r["id"], 1, "{r}");
    let res = &r["result"];
    (
        res["device"].as_str().unwrap().to_string(),
        res["token"].as_str().unwrap().to_string(),
    )
}

#[test]
fn serves_the_page() {
    let server = start();
    let r = get(
        server.port(),
        "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.contains("<title>DJ Sugar Remote</title>"));
    let r = get(
        server.port(),
        "GET /nope HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    );
    assert!(r.starts_with("HTTP/1.1 404"));
    // A domain name pointed at this computer by someone else's page.
    let r = get(
        server.port(),
        "GET / HTTP/1.1\r\nHost: evil.example.com\r\n\r\n",
    );
    assert!(r.starts_with("HTTP/1.1 403"));
}

#[test]
fn refuses_other_pages() {
    let server = start();
    let port = server.port();
    assert!(connect(port, Some("http://evil.example.com")).is_err());
    assert!(connect(port, Some(&format!("http://127.0.0.1:{port}"))).is_ok());
}

#[test]
fn pairs_then_requests_reach_the_app() {
    let server = start();
    server.publish("automix", json!({"on": false}));
    let mut ws = connect(server.port(), None).unwrap();
    // Nothing works before pairing.
    send(&mut ws, json!({"id": 7, "method": "automix.skip"}));
    assert_eq!(recv(&mut ws)["error"]["message"], "Not paired");

    let (device, token) = pair(&server, &mut ws);
    // The latest state follows straight away.
    let state = recv(&mut ws);
    assert_eq!(state["event"], "automix");
    assert_eq!(state["data"]["on"], false);
    assert!(matches!(
        &events(&server, 1)[..],
        [Event::Connected { new: true, .. }]
    ));
    assert_eq!(server.connected(), vec![device.clone()]);

    send(
        &mut ws,
        json!({"id": 8, "method": "automix.skip", "params": {}}),
    );
    let ev = events(&server, 1);
    let Event::Request {
        client, id, method, ..
    } = &ev[0]
    else {
        panic!("{ev:?}");
    };
    assert_eq!(method, "automix.skip");
    server.reply(*client, id.clone(), Ok(json!(true)));
    assert_eq!(recv(&mut ws), json!({"id": 8, "result": true}));

    // Changes are pushed; the same state twice isn't.
    server.publish("automix", json!({"on": false}));
    server.publish("automix", json!({"on": true}));
    assert_eq!(recv(&mut ws)["data"]["on"], true);

    // The device token logs in again on a new connection.
    let mut again = connect(server.port(), None).unwrap();
    send(
        &mut again,
        json!({"id": 1, "method": "hello", "params": {"device": device, "token": token}}),
    );
    assert_eq!(recv(&mut again)["result"]["name"], "Test phone");
    assert_eq!(recv(&mut again)["data"]["on"], true);
}

#[test]
fn wrong_tokens_and_revoked_devices() {
    let server = start();
    let mut ws = connect(server.port(), None).unwrap();
    send(
        &mut ws,
        json!({"id": 1, "method": "hello", "params": {"device": "x", "token": "y"}}),
    );
    assert_eq!(recv(&mut ws)["error"]["unpaired"], true);
    send(
        &mut ws,
        json!({"id": 2, "method": "pair", "params": {"code": "guess"}}),
    );
    assert!(recv(&mut ws)["error"]["message"].is_string());

    let (device, token) = pair(&server, &mut ws);
    server.revoke(&device);
    assert_eq!(recv(&mut ws)["event"], "revoked");
    assert!(server.devices().is_empty());
    let mut again = connect(server.port(), None).unwrap();
    send(
        &mut again,
        json!({"id": 1, "method": "hello", "params": {"device": device, "token": token}}),
    );
    assert_eq!(recv(&mut again)["error"]["unpaired"], true);
}

#[test]
fn saved_devices_log_in_after_a_restart() {
    let server = start();
    let mut ws = connect(server.port(), None).unwrap();
    let (device, token) = pair(&server, &mut ws);
    let saved = server.take_changed_devices().unwrap();
    drop(ws);
    drop(server);
    let server = Server::start(0, saved).unwrap();
    let mut ws = connect(server.port(), None).unwrap();
    send(
        &mut ws,
        json!({"id": 1, "method": "hello", "params": {"device": device, "token": token}}),
    );
    assert!(recv(&mut ws)["result"].is_object());
}

#[test]
fn urls() {
    let ip: IpAddr = "192.168.1.20".parse().unwrap();
    assert_eq!(
        pair_url(ip, 7477, "abc"),
        "http://192.168.1.20:7477/#pair=abc"
    );
}

#[test]
fn serves_art_to_paired_phones_only() {
    let server = start();
    server.set_art_source(Arc::new(|id: &str, size: u32| {
        (id == "abc123").then(|| format!("jpeg {size}").into_bytes())
    }));
    let mut ws = connect(server.port(), None).unwrap();
    let (code, _) = server.pair_code();
    send(
        &mut ws,
        json!({"id": 1, "method": "pair", "params": {"code": code, "name": "P"}}),
    );
    let key = recv(&mut ws)["result"]["art_key"]
        .as_str()
        .unwrap()
        .to_string();
    let port = server.port();
    let ask = |path: &str| {
        get(
            port,
            &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
        )
    };
    let ok = ask(&format!("/art/abc123?s=96&k={key}"));
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("image/jpeg") && ok.ends_with("jpeg 96"));
    // Wrong key, unknown cover, odd size.
    assert!(ask("/art/abc123?s=96&k=nope").starts_with("HTTP/1.1 404"));
    assert!(ask(&format!("/art/fff?s=96&k={key}")).starts_with("HTTP/1.1 404"));
    assert!(ask(&format!("/art/abc123?s=5000&k={key}")).starts_with("HTTP/1.1 404"));
}
