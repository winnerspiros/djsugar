//! Tests over a real UDP socket on this computer.

use super::*;
use djsugar_control::Behavior;

fn registry() -> Arc<ControlRegistry> {
    let reg = ControlRegistry::new();
    reg.toggle("[Channel1]", "play");
    reg.toggle("[Channel2]", "play");
    reg.create("[Channel1]", "volume", 1.0, 0.0, 1.0, Behavior::Linear);
    reg.create("[Master]", "crossfader", 0.0, -1.0, 1.0, Behavior::Linear);
    reg.toggle("[Mixer Profile]", "xFaderReverse");
    reg
}

fn start(reg: &Arc<ControlRegistry>, config: Config) -> Osc {
    Osc::start(reg.clone(), Config { port: 0, ..config }).unwrap()
}

fn client() -> UdpSocket {
    let s = UdpSocket::bind("127.0.0.1:0").unwrap();
    s.set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    s
}

fn send(s: &UdpSocket, osc: &Osc, addr: &str, args: Vec<OscType>) {
    let bytes = rosc::encoder::encode(&OscPacket::Message(OscMessage {
        addr: addr.into(),
        args,
    }))
    .unwrap();
    s.send_to(&bytes, ("127.0.0.1", osc.port())).unwrap();
}

/// The next message, or `None` after half a second.
fn recv(s: &UdpSocket) -> Option<OscMessage> {
    let mut buf = [0u8; 4096];
    let (n, _) = s.recv_from(&mut buf).ok()?;
    match rosc::decoder::decode_udp(&buf[..n]).ok()?.1 {
        OscPacket::Message(m) => Some(m),
        OscPacket::Bundle(_) => None,
    }
}

fn value(m: &OscMessage) -> f32 {
    match m.args[0] {
        OscType::Float(v) => v,
        ref other => panic!("not a float: {other:?}"),
    }
}

/// Waits for `check` to hold, up to a second.
fn eventually(check: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn addresses_drop_brackets_and_lower_case_the_group() {
    assert_eq!(
        address_of(&ConfigKey::new("[Channel1]", "play")),
        "/channel1/play"
    );
    assert_eq!(
        address_of(&ConfigKey::new(
            "[EqualizerRack1_[Channel1]_Effect1]",
            "parameter1"
        )),
        "/equalizerrack1_channel1_effect1/parameter1"
    );
    assert_eq!(
        address_of(&ConfigKey::new("[Mixer Profile]", "xFaderReverse")),
        "/mixer_profile/xFaderReverse"
    );
}

#[test]
fn parses_targets() {
    assert_eq!(
        parse_target("192.168.1.20:8000"),
        Some("192.168.1.20:8000".parse().unwrap())
    );
    assert_eq!(
        parse_target(" 10.0.0.5 "),
        Some("10.0.0.5:9000".parse().unwrap())
    );
    assert_eq!(parse_target(""), None);
}

#[test]
fn a_number_sets_the_control() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let c = client();
    send(&c, &osc, "/channel1/play", vec![OscType::Float(1.0)]);
    let play = reg.get("[Channel1]", "play").unwrap();
    assert!(eventually(|| play.get_bool()));
    send(&c, &osc, "/master/crossfader", vec![OscType::Int(-1)]);
    let xf = reg.get("[Master]", "crossfader").unwrap();
    assert!(eventually(|| xf.get() == -1.0));
    send(
        &c,
        &osc,
        "/mixer_profile/xfaderreverse",
        vec![OscType::Bool(true)],
    );
    let rev = reg.get("[Mixer Profile]", "xFaderReverse").unwrap();
    assert!(eventually(|| rev.get_bool()));
    // Unknown addresses and strings are ignored.
    send(&c, &osc, "/channel9/play", vec![OscType::Float(1.0)]);
    send(
        &c,
        &osc,
        "/channel2/play",
        vec![OscType::String("1".into())],
    );
    std::thread::sleep(Duration::from_millis(50));
    assert!(!reg.get("[Channel2]", "play").unwrap().get_bool());
}

#[test]
fn no_arguments_asks_for_the_value() {
    let reg = registry();
    reg.get("[Master]", "crossfader").unwrap().set(0.5);
    let osc = start(&reg, Config::default());
    let c = client();
    send(&c, &osc, "/master/crossfader", vec![]);
    let m = recv(&c).expect("a reply");
    assert_eq!(m.addr, "/master/crossfader");
    assert_eq!(value(&m), 0.5);
    // Asked once, it's followed from then on.
    reg.get("[Master]", "crossfader").unwrap().set(-0.25);
    let m = recv(&c).expect("feedback");
    assert_eq!(value(&m), -0.25);
}

#[test]
fn a_change_made_elsewhere_reaches_subscribers_but_not_an_echo() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let c = client();
    send(
        &c,
        &osc,
        "/subscribe",
        vec![OscType::String("/channel1/*".into())],
    );
    // The current values arrive at once.
    let mut first: Vec<String> = (0..2).filter_map(|_| recv(&c)).map(|m| m.addr).collect();
    first.sort();
    assert_eq!(first, ["/channel1/play", "/channel1/volume"]);
    // Moving the fader from here isn't sent back...
    send(&c, &osc, "/channel1/volume", vec![OscType::Float(0.3)]);
    assert!(recv(&c).is_none());
    // ...but a MIDI controller moving it is.
    reg.get("[Channel1]", "volume").unwrap().set(0.7);
    let m = recv(&c).expect("feedback");
    assert_eq!(m.addr, "/channel1/volume");
    assert!((value(&m) - 0.7).abs() < 1e-6);
    // Deck 2 wasn't subscribed.
    reg.get("[Channel2]", "play").unwrap().set(1.0);
    assert!(recv(&c).is_none());
    send(
        &c,
        &osc,
        "/unsubscribe",
        vec![OscType::String("/channel1/*".into())],
    );
    std::thread::sleep(Duration::from_millis(50));
    reg.get("[Channel1]", "volume").unwrap().set(0.1);
    assert!(recv(&c).is_none());
}

