//! The Tube 610 microphone preamplifier: a 1960s modular console channel.
//!
//! Engineering reference (developer documentation, not panel text): Universal
//! Audio "Modular Amplifier Model 610-A", from John Hinson's redraw of UA drawing
//! C-10068. UTC O-1 input transformer (UTC 1963 catalogue and 1952 terminal
//! arrangements), UTC PA-5946 output transformer, 12AX7 and 12AY7 valves (RCA
//! data). See `docs/models/tube_610.md`.
//!
//! Two valves, four triodes, and feedback around both pairs, which is what the
//! later reissue's manual describes: "variable negative feedback is applied to
//! both of these stages to control gain, distortion, and frequency response."
//!
//! - **First pair.** V1-A and V1-B, both 12AX7. The output of V1-B's plate comes
//!   back through C4, R11 68 k and R9 39 k to V1-A's cathode, which sits on R27
//!   4K7 with nothing across it. The Lo/Hi gain switch takes the signal from one
//!   side of R11 or the other; this model is at Hi.
//! - **The Level control**, R13 250 k, between the pairs. The Drive knob turns it.
//! - **Second pair.** V2-A and V2-B, both 12AY7. V2-B's plate drives the output
//!   transformer's primary single-ended, and comes back through C6 and the EQ
//!   network to V2-A's cathode. The EQ is that feedback network: with both
//!   switches at 0 it is R22 220 k (R21 and C9 shorted by the LF switch), with a
//!   tiny capacitive chain hanging off it that the HF switch would select.
//!
//! ## What is measured, what is estimated, and what is doubted
//!
//! From the drawing: every resistor and capacitor in the mic path; the 50 ohm
//! primary connection (pins 3-4, which UTC's table gives as the 50 ohm winding);
//! the output transformer's 30 k : 600 ohm and "L = 55 HY Min."
//!
//! Estimated: the B+ (275 V, from the redraw's author: "around the 275V range"),
//! the O-1's copper on the 50 ohm tap and its knee, the PA-5946's copper, leakage
//! and knee.
//!
//! Doubted, and built as drawn: C15 82 pF from V1-B's cathode to V2-A's cathode,
//! which a GroupDIY reviewer doubted. It is a few picofarads against kilohms and
//! changes nothing audible either way. The same reviewer read the Lo/Hi switch
//! as shorting R11; at Hi the two readings differ only by where the signal is
//! taken, and the drawing is followed.

use crate::dsp::netlist::{Circuit, CoreSpec, Fault, Netlist, Taper, TriodeSpec};

/// R13, the 250 k Level control at V2-A's grid.
pub const LEVEL: usize = 0;

/// ESTIMATED: the console's B+.
pub const BPLUS: f64 = 275.0;

/// UTC O-1, 50 ohm primary to 50 k secondary: `sqrt(50 / 50,000)`.
pub const INPUT_RATIO: f64 = 0.031_622_777;

/// ESTIMATED from UTC's ±1 dB 30 Hz-20 kHz and +8 dBm rating on the 50 ohm tap.
pub const INPUT_CORE: CoreSpec = CoreSpec {
    henry: 0.9,
    knee: 0.021,
    sharpness: 3.0,
};
/// ESTIMATED copper on the 50 ohm tap (UTC gives 52 ohm for the primary).
const INPUT_PRIMARY_R: f64 = 16.0;
/// UTC's secondary DC resistance.
const INPUT_SECONDARY_R: f64 = 3_900.0;
/// ESTIMATED winding and grid capacitance at the secondary.
const INPUT_SHUNT: f64 = 40e-12;

/// UTC PA-5946, 30 k : 600 ohm: `sqrt(30,000 / 600)`.
pub const OUTPUT_RATIO: f64 = 7.071_067_8;
/// "L = 55 HY Min." on the drawing.
const OUTPUT_PRIMARY_L: f64 = 55.0;
/// ESTIMATED copper (a rewind measured 826 and 118 ohm).
const OUTPUT_PRIMARY_R: f64 = 826.0;
const OUTPUT_SECONDARY_R: f64 = 118.0;
/// ESTIMATED knee, on the secondary where no direct current flows.
pub const OUTPUT_CORE: CoreSpec = CoreSpec {
    henry: 44.0,
    knee: 0.07,
    sharpness: 5.0,
};

