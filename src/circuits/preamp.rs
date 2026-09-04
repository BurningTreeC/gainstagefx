//! Valve stages in a row, which is where high gain comes from.
//!
//! One triode gives about fifty-eight times and a couple of per cent of
//! second harmonic when it is leaned on -- that is a warm preamplifier and it
//! is not a metal sound. What makes the difference is not a hotter valve, it
//! is another valve: each stage amplifies the last one's distortion as well as
//! its signal, so the harmonics compound rather than add, and by the third
//! stage the grid is being driven well past its own bias on every note.
//!
//! The gain control is a volume pot, and where it sits is the whole design.
//!
//! The first attempt at this put it on the cathode bypasses instead, on the
//! reasoning that a gain control sets how much local feedback each stage keeps.
//! Measured, that control moved the Clean voice 6 dB end to end and the
//! two-stage voice 12, and a 6 dB knob is not a gain control. The bypass is
//! bounded by the cathode resistor and cannot be anything else.
//!
//! So the control is a volume, and it goes *after the first stage* wherever
//! there is more than one. That is where the hardware puts it, and the reason
//! is audible: the first valve then always sees the instrument at full level
//! and contributes its own character no matter where the knob is, while
//! everything downstream is driven as hard or as gently as the knob says.
//! Putting it at the input instead gives a channel that simply gets quieter.
//! With a single stage there is no "after the first stage" to use, so it goes
//! at the input -- which for one valve is the same thing, since the only stage
//! there is is the one being driven.
//!
//! The bypasses stay fully in circuit, which is where an amplifier of this
//! kind leaves them.

use super::valve::{self, Values};
use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper};

/// The gain control: the volume between the stages.
pub const GAIN: usize = 0;

/// How far down the gain control can turn the stage that follows it.
///
/// A volume pot goes to silence, and a gain control that goes to silence is
/// a gain control whose whole useful range is crammed into its last tenth.
/// Measured with one: the High Gain voice made 0.1 % distortion at a quarter
/// turn, 1.9 at eight tenths, and 25 at the stop -- everything that matters
/// happened after nine tenths, which is not a control, it is a switch with a
/// long approach.
///
/// So the pot works into a resistor rather than to ground, and the span
/// between the two ends is this. Eighteen decibels, which is about what the
/// gain control on an amplifier of this kind actually covers.
///
/// The track is then shaped to sweep those decibels evenly: with the wiper
/// feeding `floor + f * track`, an even sweep wants `f = (k^p - 1)/(k - 1)`
/// for `k` the ratio of the two ends, which is exactly `Taper::Log`.
pub const SPAN: f64 = 8.0;

/// The resistance the track works into, from its own value.
pub fn floor_of(track: f64) -> f64 {
    track / (SPAN - 1.0)
}

/// The cathode bypasses, left fully in circuit unless something asks
/// otherwise. This is a separate control so it can be exposed later without
/// disturbing the gain knob.
pub const BYPASS: usize = 1;

/// How many stages, and how each is set up.
#[derive(Clone, Copy, Debug)]
pub struct Preamp {
    pub stages: usize,
    pub values: Values,
    /// The grid leak between one plate and the next grid, which is also the
    /// volume pot's track where the control sits.
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
    let stages = p.stages.max(1);
    let mut net = Netlist::new("valve preamp");
    net.input("in", source);
    let mut node = String::from("in");

    // One stage has no "between", so the volume goes in front of it.
    if stages == 1 {
        let floor = floor_of(p.interstage);
        net.pot("in", "vol", "vol_floor", p.interstage, Taper::Log { span: SPAN }, GAIN)
            .resistor("vol_floor", "gnd", floor);
        node = String::from("vol");
    }

    for index in 0..stages {
        let prefix = format!("v{index}");
        let out = valve::stage(&mut net, &prefix, &node, &p.values, BYPASS);
        let is_last = index + 1 == stages;
        if is_last {
            net.resistor(&out, "gnd", load);
            node = out;
        } else if index == 0 {
            // The volume pot *is* the grid leak here: its whole track loads
            // the plate and its wiper feeds the next grid.
            let wiper = format!("{prefix}_vol");
            let floor_node = format!("{prefix}_vol_floor");
            let floor = floor_of(p.interstage);
            net.pot(&out, &wiper, &floor_node, p.interstage, Taper::Log { span: SPAN }, GAIN)
                .resistor(&floor_node, "gnd", floor);
            node = wiper;
        } else {
            net.resistor(&out, "gnd", p.interstage);
            node = out;
        }
    }
    net.build(&node)
}
