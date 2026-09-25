//! The Brit DR103 preamplifier: the brilliant channel of a British 100 W head
//! built for headroom rather than for breakup.
//!
//! Engineering reference (developer documentation, not panel text): Hiwatt
//! Custom 100 DR103, factory drawing "HIWATT DR103 PREAMPLIFIER CIRCUIT DIAGRAM",
//! Issue 4, 19 May 1995, S/No. AB0309 onwards, with the output stage (Issue 1,
//! 25 April 1994) and power supply (Issue 3, 22 November 1994) sheets. See
//! `docs/models/brit_dr103.md`.
//!
//! Five triodes, and every one of them is run cool. The input stage has a 220 k
//! plate and shares a fully bypassed 1.5 k cathode with the other channel; the
//! brilliant channel reaches its volume through 1000 pF, which throws away
//! everything below the top of the guitar's range before any of it is amplified.
//! The second stage drives a cathode follower into a Fender stack -- the same
//! arrangement a Marshall uses -- and then, unlike a Marshall, there is a master
//! volume. This block ends at the master's wiper. The gain stage after it (V3a),
//! the follower that holds the phase inverter's grids (V3b) and the presence
//! control are built in the power stage (`power::DriverSpec::HIWATT_DR103`),
//! because the presence control is a loop from the output transformer back to
//! V3a's plate, and a loop has to be inside one netlist.
//!
//! What makes it a Hiwatt is what happens after that: 22 k grid stoppers on the
//! output valves and a tight feedback loop, on a supply near 480 V. It is an
//! amplifier that stays clean where the others are breaking up, which is why the
//! records it is on are the clean ones.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// BRILLIANT VOLUME, 470 k log. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// TREBLE, 220 k linear.
pub const TREBLE: usize = 1;
/// BASS, 470 k log, wired as a variable resistor.
pub const BASS: usize = 2;
/// MIDDLE, 100 k linear.
pub const MIDDLE: usize = 3;
/// MASTER VOLUME, 220 k linear, between the stack and the last two triodes.
pub const MASTER: usize = 4;

/// Where the master rests when the panel's Master knob is not turning it.
pub const MASTER_REST: f64 = 0.5;

/// H.T. SUPPLY 3 as the preamplifier's V1 and V2 see it. ESTIMATED: the sheets
/// give one preamp voltage (V2a's plate at 140 V) and no rails. This is what
/// stands the model's V2a plate at that figure; checked in `tests/dr103.rs`.
///
/// It is **in conflict** with the power stage's reading of the same node
/// (`PowerSpec::DR103_EL34.pi_supply`, 465 V, from the supply sheet's 100 R and
/// 1 k behind a 480 V reservoir), where V3a, V3b and the inverter hang. Both
/// are estimates of one rail from different evidence; neither is measured. See
/// `docs/models/brit_dr103.md`.
pub const RAIL: f64 = 300.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the BRILLIANT input to the master's wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit DR103 preamp");

    // --- supplies --------------------------------------------------------------
    // H.T. SUPPLY 3 through the sheet's 10 k droppers, with 16 uF and 32 uF.
    net.supply("n3", 10_000.0, RAIL)
        .capacitor("n3", "gnd", 32e-6)
        .resistor("n3", "n1", 10_000.0)
        .capacitor("n1", "gnd", 16e-6);

    // --- V1b, the brilliant input stage ------------------------------------------
    // 220 k plate, and a 1.5 k cathode with 100 uF across it that it shares with
    // the normal channel's triode -- which is not built, so its idle current is a
    // resistor here.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "v1_g", 68_000.0)
        .resistor("v1_k", "gnd", 1_500.0)
        .capacitor("v1_k", "gnd", 100e-6)
        .resistor("n1", "v1_k", 430_000.0)
        .resistor("n1", "v1_p", 220_000.0)
        .triode("v1_p", "v1_g", "v1_k", ECC83);

    // --- the brilliant volume and the mixer ----------------------------------------
    // 1000 pF into a 470 k log track: this channel is the treble one, and that
    // capacitor is why. Both channels reach V2a through 470 k; the normal
    // channel's volume is at zero, so its 470 k is a resistor to ground.
    net.capacitor("v1_p", "vol_top", 1_000e-12)
        .pot("vol_top", "vol", "gnd", 470_000.0, Taper::Audio, VOLUME)
        .resistor("vol", "v2_g", 470_000.0)
        .resistor("v2_g", "gnd", 470_000.0);

    // --- V2a and its cathode follower ------------------------------------------------
    net.resistor("v2_k", "gnd", 1_000.0)
        .resistor("n1", "v2_p", 100_000.0)
        .triode("v2_p", "v2_g", "v2_k", ECC83)
        .resistor("cf1", "gnd", 100_000.0)
        .capacitor("cf1", "gnd", 47e-9)
        .triode("n1", "v2_p", "cf1", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // 1000 pF and 220 pF in series to the top of Treble, with 220 k across the
    // first of them; 100 k slope; 47 nF to the bottom of Treble and the top of
    // Bass; 47 nF to the Middle wiper; 1000 pF across the Middle track.
    net.capacitor("cf1", "t_mid", 1_000e-12)
        .resistor("cf1", "t_mid", 220_000.0)
        .capacitor("t_mid", "t_top", 220e-12)
        .resistor("cf1", "slope", 100_000.0)
        .pot("t_top", "t_w", "t_bot", 220_000.0, Taper::Linear, TREBLE)
        .capacitor("slope", "t_bot", 47e-9)
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            470_000.0,
            Taper::ReverseAudio,
            BASS,
        )
        .pot("b_bot", "m_w", "gnd", 100_000.0, Taper::Linear, MIDDLE)
        .capacitor("slope", "m_w", 47e-9)
        .capacitor("b_bot", "gnd", 1_000e-12);

    // --- the master volume -------------------------------------------------------
    // 22 k from the treble wiper into a 220 k linear track. This is the control
    // the British competition did not have in 1969. Its wiper is V3a's grid,
    // which is where this block ends: V3a draws no grid current in normal play,
    // so the power stage can take the wiper's voltage as it stands.
    net.resistor("t_w", "master_top", 22_000.0)
        .rest(MASTER, MASTER_REST)
        .pot("master_top", "out", "gnd", 220_000.0, Taper::Linear, MASTER)
        .resistor("out", "gnd", load);

    net.build(at)
}
