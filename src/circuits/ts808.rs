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

/// The span RV1 has to cover against R6, which is what sets its law.
const DRIVE_SPAN: f64 = (51_000.0 + 500_000.0) / 51_000.0;

/// D1 and D2, the 1N914/1N4148 silicon switching pair the drawing calls for.
///
/// A saturation current of a few nanoamps and an emission coefficient near two
/// is what puts the knee where a silicon switching diode's is. This was once
/// changed to a Schottky-like part -- a hundred nanoamps and an emission of
/// 1.5 -- to make the pedal saturate sooner. It did, and it also pulled the
/// clipping stage's small-signal gain twelve decibels below the arithmetic its
/// own resistors say, because diodes that soft conduct before the stage has
/// finished amplifying. See `docs/experiments/ts808-diode-choice.md`.
const D1N4148: DiodeSpec = DiodeSpec {
    saturation: 4.352e-9,
    emission: 1.906,
};

/// What U1 can swing either way from the 4.5 V bias point.
///
/// The part is a JRC4558D, not the NE5532 this comment used to name -- the
/// wrong part, though as it happens not a wrong number: a 4558 on a 9 V
/// single supply swings to within about a volt and a half of each rail, which
/// from half of nine is three either way.
///
/// It matters less here than it looks. The clipping diodes are inside the
/// feedback loop and conduct at six tenths of a volt, so they have taken the
/// stage over long before the output is anywhere near three volts from bias.
/// The rail is what the stage meets on a transient with the Drive shut, not
/// what shapes its everyday sound. See `TS808.md` sections 8 and 9.
const RAIL: f64 = 3.0;

/// The values that differ between the revisions this netlist is built as.
///
/// See `docs/models/green_808.md` and `docs/models/green_9.md` for the sources.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Values {
    /// C1, into the input buffer.
    pub input_cap: f64,
    /// The resistor under C6 in the tone control's shunt leg.
    pub tone_shunt: f64,
    /// From U1B's coupling capacitor to the Level pot.
    pub level_feed: f64,
    /// The output buffer's series resistor and the shunt after its capacitor.
    pub out_series: f64,
    pub out_shunt: f64,
}

/// The values this netlist carried until 2026-09-15. They depart from all three
/// drawings consulted in the tone shunt (1 k instead of 220 ohm), the level feed
/// (220 ohm instead of 1 k) and C1 (22 nF instead of 20 nF). Kept only so the
/// correction stays measurable; nothing builds with them. See green_808.md.
pub const LEGACY: Values = Values {
    input_cap: 22e-9,
    tone_shunt: 1_000.0,
    level_feed: 220.0,
    out_series: 100.0,
    out_shunt: 10_000.0,
};

/// The TS-808 as the three consulted sources agree it: S. Cerutti's traced
/// drawing, ElectroSmash's analysis drawing, and R. G. Keen's "The Technology of
/// the Tube Screamer" (220 ohm shunt with 0.22 uF, turnover near 3.2 kHz). What the
/// catalogue voice and the pedal slot both build.
pub const TS808: Values = Values {
    input_cap: 20e-9,
    tone_shunt: 220.0,
    level_feed: 1_000.0,
    out_series: 100.0,
    out_shunt: 10_000.0,
};

/// The TS9: the same circuit with the output buffer's 470 ohm series and 100 k
/// shunt resistors (R. G. Keen's model table, confirmed by builder comparisons).
/// The op-amp type also varies across TS9 production; the op-amp here is ideal
/// with a rail, so that difference is not represented.
pub const TS9: Values = Values {
    out_series: 470.0,
    out_shunt: 100_000.0,
    ..TS808
};

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// A chosen revision. `build` is `TS808`.
pub fn build_with(values: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    assemble(values, source, load, "out")
}

/// The same pedal, brought out at a chosen node. For measuring one stage at a
/// time, which is the only way to find out which of them is wrong.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    assemble(&TS808, source, load, at)
}

