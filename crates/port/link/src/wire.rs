//! The messages on the network: discovery (multicast, every peer's state) and measurement (unicast ping/pong that
//! maps a peer's session clock to ours). All integers are big-endian. A payload is a run of entries, each a four-character
//! key, a 32-bit size and that many bytes; unknown keys are skipped, so newer peers can add entries.

pub const DISCOVERY_HEADER: [u8; 8] = *b"_asdp_v\x01";
pub const MEASUREMENT_HEADER: [u8; 8] = *b"_link_v\x01";

pub const ALIVE: u8 = 1;
pub const RESPONSE: u8 = 2;
pub const BYEBYE: u8 = 3;

pub const PING: u8 = 1;
pub const PONG: u8 = 2;

const fn key(k: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*k)
}

pub const TIMELINE: u32 = key(b"tmln");
pub const SESSION: u32 = key(b"sess");
pub const START_STOP: u32 = key(b"stst");
pub const ENDPOINT_V4: u32 = key(b"mep4");
pub const HOST_TIME: u32 = key(b"__ht");
pub const GHOST_TIME: u32 = key(b"__gt");
pub const PREV_GHOST_TIME: u32 = key(b"_pgt");

/// Largest message read or written.
pub const MAX_MESSAGE: usize = 512;

pub type NodeId = [u8; 8];

/// A session's timeline on the shared clock: `beat_origin` (micro-beats)
/// falls at `time_origin` (microseconds) and beats advance one per
/// `micros_per_beat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireTimeline {
    pub micros_per_beat: i64,
    pub beat_origin: i64,
    pub time_origin: i64,
}

/// What a peer announces about itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerState {
    pub session: NodeId,
    pub timeline: WireTimeline,
    pub endpoint: Option<([u8; 4], u16)>,
}

/// A discovery message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Discovery {
    pub kind: u8,
    pub ttl: u8,
    pub node: NodeId,
    pub state: Option<PeerState>,
}

/// The entries of a measurement message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Measurement {
    pub session: Option<NodeId>,
    pub host_time: Option<i64>,
    pub ghost_time: Option<i64>,
    pub prev_ghost_time: Option<i64>,
}

fn entry(out: &mut Vec<u8>, key: u32, value: &[u8]) {
    out.extend_from_slice(&key.to_be_bytes());
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}

fn timeline_bytes(t: &WireTimeline) -> [u8; 24] {
    let mut b = [0; 24];
    b[..8].copy_from_slice(&t.micros_per_beat.to_be_bytes());
    b[8..16].copy_from_slice(&t.beat_origin.to_be_bytes());
    b[16..].copy_from_slice(&t.time_origin.to_be_bytes());
    b
}

pub fn encode_discovery(m: &Discovery) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&DISCOVERY_HEADER);
    out.push(m.kind);
    out.push(m.ttl);
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&m.node);
    if let Some(s) = &m.state {
        entry(&mut out, TIMELINE, &timeline_bytes(&s.timeline));
        entry(&mut out, SESSION, &s.session);
        // Start/stop sharing is not used: not playing, at beat 0.
        entry(&mut out, START_STOP, &[0; 17]);
        if let Some((ip, port)) = s.endpoint {
            let mut b = [0; 6];
            b[..4].copy_from_slice(&ip);
            b[4..].copy_from_slice(&port.to_be_bytes());
            entry(&mut out, ENDPOINT_V4, &b);
        }
    }
    out
}

/// Calls `f` with each entry's key and value; None if the payload is cut short.
fn entries<'a>(mut p: &'a [u8], mut f: impl FnMut(u32, &'a [u8])) -> Option<()> {
    while !p.is_empty() {
        let key = u32::from_be_bytes(p.get(..4)?.try_into().ok()?);
        let size = u32::from_be_bytes(p.get(4..8)?.try_into().ok()?) as usize;
        let value = p.get(8..8 + size)?;
        f(key, value);
        p = &p[8 + size..];
    }
    Some(())
}

