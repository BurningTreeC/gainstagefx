//! Valve stages in a row, which is where high gain comes from.
//!
//! One triode gives about fifty-eight times and a couple of per cent of
//! second harmonic when it is leaned on -- that is a warm preamplifier and it
//! is not a metal sound. What makes the difference is not a hotter valve, it
//! is another valve: each stage amplifies the last one's distortion as well as
//! its signal, so the harmonics compound rather than add, and by the third
//! stage the grid is being driven well past its own bias on every note.
//!
//! Every stage's cathode bypass is on the same control, so one knob opens all
//! of them together. That is what a gain control on this kind of amplifier
//! does: it is not a volume between stages, it is how much local feedback each
//! stage is allowed to keep.

use super::valve::{self, Values};
use crate::dsp::netlist::{Circuit, Fault, Netlist};

/// The gain control: how much of each cathode resistor its bypass covers.
pub const GAIN: usize = 0;

/// How many stages, and how each is set up.
#[derive(Clone, Copy, Debug)]
pub struct Preamp {
    pub stages: usize,
    pub values: Values,
    /// What sits between one plate and the next grid. A real amplifier has a
    /// volume control here; this is the fixed part of it.
    pub interstage: f64,
}

/// One stage, run gently. Subtle saturation and nothing more.
pub const CLEAN: Preamp = Preamp {
    stages: 1,
    values: valve::CLASSIC,
    interstage: 1_000_000.0,
};

/// Two stages: the sound of an amplifier being pushed.
pub const CRUNCH: Preamp = Preamp {
    stages: 2,
    values: valve::CLASSIC,
    interstage: 470_000.0,
};

/// Three, run hard. Every one of them clipping on every note.
pub const HIGH_GAIN: Preamp = Preamp {
    stages: 3,
    values: valve::HOT,
    interstage: 220_000.0,
};

pub fn build(p: &Preamp, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("valve preamp");
    net.input("in", source);
    let mut node = String::from("in");
    for index in 0..p.stages.max(1) {
        let prefix = format!("v{index}");
        let out = valve::stage(&mut net, &prefix, &node, &p.values, GAIN);
        // Each plate works into the next grid leak, and the last into whatever
        // follows the preamplifier.
        let is_last = index + 1 == p.stages.max(1);
        let ohms = if is_last { load } else { p.interstage };
        net.resistor(&out, "gnd", ohms);
        node = out;
    }
    net.build(&node)
}
