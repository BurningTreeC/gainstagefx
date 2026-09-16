//! The British 4K E microphone amplifier: a console channel's mic input stage.
//!
//! Engineering reference (developer documentation, not panel text): the Solid
//! State Logic SL 4000 E channel amplifier card 82E01, mic amplifier section, from
//! SSL's drawing T82001-71 revision 16 (11/84), cross-checked against the
//! 82E01-710-8208-12 drawing. Input transformer Jensen JE-115K-E, from Jensen's
//! data sheet. See `docs/models/british_4k_e.md`.
//!
//! Two op-amps and one pot, and the pot is the trick. T1 is a non-inverting
//! stage and T9 an inverting one, and the 10 k linear MIC GAIN control sits
//! between them with T1's output on its wiper: the part of the track on T1's
//! side is T1's feedback resistor, and the part on T9's side is T9's input
//! resistor. Turning it up moves resistance from one to the other, so both
//! gains rise together and a linear track sweeps 0 dB to 50 dB in a curve close
//! to the ear's. The Jensen's 1:10 in front adds 20 dB.
//!
//! ## What is measured and what is estimated
//!
//! From SSL's drawing: every resistor and capacitor in the mic path, both op-amps
//! and their supplies (±18 V). From Jensen's data sheet: the 1:10 ratio, 19.7 ohm
//! and 2465 ohm windings, the 150 k load it is specified into, the -3 dB points at
//! 2.5 Hz and 90 kHz, and the 20 Hz level for 1 % distortion.
//!
//! Estimated: the core constants and the winding capacitance, which are fitted to
//! those published figures (`tests/console_e.rs` holds them). The 5534s are ideal
//! op-amps with a rail; their 22 pF compensation capacitors are internal
//! compensation and are not built. The pad (SW1), phantom feed and BBC-only parts
//! (C2, R8) are out.

use crate::dsp::netlist::{Circuit, CoreSpec, Fault, Netlist, Taper};

/// MIC GAIN, the 10 k linear pot between T1 and T9.
pub const GAIN: usize = 0;

/// What a 5534 on the card's ±18 V rails swings either way into its load.
pub const RAIL: f64 = 16.0;

/// Jensen JE-115K-E, primary volts per secondary volt.
pub const INPUT_RATIO: f64 = 0.1;

/// ESTIMATED from Jensen's figures. Magnetising inductance for the 2.5 Hz corner
/// from a 150 ohm source into 150 k; knee and sharpness for 1 % at 20 Hz with
/// -2.5 dBu in, and the "10 dB per decade" soft overload the older sheet describes.
pub const INPUT_CORE: CoreSpec = CoreSpec {
    henry: 9.7,
    knee: 0.0109,
    sharpness: 3.0,
};

/// Jensen's DC resistances, primary (RED-BRN) and secondary (YEL-ORG).
const PRIMARY_R: f64 = 19.7;
const SECONDARY_R: f64 = 2_465.0;
/// ESTIMATED: the effective capacitance at the secondary that puts the upper
/// -3 dB point at Jensen's 90 kHz against its stated 17 k output impedance.
const SHUNT: f64 = 85e-12;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("British 4K E mic amp");

    // --- the input: phantom feed resistors and the Jensen --------------------
    // R1 and R2 6K81 from each leg to the phantom rail, C1 3.3 uF from there to
    // ground; with phantom off and one leg taken as ground, R1 reaches ground
    // through R2 and C1 in parallel.
    net.input("in", source)
        .resistor("in", "p2", 6_810.0) // R1
        .resistor("p2", "gnd", 6_810.0) // R2
        .capacitor("p2", "gnd", 3.3e-6) // C1
        .resistor("in", "tp", PRIMARY_R)
        .core("tp", "gnd", INPUT_CORE)
        .transformer("tp", "gnd", "ts", "gnd", INPUT_RATIO)
        .resistor("ts", "plus", SECONDARY_R)
        .capacitor("plus", "gnd", SHUNT)
        .resistor("plus", "gnd", 150_000.0); // R7

    // --- T1 -------------------------------------------------------------------
    // Non-inverting. Its inverting input (pin 19) goes to ground through R9 619R
    // and C3 100 uF, and to the output through R10 and R11 10 k with C4 100 uF
    // from their junction to ground: unity gain at DC, and at audio the gain is
    // set by the track between the wiper and pin 19.
    net.opamp("o1", "plus", "m1", RAIL)
        .resistor("m1", "r9", 619.0) // R9
        .capacitor("r9", "gnd", 100e-6) // C3
        .resistor("m1", "n", 10_000.0) // R10
        .capacitor("n", "gnd", 100e-6) // C4
        .resistor("n", "o1", 10_000.0); // R11

    // --- T1's output onto the wiper --------------------------------------------
    // C54 and C55 220 uF back to back, with R72 100 k across C54 (R73's 1 M to the
    // positive rail, which only polarises them, is not built).
    net.capacitor("o1", "k", 220e-6) // C54
        .resistor("o1", "k", 100_000.0) // R72
        .capacitor("k", "wiper", 220e-6) // C55
        // MIC GAIN: pin 33 at the top, wiper pin 10, pin 19 at the bottom. The
        // wiper-to-pin-19 part is T1's feedback and grows as the knob turns up.
        .pot("p33", "wiper", "m1", 10_000.0, Taper::Linear, GAIN);

    // --- T9 ---------------------------------------------------------------------
    // Inverting. R69 619R from pin 33, R71 10K7 with C56 100 pF from the output,
    // R70 5K11 from the non-inverting input to ground. R46 100 ohm out.
    net.resistor("p33", "m9", 619.0) // R69
        .resistor("pl9", "gnd", 5_110.0) // R70
        .opamp("o9", "pl9", "m9", RAIL)
        .resistor("o9", "m9", 10_700.0) // R71
        .capacitor("o9", "m9", 100e-12) // C56
        .resistor("o9", "out", 100.0) // R46
        .resistor("out", "gnd", load);

    net.build(at)
}
