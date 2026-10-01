//! The Gold Drive: an overdrive whose clipped signal is summed with two clean
//! ones.
//!
//! Engineering reference (developer documentation, not panel text): Klon
//! Centaur, from ElectroSmash's "Klon Centaur Analysis" schematic and its own
//! arithmetic. No Klon drawing exists; the board was potted. See
//! `docs/models/gold_drive.md`.
//!
//! A TL072 buffer feeds three paths. The **gain stage** is a non-inverting
//! TL072 whose dual-gang Gain pot does two things with its first gang: the
//! track divides the stage's input down to the 4.5 V bias, and the rest of it
//! is the bottom of the stage's feedback leg, so turned down the stage has no
//! input at all and turned up it has about 40 dB around a kilohertz; its output
//! is clipped by two germanium diodes to ground. Beside it run two **clean
//! feed-forward paths** -- one a 106 Hz low-pass, the other through the pot's
//! second gang, which turns down as the first turns up -- and a TL072 on the
//! charge pump's +16.2 / -8.6 V **sums** all three, inverting, with a 495 Hz
//! top. A shelving **treble** control around the second half of that op-amp,
//! hinged at 408 Hz, and a passive **output** pot follow.
//!
//! The pedal's name for its sound, transparent, is the summing: at low gain the
//! clean paths dominate, and the clipping arrives under them rather than
//! replacing them.

use crate::dsp::netlist::{Circuit, DiodeSpec, Fault, Netlist, Taper};

/// RV-GAIN, 100 k B, both gangs. The Drive knob turns it.
pub const GAIN: usize = 0;
/// RV2 TREBLE, 10 k B.
pub const TREBLE: usize = 1;
/// RV3 LEVEL ("OUTPUT" on the box), 10 k B.
pub const LEVEL: usize = 2;

/// Where the output control rests: unity through the pedal, measured by
/// `examples/drive_pedals_level.rs`.
pub const LEVEL_REST: f64 = 0.116;

/// What the two op-amps on the 9 V rail can swing either way of 4.5 V: a
/// TL072 gets to within about a volt and a half of each rail, as the TS808's
/// 4558 does. APPROXIMATED.
const RAIL_9V: f64 = 3.0;

/// The same on the charge pump's +16.2 / -8.6 V, either way of 4.5 V: the
/// smaller side, 11.7 V less a volt and a half. APPROXIMATED.
const RAIL_PUMP: f64 = 10.2;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Gold Drive");

    // The 4.5 V bias: R29 / R30 27 k each from 9 V, C18 47 uF.
    net.supply("vref", 13_500.0, 4.5)
        .capacitor("vref", "gnd", 47e-6);

    // --- U1A, the input buffer ---------------------------------------------------
    net.input("in", source)
        .resistor("in", "c1a", 10_000.0) // R1
        .capacitor("c1a", "p1", 0.1e-6) // C1
        .resistor("p1", "vref", 1_000_000.0) // R2
        .opamp_biased("buf", "p1", "buf", "vref", RAIL_9V);

    // --- the gain stage, U1B --------------------------------------------------------
    // RV-GAIN_a's track runs from U1B's +in to R10 with its wiper on the bias:
    // `pot(a, wiper, b)` puts `R f(p)` between the wiper and `b`, so turned up
    // the whole track shunts the input and none of it is in the feedback leg.
    net.capacitor("buf", "c3b", 0.1e-6) // C3
        .resistor("c3b", "p5", 10_000.0) // R6
        .capacitor("c3b", "p5", 68e-9) // C5
        .pot("ga_bot", "vref", "p5", 100_000.0, Taper::Linear, GAIN) // RV-GAIN_a
        .resistor("ga_bot", "r10b", 2_000.0) // R10
        .resistor("r10b", "m6", 15_000.0) // R11
        .capacitor("r10b", "m6", 82e-9) // C7
        .resistor("m6", "u1b", 422_000.0) // R12
        .capacitor("m6", "u1b", 390e-12) // C8
        .opamp_biased("u1b", "p5", "m6", "vref", RAIL_9V);

    // --- the clipper ------------------------------------------------------------
    // C9, R13, and two 1N34As back to back to ground (not to the bias).
    net.capacitor("u1b", "c9b", 1e-6) // C9
        .resistor("c9b", "clip", 1_000.0) // R13
        .diode("clip", "gnd", DiodeSpec::GERMANIUM) // D2
        .diode("gnd", "clip", DiodeSpec::GERMANIUM) // D3
        .capacitor("clip", "c10b", 1e-6) // C10
        .resistor("c10b", "sum", 47_000.0) // R16
        .capacitor("c10b", "c11b", 2.2e-9) // C11
        .resistor("c11b", "ff2", 22_000.0); // R15

    // --- feed-forward 1: after C3, a 106 Hz low-pass into the summing node ----------
    net.resistor("c3b", "ff1", 1_500.0) // R7
        .capacitor("ff1", "gnd", 1e-6) // C16
        .resistor("ff1", "sum", 15_000.0); // R19

    // --- feed-forward 2: from the buffer, through RV-GAIN_b --------------------------
    // RV-GAIN_b's track from node b8 to the bias, its wiper the path on: turned
    // up the wiper is at the bias end, so this path turns down as the gain
    // stage turns up.
    net.resistor("buf", "b8", 5_100.0) // R5
        .capacitor("buf", "b8", 68e-9) // C4
        .resistor("b8", "vref", 1_500.0) // R8
        .capacitor("b8", "c6b", 390e-9) // C6
        .resistor("c6b", "vref", 1_000.0) // R9
        .pot("vref", "ff2", "b8", 100_000.0, Taper::Linear, GAIN) // RV-GAIN_b
        .resistor("ff2", "sum", 27_000.0) // R17
        .capacitor("ff2", "c12b", 27e-9) // C12
        .resistor("c12b", "sum", 12_000.0); // R18

    // --- U2A, the summing amplifier, on the charge pump's rails ----------------------
    net.resistor("sum", "u2a", 392_000.0) // R20
        .capacitor("sum", "u2a", 820e-12) // C13
        .opamp_biased("u2a", "vref", "sum", "vref", RAIL_PUMP);

    // --- U2B, the treble shelf ---------------------------------------------------------
    // R22 in and R24 back, unity; R21 from the input to one end of RV2 and R23
    // from its other end to the output, the wiper through C14 onto -in. Turned
    // up the wiper is at R21's end: (RV2 + R23) / R21, 18 dB of boost.
    net.resistor("u2a", "m2", 100_000.0) // R22
        .resistor("m2", "u2b", 100_000.0) // R24
        .resistor("u2a", "t_bot", 1_800.0) // R21
        .pot("t_bot", "t_w", "t_top", 10_000.0, Taper::Linear, TREBLE) // RV2
        .resistor("t_top", "u2b", 4_700.0) // R23
        .capacitor("t_w", "m2", 3.9e-9) // C14
        .opamp_biased("u2b", "vref", "m2", "vref", RAIL_PUMP);

    // --- the output ----------------------------------------------------------------
    net.capacitor("u2b", "c15b", 4.7e-6) // C15
        .resistor("c15b", "lv_top", 560.0) // R25
        .rest(LEVEL, LEVEL_REST)
        .pot("lv_top", "out", "gnd", 10_000.0, Taper::Linear, LEVEL) // RV3
        .resistor("out", "gnd", 100_000.0) // R28, at the jack
        .resistor("out", "gnd", load);

    net.build(at)
}
