//! The network side: announces this app's state, tracks peers, measures other sessions' clocks and joins the session that
//! has been running longest. Every session has a shared ("ghost") clock: each peer's host clock plus an offset it measures
//! by pinging a member. A timeline on the wire is in ghost time, so peers with unrelated clocks agree on where beats fall.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};

use crate::timeline::Timeline;
use crate::wire::{self, Discovery, Measurement, NodeId, PeerState, WireTimeline};
use crate::{now_micros, Shared};

const GROUP: Ipv4Addr = Ipv4Addr::new(224, 76, 78, 75);
const PORT: u16 = 20808;
/// Seconds a peer's announcement stays valid.
const TTL: u8 = 5;
const ANNOUNCE_EVERY: Duration = Duration::from_secs(1);
/// Soonest a change is announced after the last announcement.
const MIN_ANNOUNCE_GAP: Duration = Duration::from_millis(50);
const PING_TIMEOUT: Duration = Duration::from_millis(50);
const PING_TRIES: u32 = 5;
/// Clock offset samples a measurement collects.
const SAMPLES: usize = 100;
const REMEASURE_EVERY: Duration = Duration::from_secs(30);
/// A session must have been running this much longer to be joined.
const SESSION_EPS: f64 = 500_000.0;

pub(crate) struct Running {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Running {
    pub(crate) fn start(shared: Arc<Shared>) -> io::Result<Self> {
        let mut node = Node::open(shared)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new().name("link".into()).spawn(move || node.run(&flag))?;
        Ok(Self { stop, thread })
    }

