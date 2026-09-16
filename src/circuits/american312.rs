//! The American 312 microphone preamplifier: a transformer, one discrete
//! op-amp, and a transformer.
//!
//! Engineering reference (developer documentation, not panel text): the API 312
//! mic preamp card, from API's own "MIC PREAMP SCHEMATIC" (T2 2622, one 2520, T1
//! 2503), API's transformer data sheet and its Model 2520 data sheet. See
//! `docs/models/american_312.md`.
//!
//! The whole card is three parts doing three jobs. The input transformer gives
//! gain for no noise and unbalances the microphone. The 2520 is a non-inverting
//! stage whose gain the card leaves to an external resistor. The output
//! transformer takes the op-amp's output to a balanced line. What it sounds
//! like pushed is the op-amp meeting its rails and the iron at both ends meeting
//! the bottom of the band.
//!
//! ## What is measured and what is estimated
//!
//! From the drawings and data sheets:
//! - every resistor and capacitor on the card;
//! - the 2622's 150 ohm to 10 k impedance ratio, the 2503's 1:1/1:2/1:3 windings;
//! - the 2520's output swing.
//!
//! Estimated, and fitted to the published ±0.5 dB, 30 Hz-20 kHz and maximum-level
//! figures because API does not print them: both transformers' winding
//! resistances, magnetising inductances, cores and leakage. The 2520 is an ideal
//! op-amp with a rail: nine transistors including PNPs, and the solver has no PNP
//! device.

use crate::dsp::netlist::{Circuit, CoreSpec, Fault, Netlist, Taper};

/// The gain trim: the resistance the card leaves to its host at pins 9 and 10,
/// as a control. Turned up, less resistance and more gain.
pub const GAIN: usize = 0;

/// What the 2520 swings either way. API's sheet guarantees more than 7.75 V rms
/// (10.96 V peak) at ±15 V; twelve volts is that figure with the part's usual
/// margin, stated rather than measured.
pub const RAIL: f64 = 12.0;

/// The 2622 strapped for a 150 ohm microphone, primary volts per secondary volt:
/// `sqrt(150 / 10,000)`.
pub const INPUT_RATIO: f64 = 0.122_474_487;

/// The 2503 with SEC 1 and SEC 2 in series (pins 4-6): 1:2.
pub const OUTPUT_RATIO: f64 = 0.5;

/// The largest trim resistance the sweep reaches. With R2's 200 ohm the stage
/// then gains about 6 dB; at zero, 40 dB.
pub const TRIM: f64 = 20_000.0;

/// ESTIMATED. The 2622's magnetising inductance and knee: -0.5 dB at 30 Hz from a
/// 150 ohm source, and about 1 % distortion at its 0 dBm rating at 30 Hz. Fitted
/// by `tests/american312.rs`' measurements.
pub const INPUT_CORE: CoreSpec = CoreSpec {
    henry: 2.6,
    knee: 0.029,
    sharpness: 3.0,
};
/// ESTIMATED copper, leakage and winding capacitance of the 2622.
const INPUT_PRIMARY_R: f64 = 20.0;
const INPUT_SECONDARY_R: f64 = 800.0;
const INPUT_LEAKAGE: f64 = 1e-3;
const INPUT_SHUNT: f64 = 50e-12;

/// ESTIMATED. The 2503's magnetising inductance and knee: flat to 30 Hz from the
/// op-amp, and about 1 % distortion near the op-amp's own full swing at 30 Hz,
/// which is where its +30 dBm rating puts it.
pub const OUTPUT_CORE: CoreSpec = CoreSpec {
    henry: 0.6,
    knee: 0.08,
    sharpness: 5.0,
};
const OUTPUT_PRIMARY_R: f64 = 8.0;
const OUTPUT_SECONDARY_R: f64 = 30.0;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "line")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("American 312");

    // --- T2, the 2622 --------------------------------------------------------
    // C6 and C7 1000 pF from the primary legs to common; with one leg of the
    // microphone taken as ground here, one of them is across the input.
    net.input("in", source)
        .capacitor("in", "gnd", 1000e-12) // C6/C7
        .resistor("in", "t2p", INPUT_PRIMARY_R)
        .core("t2p", "gnd", INPUT_CORE)
        .transformer("t2p", "gnd", "t2s", "gnd", INPUT_RATIO)
        .resistor("t2s", "t2r", INPUT_SECONDARY_R)
        .inductor("t2r", "plus", INPUT_LEAKAGE)
        .capacitor("plus", "gnd", INPUT_SHUNT)
        // R4 5.1 k and C5 220 pF across the secondary: the damping network.
        .resistor("plus", "zobel", 5_100.0) // R4
        .capacitor("zobel", "gnd", 220e-12); // C5

    // --- the 2520 ------------------------------------------------------------
    // Non-inverting. R3 20 k from the output with C4 120 pF across it; R2 200 ohm
    // to the gain trim at pins 9/10, then C3 250 uF to common.
    net.opamp("out", "plus", "minus", RAIL)
        .resistor("out", "minus", 20_000.0) // R3
        .capacitor("out", "minus", 120e-12) // C4
        .resistor("minus", "trim", 200.0) // R2
        .pot(
            "trim",
            "c3",
            "c3",
            TRIM,
            // A span below one: the resistance left falls as 101^-p, so
            // 200 + R, and with it the gain, moves evenly in decibels.
            Taper::Log {
                span: 200.0 / (200.0 + TRIM),
            },
            GAIN,
        )
        .capacitor("c3", "gnd", 250e-6); // C3

    // --- T1, the 2503 ----------------------------------------------------------
    // The op-amp drives the 75 ohm primary directly (RED to BRN, BRN at common).
    net.resistor("out", "t1p", OUTPUT_PRIMARY_R)
        .core("t1p", "gnd", OUTPUT_CORE)
        .transformer("t1p", "gnd", "t1s", "gnd", OUTPUT_RATIO)
        .resistor("t1s", "line", OUTPUT_SECONDARY_R)
        .resistor("line", "gnd", load);

    net.build(at)
}
