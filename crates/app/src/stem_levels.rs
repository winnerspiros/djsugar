//! Each stem's level along a track, for the Stems waveform look: worked
//! out on a background thread once a deck's stems are complete (and again
//! when better stems replace them), then uploaded with the waveform. Show
//! vocals reads the vocal stem's level from here too.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use opendj_engine::stems::Stems;
use opendj_waveform::MAX_STEMS;

/// Levels per second: a level every 10 ms, enough for the scrolling view.
pub const RATE: f64 = 100.0;

/// Levels found, by the stems they are for (see [`key`]), with those
/// stems so a new track's stems at the same address aren't taken for
/// them; None while they are being worked out.
type Found = HashMap<usize, (Weak<Stems>, Option<Arc<Levels>>)>;

/// A track's stem levels and each stem's colour, and for high-quality
/// stems the vocal stem's level for Vocals › Bar (see
/// [`Stems::vocal_levels`]).
pub struct Levels {
    pub levels: Vec<[u8; MAX_STEMS]>,
    pub colors: Vec<u32>,
    pub vocals: Option<Arc<Vec<u8>>>,
}

/// Levels kept at most; older ones are dropped (decks reload them).
const KEPT: usize = 16;

static FOUND: OnceLock<Mutex<(Found, Vec<usize>)>> = OnceLock::new();
static WAKE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// `wake` runs when levels arrive, to draw again.
pub fn start(wake: impl Fn() + Send + Sync + 'static) {
    let _ = WAKE.set(Box::new(wake));
}

/// Which stems play now: these, or their upgrade once it is complete.
fn key(stems: &Stems) -> usize {
    stems.current() as *const Stems as usize
}

/// The stem levels of `stems` once they are found; starts finding them
/// the first time they are asked for complete.
pub fn levels(stems: &Arc<Stems>) -> Option<Arc<Levels>> {
    if !stems.current().is_complete() {
        return None;
    }
    let k = key(stems);
    let found = FOUND.get_or_init(Default::default);
    let mut guard = found.lock().unwrap();
    let (map, order) = &mut *guard;
    if let Some((w, l)) = map.get(&k) {
        if w.upgrade().is_some_and(|s| Arc::ptr_eq(&s, stems)) {
            return l.clone();
        }
    }
    map.insert(k, (Arc::downgrade(stems), None));
    order.retain(|&o| o != k);
    order.push(k);
    if order.len() > KEPT {
        let old = order.remove(0);
        map.remove(&old);
    }
    drop(guard);
    let stems = stems.clone();
    std::thread::Builder::new()
        .name("stem levels".into())
        .spawn(move || {
            let current = stems.current();
            let levels = Levels {
                levels: current.levels(RATE).unwrap_or_default(),
                colors: current.info.iter().map(|i| i.color).collect(),
                vocals: current.vocal_levels(RATE).map(Arc::new),
            };
            if let Some(m) = FOUND.get() {
                let mut guard = m.lock().unwrap();
                if let Some((w, slot)) = guard.0.get_mut(&k) {
                    if w.upgrade().is_some_and(|s| Arc::ptr_eq(&s, &stems)) {
                        *slot = Some(Arc::new(levels));
                    }
                }
            }
            if let Some(wake) = WAKE.get() {
                wake();
            }
        })
        .ok();
    None
}
