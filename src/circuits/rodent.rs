//! The Rodent: a hard-clipping distortion built round a slow op-amp.
//!
//! Engineering reference (developer documentation, not panel text): Pro Co RAT,
//! LM308 version, per ElectroSmash's "Pro Co Rat Analysis" schematic and parts list;
//! LM308 dynamics from National Semiconductor's LM308 data sheet curves (open loop
//! frequency response and voltage-follower pulse response, reproduced in the same
//! analysis). See `docs/models/rodent.md`.
//!
//! A non-inverting op-amp stage with up to 67 dB of gain, diodes to ground after
//! it, a one-knob low-pass, and a JFET follower. What makes it the pedal it is,
//! rather than a generic clipper, is the op-amp: an LM308 with a 30 pF
//! compensation capacitor has about a megahertz of gain-bandwidth and slews at
//! 0.3 V/us, so at full Distortion it cannot deliver its gain above a few hundred
//! hertz and cannot follow a fast edge at all. An ideal op-amp here would be a
//! different, fizzier pedal.
//!
//! ## The op-amp
//!
//! `lm308` builds it from parts the solver has: a saturating transconductance
//! into an integrating capacitor (gain-bandwidth `gm / 2 pi C`, slew `limit / C`),
//! a resistance across the capacitor for the finite open-loop gain, diode clamps
//! at the output swing of an LM308 on a 9 V battery, and an ideal follower. The internal capacitor is a scaling
//! choice, not the pedal's C3: gain-bandwidth and slew are what C3 sets, and they
//! are what is modelled.

use crate::dsp::netlist::{Circuit, DiodeSpec, Fault, JfetSpec, Netlist, Taper};

/// R DISTORTION, 100 k audio, in the op-amp's feedback.
pub const DISTORTION: usize = 0;
/// R TONE (the FILTER knob), 100 k audio. Its position is the pedal's rotation:
/// turned up, more resistance and a darker sound.
pub const FILTER: usize = 1;
/// R VOLUME, 100 k audio.
pub const VOLUME: usize = 2;

/// Where Volume rests, like the other pedals' level controls.
pub const VOLUME_REST: f64 = 0.7;

/// LM308 gain-bandwidth with Cf = 30 pF: the data sheet curve gives 67 dB at 500 Hz.
pub const GBW_HZ: f64 = 1.1e6;
/// LM308 slew rate with Cf = 30 pF, volts per second (0.3 V/us).
pub const SLEW: f64 = 0.3e6;
/// LM308 open-loop gain at low frequency: about 108 dB on the same curve.
pub const OPEN_LOOP: f64 = 2.5e5;
/// What the output swings either way from the 4.5 V bias point on a 9 V battery.
pub const SWING: f64 = 3.5;

/// 1N914, as the catalogue's silicon switching diode.
const D1N914: DiodeSpec = DiodeSpec {
    saturation: 4.352e-9,
    emission: 1.906,
};

/// 2N5458. ESTIMATED inside the data sheet's ranges (IDSS 2-9 mA, VGS(off) -1 to
/// -7 V): a source follower's output hardly depends on either.
const J2N5458: JfetSpec = JfetSpec {
    idss: 4.0e-3,
    pinch_off: -2.5,
};

/// The internal integrating capacitor of the op-amp model. See the module note.
const INTEGRATOR: f64 = 1e-9;

/// The bias point the pedal runs at: half the battery.
pub const BIAS: f64 = 4.5;

