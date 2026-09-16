//! The Brit 800 preamplifier: the master-volume preamp of a 1981 British 100 W
//! lead amplifier, from the manufacturer's drawing.
//!
//! Engineering reference (developer documentation, not panel text): Marshall
//! JCM800 Lead Series 2203, "Master Volume Preamp, used on 2103, 2203, 2104,
//! 2204", drawing dated 24-4-81, cross-checked against Marshall's 2203 preamp
//! drawing issue 4 of 19-5-88. See `docs/models/brit_800.md`.
//!
//! Four triodes. The first two are ordinary common-cathode stages with the
//! Preamp Volume between them; the third is the famous one, fed through
//! 470 k with 470 pF across it into 470 k to ground, so it is handed half the
//! signal and all of the top end, and it is run hot on an 820 ohm cathode with
//! nothing across it. The fourth is a cathode follower, direct-coupled from the
//! third's plate, and it drives the passive tone stack from a low impedance --
//! which is why this stack sits after the gain and not between two stages.
//!
//! The drawing's handover to the power stage is the treble wiper into the
//! Master Volume, and the Brit EL34 power stage (`power::PowerSpec::BRIT_EL34`)
//! begins with that pot. So this netlist ends at the treble wiper, loaded by
//! the master's 1 M.
//!
//! ## The supplies
//!
//! The drawing's own voltage table ("all voltages are average only", 100 W
//! master-volume column) gives node 10, the phase inverter's rail, at 330 V,
//! and the two preamp nodes behind their 10 k droppers at 290 V and 280 V. The
//! netlist has the droppers and the 50 uF reservoirs from the drawing, fed from
//! an ideal 330 V; the preamp's own current then sets the two nodes, which is
//! checked against the table in `tests/brit800.rs`.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// VR1, the Preamp Volume, 1 M log. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// VR3, Treble, 220 k linear.
pub const TREBLE: usize = 1;
/// VR5, Bass, 1 M, wired as a variable resistor.
pub const BASS: usize = 2;
/// VR4, Middle, 22 k linear.
pub const MIDDLE: usize = 3;

/// Node 10 in the drawing's voltage table, 100 W master-volume column: the
/// rail the two preamp droppers hang from.
pub const RAIL: f64 = 330.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the HIGH input to the treble wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node, for measuring one stage
/// at a time. Reference designators are the 1988 drawing's; the 1981 sheet
/// carries none.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit 800 preamp");

    // --- supplies ------------------------------------------------------------
    // Node 6 behind 10 k 1 W from the rail, node 5 behind R8 10 k 1 W from node 6,
    // each with 50 uF (C21 50+50).
    net.supply("n6", 10_000.0, RAIL)
        .capacitor("n6", "gnd", 50e-6)
        .resistor("n6", "n5", 10_000.0) // R8
        .capacitor("n5", "gnd", 50e-6);

    // --- the HIGH input and the first stage ----------------------------------
    // R2 1 M across the jack, R3 68 k grid stopper. The cathode is R1 2k7 with
    // C1 .68 uF across it, so the stage is bypassed from about 87 Hz up.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R2
        .resistor("in", "v1_g", 68_000.0) // R3
        .resistor("v1_k", "gnd", 2_700.0) // R1
        .capacitor("v1_k", "gnd", 0.68e-6) // C1
        .resistor("n5", "v1_p", 100_000.0) // R4
        .triode("v1_p", "v1_g", "v1_k", ECC83);

    // --- the Preamp Volume ----------------------------------------------------
    // C3 .022 uF, through the LOW jack's switch (closed with nothing in LOW),
    // then R5 470 k with C4 470 pF across it into the top of VR1. C5 1 nF from
    // the top of the track to the wiper is the bright cap: it passes treble
    // round the pot, most of all with the control low. The 100 pF (C2) the 1988
    // drawing adds across the first stage is not on the 1981 sheet and is not
    // built.
    net.capacitor("v1_p", "low", 0.022e-6) // C3
        .resistor("low", "vol_top", 470_000.0) // R5
        .capacitor("low", "vol_top", 470e-12) // C4
        .pot("vol_top", "v2_g", "gnd", 1_000_000.0, Taper::Audio, VOLUME) // VR1
        .capacitor("vol_top", "v2_g", 1e-9); // C5

    // --- the second stage ------------------------------------------------------
    // R6 10 k at the cathode and nothing across it: a low-gain stage, and a
    // linear one until it is hit hard.
    net.resistor("v2_k", "gnd", 10_000.0) // R6
        .resistor("n5", "v2_p", 100_000.0) // R7
        .triode("v2_p", "v2_g", "v2_k", ECC83);

    // --- the third stage ---------------------------------------------------------
    // C7 .022 uF, R10 470 k with C8 470 pF across it, R11 470 k to ground: half
    // the signal below a few kilohertz and nearly all of it above. R9 820 ohm,
    // unbypassed.
    net.capacitor("v2_p", "mix", 0.022e-6) // C7
        .resistor("mix", "v3_g", 470_000.0) // R10
        .capacitor("mix", "v3_g", 470e-12) // C8
        .resistor("v3_g", "gnd", 470_000.0) // R11
        .resistor("v3_k", "gnd", 820.0) // R9
        .resistor("n6", "v3_p", 100_000.0) // R12
        .triode("v3_p", "v3_g", "v3_k", ECC83);

    // --- the cathode follower ------------------------------------------------------
    // Grid direct to the third stage's plate, plate to node 6, R13 100 k from
    // the cathode to ground. Its grid sits at about 165 V and its cathode a volt
    // or two above that, so on a big positive swing the grid draws current
    // through the previous stage's 100 k plate load -- part of this preamp's
    // sound, and modelled because both valves are in the same netlist.
    net.resistor("v4_k", "gnd", 100_000.0) // R13
        .triode("n6", "v3_p", "v4_k", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // C10 470 pF to the top of Treble, R15 33 k to the slope node, C11 .022 uF from
    // there to the bottom of Treble and the top of Bass, C12 .022 uF to the
    // Middle wiper. Bass is a variable resistor between Treble and Middle; turned
    // up it puts more resistance in, so it takes the reverse law. The Treble
    // wiper is the output, into the Master Volume.
    net.capacitor("v4_k", "t_top", 470e-12) // C10
        .resistor("v4_k", "slope", 33_000.0) // R15
        .pot("t_top", "out", "t_bot", 220_000.0, Taper::Linear, TREBLE) // VR3
        .capacitor("slope", "t_bot", 0.022e-6) // C11
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        ) // VR5
        .pot("b_bot", "m_w", "gnd", 22_000.0, Taper::Linear, MIDDLE) // VR4
        .capacitor("slope", "m_w", 0.022e-6) // C12
        .resistor("out", "gnd", load);

    net.build(at)
}