fn assemble(v: &Values, source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("TS808");

    // Where the player leaves the Level control.
    //
    // The plugin reaches a circuit's Drive and its tone control and nothing
    // else, so any other front-panel knob sits wherever `Simulation::new`
    // leaves it, which is the middle of its travel. Measured at a guitar
    // level with the Drive at 0.85, the middle of this hundred-kilohm audio
    // track is **9.6 dB down**, and the calibration table was handing back
    // twelve decibels of clean digital gain to make up for a knob nobody had
    // turned. This is unity instead: the pedal slot's Level knob at noon is
    // unity through whichever pedal is in it, and a Screamer is a boost that
    // is normally run above that. See `Netlist::rest`, `rodent::VOLUME_REST`
    // and `examples/screamer.rs`.
    net.rest(LEVEL, 0.626);

    // The supply, and the half-supply bias every stage sits on. R16 and R17
    // are 10 k each from 9 V, so the bias is 4.5 V behind 5 k, and C11 47 uF
    // is what makes it a signal ground rather than a shared impedance.
    net.supply("vref", 5_000.0, 4.5)
        .capacitor("vref", "gnd", 47e-6);

    // --- input buffer, Q1 -------------------------------------------------
    net.input("in", source)
        .capacitor("in", "r1", v.input_cap) // C1
        .resistor("r1", "q1base", 1_000.0) // R1
        .resistor("q1base", "vref", 510_000.0) // R2
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
        // RV1, wiper tied to the output, so what is in circuit is the track
        // from R6 up to the wiper.
        //
        // The law is the geometric one over this rheostat's own span, not a
        // volume taper. Both are called "log" and they are not the same
        // curve: a volume track is specified by what fraction is left at half
        // rotation -- a tenth -- and is two straight segments meeting there,
        // which is right for a divider feeding an ear and wrong for a
        // rheostat setting a gain. Measured with the volume law, half the
        // Drive control dialled in a *tenth* of the track and moved the
        // stage 5.4 dB, the other half moved it 13.7 dB, and the two segments
        // met in a step: 8.6 % of the track at the middle of the knob and
        // 25.0 % a tenth of a turn later.
        //
        // The span is what the track has to cover against R6, `(R6 + RV1) /
        // R6` = 10.8, and `Taper::Log`'s own note derives why that is the law
        // that makes decibels even in rotation. Reversed because the wiper is
        // tied to the `b` end, so what is left in circuit is `1 - fraction`.
        .pot(
            "rv1",
            "u1a",
            "u1a",
            500_000.0,
            Taper::ReverseLog { span: DRIVE_SPAN },
            DRIVE,
        )
        .diode("u1a", "minus", D1N4148) // D1
        .diode("minus", "u1a", D1N4148) // D2
        .capacitor("minus", "u1a", 51e-12); // C4

    // --- tone and level, U1B ---------------------------------------------
    // The 1 k into C5 to ground is the first of the two poles; the Tone pot
    // then decides how much of what is left goes through C6 and R8 to ground,
    // against how much goes to the inverting input of U1B.
    //
    // R8 (the shunt under C6) and R11 (the feed to the Level pot) are 220 ohm
    // and 1 k. They were once swapped to 1 k and 220 ohm on the strength of a
    // reference file this repository never had; three independent sources --
    // S. Cerutti's traced TS-808 drawing, ElectroSmash's drawing and R. G.
    // Keen's analysis, which derives the tone turnover near 3.2 kHz from exactly
    // 220 ohm and 0.22 uF -- all give 220 ohm and 1 k, and C6 = 0.22 uF. The
    // "R9 = 10 k" once recorded as unresolved is U1B's bias resistor (R10 here),
    // already 10 k; U1B's feedback resistor is 1 k. See docs/models/green_808.md.
    net.resistor("u1a", "tone_in", 1_000.0) // R7
        .capacitor("tone_in", "gnd", 220e-9) // C5
        .resistor("tone_in", "vref", 10_000.0) // R10
        // The far end of the tone pot *is* the inverting input -- they are the
        // same node on the drawing. Joining them with a small resistor instead
        // buys an extra unknown in the matrix for nothing, and the matrix is
        // solved a hundred thousand times a second.
        //
        // The inverting input is the `a` end, which is where the wiper sits
        // with the knob fully clockwise. That is the bright end: the shorter
        // the path from the wiper to pin 6, the more of the current C6 and R8
        // draw has to come back through R9, and it is that current through R9
        // that lifts the top. Wired the other way round the Tone knob darkens
        // as it is turned up, which is what it used to do here.
        .pot(
            "minus_b",
            "tone_w",
            "tone_in",
            20_000.0,
            Taper::Linear,
            TONE,
        )
        .capacitor("tone_w", "r8", 220e-9) // C6
        .resistor("r8", "gnd", v.tone_shunt) // R8
        .opamp_biased("u1b", "tone_in", "minus_b", "vref", RAIL)
        .resistor("minus_b", "u1b", 1_000.0); // R9

    // --- output buffer, Q2 ------------------------------------------------
    net.capacitor("u1b", "r11", 1e-6) // C7
        .resistor("r11", "lvl_top", v.level_feed) // R11
        .pot("lvl_top", "lvl", "gnd", 100_000.0, Taper::Audio, LEVEL)
        .capacitor("lvl", "q2base", 100e-9) // C8
        .resistor("q2base", "vref", 510_000.0) // R12
        .bipolar("v9", "q2base", "q2e", BC549) // Q2 BC549
        .resistor("q2e", "gnd", 10_000.0) // R13
        .resistor("q2e", "r14", v.out_series) // R14
        .capacitor("r14", "out", 10e-6) // C9
        .resistor("out", "gnd", v.out_shunt) // R15
        .resistor("out", "gnd", load);

    net.build(at)
}
