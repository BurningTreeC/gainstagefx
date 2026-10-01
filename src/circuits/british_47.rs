//! The British 47: a two-valve microphone amplifier with a 1:7 transformer
//! in, a 7:1 transformer out, and two feedback loops.
//!
//! Engineering reference (developer documentation, not panel text): EMI Line
//! Amplifier Type REDD.47, "cassette type", drawing REDD.47/C1 of 1.10.59 and
//! EMI's description Ref. REDD.M47 (E.M.I. International, Record Engineering
//! Development). Cross-checked against R. Gorbutt's redraw of 2001, which
//! agrees on every topology question below. See `docs/models/british_47.md`.
//!
//! ```text
//! 200 ohm -- T1 1:7 -- V1 EF86 (100 k plate, 390 k screen, 1 k + 120 cathode)
//!   -- R8 330 k -- J -- C4 .02 -- V2 E88CC, both halves (8.2 k plate, 200 ohm)
//!                  \__ R9 1.6 M from V2's anode: the inner loop
//!   V2 -- C5 1 uF -- T2 7:1 -- 2/6, into 200 ohm
//!   T2's primary -- S1's network -- across R5: the outer loop
//! ```
//!
//! EMI's own account of it: "The output stage distortion is still relatively
//! high, being mainly second harmonic, and negative feedback is therefore
//! applied directly from anode to grid ... developed across the resistor R8
//! ... Both stages are therefore generating about the same amount of second
//! harmonic distortion, but as their anodes are in antiphase, this second
//! harmonic tends to cancel". And then the overall loop, from V2's anode to
//! V1's cathode, whose depth is the gain switch: 34, 40 or 46 dB into 200 ohm.

use crate::dsp::netlist::{
    steps, Circuit, CoreSpec, Fault, Netlist, PentodeSpec, Taper, TriodeSpec,
};

/// S1, the gain switch, on the Drive knob's thirds: 34, 40 and 46 dB.
pub const GAIN: usize = 0;

/// S1 as a rheostat between T2's primary and the network's first section:
/// nothing at 34 dB (A shorted to the output), R14's 10 k at 40 dB (B shorted
/// to the output), R14 and R13's 47 k at 46 dB. Its fractions are of the
/// whole 57 k, as resistance *taken out*: `pot(a, wiper, wiper)` leaves
/// `R (1 - f)` in circuit.
pub const S1_TRACK: f64 = 57_000.0;
pub const S1: Taper = steps([1.0, 1.0 - 10_000.0 / S1_TRACK, 0.0]);

/// The fine gain set: VR1 500 with R6 240 across R5, a screwdriver control
/// "to compensate for small changes in gain due to change of valves" (EMI).
/// Set, as EMI sets it, so that the middle position measures 40 dB into
/// 200 ohm (`examples/british_47_op.rs`). EMI gives its limits as 77 to 120
/// ohm for R5 shunted; this is the shunt.
pub const FINE_GAIN_SHUNT: f64 = 659.0;

/// The OA2 pair's regulated rail, 290 V on the drawing, behind the pair's
/// dynamic resistance (about 50 ohm each). APPROXIMATED.
const RAIL: f64 = 290.0;
const RAIL_SOURCE: f64 = 100.0;

/// T1, REDD.A92: 1:7. Its rumble filter (C14 0.75-1.8 uF and R24 3 k on the
/// primary taps, tuned on test to about 28 Hz) is strapped out, as EMI's note
/// says to do it -- "disconnect 2-4 and connect 3-4" -- because the tap wiring
/// cannot be read at the scan's resolution. Primary inductance, copper and
/// knee are ESTIMATED: a 200 ohm winding flat to well below 20 Hz from a
/// microphone, saturating past +10 dBu at 30 Hz.
const T1_RATIO: f64 = 1.0 / 7.0;
const T1_CORE: CoreSpec = CoreSpec {
    henry: 2.0,
    knee: 0.024,
    sharpness: 3.0,
};
const T1_PRIMARY_R: f64 = 10.0;
const T1_SECONDARY_R: f64 = 400.0;

