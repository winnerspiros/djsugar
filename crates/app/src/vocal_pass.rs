//! The vocal pass behind Vocals › Bar (the waveforms button's drop-down): for each file
//! loaded on a deck, the levels of [`opendj_engine::vocals`], read from the
//! cache or worked out on one background thread, a file at a time. The
//! renderer asks with [`levels`] every frame and uploads them with the
//! waveform once they are there.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};

use opendj_engine::vocals;

/// Called when a file's levels arrive, to draw again.
type Wake = Box<dyn Fn() + Send + Sync>;

struct Pass {
    /// Files done: their levels, or None when they could not be found.
    done: Mutex<HashMap<PathBuf, Option<Arc<vocals::Levels>>>>,
    /// Files asked for and not done yet.
    asked: Mutex<HashSet<PathBuf>>,
    send: Mutex<Sender<PathBuf>>,
}

static PASS: OnceLock<Pass> = OnceLock::new();

/// Starts the background thread; `wake` runs when levels arrive.
pub fn start(wake: impl Fn() + Send + Sync + 'static) {
    let (send, receive) = channel::<PathBuf>();
    if PASS
        .set(Pass {
            done: Mutex::default(),
            asked: Mutex::default(),
            send: Mutex::new(send),
        })
        .is_err()
    {
        return;
    }
    let wake: Wake = Box::new(wake);
    std::thread::Builder::new()
        .name("vocal pass".into())
        .spawn(move || {
            let pass = PASS.get().expect("set above");
            for path in receive {
                let found = vocals::analyse(&path, None, crate::wave_look::show_vocals)
                    .map_err(|e| eprintln!("vocals {}: {e}", path.display()))
                    .ok()
                    .filter(|l| !l.is_empty())
                    .map(Arc::new);
                pass.asked.lock().unwrap().remove(&path);
                // Switched off part way: ask again when it is back on.
                if found.is_none() && !crate::wave_look::show_vocals() {
                    continue;
                }
                pass.done.lock().unwrap().insert(path, found);
                wake();
            }
        })
        .expect("start the vocal pass");
}

/// The vocal levels of the file at `path`, if found; asks for them the
/// first time.
pub fn levels(path: &Path) -> Option<Arc<vocals::Levels>> {
    let pass = PASS.get()?;
    if let Some(found) = pass.done.lock().unwrap().get(path) {
        return found.clone();
    }
    if pass.asked.lock().unwrap().insert(path.to_path_buf()) {
        let _ = pass.send.lock().unwrap().send(path.to_path_buf());
    }
    None
}