    pub(crate) fn stop(self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.thread.join();
    }
}

struct Peer {
    state: PeerState,
    expires: Instant,
}

struct Measuring {
    session: NodeId,
    to: SocketAddr,
    samples: Vec<f64>,
    sent: Instant,
    tries: u32,
    prev_ghost: Option<i64>,
}

/// The ghost clock is `host + intercept`.
#[derive(Clone, Copy)]
struct XForm {
    intercept: f64,
}

impl XForm {
    fn to_ghost(self, host: f64) -> f64 {
        host + self.intercept
    }
    fn to_host(self, ghost: f64) -> f64 {
        ghost - self.intercept
    }
}

struct Node {
    shared: Arc<Shared>,
    id: NodeId,
    session: NodeId,
    xform: XForm,
    /// The session timeline as last announced or adopted, in ghost time.
    timeline: WireTimeline,
    peers: HashMap<NodeId, Peer>,
    measuring: Option<Measuring>,
    measured_at: Instant,
    /// Sessions that couldn't be measured, not retried until this time.
    unreachable: HashMap<NodeId, Instant>,
    discovery: UdpSocket,
    unicast: UdpSocket,
    endpoint: SocketAddrV4,
    announced: Instant,
    changed: bool,
}

fn random_id() -> NodeId {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut b = [0u8; 8];
    if getrandom::fill(&mut b).is_err() {
        let t = now_micros() as u64 ^ (std::process::id() as u64) << 32;
        b = t.to_le_bytes();
    }
    b.map(|x| CHARS[x as usize % CHARS.len()])
}

/// The address of the interface multicast goes out on.
fn local_ip() -> Ipv4Addr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|s| {
            s.connect((GROUP, PORT))?;
            s.local_addr()
        })
        .ok()
        .and_then(|a| match a {
            SocketAddr::V4(a) if !a.ip().is_unspecified() => Some(*a.ip()),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

fn to_wire(t: &Timeline, x: XForm) -> WireTimeline {
    WireTimeline {
        micros_per_beat: (60e6 / t.bpm).round() as i64,
        beat_origin: (t.beat_origin * 1e6).round() as i64,
        time_origin: x.to_ghost(t.time_origin).round() as i64,
    }
}

fn from_wire(w: &WireTimeline, x: XForm) -> Timeline {
    Timeline {
        bpm: 60e6 / w.micros_per_beat as f64,
        beat_origin: w.beat_origin as f64 / 1e6,
        time_origin: x.to_host(w.time_origin as f64),
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

impl Node {
    fn open(shared: Arc<Shared>) -> io::Result<Self> {
        let ip = local_ip();
        let d = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        d.set_reuse_address(true)?;
        #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
        d.set_reuse_port(true)?;
        d.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT)).into())?;
        d.join_multicast_v4(&GROUP, &ip).or_else(|_| d.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED))?;
        d.set_nonblocking(true)?;
        let u = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        u.bind(&SocketAddr::from((ip, 0)).into())?;
        u.set_multicast_if_v4(&ip)?;
        u.set_multicast_loop_v4(true)?;
        u.set_nonblocking(true)?;
        let unicast: UdpSocket = u.into();
        let endpoint = match unicast.local_addr()? {
            SocketAddr::V4(a) => a,
            SocketAddr::V6(_) => return Err(io::Error::other("no IPv4 address")),
        };
        let id = random_id();
        let host = now_micros() as f64;
        // A new session's clock starts at zero.
        let xform = XForm { intercept: -host };
        let (t, _) = shared.timeline.read();
        let timeline = to_wire(&t.rebased(host), xform);
        Ok(Self {
            shared,
            id,
            session: id,
            xform,
            timeline,
            peers: HashMap::new(),
            measuring: None,
            measured_at: Instant::now(),
            unreachable: HashMap::new(),
            discovery: d.into(),
            unicast,
            endpoint,
            announced: Instant::now() - ANNOUNCE_EVERY,
            changed: true,
        })
    }

    fn run(&mut self, stop: &AtomicBool) {
        let mut buf = [0u8; wire::MAX_MESSAGE];
        while !stop.load(Ordering::Acquire) {
            let mut busy = false;
            while let Ok((n, from)) = self.discovery.recv_from(&mut buf) {
                busy = true;
                self.discovery_message(&buf[..n], from);
            }
            while let Ok((n, from)) = self.unicast.recv_from(&mut buf) {
                busy = true;
                if buf[..n].starts_with(&wire::DISCOVERY_HEADER) {
                    self.discovery_message(&buf[..n], from);
                } else {
                    self.measurement_message(&buf[..n], from);
                }
            }
            self.tick();
            if !busy {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        self.send_discovery(wire::BYEBYE, None);
    }

    fn state(&self) -> PeerState {
        PeerState {
            session: self.session,
            timeline: self.timeline,
            endpoint: Some((self.endpoint.ip().octets(), self.endpoint.port())),
        }
    }

    fn send_discovery(&self, kind: u8, to: Option<SocketAddr>) {
        let m = Discovery {
            kind,
            ttl: if kind == wire::BYEBYE { 0 } else { TTL },
            node: self.id,
            state: (kind != wire::BYEBYE).then(|| self.state()),
        };
        let to = to.unwrap_or(SocketAddr::from((GROUP, PORT)));
        let _ = self.unicast.send_to(&wire::encode_discovery(&m), to);
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if self.shared.committed.swap(false, Ordering::AcqRel) {
            self.local_commit();
        }
        let before = self.peers.len();
        self.peers.retain(|_, p| p.expires > now);
        if self.peers.len() != before {
            self.count_peers();
        }
        let gap = now.duration_since(self.announced);
        if gap >= ANNOUNCE_EVERY || (self.changed && gap >= MIN_ANNOUNCE_GAP) {
            self.send_discovery(wire::ALIVE, None);
            self.announced = now;
            self.changed = false;
        }
        if let Some(m) = &mut self.measuring {
            if now.duration_since(m.sent) > PING_TIMEOUT {
                m.tries += 1;
                if m.tries > PING_TRIES {
                    let session = m.session;
                    self.measuring = None;
                    self.unreachable.insert(session, now + REMEASURE_EVERY);
                } else {
                    self.ping();
                }
            }
        } else if now.duration_since(self.measured_at) > REMEASURE_EVERY {
            self.measured_at = now;
            let member = self.peers.values().find(|p| p.state.session == self.session).and_then(|p| p.state.endpoint);
            if let Some(e) = member {
                self.start_measuring(self.session, e);
            }
        }
    }

    /// The app committed a change: announce it, ordered after the timeline
    /// in force by a higher beat origin.
    fn local_commit(&mut self) {
        let (t, _) = self.shared.timeline.read();
        let host = now_micros() as f64;
        let mut t = t.rebased(host);
        let floor = (self.timeline.beat_origin + 1) as f64 / 1e6;
        if t.beat_origin < floor {
            t = t.rebased(t.time_of(floor));
        }
        self.timeline = to_wire(&t, self.xform);
        self.changed = true;
    }

    /// Puts the session timeline into the shared one, keeping the app's offset.
    fn publish(&self) {
        let (_, offset) = self.shared.timeline.read();
        self.shared.timeline.write(from_wire(&self.timeline, self.xform), offset);
    }

    fn count_peers(&self) {
        let n = self.peers.values().filter(|p| p.state.session == self.session).count();
        self.shared.peers.store(n, Ordering::Relaxed);
    }

    fn discovery_message(&mut self, b: &[u8], from: SocketAddr) {
        let Some(m) = wire::decode_discovery(b) else {
            return;
        };
        if m.node == self.id {
            return;
        }
        if m.kind == wire::BYEBYE {
            self.peers.remove(&m.node);
            self.count_peers();
            return;
        }
        let Some(state) = m.state else { return };
        let new = !self.peers.contains_key(&m.node);
        self.peers
            .insert(m.node, Peer { state, expires: Instant::now() + Duration::from_secs(m.ttl.max(1) as u64 + 1) });
        if m.kind == wire::ALIVE && new {
            self.send_discovery(wire::RESPONSE, Some(from));
        }
        if state.session == self.session {
            if state.timeline.beat_origin > self.timeline.beat_origin {
                self.timeline = state.timeline;
                self.publish();
            }
        } else if self.measuring.is_none() {
            let blocked = self.unreachable.get(&state.session).is_some_and(|until| *until > Instant::now());
            if let (false, Some(e)) = (blocked, state.endpoint) {
                self.start_measuring(state.session, e);
            }
        }
        self.count_peers();
    }

    fn start_measuring(&mut self, session: NodeId, (ip, port): ([u8; 4], u16)) {
        self.measuring = Some(Measuring {
            session,
            to: SocketAddr::from((ip, port)),
            samples: Vec::with_capacity(SAMPLES + 2),
            sent: Instant::now(),
            tries: 0,
            prev_ghost: None,
        });
        self.ping();
    }

    fn ping(&mut self) {
        let Some(m) = &mut self.measuring else { return };
        let msg = Measurement { host_time: Some(now_micros()), prev_ghost_time: m.prev_ghost, ..Default::default() };
        m.sent = Instant::now();
        let _ = self.unicast.send_to(&wire::encode_measurement(wire::PING, &msg, &[]), m.to);
    }

    fn measurement_message(&mut self, b: &[u8], from: SocketAddr) {
        let Some((kind, m, payload)) = wire::decode_measurement(b) else {
            return;
        };
        let now = now_micros();
        if kind == wire::PING {
            let pong = Measurement {
                session: Some(self.session),
                ghost_time: Some(self.xform.to_ghost(now as f64).round() as i64),
                ..Default::default()
            };
            let _ = self.unicast.send_to(&wire::encode_measurement(wire::PONG, &pong, payload), from);
            return;
        }
        let Some(meas) = &mut self.measuring else {
            return;
        };
        if kind != wire::PONG || m.session != Some(meas.session) {
            return;
        }
        let (Some(ghost), Some(host)) = (m.ghost_time, m.host_time) else {
            return;
        };
        let (ghost, host) = (ghost as f64, host as f64);
        meas.samples.push(ghost - (host + now as f64) / 2.0);
        if let Some(prev) = m.prev_ghost_time {
            meas.samples.push((ghost + prev as f64) / 2.0 - host);
        }
        meas.prev_ghost = Some(ghost as i64);
        meas.tries = 0;
        if meas.samples.len() < SAMPLES {
            self.ping();
            return;
        }
        let session = meas.session;
        let intercept = median(&mut meas.samples);
        self.measuring = None;
        self.measured(session, XForm { intercept });
    }

    fn measured(&mut self, session: NodeId, x: XForm) {
        self.measured_at = Instant::now();
        if session == self.session {
            self.xform = x;
            self.publish();
            return;
        }
        let host = now_micros() as f64;
        let ours = self.xform.to_ghost(host);
        let theirs = x.to_ghost(host);
        let diff = theirs - ours;
        if diff > SESSION_EPS || (diff.abs() < SESSION_EPS && session < self.session) {
            let timeline = self
                .peers
                .values()
                .filter(|p| p.state.session == session)
                .map(|p| p.state.timeline)
                .max_by_key(|t| t.beat_origin);
            if let Some(t) = timeline {
                self.session = session;
                self.xform = x;
                self.timeline = t;
                self.publish();
                self.changed = true;
                self.count_peers();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinkSession, SessionState};

    fn wait_for(mut what: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn median_of_even_and_odd() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), 2.5);
    }

    #[test]
    fn wire_conversion_round_trips() {
        let x = XForm { intercept: -5e6 };
        let t = Timeline { bpm: 128.0, beat_origin: 3.5, time_origin: 9e6 };
        let back = from_wire(&to_wire(&t, x), x);
        assert!((back.bpm - 128.0).abs() < 1e-3);
        assert_eq!(back.beat_origin, 3.5);
        assert_eq!(back.time_origin, 9e6);
    }

    /// Two instances on this machine find each other, settle on one
    /// session and share a tempo change. Needs multicast on the machine's
    /// network interface.
    #[test]
    #[ignore = "needs multicast networking"]
    fn two_instances_share_a_session() {
        let a = LinkSession::new(120.0);
        let b = LinkSession::new(100.0);
        a.enable(true);
        b.enable(true);
        assert!(wait_for(|| a.num_peers() == 1 && b.num_peers() == 1));
        let mut s = SessionState::new();
        a.capture_app_session_state(&mut s);
        s.set_tempo(133.0, a.clock_micros());
        a.commit_app_session_state(&s);
        assert!(wait_for(|| {
            b.capture_app_session_state(&mut s);
            (s.tempo() - 133.0).abs() < 0.01
        }));
        let (mut sa, mut sb) = (SessionState::new(), SessionState::new());
        a.capture_app_session_state(&mut sa);
        b.capture_app_session_state(&mut sb);
        let t = a.clock_micros();
        let d = crate::timeline::phase(sa.beat_at_time(t, 4.0) - sb.beat_at_time(t, 4.0), 4.0);
        assert!(d < 0.01 || d > 3.99, "phase apart by {d}");
    }
}