/// An LM308 between `plus`, `minus` and `out` on the pedal's 9 V battery. The
/// internal nodes are named with `prefix`.
///
/// Every part of it is referenced to ground, not to the 4.5 V bias point. A
/// chip's internal currents come from its supply pins; built against the bias
/// divider (100 k || 100 k and 1 uF), the integrator's slewing current and the
/// clamps pulled that node about by a few hundred millivolts at every clipped
/// edge, and R2 put it straight back on the input.
///
/// The output swing is two diodes on the integrator node against fixed levels,
/// so the stage rounds into its limits rather than switching state there.
pub fn lm308(net: &mut Netlist, prefix: &str, plus: &str, minus: &str, out: &str) {
    let stage = format!("{prefix}_int");
    let high = format!("{prefix}_high");
    let low = format!("{prefix}_low");
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    let limit = SLEW * INTEGRATOR;
    // A 1N914 carrying the whole slewing current drops about 0.55 V, which puts
    // the output limits at the bias point plus and minus `SWING`.
    let drop = 0.55;
    net.transconductor(plus, minus, &stage, "gnd", gm, limit)
        .capacitor(&stage, "gnd", INTEGRATOR)
        .resistor(&stage, "gnd", OPEN_LOOP / gm)
        .supply(&high, 1.0, BIAS + SWING - drop)
        .supply(&low, 1.0, BIAS - SWING + drop)
        .diode(&stage, &high, D1N914)
        .diode(&low, &stage, D1N914)
        // The output stage: a follower with no swing limit of its own.
        .opamp(out, &stage, out, 1_000.0);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Rodent");

    // --- power supply --------------------------------------------------------
    // R10 47R from the battery, C11 100 uF and C12 .1 uF; R11/R12 100 k make the
    // 4.5 V bias point, C13 1 uF across it.
    net.supply("v9", 47.0, 9.0) // R10
        .capacitor("v9", "gnd", 100e-6) // C11
        .capacitor("v9", "gnd", 0.1e-6) // C12
        .resistor("v9", "vref", 100_000.0) // R11
        .resistor("vref", "gnd", 100_000.0) // R12
        .capacitor("vref", "gnd", 1e-6); // C13

    // --- input ---------------------------------------------------------------
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R1
        .capacitor("in", "c1", 22e-9) // C1
        .resistor("c1", "vref", 1_000_000.0) // R2
        .resistor("c1", "plus", 1_000.0) // R3
        .capacitor("plus", "gnd", 1e-9); // C2

    // --- the clipping amplifier ---------------------------------------------------
    // Gain 1 + R DISTORTION / (R4 || R5), from 0 dB to 67 dB, with R4/C5 and R5/C6
    // high-passing the gain leg at 1.5 kHz and 60 Hz and C4 100 pF across the
    // feedback.
    lm308(&mut net, "u1", "plus", "minus", "amp");
    net.pot(
        "amp",
        "minus",
        "minus",
        100_000.0,
        Taper::ReverseAudio,
        DISTORTION,
    ) // R DISTORTION, turned up puts resistance in
    .capacitor("amp", "minus", 100e-12) // C4
    .resistor("minus", "r4", 47.0) // R4
    .capacitor("r4", "gnd", 2.2e-6) // C5
    .resistor("minus", "r5", 560.0) // R5
    .capacitor("r5", "gnd", 4.7e-6); // C6

    // --- the clipper ------------------------------------------------------------------
    net.capacitor("amp", "c7", 4.7e-6) // C7
        .resistor("c7", "clip", 1_000.0) // R6
        .diode("clip", "gnd", D1N914) // D1
        .diode("gnd", "clip", D1N914); // D2

    // --- the filter ------------------------------------------------------------------
    // R TONE as a rheostat and R7 1.5 k into C8 3.3 nF: 32 kHz with the knob down,
    // 475 Hz with it up.
    net.pot("clip", "r7", "r7", 100_000.0, Taper::ReverseAudio, FILTER) // R TONE
        .resistor("r7", "tone", 1_500.0) // R7
        .capacitor("tone", "gnd", 3.3e-9); // C8

    // --- the output stage ---------------------------------------------------------------
    net.capacitor("tone", "gate", 22e-9) // C9
        .resistor("gate", "gnd", 1_000_000.0) // R8
        .jfet("v9", "gate", "source", J2N5458) // Q1
        .resistor("source", "gnd", 10_000.0) // R9
        .capacitor("source", "vol", 1e-6) // C10
        .rest(VOLUME, VOLUME_REST)
        .pot("vol", "out", "gnd", 100_000.0, Taper::Audio, VOLUME) // R VOLUME
        .resistor("out", "gnd", load);

    net.build(at)
}
