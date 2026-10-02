//! The Brit 45 preamplifier: a mid-60s British 45 W head, the first of its
//! maker's amplifiers, built on an American bass amplifier's circuit.
//!
//! Engineering reference (developer documentation, not panel text): the
//! Marshall JTM45 of 1965, the lead head (KT66, GZ34), from Marshall's period
//! drawing "JTM 45 -- basic schematic for Marshall trem amps, types 1961, 1962,
//! 1987/T" with its valve voltage chart, read part by part against Marshall
//! Amplification's CAD "Circuit diagram, JTM45" (issue 7). See
//! `docs/models/brit_jtm45.md`.
//!
//! The Brit Plexi Bass's node structure -- it is the same family -- with the
//! JTM45's own values:
//!
//! - **V1's halves share one cathode**, 820 ohm with 250 uF across it; both
//!   halves are built, the idle one's current setting the played one's bias.
//! - **.02 uF couplings**, and the bright channel's **100 pF** across its
//!   volume (the period drawing; Marshall's later CAD sheet has 500 pF).
//! - **270 k mixers**, the bright channel's with **500 pF** across it (the CAD
//!   prints 500; the period drawing's "5[00]pF" is half under the capacitor's
//!   symbol).
//! - **V2a unbypassed**, 820 ohm, into a cathode follower on 100 k.
//! - **A 270 pF / 56 k stack**: TREBLE 250 k, BASS 1 M, MIDDLE 25 k, .02 uF caps.
//! - **No master**: the treble wiper drives the inverter, which is
//!   `power::PowerSpec::JTM45_KT66`.
//!
//! The supply: the screen node (440 V on the drawing) -- 8.2 k 1 W -- the
//! inverter node (380 V, 16 uF) -- 10 k 1 W -- the preamplifier node (310 V,
//! 16 uF), both drawn and circled with their voltages. The channel built is the
//! bright one, played from its HIGH input, the other channel's volume at zero.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// The bright channel's VOLUME, 1 M (log). The Drive knob turns it.
pub const VOLUME: usize = 0;
/// TREBLE, 250 k.
pub const TREBLE: usize = 1;
/// BASS, 1 M, wired as a variable resistor.
pub const BASS: usize = 2;
/// MIDDLE, 25 k.
pub const MIDDLE: usize = 3;

/// The screen node, 440 V circled on the drawing at the choke: what the
/// preamplifier's dropping chain hangs from.
pub const SCREEN_NODE: f64 = 440.0;
/// 8.2 k 1 W from the screen node to the inverter node.
pub const INVERTER_DROPPER: f64 = 8_200.0;
/// The inverter's draw from its node, as a resistor: its two plates from 380 V
/// to the chart's 250 V through 82 k and 100 k, 2.9 mA (DERIVED).
pub const INVERTER_IDLE: f64 = 131_000.0;
/// The inverter node the power stage's inverter is fed from: 380 V circled on
/// the drawing; re-measured by `tests/brit_jtm45.rs`.
pub const INVERTER_NODE: f64 = 380.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the bright channel's HIGH input to the treble wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit 45 preamp");

    // --- supplies ------------------------------------------------------------
    net.supply("pin", INVERTER_DROPPER, SCREEN_NODE)
        .capacitor("pin", "gnd", 16e-6)
        .resistor("pin", "gnd", INVERTER_IDLE)
        .resistor("pin", "v1n", 10_000.0)
        .capacitor("v1n", "gnd", 16e-6);

    // --- the HIGH input and V1, both halves -------------------------------------
    // 1 M across the jack; with nothing in LOW its switch parallels the two
    // 68 k. The other channel's grid sits on its own 68 k pair.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "v1_g", 34_000.0) // 68 k || 68 k
        .resistor("v1_k", "gnd", 820.0)
        .capacitor("v1_k", "gnd", 250e-6)
        .resistor("v1n", "v1_p", 100_000.0)
        .triode("v1_p", "v1_g", "v1_k", ECC83)
        .resistor("v1b_g", "gnd", 34_000.0)
        .resistor("v1n", "v1b_p", 100_000.0)
        .triode("v1b_p", "v1b_g", "v1_k", ECC83);

    // --- the bright volume and the mixer ---------------------------------------
    // .02 uF into the top of the 1 M track, 100 pF from the top to the wiper,
    // the wiper through 270 k || 500 pF to V2a's grid, and the other channel's
    // 270 k from that grid to its volume wiper, which is at ground.
    net.capacitor("v1_p", "vol_top", 0.02e-6)
        .pot("vol_top", "vol", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .capacitor("vol_top", "vol", 100e-12)
        .resistor("vol", "v2_g", 270_000.0)
        .capacitor("vol", "v2_g", 500e-12)
        .resistor("v2_g", "gnd", 270_000.0);

    // --- V2a and the cathode follower -------------------------------------------
    net.resistor("v2_k", "gnd", 820.0)
        .resistor("v1n", "v2_p", 100_000.0)
        .triode("v2_p", "v2_g", "v2_k", ECC83)
        .resistor("cf", "gnd", 100_000.0)
        .triode("v1n", "v2_p", "cf", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // 270 pF to the top of Treble, 56 k to the slope node, .02 uF from there to
    // the bottom of Treble and the top of Bass, .02 uF to the Middle wiper.
    net.capacitor("cf", "t_top", 270e-12)
        .resistor("cf", "slope", 56_000.0)
        .pot("t_top", "out", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .capacitor("slope", "t_bot", 0.02e-6)
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        )
        .pot("b_bot", "m_w", "gnd", 25_000.0, Taper::Linear, MIDDLE)
        .capacitor("slope", "m_w", 0.02e-6)
        .resistor("out", "gnd", load);

    net.build(at)
}
