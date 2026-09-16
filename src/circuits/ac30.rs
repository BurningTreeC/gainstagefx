//! The Brit AC30 preamplifier: the brilliant channel of a British 30 W combo
//! through its Top Boost valve and stack.
//!
//! Engineering reference (developer documentation, not panel text): Vox AC30/6
//! Top Boost, as drawn by Dallas Music Industries, "Vox AC30 (Treble & Bass)",
//! Sc/V/1313, 24 June 1974, cross-checked against Vox Sound Limited's AC30/6 Top
//! Boost sheet of 12 February 1971. See `docs/models/brit_ac30.md`.
//!
//! Three triodes before the phase inverter and no master volume anywhere. The
//! first is shared between the amplifier's two channels and is barely a gain
//! stage at all -- 220 k plate, 1.5 k cathode, fully bypassed -- and the volume
//! that follows it has a 100 pF capacitor round it, so turning down keeps the
//! top end. The Top Boost valve is the amplifier's voice: a gain stage on an
//! unbypassed 1.5 k, direct-coupled into a cathode follower that drives the
//! stack from a low impedance.
//!
//! The stack is a Fender stack with its Middle control replaced by a 10 k
//! resistor, which is why an AC30 has no middle knob and why what it does have
//! interacts the way it does. Its output goes through 220 k into the phase
//! inverter, where the other channel's 220 k meets it -- so an unused channel
//! still loads this one, and that loading is part of the sound.
//!
//! The panel's Middle knob is greyed out for this amplifier, because the
//! amplifier does not have one.
//!
//! Its matched power stage is `power::PowerSpec::AC30_EL84`: four EL84s on a
//! shared cathode resistor with no feedback loop at all.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// The brilliant channel's VOLUME, 500 k log.
pub const VOLUME: usize = 0;
/// TREBLE, 1 M log.
pub const TREBLE: usize = 1;
/// BASS, 1 M log, wired as a variable resistor.
pub const BASS: usize = 2;

/// The high-tension node the droppers hang from, in volts. The drawings carry no
/// voltages; this is where the power stage's own supply settles
/// (`PowerSpec::AC30_EL84`, whose transformer and rectifier valve are set so the
/// output valves idle at the factory sheet's 10 V of cathode bias). Checked in
/// `tests/ac30.rs`.
pub const HT: f64 = 331.0;

/// R11 22 k feeds the phase inverter from the same high-tension node.
/// The inverter is solved in the power stage's netlist, so its supply is stated
/// here: HT less the drop its own idle current makes across R11. Re-measured by
/// `tests/ac30.rs`.
pub const INVERTER_NODE: f64 = 305.0;

/// The normal channel's triode, which is not built. It shares V1's cathode
/// resistor, so its current sets the bias of the stage that *is* built: this is
/// that current as a resistance from the shared cathode's supply side.
/// APPROXIMATED; see `examples/ac30_op.rs`.
pub const NORMAL_TRIODE_IDLE: f64 = 260_000.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the brilliant channel's input to the treble wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node. Reference designators are
/// the 1974 drawing's.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit AC30 preamp");

    // --- supplies ------------------------------------------------------------
    // Two droppers leave the high-tension node: R10 22 k feeds the preamplifier
    // (C4 8 uF), and R11 22 k feeds the phase inverter, which lives in the power
    // stage's netlist -- see `INVERTER_NODE`. The Top Boost valve sits behind a
    // further R76 10 k with C42 32 uF.
    net.supply("n4", 22_000.0, HT) // R10
        .capacitor("n4", "gnd", 8e-6) // C4
        .resistor("n4", "n42", 10_000.0) // R76
        .capacitor("n42", "gnd", 32e-6); // C42

    // --- V1, the shared input stage -------------------------------------------
    // R1 68 k at the jack, R3 1 M across it. R4 1.5 k with C1 2.5 uF is shared
    // with the normal channel's triode, which is not built: `NORMAL_TRIODE_IDLE`
    // is its idle current, because it sets this stage's bias.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R3
        .resistor("in", "v1_g", 68_000.0) // R1
        .resistor("v1_k", "gnd", 1_500.0) // R4
        .capacitor("v1_k", "gnd", 2.5e-6) // C1
        .resistor("n4", "v1_k", NORMAL_TRIODE_IDLE)
        .resistor("n4", "v1_p", 220_000.0) // R5
        .triode("v1_p", "v1_g", "v1_k", ECC83);

    // --- the volume, with its bright capacitor --------------------------------
    // C2 .047 uF into the top of a 500 k log track, C41 100 pF from the top to
    // the wiper.
    net.capacitor("v1_p", "vol_top", 0.047e-6) // C2
        .pot("vol_top", "vol", "gnd", 500_000.0, Taper::Audio, VOLUME)
        .capacitor("vol_top", "vol", 100e-12); // C41

    // --- the Top Boost valve ---------------------------------------------------
    // R75 100 k plate, R77 1.5 k cathode with nothing across it, then a cathode
    // follower direct-coupled from that plate with R78 56 k to ground.
    net.resistor("tb_k", "gnd", 1_500.0) // R77
        .resistor("n42", "tb_p", 100_000.0) // R75
        .triode("tb_p", "vol", "tb_k", ECC83)
        .resistor("cf", "gnd", 56_000.0) // R78
        .triode("n42", "tb_p", "cf", ECC83);

    // --- the tone stack ---------------------------------------------------------
    // C43 50 pF to the top of Treble, R79 100 k to the slope node, C44 .022 uF
    // from there to the bottom of Treble and the top of Bass, C45 .022 uF to the
    // node the bass wiper returns to, and R80 10 k from that node to ground
    // where a Fender stack has its Middle control.
    net.capacitor("cf", "t_top", 50e-12) // C43
        .resistor("cf", "slope", 100_000.0) // R79
        .pot("t_top", "tw", "t_bot", 1_000_000.0, Taper::Audio, TREBLE)
        .capacitor("slope", "t_bot", 0.022e-6) // C44
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        )
        .capacitor("slope", "b_bot", 0.022e-6) // C45
        .resistor("b_bot", "gnd", 10_000.0) // R80
        // R9 220 k into the inverter's grid, where the normal channel's R7 220 k
        // arrives as well. Its volume is at zero, so that one is 220 k to ground:
        // six decibels of the treble wiper's signal, thrown away by a channel
        // nobody is using. That is what an AC30 does.
        .resistor("tw", "out", 220_000.0) // R9
        .resistor("out", "gnd", 220_000.0) // R7, the normal channel at zero
        .resistor("out", "gnd", load);

    net.build(at)
}
