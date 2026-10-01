//! The Brit Drive: two op-amp stages into red LEDs and a three-knob tone stack.
//!
//! Engineering reference (developer documentation, not panel text): Marshall
//! The Guv'nor, the original (1988-1992), from ElectroSmash's "Marshall The
//! Guvnor Analysis" schematic and its own arithmetic. No Marshall drawing was
//! located. See `docs/models/brit_drive.md`.
//!
//! The first TL072 stage is non-inverting with up to 33 dB, a 723 Hz shelf
//! below and a 13 kHz top. The second is inverting with up to 36 dB, and its
//! input resistor is not only R4: **the Gain pot is one track** whose wiper is
//! the first stage's output, so turned up more of the track is the first
//! stage's feedback and less of it is in series with R4, and both stages gain
//! together -- about 70 dB in all, which the tone stack then spends 10 to 20 of.
//! Two red LEDs to ground clip at about 1.6 V (ElectroSmash: 1.8 to 2 V; the
//! catalogue's red LED knees a little lower). The tone stack is a variation of
//! the amplifier stacks, its bass, middle and treble interacting as the drawing
//! wires them, and a passive level with a 15 kHz top ends it.

use crate::dsp::netlist::{Circuit, DiodeSpec, Fault, Netlist, Taper};

/// VR1 GAIN, 100 k, one track doing two jobs. Audio law, PLAUSIBLE. The Drive
/// knob turns it.
pub const GAIN: usize = 0;
/// VR2 BASS, 10 k (linear, PLAUSIBLE).
pub const BASS: usize = 1;
/// VR3 MIDDLE, 10 k (linear, PLAUSIBLE).
pub const MIDDLE: usize = 2;
/// VR4 TREBLE, 10 k (linear, PLAUSIBLE).
pub const TREBLE: usize = 3;
/// VR5 LEVEL, 100 k (audio, PLAUSIBLE).
pub const LEVEL: usize = 4;

/// Where the level control rests: unity through the pedal, measured by
/// `examples/drive_pedals_level.rs`.
pub const LEVEL_REST: f64 = 0.714;

/// What a TL072 on 9 V swings either way of 4.5 V. APPROXIMATED, as the TS808.
const RAIL: f64 = 3.0;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit Drive");

    // R13 / R14 47 k from 9 V, C14 10 uF.
    net.supply("vref", 23_500.0, 4.5)
        .capacitor("vref", "gnd", 10e-6);

    // --- U1A ---------------------------------------------------------------------
    // VR1's track from U1A's -in to its far end, the wiper on U1A's output:
    // `pot(a, wiper, b)` puts `R f(p)` between the wiper and `b` (-in), so
    // turned up that part -- the feedback -- is the whole track, and the part
    // from the wiper to `a`, in series with R4, is nothing.
    net.input("in", source)
        .resistor("in", "gnd", 2_200_000.0) // R1
        .capacitor("in", "p1", 9.6e-9) // C1
        .resistor("p1", "vref", 1_000_000.0) // R2
        .resistor("m1", "c3a", 2_200.0) // R3
        .capacitor("c3a", "gnd", 100e-9) // C3
        .capacitor("m1", "u1a", 120e-12) // C2
        .pot("vr1_far", "u1a", "m1", 100_000.0, Taper::Audio, GAIN) // VR1
        .opamp_biased("u1a", "p1", "m1", "vref", RAIL);

    // --- U2B, inverting -----------------------------------------------------------------
    net.capacitor("vr1_far", "c4b", 220e-9) // C4
        .resistor("c4b", "x", 10_000.0) // R4
        .capacitor("x", "m2", 100e-9) // C5
        .resistor("m2", "u2b", 680_000.0) // R5
        .capacitor("m2", "u2b", 220e-12) // C6
        .opamp_biased("u2b", "vref", "m2", "vref", RAIL);

    // --- the clipper: two red LEDs to ground ---------------------------------------------
    net.capacitor("u2b", "c7b", 220e-9) // C7
        .resistor("c7b", "clip", 1_000.0) // R6
        .diode("clip", "gnd", DiodeSpec::LED) // D1
        .diode("gnd", "clip", DiodeSpec::LED); // D2

    // --- the tone stack, as drawn -------------------------------------------------------
    // Directions: each knob raises the band it is named for (tests/brit_drive.rs).
    net.resistor("clip", "a", 1_500.0) // R7
        .capacitor("clip", "t", 4.7e-9) // C9
        .capacitor("t", "c11b", 10e-9) // C11
        .resistor("c11b", "m_top", 100.0) // R10
        .pot("m_top", "m_w", "gnd", 10_000.0, Taper::Linear, MIDDLE) // VR3
        .capacitor("a", "m_w", 220e-9) // C10
        .pot("t", "t_w", "b", 10_000.0, Taper::Linear, TREBLE) // VR4
        .resistor("a", "b", 680.0) // R9
        .capacitor("b", "gnd", 68e-9) // C12
        // Turned up, the wiper is at ground: C8 and R8's node is grounded and
        // B carries the whole track to ground. The other way round cut the
        // bottom as the knob came up (`examples/drive_pedals_op.rs`).
        .pot("gnd", "b_w", "b", 10_000.0, Taper::Linear, BASS) // VR2
        .capacitor("a", "b_w", 100e-9) // C8
        .resistor("b_w", "gnd", 680.0); // R8

    // --- the output ---------------------------------------------------------------------
    net.rest(LEVEL, LEVEL_REST)
        .pot("t_w", "l_w", "gnd", 100_000.0, Taper::Audio, LEVEL) // VR5
        .resistor("l_w", "out", 22_000.0) // R11
        .capacitor("out", "gnd", 470e-12) // C13
        .resistor("out", "gnd", load);

    net.build(at)
}
