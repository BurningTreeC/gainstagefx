//! What the panel is fed, written by the audio thread and read by the editor.

use std::sync::atomic::{AtomicU32, Ordering};

/// Where the meter's scale starts, in dB relative to the circuits' nominal.
/// Everything is stored as an offset from this so it fits an unsigned atomic.
const FLOOR: f32 = -60.0;

#[derive(Default)]
pub struct Meters {
    /// Input level relative to nominal, in hundredths of a dB above `FLOOR`.
    input: AtomicU32,
}

impl Meters {
    /// How far the signal arriving at the circuit is from the level the
    /// circuit was calibrated at. Zero is where the voices were measured, and
    /// it is the only number on the panel that says whether the rest of it
    /// means what it says.
    pub fn input_db(&self) -> f32 {
        FLOOR + self.input.load(Ordering::Relaxed) as f32 / 100.0
    }

    pub fn set_input_db(&self, relative: f32) {
        let stored = ((relative - FLOOR) * 100.0).clamp(0.0, 12_000.0) as u32;
        self.input.store(stored, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.set_input_db(FLOOR);
    }
}
