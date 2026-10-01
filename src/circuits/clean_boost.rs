//! The Clean Boost: one op-amp, one knob, up to 26 dB.
//!
//! Engineering reference (developer documentation, not panel text): MXR M-133
//! MicroAmp, from ElectroSmash's "MXR MicroAmp Analysis" schematic, parts list
//! and arithmetic. See `docs/models/clean_boost.md`.
//!
//! A non-inverting TL061 with 56 k of feedback against a 500 k reverse-log
//! rheostat and 2.7 k to a large capacitor: from unity at the stop to 21.7 at
//! the other end, with "everything between 0 and 50 K" of the rheostat, which
//! is why its track is reverse log. An output divider of 470 ohm and 10 k
//! takes the stage to 20.6, 26.2 dB. Flat across the band; a hot pickup at full
//! gain meets the 9 V rail, and that is the only distortion it has.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper};

/// R5 GAIN, 500 k reverse log, wired as a rheostat. The Drive knob turns it.
pub const GAIN: usize = 0;

/// What a TL061 on 9 V swings either way of 4.5 V. APPROXIMATED, as the TS808.
const RAIL: f64 = 3.0;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Clean Boost");

    // R7 / R8 100 k from 9 V, C4 1 uF.
    net.supply("vref", 50_000.0, 4.5)
        .capacitor("vref", "gnd", 1e-6);

    net.input("in", source)
        .resistor("in", "gnd", 22_000_000.0) // R1
        .capacitor("in", "p", 0.1e-6) // C1
        .resistor("p", "vref", 10_000_000.0) // R2
        .resistor("p", "pp", 1_000.0) // R3
        .resistor("u", "m", 56_000.0) // R4
        .capacitor("u", "m", 47e-12) // C2
        // R5: the part of the track in circuit is between -in and the wiper,
        // and the wiper is tied to the end toward R6. With `b` and the wiper
        // both on that node, what is left is `R (1 - f(p))`, and a C track
        // makes that `R audio(1 - p)`: all of it at the stop, a tenth at half
        // rotation, none at the other end.
        .pot("m", "r5", "r5", 500_000.0, Taper::AntiAudio, GAIN) // R5
        .resistor("r5", "c3a", 2_700.0) // R6
        .capacitor("c3a", "gnd", 4.7e-6) // C3
        .opamp_biased("u", "pp", "m", "vref", RAIL)
        .capacitor("u", "c5b", 15e-6) // C5
        .resistor("c5b", "out", 470.0) // R9
        .resistor("out", "gnd", 10_000.0) // R10
        .resistor("out", "gnd", load);

    net.build(at)
}