#[test]
fn controls_made_later_are_followed_by_pattern() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let c = client();
    send(
        &c,
        &osc,
        "/subscribe",
        vec![OscType::String("/channel3/*".into())],
    );
    assert!(recv(&c).is_none());
    let play = reg.toggle("[Channel3]", "play");
    // New controls are looked for once a second.
    c.set_read_timeout(Some(TABLE_CHECK * 2)).unwrap();
    let m = recv(&c).expect("the new control's value");
    assert_eq!(m.addr, "/channel3/play");
    play.set(1.0);
    assert_eq!(value(&recv(&c).expect("feedback")), 1.0);
}

#[test]
fn feedback_is_at_most_thirty_a_second() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let c = client();
    send(
        &c,
        &osc,
        "/subscribe",
        vec![OscType::String("/master/crossfader".into())],
    );
    recv(&c).expect("the current value");
    let xf = reg.get("[Master]", "crossfader").unwrap();
    let start = Instant::now();
    let mut i = 0;
    while start.elapsed() < Duration::from_millis(500) {
        i += 1;
        xf.set(if i % 2 == 0 { 0.5 } else { -0.5 });
        std::thread::sleep(Duration::from_millis(1));
    }
    c.set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut got = 0;
    while recv(&c).is_some() {
        got += 1;
    }
    assert!(got <= 20, "{got} messages in half a second");
    assert!(got >= 5, "only {got} messages");
}

#[test]
fn a_pattern_address_sets_every_match() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let c = client();
    send(&c, &osc, "/channel*/play", vec![OscType::Float(1.0)]);
    let p1 = reg.get("[Channel1]", "play").unwrap();
    let p2 = reg.get("[Channel2]", "play").unwrap();
    assert!(eventually(|| p1.get_bool() && p2.get_bool()));
}

#[test]
fn send_only_ignores_everything_but_subscriptions() {
    let reg = registry();
    let osc = start(
        &reg,
        Config {
            send_only: true,
            ..Config::default()
        },
    );
    let c = client();
    send(&c, &osc, "/channel1/play", vec![OscType::Float(1.0)]);
    send(&c, &osc, "/master/crossfader", vec![]);
    assert!(recv(&c).is_none());
    assert!(!reg.get("[Channel1]", "play").unwrap().get_bool());
    send(
        &c,
        &osc,
        "/subscribe",
        vec![OscType::String("/channel1/play".into())],
    );
    assert_eq!(recv(&c).expect("subscribed").addr, "/channel1/play");
}

#[test]
fn a_target_on_the_same_computer_gets_the_replies() {
    let reg = registry();
    // The app listens on one port and sends from another, as TouchOSC can.
    let listen = client();
    let target = listen.local_addr().unwrap();
    let osc = start(
        &reg,
        Config {
            targets: vec![target],
            ..Config::default()
        },
    );
    let sender = client();
    send(&sender, &osc, "/master/crossfader", vec![]);
    assert_eq!(
        recv(&listen).expect("reply at the target").addr,
        "/master/crossfader"
    );
    assert!(recv(&sender).is_none());
    assert_eq!(osc.clients(), 0);
    // Events go to every target.
    osc.send_event(OscMessage {
        addr: "/deck/1/track".into(),
        args: vec![OscType::String("Title".into())],
    });
    assert_eq!(recv(&listen).expect("event").addr, "/deck/1/track");
}

#[test]
fn a_locked_control_answers_with_where_it_stayed() {
    let reg = registry();
    let xf = reg.get("[Master]", "crossfader").unwrap();
    xf.lock(true);
    let osc = start(&reg, Config::default());
    let c = client();
    send(&c, &osc, "/master/crossfader", vec![OscType::Float(1.0)]);
    let m = recv(&c).expect("the real value");
    assert_eq!(value(&m), 0.0);
}

#[test]
fn the_port_closes_when_dropped() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let port = osc.port();
    drop(osc);
    UdpSocket::bind(("127.0.0.1", port)).expect("port free again");
}

#[test]
fn events_reach_targets_and_matching_subscribers() {
    let reg = registry();
    let osc = start(&reg, Config::default());
    let follower = client();
    let other = client();
    send(
        &follower,
        &osc,
        "/subscribe",
        vec![OscType::String("/beat".into())],
    );
    send(
        &other,
        &osc,
        "/subscribe",
        vec![OscType::String("/channel2/play".into())],
    );
    // Subscribing to a control sends its value first.
    assert_eq!(recv(&other).expect("value").addr, "/channel2/play");
    std::thread::sleep(Duration::from_millis(50));
    osc.send_event(OscMessage {
        addr: "/beat".into(),
        args: vec![OscType::Int(1), OscType::Int(1), OscType::Float(128.0)],
    });
    let m = recv(&follower).expect("the beat");
    assert_eq!(m.addr, "/beat");
    assert_eq!(m.args.len(), 3);
    assert!(recv(&other).is_none());
}
