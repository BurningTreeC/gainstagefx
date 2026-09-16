//! The Yellow Dist: one op-amp, two germanium diodes and two knobs.
//!
//! Engineering reference (developer documentation, not panel text): MXR
//! Distortion+, the 741 version, per ElectroSmash's "MXR Distortion+ Circuit
//! Analysis". See `docs/models/yellow_dist.md`.
//!
//! The simplest of the distortion pedals and the one the others are measured
//! against: a non-inverting stage with up to forty-six decibels of gain, and a
//! pair of germanium diodes to ground after it. Germanium conducts at about a
//! third of what silicon needs, so the ceiling is low and the pedal is fuzzy
//! rather than sharp -- and because the diodes come *after* the amplifier rather
//! than inside its loop, what reaches them is already clipped by the op-amp
//! itself on the way.
//!
//! The 741 matters here for the same reason the LM308 matters in the Rodent: it
//! is slow. At full Distortion it cannot deliver its gain much above a kilohertz
//! and it cannot follow a fast edge, so the pedal rounds what a faster chip
//! would square off.

use crate::dsp::netlist::{Circuit, DiodeSpec, Fault, Netlist, Taper};

/// RV1, the Distortion control, 1 M. Turned up it takes resistance out of the
/// gain leg, which is how a pedal with one op-amp gets its range.
pub const DISTORTION: usize = 0;
/// RV2, the Output control, 10 k.
pub const VOLUME: usize = 1;

/// Where the level control rests: **unity through the pedal**, which is what
/// the panel's Level knob means at noon. See `rodent::VOLUME_REST`.
pub const VOLUME_REST: f64 = 0.631;

/// The 741's gain-bandwidth, from its data sheet: 1 MHz.
pub const GBW_HZ: f64 = 1.0e6;
/// And its slew rate, 0.5 V/us.
pub const SLEW: f64 = 0.5e6;
/// Open-loop gain at low frequency: about 106 dB.
pub const OPEN_LOOP: f64 = 2.0e5;
/// What the output swings either way from the 4.5 V bias point on a battery.
pub const SWING: f64 = 3.5;
/// The bias point itself.
pub const BIAS: f64 = 4.5;

/// The internal integrating capacitor of the op-amp model. A scaling choice:
/// what is modelled is the gain-bandwidth and the slew rate. See
/// `circuits::rodent::lm308`, which is the same construction with an LM308's
/// numbers.
const INTEGRATOR: f64 = 1e-9;

/// The 1N270 germanium pair.
///
/// ESTIMATED to the analysis's own figure -- it says the diodes "clip any signal
/// bigger than 350mV" -- because no SPICE model for this part was located. The
/// catalogue's generic germanium is softer than that (it clips around 180 mV
/// here), and which of the two a pedal has is most of what it sounds like, so
/// this one carries its own.
const T1N270: DiodeSpec = DiodeSpec {
    saturation: 3.3e-9,
    emission: 1.2,
};

/// A 741 between `plus`, `minus` and `out`, on the pedal's battery.
pub fn op741(net: &mut Netlist, prefix: &str, plus: &str, minus: &str, out: &str) {
    let stage = format!("{prefix}_int");
    let high = format!("{prefix}_high");
    let low = format!("{prefix}_low");
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    let limit = SLEW * INTEGRATOR;
    let drop = 0.55;
    net.transconductor(plus, minus, &stage, "gnd", gm, limit)
        .capacitor(&stage, "gnd", INTEGRATOR)
        .resistor(&stage, "gnd", OPEN_LOOP / gm)
        .supply(&high, 1.0, BIAS + SWING - drop)
        .supply(&low, 1.0, BIAS - SWING + drop)
        .diode(&stage, &high, DiodeSpec::SILICON)
        .diode(&low, &stage, DiodeSpec::SILICON)
        .opamp(out, &stage, out, 1_000.0);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Yellow Dist");

    // --- supply and bias ---------------------------------------------------------
    // R6 and R7 1 M each with C6 1 uF: half the battery, at half a megohm.
    net.supply("v9", 47.0, 9.0)
        .capacitor("v9", "gnd", 100e-6)
        .resistor("v9", "vref", 1_000_000.0) // R6
        .resistor("vref", "gnd", 1_000_000.0) // R7
        .capacitor("vref", "gnd", 1e-6); // C6

    // --- input --------------------------------------------------------------------
    // R1 10 k with C1 1 nF across the input, C2 10 nF into the grid of the
    // amplifier, R2 1 M holding it at the bias point.
    net.input("in", source)
        .resistor("in", "c1", 10_000.0) // R1
        .capacitor("c1", "gnd", 1e-9) // C1
        .capacitor("c1", "plus", 10e-9) // C2
        .resistor("plus", "vref", 1_000_000.0); // R2

    // --- the amplifier ---------------------------------------------------------------
    // R4 1 M of feedback into R3 4k7 and the Distortion track, then C3 47 nF to
    // ground: 3.5 dB with the control down and 46.5 dB with it up.
    op741(&mut net, "u1", "plus", "minus", "amp");
    net.resistor("amp", "minus", 1_000_000.0) // R4
        // The track is in the *gain leg*, not in the feedback, so turning the
        // control up has to take resistance out: the pot leaves `1 - f` of its
        // megohm between the amplifier's input and R3. (The Rodent's is in its
        // feedback and so runs the other way round for the same knob.)
        //
        // The law is ESTIMATED: the analysis gives the value and not the taper.
        // Gain here goes as one over that resistance, so a linear track would do
        // nothing for three quarters of its travel and everything in the last
        // few millimetres. A span of a hundredth makes the resistance fall
        // geometrically, which puts the gain at about 12, 21 and 32 dB at the
        // quarters of a range that runs from 3.5 to 46.5 -- an even sweep, which
        // is what the pot in the pedal is for.
        .pot("minus", "r3", "r3", 1_000_000.0, Taper::Log { span: 0.01 }, DISTORTION) // RV1
        .resistor("r3", "c3", 4_700.0) // R3
        .capacitor("c3", "gnd", 47e-9); // C3

    // --- the clipper and the output -----------------------------------------------------
    // C4 1 uF off the bias point, R5 10 k, then the diodes straight to ground
    // with C5 1 nF across them, and RV2 10 k.
    net.capacitor("amp", "c4", 1e-6) // C4
        .resistor("c4", "clip", 10_000.0) // R5
        .diode("clip", "gnd", T1N270) // D1
        .diode("gnd", "clip", T1N270) // D2
        .capacitor("clip", "gnd", 1e-9) // C5
        .rest(VOLUME, VOLUME_REST)
        .pot("clip", "out", "gnd", 10_000.0, Taper::Audio, VOLUME) // RV2
        .resistor("out", "gnd", load);

    net.build(at)
}
