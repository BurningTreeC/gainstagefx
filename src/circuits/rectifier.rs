//! The Cali Rectifier preamplifier: the red channel of a two-channel American
//! high-gain head, in its modern setting.
//!
//! Engineering reference (developer documentation, not panel text): Mesa/Boogie
//! Dual Rectifier, "MESA-BOOGIE DUAL RECTIFIER PREAMP RF-1F" sheet dated 6-93,
//! which is the Revision F board. See `docs/models/cali_rectifier.md`.
//!
//! Five triodes, and the one that matters is the third. V2B runs on a 39 k
//! cathode resistor with nothing across it -- about twenty times what a normal
//! gain stage uses -- so it is biased almost to cutoff and squares off the
//! bottom half of the wave before the signal has even reached the tone controls.
//! That is where this amplifier's bottom end comes from: not from the stack, and
//! not from the power valves, but from one stage run cold on purpose.
//!
//! The rest is a high-gain chain of the usual kind: a 220 k first stage, an
//! interstage network of two capacitors and two 2.2 M resistors that sets how
//! much low end survives into the gain control, a 1 M gain pot, two more stages,
//! a cathode follower and a Fender stack.
//!
//! What the name promises and this does not do: the amplifier switches between
//! silicon and valve rectifiers, and the supply here is a voltage behind a
//! resistance, which is the silicon setting.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// RED GAIN, 1 M. The Drive knob turns it.
pub const GAIN: usize = 0;
/// TREBLE, 250 k.
pub const TREBLE: usize = 1;
/// BASS, 1 M, wired as a variable resistor.
pub const BASS: usize = 2;
/// MIDDLE, 25 k.
pub const MIDDLE: usize = 3;
/// RED MASTER, 1 M.
pub const MASTER: usize = 4;

/// Where the master rests when the panel's Master knob is not turning it.
pub const MASTER_REST: f64 = 0.5;

/// The rails, from the drawing's own node voltages: V2B's plate reads 384 V and
/// the follower's plate 415 V, so those two nodes are fed from 415 V through the
/// droppers the sheet has. ESTIMATED to land on the sheet's figures; checked in
/// `tests/rectifier.rs`.
pub const RAIL: f64 = 415.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the input to the red channel's master wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Cali Rectifier preamp");

    // --- supplies ---------------------------------------------------------------
    // The sheet marks four nodes: 200 V at V1A's plate, 280 V at V2A's, 384 V at
    // V2B's and 415 V at the follower's. Those are what these droppers stand for.
    net.supply("d", 1_000.0, RAIL)
        .capacitor("d", "gnd", 50e-6)
        .resistor("d", "e", 47_000.0)
        .capacitor("e", "gnd", 50e-6)
        .resistor("d", "c", 6_800.0)
        .capacitor("c", "gnd", 50e-6);

    // --- V1A ----------------------------------------------------------------------
    // R211 1 M at the jack, R221 220 k plate, R291 1k8 with C31 1 uF. R271 47 k
    // is switched in by LDR3 on the clean channel only and is not built.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R211
        .resistor("v1_k", "gnd", 1_800.0) // R291
        .capacitor("v1_k", "gnd", 1e-6) // C31
        .resistor("e", "v1_p", 220_000.0) // R221
        .triode("v1_p", "in", "v1_k", ECC83);

    // --- the interstage network and the gain control ---------------------------------
    // C21 .02 into C10 82 pF with R322 2.2 M across it, and R321 2.2 M to ground:
    // in the modern setting this is what reaches the gain pot, and the 82 pF is
    // why the bottom end arrives thinned rather than whole. C9 .002 with R110
    // 680 k is the other setting and is not built. The gain pot has .001 across
    // its wiper.
    net.capacitor("v1_p", "c21", 0.02e-6) // C21
        .capacitor("c21", "gain_top", 82e-12) // C10
        .resistor("c21", "gain_top", 2_200_000.0) // R322
        .resistor("gain_top", "gnd", 2_200_000.0) // R321
        .pot("gain_top", "gain", "gnd", 1_000_000.0, Taper::Audio, GAIN)
        .capacitor("gain", "gnd", 0.001e-6)
        .resistor("gain", "v2_g", 470_000.0); // R241

    // --- V2A ------------------------------------------------------------------------
    net.resistor("v2_k", "gnd", 1_800.0) // R292
        .capacitor("v2_k", "gnd", 1e-6) // C32
        .resistor("d", "v2_p", 100_000.0) // R201
        .capacitor("v2_p", "gnd", 20e-12) // C12
        .triode("v2_p", "v2_g", "v2_k", ECC83);

    // --- V2B, the cold stage ------------------------------------------------------------
    // R103 39 k at the cathode with nothing across it. A normal stage uses 1k5 and
    // a capacitor; this one is biased nearly to cutoff, so it clips one side of
    // the wave long before the other. It is the sound.
    net.capacitor("v2_p", "c22", 0.02e-6) // C22
        .resistor("c22", "v2b_g", 470_000.0) // R242
        .resistor("v2b_g", "gnd", 1_000_000.0) // R212
        .resistor("v2b_k", "gnd", 39_000.0) // R103
        .resistor("d", "v2b_p", 100_000.0) // R202
        .capacitor("v2b_p", "gnd", 0.001e-6) // C6
        .triode("v2b_p", "v2b_g", "v2b_k", ECC83);

    // --- V3A ---------------------------------------------------------------------------
    net.capacitor("v2b_p", "c27", 0.02e-6) // C27
        .resistor("c27", "v3_g", 220_000.0) // R225
        .resistor("v3_g", "gnd", 330_000.0) // R106
        .resistor("v3_k", "gnd", 1_800.0) // R293
        .capacitor("v3_k", "gnd", 1e-6) // C33
        .resistor("c", "v3_p", 220_000.0) // R224
        .triode("v3_p", "v3_g", "v3_k", ECC83);

    // --- the cathode follower -------------------------------------------------------------
    net.resistor("cf", "gnd", 100_000.0) // R207
        .triode("d", "v3_p", "cf", ECC83);

    // --- the red channel's tone stack and master --------------------------------------------
    // C4 680 pF to the top of Treble, R273 47 k as the slope, C23 .02 to the
    // bottom of Treble and the top of Bass, C24 .02 to the Middle wiper. R215 1 M
    // joins this stack to the other channel's, which is what that resistor does
    // here: a load that is always present.
    net.capacitor("cf", "t_top", 680e-12) // C4
        .resistor("cf", "slope", 47_000.0) // R273
        .pot("t_top", "t_w", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .capacitor("slope", "t_bot", 0.02e-6) // C23
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        )
        .pot("b_bot", "m_w", "gnd", 25_000.0, Taper::Linear, MIDDLE)
        .capacitor("slope", "m_w", 0.02e-6) // C24
        .resistor("t_w", "gnd", 1_000_000.0) // R215, the other channel's stack
        .rest(MASTER, MASTER_REST)
        .pot("t_w", "out", "gnd", 1_000_000.0, Taper::Audio, MASTER)
        .resistor("out", "gnd", load);

    net.build(at)
}
