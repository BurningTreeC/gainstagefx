//! The Brum 100 preamplifier: a late-60s British 100 W head from the Midlands,
//! Marshall-like and not a Marshall.
//!
//! Engineering reference (developer documentation, not panel text): Laney
//! Supergroup 100 Mk I (LA100BL), a 1969 build, from the traced drawing
//! "SUPERGROUP 100 mk I build '69" by vddj, 22-09-2008, revision 1.0, three
//! sheets (preamp, power amp, supply). No factory sheet was retrieved; the
//! trace is of this amplifier, with designators and values throughout, and
//! its two features independent accounts mention -- split 1.5 k / 25 uF V1
//! cathodes and a 56 k / 22 nF / 22 nF stack -- agree with it. See
//! `docs/models/brum_100.md`.
//!
//! Two channels, TREBLE and BASS, each one ECC83 half with its own 1.5 k and
//! 25 uF -- which is why the BASS channel's half need not be built: nothing it
//! does reaches the played one but its current, through the supply. The
//! TREBLE channel's only difference from the BASS channel is C12, 270 pF across
//! its 470 k mixer. Then V2b unbypassed on 820 ohm, a direct-coupled cathode
//! follower, and a stack whose treble capacitor is 270 pF, slope 56 k, middle
//! pot 22 k. No master: the treble wiper drives the phase inverter, so the
//! volume decides how hard the 600 V power stage works. Its matched power stage
//! is `power::PowerSpec::BRUM_EL34`.
//!
//! The supply chain is as the trace draws it, including one thing a builder
//! would not expect: **HT4 has no reservoir.** V2b and the cathode follower sit
//! on the far side of R38's 22 k from HT3 with nothing to ground but HT5's
//! 15 uF behind another 10 k, so the second stage's own current moves its
//! supply. Built as drawn, PLAUSIBLE that a capacitor was missed in tracing.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// GAIN TWO (P1), the TREBLE channel's 1 M volume. The trace marks no law;
/// audio, as Marshall's of the period. APPROXIMATED. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// TREBLE (R23), 250 k; linear, as Marshall's of the period (APPROXIMATED).
pub const TREBLE: usize = 1;
/// BASS (R24), 1 M wired as a variable resistor; audio (APPROXIMATED).
pub const BASS: usize = 2;
/// MIDDLE (R25), 22 k; linear (APPROXIMATED).
pub const MIDDLE: usize = 3;

/// HT2, the screens' node behind the 20 H choke, which the whole low-current
/// chain hangs from. ESTIMATED from HT1's printed 600 V less the choke's
/// copper (not printed) under the screens' and the chain's current.
pub const SCREEN_NODE: f64 = 597.0;

/// R39, 2.7 k from HT2 to HT3.
pub const INVERTER_DROPPER: f64 = 2_700.0;

/// The phase inverter's idle current, as a resistance on HT3: it is solved in
/// the power stage's netlist, but its current flows through this chain.
/// APPROXIMATED; re-measured by `tests/brum100.rs`.
pub const INVERTER_IDLE: f64 = 130_000.0;

/// The BASS channel's half of V1, which is not built: its volume is down and
/// its cathode is its own, so all it does is draw its idle current through
/// HT5. That current as a resistance, on the model's own ECC83 with 100 k and
/// 1.5 k. APPROXIMATED.
pub const BASS_V1_IDLE: f64 = 300_000.0;

/// HT3, which the power stage's inverter is fed from. Re-measured by
/// `tests/brum100.rs`.
pub const INVERTER_NODE: f64 = 567.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the TREBLE channel's first input to the treble wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brum 100 preamp");

    // --- supplies ------------------------------------------------------------
    // HT2 -- R39 2.7 k -- HT3 (C22 + C23, 33 uF in series) -- R38 22 k -- HT4
    // (nothing) -- R14 10 k -- HT5 (C9 15 uF).
    net.supply("ht3", INVERTER_DROPPER, SCREEN_NODE)
        .capacitor("ht3", "gnd", 16.5e-6)
        .resistor("ht3", "gnd", INVERTER_IDLE)
        .resistor("ht3", "ht4", 22_000.0)
        .resistor("ht4", "ht5", 10_000.0)
        .capacitor("ht5", "gnd", 15e-6)
        .resistor("ht5", "gnd", BASS_V1_IDLE);

    // --- the input and U1B ---------------------------------------------------
    // R6 1 M across the jack; with nothing in TREBLE2 its switch puts R4 and
    // R5, 68 k each, in parallel.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "v1_g", 34_000.0) // R4 || R5
        .resistor("v1_k", "gnd", 1_500.0) // R8
        .capacitor("v1_k", "gnd", 25e-6) // C7
        .resistor("ht5", "v1_p", 100_000.0) // R7
        .triode("v1_p", "v1_g", "v1_k", ECC83);

    // --- GAIN TWO and the mixer --------------------------------------------------
    // C10 22 nF into the top of P1, the wiper through R15 470 k || C12 270 pF to
    // U2B's grid, and the BASS channel's R16 470 k from that grid to its volume
    // wiper, which is at ground.
    net.capacitor("v1_p", "vol_top", 22e-9)
        .pot("vol_top", "vol", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .resistor("vol", "v2_g", 470_000.0)
        .capacitor("vol", "v2_g", 270e-12)
        .resistor("v2_g", "gnd", 470_000.0);

    // --- U2B -----------------------------------------------------------------------
    // R17 100 k from HT4, R18 820 ohm unbypassed.
    net.resistor("v2_k", "gnd", 820.0)
        .resistor("ht4", "v2_p", 100_000.0)
        .triode("v2_p", "v2_g", "v2_k", ECC83);

    // --- U2A, the cathode follower ------------------------------------------------
    // Grid on U2B's plate, plate on HT4, R19 100 k from the cathode to ground.
    net.resistor("cf", "gnd", 100_000.0)
        .triode("ht4", "v2_p", "cf", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // C13 270 pF to the top of R23, R20 56 k to the slope node, C14 22 nF from
    // there to the bottom of R23 and the top of R24, C15 22 nF to R25's wiper.
    net.capacitor("cf", "t_top", 270e-12)
        .resistor("cf", "slope", 56_000.0)
        .pot("t_top", "out", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .capacitor("slope", "t_bot", 22e-9)
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        )
        .pot("b_bot", "m_w", "gnd", 22_000.0, Taper::Linear, MIDDLE)
        .capacitor("slope", "m_w", 22e-9)
        .resistor("out", "gnd", load);

    net.build(at)
}