fn i64_at(b: &[u8], at: usize) -> Option<i64> {
    Some(i64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

pub fn decode_discovery(b: &[u8]) -> Option<Discovery> {
    if b.get(..8)? != DISCOVERY_HEADER {
        return None;
    }
    let kind = *b.get(8)?;
    let ttl = *b.get(9)?;
    let node: NodeId = b.get(12..20)?.try_into().ok()?;
    let (mut timeline, mut session, mut endpoint) = (None, None, None);
    entries(&b[20..], |key, v| match key {
        TIMELINE if v.len() == 24 => {
            timeline = Some(WireTimeline {
                micros_per_beat: i64_at(v, 0).unwrap_or(0),
                beat_origin: i64_at(v, 8).unwrap_or(0),
                time_origin: i64_at(v, 16).unwrap_or(0),
            })
        }
        SESSION if v.len() == 8 => session = v.try_into().ok(),
        ENDPOINT_V4 if v.len() == 6 => {
            let ip: [u8; 4] = [v[0], v[1], v[2], v[3]];
            endpoint = Some((ip, u16::from_be_bytes([v[4], v[5]])));
        }
        _ => {}
    })?;
    let state = match (session, timeline) {
        (Some(session), Some(timeline)) if timeline.micros_per_beat > 0 => {
            Some(PeerState { session, timeline, endpoint })
        }
        _ => None,
    };
    Some(Discovery { kind, ttl, node, state })
}

pub fn encode_measurement(kind: u8, m: &Measurement, echo: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(&MEASUREMENT_HEADER);
    out.push(kind);
    if let Some(s) = &m.session {
        entry(&mut out, SESSION, s);
    }
    for (key, v) in [(GHOST_TIME, m.ghost_time), (PREV_GHOST_TIME, m.prev_ghost_time), (HOST_TIME, m.host_time)] {
        if let Some(v) = v {
            entry(&mut out, key, &v.to_be_bytes());
        }
    }
    out.extend_from_slice(echo);
    out
}

/// The kind, the entries and the raw payload (a pong echoes a ping's).
pub fn decode_measurement(b: &[u8]) -> Option<(u8, Measurement, &[u8])> {
    if b.get(..8)? != MEASUREMENT_HEADER {
        return None;
    }
    let kind = *b.get(8)?;
    let payload = &b[9..];
    let mut m = Measurement::default();
    entries(payload, |key, v| match key {
        SESSION if v.len() == 8 => m.session = v.try_into().ok(),
        HOST_TIME => m.host_time = i64_at(v, 0),
        GHOST_TIME => m.ghost_time = i64_at(v, 0),
        PREV_GHOST_TIME => m.prev_ghost_time = i64_at(v, 0),
        _ => {}
    })?;
    Some((kind, m, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_round_trips() {
        let m = Discovery {
            kind: ALIVE,
            ttl: 5,
            node: *b"abcdefgh",
            state: Some(PeerState {
                session: *b"sessionA",
                timeline: WireTimeline { micros_per_beat: 500_000, beat_origin: -3_250_000, time_origin: 123_456_789 },
                endpoint: Some(([192, 168, 1, 7], 50123)),
            }),
        };
        let b = encode_discovery(&m);
        assert_eq!(&b[..8], b"_asdp_v\x01");
        assert_eq!(&b[20..24], b"tmln");
        assert_eq!(decode_discovery(&b), Some(m));
    }

    #[test]
    fn byebye_has_no_state() {
        let m = Discovery { kind: BYEBYE, ttl: 0, node: *b"12345678", state: None };
        let b = encode_discovery(&m);
        assert_eq!(b.len(), 20);
        assert_eq!(decode_discovery(&b), Some(m));
    }

    #[test]
    fn unknown_entries_are_skipped_and_short_ones_rejected() {
        let mut b = encode_discovery(&Discovery { kind: ALIVE, ttl: 5, node: [1; 8], state: None });
        entry(&mut b, key(b"zzzz"), &[9; 3]);
        assert!(decode_discovery(&b).is_some());
        b.pop();
        assert!(decode_discovery(&b).is_none());
    }

    #[test]
    fn pong_echoes_the_ping() {
        let ping = encode_measurement(
            PING,
            &Measurement { host_time: Some(42), prev_ghost_time: Some(7), ..Default::default() },
            &[],
        );
        let (kind, m, payload) = decode_measurement(&ping).unwrap();
        assert_eq!((kind, m.host_time, m.prev_ghost_time), (PING, Some(42), Some(7)));
        let pong = encode_measurement(
            PONG,
            &Measurement { session: Some(*b"sessionB"), ghost_time: Some(1000), ..Default::default() },
            payload,
        );
        let (kind, m, _) = decode_measurement(&pong).unwrap();
        assert_eq!(kind, PONG);
        assert_eq!(m.session, Some(*b"sessionB"));
        assert_eq!(m.ghost_time, Some(1000));
        assert_eq!(m.host_time, Some(42));
        assert_eq!(m.prev_ghost_time, Some(7));
    }
}
