//! Who may use the remote: phones paired once, by the QR code's one-time
//! code or by the PIN, and kept as devices that can be revoked.
//!
//! A paired phone keeps a long random token; DJ Sugar keeps only its SHA-256,
//! so the settings file never holds anything a phone could log in with.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// How long the QR code's code works, from when it is shown.
pub const CODE_LIFETIME: Duration = Duration::from_secs(120);
/// Wrong codes or PINs in a row before pairing stops for a while.
pub const MAX_FAILURES: u32 = 5;
pub const LOCKOUT: Duration = Duration::from_secs(60);

/// A paired phone.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// SHA-256 of its token, in hex.
    pub hash: String,
    /// When it was paired and last connected, in seconds since 1970.
    pub added: u64,
    pub last_seen: u64,
}

impl Device {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "hash": self.hash,
            "added": self.added,
            "last_seen": self.last_seen,
        })
    }

    pub fn from_json(v: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id: v["id"].as_str()?.to_string(),
            name: v["name"].as_str().unwrap_or("Phone").to_string(),
            hash: v["hash"].as_str()?.to_string(),
            added: v["added"].as_u64().unwrap_or(0),
            last_seen: v["last_seen"].as_u64().unwrap_or(0),
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum PairError {
    /// Wrong, used or expired.
    Wrong,
    /// Too many wrong tries: wait this long.
    Locked(Duration),
}

impl PairError {
    pub fn message(&self) -> String {
        match self {
            PairError::Wrong => {
                "That code didn't work. Scan the QR code again, or check the PIN.".into()
            }
            PairError::Locked(d) => format!(
                "Too many wrong tries. Try again in {} s.",
                d.as_secs().max(1)
            ),
        }
    }
}

pub struct Pairing {
    code: String,
    code_made: Instant,
    /// Counts new codes, so the QR code is drawn again only when it changes.
    code_generation: u64,
    pin: String,
    failures: u32,
    locked_until: Option<Instant>,
    devices: Vec<Device>,
    /// Devices changed since [`Pairing::take_changed`] last said so.
    changed: bool,
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("system random numbers");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn random_pin() -> String {
    let mut buf = [0u8; 4];
    getrandom::fill(&mut buf).expect("system random numbers");
    format!("{:06}", u32::from_le_bytes(buf) % 1_000_000)
}

pub fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Compares without stopping at the first difference.
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Pairing {
    pub fn new(devices: Vec<Device>) -> Self {
        Self {
            code: random_hex(16),
            code_made: Instant::now(),
            code_generation: 1,
            pin: random_pin(),
            failures: 0,
            locked_until: None,
            devices,
            changed: false,
        }
    }

    /// The QR code's one-time code, and its generation; a fresh one once the
    /// last has expired or been used.
    pub fn code(&mut self, now: Instant) -> (String, u64) {
        if now.duration_since(self.code_made) >= CODE_LIFETIME {
            self.renew_code(now);
        }
        (self.code.clone(), self.code_generation)
    }

    /// When the current code stops working.
    pub fn code_expires(&self) -> Instant {
        self.code_made + CODE_LIFETIME
    }

    pub fn renew_code(&mut self, now: Instant) {
        self.code = random_hex(16);
        self.code_made = now;
        self.code_generation += 1;
    }

    pub fn pin(&self) -> &str {
        &self.pin
    }

    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    /// Whether the devices changed since the last call, to save them.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    fn check_lock(&mut self, now: Instant) -> Result<(), PairError> {
        match self.locked_until {
            Some(t) if now < t => Err(PairError::Locked(t - now)),
            Some(_) => {
                self.locked_until = None;
                self.failures = 0;
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn fail(&mut self, now: Instant) -> PairError {
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            self.locked_until = Some(now + LOCKOUT);
            // Whatever was being guessed is no use afterwards.
            self.pin = random_pin();
            self.renew_code(now);
            return PairError::Locked(LOCKOUT);
        }
        PairError::Wrong
    }

    /// Pairs a phone with the QR code's code. Returns the new device and
    /// the token the phone keeps; the code is used up.
    pub fn pair_with_code(
        &mut self,
        code: &str,
        name: &str,
        now: Instant,
    ) -> Result<(Device, String), PairError> {
        self.check_lock(now)?;
        let fresh = now.duration_since(self.code_made) < CODE_LIFETIME;
        if !fresh || !same(code, &self.code) {
            return Err(self.fail(now));
        }
        self.renew_code(now);
        Ok(self.add(name))
    }

    /// Pairs a phone with the PIN shown on the laptop; a new PIN follows.
    pub fn pair_with_pin(
        &mut self,
        pin: &str,
        name: &str,
        now: Instant,
    ) -> Result<(Device, String), PairError> {
        self.check_lock(now)?;
        let pin: String = pin.chars().filter(|c| c.is_ascii_digit()).collect();
        if !same(&pin, &self.pin) {
            return Err(self.fail(now));
        }
        self.pin = random_pin();
        Ok(self.add(name))
    }

    fn add(&mut self, name: &str) -> (Device, String) {
        self.failures = 0;
        let token = random_hex(32);
        let name: String = name.trim().chars().take(40).collect();
        let now = unix_now();
        let device = Device {
            id: random_hex(6),
            name: if name.is_empty() {
                "Phone".into()
            } else {
                name
            },
            hash: hash(&token),
            added: now,
            last_seen: now,
        };
        self.devices.push(device.clone());
        self.changed = true;
        (device, token)
    }

    /// Whether `token` belongs to device `id`; notes that it was seen.
    pub fn check(&mut self, id: &str, token: &str) -> Option<Device> {
        let h = hash(token);
        let d = self
            .devices
            .iter_mut()
            .find(|d| d.id == id && same(&d.hash, &h))?;
        d.last_seen = unix_now();
        self.changed = true;
        Some(d.clone())
    }

    /// Forgets a device; returns whether it was there.
    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        let gone = self.devices.len() != before;
        self.changed |= gone;
        gone
    }

    pub fn is_paired(&self, id: &str) -> bool {
        self.devices.iter().any(|d| d.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_pairs_once() {
        let now = Instant::now();
        let mut p = Pairing::new(Vec::new());
        let (code, generation) = p.code(now);
        let (device, token) = p.pair_with_code(&code, "DJ's iPhone", now).unwrap();
        assert_eq!(device.name, "DJ's iPhone");
        assert!(p.take_changed());
        // Used up: the same code fails, and a new one is out.
        assert_eq!(p.pair_with_code(&code, "x", now), Err(PairError::Wrong));
        assert!(p.code(now).1 > generation);
        // The token logs in; another doesn't.
        assert!(p.check(&device.id, &token).is_some());
        assert!(p.check(&device.id, &random_hex(32)).is_none());
        assert!(p.check("other", &token).is_none());
        // Only the hash is kept.
        assert!(!p.devices()[0].hash.contains(&token));
    }

    #[test]
    fn code_expires() {
        let now = Instant::now();
        let mut p = Pairing::new(Vec::new());
        let (code, _) = p.code(now);
        let later = now + CODE_LIFETIME + Duration::from_secs(1);
        assert_eq!(p.pair_with_code(&code, "x", later), Err(PairError::Wrong));
        // Showing the QR code again makes a new one that works.
        let (code, _) = p.code(later);
        assert!(p.pair_with_code(&code, "x", later).is_ok());
    }

    #[test]
    fn pin_pairs_and_changes() {
        let now = Instant::now();
        let mut p = Pairing::new(Vec::new());
        let pin = p.pin().to_string();
        assert_eq!(pin.len(), 6);
        let spaced = format!("{} {}", &pin[..3], &pin[3..]);
        assert!(p.pair_with_pin(&spaced, "Tablet", now).is_ok());
        assert_ne!(p.pin(), pin);
    }

    #[test]
    fn wrong_tries_lock() {
        let now = Instant::now();
        let mut p = Pairing::new(Vec::new());
        let pin = p.pin().to_string();
        let wrong = if pin == "000000" { "111111" } else { "000000" };
        for _ in 0..MAX_FAILURES - 1 {
            assert_eq!(p.pair_with_pin(wrong, "x", now), Err(PairError::Wrong));
        }
        assert_eq!(
            p.pair_with_pin(wrong, "x", now),
            Err(PairError::Locked(LOCKOUT))
        );
        // Even the right PIN (now changed) waits out the lock.
        let right = p.pin().to_string();
        assert!(matches!(
            p.pair_with_pin(&right, "x", now),
            Err(PairError::Locked(_))
        ));
        let later = now + LOCKOUT + Duration::from_secs(1);
        assert!(p.pair_with_pin(&right, "x", later).is_ok());
    }

    #[test]
    fn revoke_and_save() {
        let now = Instant::now();
        let mut p = Pairing::new(Vec::new());
        let (code, _) = p.code(now);
        let (d, token) = p.pair_with_code(&code, "Phone", now).unwrap();
        let saved: Vec<Device> = p
            .devices()
            .iter()
            .map(|d| Device::from_json(&d.to_json()).unwrap())
            .collect();
        let mut q = Pairing::new(saved);
        assert!(q.check(&d.id, &token).is_some());
        assert!(q.revoke(&d.id));
        assert!(q.check(&d.id, &token).is_none());
        assert!(!q.revoke(&d.id));
    }
}