/// T2, REDD.A83 on the drawing: 7:1, capacitor-fed through C5 so it carries
/// no direct current. Inductance, copper and knee ESTIMATED: the primary sees
/// 200 ohm times 49, 9.8 k, so it needs about 100 H to keep its own corner
/// near 16 Hz, and it holds the rated +14 dBm peaks at 30 Hz.
const T2_RATIO: f64 = 7.0;
const T2_CORE: CoreSpec = CoreSpec {
    henry: 100.0,
    knee: 0.25,
    sharpness: 4.0,
};
const T2_PRIMARY_R: f64 = 200.0;
const T2_SECONDARY_R: f64 = 5.0;

const V1: PentodeSpec = PentodeSpec::EF86;
const V2: TriodeSpec = TriodeSpec::E88CC;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    tap_with(source, load, FINE_GAIN_SHUNT, at)
}

/// With the fine gain's shunt across R5 stated, for fitting it.
pub fn tap_with(source: f64, load: f64, fine_gain: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("British 47");

    // The regulated rail; R27 51 k and C1 8 uF to V1's 205 V node.
    net.supply("reg", RAIL_SOURCE, RAIL)
        .resistor("reg", "b1", 51_000.0) // R27
        .capacitor("b1", "gnd", 8e-6); // C1

    // --- T1 -------------------------------------------------------------------------
    // R1 across the secondary through C13: the damping of the transformer's
    // resonance, which C13 takes away at low frequencies (REDD.M47).
    net.input("in", source)
        .resistor("in", "t1p", T1_PRIMARY_R)
        .core("t1p", "gnd", T1_CORE)
        .transformer("t1p", "gnd", "t1s", "gnd", T1_RATIO)
        .resistor("t1s", "g1", T1_SECONDARY_R)
        .resistor("g1", "r1", 100_000.0) // R1
        .capacitor("r1", "gnd", 100e-12); // C13

    // --- V1, EF86 -----------------------------------------------------------------------
    // R4 bypassed by C2; R5 below it unbypassed, with the fine gain across it:
    // the outer loop's voltage is developed there.
    net.resistor("b1", "p1", 100_000.0) // R2
        .resistor("b1", "s1", 390_000.0) // R3
        .capacitor("s1", "gnd", 0.47e-6) // C3
        .pentode("p1", "g1", "k1", "s1", 1.0, V1)
        .resistor("k1", "fb", 1_000.0) // R4
        .capacitor("k1", "fb", 100e-6) // C2
        .resistor("fb", "gnd", 120.0) // R5
        .resistor("fb", "gnd", fine_gain); // R6 + VR1

    // --- the inner loop and V2, E88CC with both halves in parallel ---------------------
    net.resistor("p1", "j", 330_000.0) // R8
        .resistor("p2", "j", 1_600_000.0) // R9
        .capacitor("j", "g2", 0.02e-6) // C4
        .resistor("g2", "gnd", 1_000_000.0) // R10
        .resistor("g2", "g2a", 1_000.0) // R25
        .resistor("g2", "g2b", 1_000.0) // R26
        .resistor("reg", "p2", 8_200.0) // R11
        .triode("p2", "g2a", "k2", V2)
        .triode("p2", "g2b", "k2", V2)
        .resistor("k2", "gnd", 200.0) // R12
        .capacitor("k2", "gnd", 100e-6); // C6

    // --- T2, and the output -----------------------------------------------------------
    // Terminal 2/6, the secondary itself, which is where REDD.M47 measures its
    // gains: "with a 200 ohm load connected between terminals 2/5 and 2/6".
    // R23's 150 ohm build-out leads to 2/13, the other output, and is not in
    // this path.
    net.capacitor("p2", "c5b", 1e-6) // C5
        .resistor("c5b", "t2p", T2_PRIMARY_R)
        .core("t2p", "gnd", T2_CORE)
        .transformer("t2p", "gnd", "t2s", "gnd", T2_RATIO)
        .resistor("t2s", "out", T2_SECONDARY_R)
        .resistor("out", "gnd", load);

    // --- the outer loop, S1 -----------------------------------------------------------------
    // From C5's far side, through S1's rheostat, R15A 6.8 k with R15B 120 k and
    // C8 680 pF across them, to R5. C7's 330 pF across R14 is not built: it
    // cannot follow a rheostat, and its corner is 48 kHz.
    net.pot("c5b", "s1w", "s1w", S1_TRACK, S1, GAIN)
        .resistor("s1w", "fb", 6_800.0) // R15A
        .resistor("s1w", "fb", 120_000.0) // R15B
        .capacitor("s1w", "fb", 680e-12); // C8

    net.build(at)
}
