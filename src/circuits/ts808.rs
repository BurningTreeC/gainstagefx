//! The Ibanez TS808 Tube Screamer, from the schematic.
//!
//! Not a topology that resembles one: the parts and values off the drawing,
//! and the whole signal path rather than the interesting part of it. The
//! previous overdrive here was an op-amp with diodes in its feedback loop and
//! a capacitor in the gain leg, which is the *shape* of this pedal and sounds
//! like nothing in particular. What was missing turns out to matter:
//!
//! - the input buffer, an emitter follower whose 1 k series resistor and
//!   coupling capacitor set what the clipping stage is even offered;
//! - the tone and level section after the clipper, which is most of what
//!   makes a Screamer sound like one;
//! - the output buffer, and the 100 ohm and 10 uF that follow it.
//!
//! The circuit runs from one 9 V battery with everything biased at half of
//! it, so `vref` is the signal ground of the audio path and the op-amp clips
//! about that rather than about ground.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// RV1, the 500 k log pot marked Overdrive.
pub const DRIVE: usize = 0;
/// RV2, the 20 k pot marked Tone.
pub const TONE: usize = 1;
/// RV3, the 100 k log pot marked Level.
pub const LEVEL: usize = 2;

/// The BC549 in both buffers.
const BC549: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 500.0,
    reverse_beta: 4.0,
    early: 100.0,
};

/// D1 and D2, 1N4148.
const D1N4148: DiodeSpec = DiodeSpec { saturation: 4.352e-9, emission: 1.906 };

/// The NE5532 runs on the 9 V rail and swings to within about a volt and a
/// half of each end of it, which from the 4.5 V bias point is this.
const RAIL: f64 = 3.0;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same pedal, brought out at a chosen node. For measuring one stage at a
/// time, which is the only way to find out which of them is wrong.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("TS808");

    // The supply, and the half-supply bias every stage sits on. R16 and R17
    // are 10 k each from 9 V, so the bias is 4.5 V behind 5 k, and C11 47 uF
    // is what makes it a signal ground rather than a shared impedance.
    net.supply("vref", 5_000.0, 4.5).capacitor("vref", "gnd", 47e-6);

    // --- input buffer, Q1 -------------------------------------------------
    net.input("in", source)
        .capacitor("in", "r1", 22e-9) // C1
        .resistor("r1", "q1base", 1_000.0) // R1
        .resistor("q1base", "vref", 470_000.0) // R2
        .supply("v9", 100.0, 9.0)
        .bipolar("v9", "q1base", "buf", BC549) // Q1 BC549, collector to 9 V
        .resistor("buf", "gnd", 10_000.0); // R3

    // --- clipping amplifier, U1A -----------------------------------------
    net.capacitor("buf", "plus", 1e-6) // C2
        .resistor("plus", "vref", 10_000.0) // R5
        .opamp_biased("u1a", "plus", "minus", "vref", RAIL)
        // R4 and C3 from the inverting input to ground set how much gain the
        // stage has and, with the capacitor, from what frequency upwards.
        .resistor("minus", "c3", 4_700.0) // R4
        .capacitor("c3", "gnd", 47e-9) // C3
        // R6 in series with the Overdrive pot is the feedback, with the two
        // diodes across the pair of them and C4 across as well.
        .resistor("minus", "rv1", 51_000.0) // R6
        .pot("rv1", "u1a", "u1a", 500_000.0, Taper::ReverseAudio, DRIVE)
        .diode("u1a", "minus", D1N4148) // D1
        .diode("minus", "u1a", D1N4148) // D2
        .capacitor("minus", "u1a", 47e-12); // C4

    // --- tone and level, U1B ---------------------------------------------
    // R7 into C5 to ground is the first of the two poles; the Tone pot then
    // decides how much of what is left goes through C6 and R8 to ground,
    // against how much goes to the inverting input of U1B.
    net.resistor("u1a", "tone_in", 1_000.0) // R7
        .capacitor("tone_in", "gnd", 220e-9) // C5
        .resistor("tone_in", "vref", 10_000.0) // R10
        .pot("tone_in", "tone_w", "tone_end", 20_000.0, Taper::Linear, TONE)
        .capacitor("tone_w", "r8", 220e-9) // C6
        .resistor("r8", "gnd", 220.0) // R8
        .resistor("tone_end", "minus_b", 0.1)
        .opamp_biased("u1b", "tone_in", "minus_b", "vref", RAIL)
        .resistor("minus_b", "u1b", 1_000.0); // R9

    // --- output buffer, Q2 ------------------------------------------------
    net.capacitor("u1b", "r11", 1e-6) // C7
        .resistor("r11", "lvl_top", 1_000.0) // R11
        .pot("lvl_top", "lvl", "gnd", 100_000.0, Taper::Audio, LEVEL)
        .capacitor("lvl", "q2base", 100e-9) // C8
        .resistor("q2base", "vref", 470_000.0) // R12
        .bipolar("v9", "q2base", "q2e", BC549) // Q2 BC549
        .resistor("q2e", "gnd", 10_000.0) // R13
        .resistor("q2e", "r14", 100.0) // R14
        .capacitor("r14", "out", 10e-6) // C9
        .resistor("out", "gnd", 10_000.0) // R15
        .resistor("out", "gnd", load);

    net.build(at)
}