const V1: TriodeSpec = TriodeSpec::ECC83;
const V2: TriodeSpec = TriodeSpec::T12AY7;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "line")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Tube 610");

    // --- supplies ---------------------------------------------------------------
    // B+ with C10, then R17 10 k to C2-B, R12 10 k to C1-A, R7 82 k to C1-B.
    net.supply("bplus", 10.0, BPLUS)
        .capacitor("bplus", "gnd", 10e-6) // C10
        .resistor("bplus", "n2", 10_000.0) // R17
        .capacitor("n2", "gnd", 10e-6) // C2-B
        .resistor("n2", "n1", 10_000.0) // R12
        .capacitor("n1", "gnd", 10e-6) // C1-A
        .resistor("n1", "n0", 82_000.0) // R7
        .capacitor("n0", "gnd", 10e-6); // C1-B

    // --- T1, UTC O-1 --------------------------------------------------------------
    // S1 in MIC: R5 47R in series to pin 4, the other leg to pin 3. Secondary 7 to
    // V1-A's grid, 6 to ground, R26 1M2 across it.
    net.input("in", source)
        .resistor("in", "r5", 47.0) // R5
        .resistor("r5", "t1p", INPUT_PRIMARY_R)
        .core("t1p", "gnd", INPUT_CORE)
        .transformer("t1p", "gnd", "t1s", "gnd", INPUT_RATIO)
        .resistor("t1s", "g1", INPUT_SECONDARY_R)
        .capacitor("g1", "gnd", INPUT_SHUNT)
        .resistor("g1", "gnd", 1_200_000.0); // R26

    // --- V1-A and V1-B, with their feedback -----------------------------------------
    net.resistor("k1", "gnd", 4_700.0) // R27
        .resistor("n0", "p1", 270_000.0) // R6
        .triode("p1", "g1", "k1", V1)
        .capacitor("p1", "g2", 0.047e-6) // C3
        .resistor("g2", "gnd", 1_200_000.0) // R28
        .resistor("k2", "gnd", 1_000.0) // R10
        .capacitor("k2", "gnd", 0.002e-6) // C8
        .resistor("n1", "p2", 100_000.0) // R8
        .triode("p2", "g2", "k2", V1)
        .capacitor("p2", "x", 0.047e-6) // C4
        .resistor("x", "y", 68_000.0) // R11
        .resistor("y", "k1", 39_000.0); // R9

    // --- the gain switch at Hi, the LF switch at 0, the Level control --------------
    net.capacitor("x", "level", 0.2e-6) // C14
        .pot("level", "g3", "gnd", 250_000.0, Taper::Audio, LEVEL); // R13

    // --- V2-A and V2-B, with the EQ in their feedback ----------------------------------
    net.resistor("k3", "gnd", 4_700.0) // R15
        .capacitor("k2", "k3", 82e-12) // C15, as drawn
        .resistor("n2", "p3", 470_000.0) // R14
        .triode("p3", "g3", "k3", V2)
        .capacitor("p3", "g4", 0.047e-6) // C5
        .resistor("g4", "gnd", 1_200_000.0) // R16
        .resistor("k4", "gnd", 560.0) // R18
        .triode("p4", "g4", "k4", V2)
        // Feedback: C6 from V2-B's plate, R22 220 k to V2-A's cathode (R21/C9 shorted
        // at LF 0).
        .capacitor("p4", "h", 0.047e-6) // C6
        .resistor("h", "k3", 220_000.0) // R22
        // The HF network, selected by S2 away from 0 and hanging off R22 at 0.
        .resistor("h", "r23", 100_000.0) // R23
        .capacitor("r23", "j", 100e-12) // C10
        .capacitor("j", "kk", 47e-12) // C12
        .capacitor("kk", "l", 0.01e-6) // C16
        .capacitor("l", "m", 0.01e-6) // C11
        .capacitor("l", "m", 0.002e-6) // C17
        .resistor("m", "gnd", 2_200.0) // R24
        // The echo send, always on the plate: C7, R19 82 k, R20 10 k track.
        .capacitor("p4", "c7", 0.047e-6) // C7
        .resistor("c7", "echo", 82_000.0) // R19
        .resistor("echo", "gnd", 10_000.0); // R20

    // --- T2, UTC PA-5946 ----------------------------------------------------------------
    // Single-ended: B+ through the primary to V2-B's plate. The magnetising
    // inductance carries the plate current; the ideal ratio sits across it alone,
    // so no direct voltage reaches the secondary.
    net.resistor("bplus", "t2m", OUTPUT_PRIMARY_R)
        .inductor("t2m", "p4", OUTPUT_PRIMARY_L)
        .transformer("p4", "t2m", "t2s", "gnd", OUTPUT_RATIO)
        .core("t2s", "gnd", OUTPUT_CORE)
        .resistor("t2s", "line", OUTPUT_SECONDARY_R)
        .resistor("line", "gnd", load);

    net.build(at)
}
