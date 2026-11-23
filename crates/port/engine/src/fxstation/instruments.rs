//! X-Pad instruments: kick, snare, clap and hi-hat samples.
//!
//! The C++ version loads samples from disk in background threads. In this
//! Rust port, we provide the data structures and API; sample loading would
//! be done by the host application and passed in.

use std::sync::{Arc, Mutex};

/// A sound of the X-Pad: interleaved stereo samples.
#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub samples: Vec<f32>,
    pub sample_rate: f32,
}

impl Instrument {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// A bank of 4 instruments (kick, snare, clap, hat).
#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub instruments: [Instrument; 4],
}

/// The sounds of the X-Pad. Bank 0 plays the app's built-in kit.
/// Banks 1-3 would come from user directories in the full application.
#[derive(Debug)]
pub struct Instruments {
    banks: Mutex<[Option<Arc<Bank>>; 4]>,
}

impl Default for Instruments {
    fn default() -> Self {
        Self::new()
    }
}

impl Instruments {
    pub fn new() -> Self {
        Self {
            banks: Mutex::new([None, None, None, None]),
        }
    }

    /// For the engine thread: the bank, or None while not loaded.
    pub fn bank(&self, index: usize) -> Option<Arc<Bank>> {
        if index >= 4 {
            return None;
        }
        self.banks.lock().unwrap()[index].clone()
    }

    /// Sets a bank (used by the host to provide loaded samples).
    pub fn set_bank(&self, index: usize, bank: Arc<Bank>) {
        if index >= 4 {
            return;
        }
        self.banks.lock().unwrap()[index] = Some(bank);
    }

    /// Clears all banks (for reload).
    pub fn clear(&self) {
        *self.banks.lock().unwrap() = [None, None, None, None];
    }
}