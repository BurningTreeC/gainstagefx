//! The American VT-40: the 60 W 4x10 valve combo of 1971, from channel one's
//! BRIGHT input to the EXTERNAL AMP jacks that feed its power amplifier.
//!
//! Engineering reference (developer documentation, not panel text): Ampeg
//! VT-40, schematic DWG 06500 revision B (4-71, sheet 12/71-204), with the
//! service section's parts lists, transformer data and A.C. voltage readings
//! (12/71-195 to -198). The matched power stage is
//! `power::PowerSpec::VT40_7027A`. See `docs/models/american_vt40.md`.
//!
//! ```text
//! J1 -- C1 .005 -- V1a (R1 220k; R5 390k / R4 6k8, R3 33k + C2 10u, SENSITIVITY)
//!   -- C3 .01 -- VOL 1 (1 M lin), ULTRA HI across it -- V1b (R7 3k3)
//!   V1b and V2b share R6 68k: the two inputs' mixer
//!   -- C4 .01 -- P.E.C. 6470000 (BASS, TREBLE), R101 120k
//!   -- C201 .01 -- 6K11 unit 3 (R204 220k; R202 560 + R203 7k5) -- C202 .01
//!   -- unit 2 (R207 470k / R206 3k3) -- unit 1 follower (R211 47k + R210 6k8)
//!        |__ R208 56k back to unit 3's cathode ______________|
//!   MIDRANGE between unit 3's cathode and the R211/R210 tap, L101 to ground
//!   -- C205 .1 -- R14 180k -- mix -- R15 270k -- the reverb return (R104 150k)
//!   -- C7 .01 -- V3a, the 12DW7's 12AU7 half, a bootstrapped follower
//!   -- C8 .47 -- R19 10k -- EXT AMP (R20 100k)
//! ```
//!
//! **The SVT's tone section, in one bottle.** The P.E.C. 6470000 is the SVT's
//! James stack (six parts of six, around the same 1 M log pots), and the 6K11's
//! three units are the SVT's V3b, V4a and V4b: a gain stage, a second gain
//! stage and a follower with R208 back to the first's cathode, so the three are
//! one amplifier whose gain the loop sets, and the MIDRANGE pot sits between two
//! points of that loop with a series resonance on its wiper. Ampeg's values are
//! the SVT's but for C203 / C204, .33 uF where the SVT has .68, and a 12AU7
//! unit as the follower.
//!
//! **What is not built.** Channel two's input stage (V2a) -- its volume is at
//! zero, so V2b's grid is at ground and V2b is built only as the load it puts on
//! the shared R6 -- and the reverb's driver and tank. The return network the
//! dry signal has to pass is built, with the REVERB pot at its middle and
//! V203's plate as the impedance behind it, so the dry path is the drawing's;
//! the tank itself is silent (see the log).

use crate::dsp::netlist::{Adjust, Circuit, Fault, Netlist, Taper, TriodeSpec};

/// VR101, 1 M linear, channel one's VOL. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// VR103, 1 M log, BASS.
pub const BASS: usize = 1;
/// VR105, 50 k linear, MIDRANGE.
pub const MIDDLE: usize = 2;
/// VR104, 1 M log, TREBLE.
pub const TREBLE: usize = 3;

/// SW3 ULTRA HI, channel one's pole: C102, 120 pF from VR101's top to its
/// wiper, as an adjustable capacitor -- in at 120 pF, out at nothing.
///
/// The switch is a three-position rocker (DP3T, the parts list). Up bridges
/// C102 across the volume's upper half; the centre leaves C102 and C101 in
/// series from the wiper back to the wiper, which is nothing; down puts C101,
/// .001 uF, from the wiper to ground, a treble cut. The drawing shows it down.
/// The panel offers the first two (`Gain::bright_switch`), and the netlist is
/// built in the centre, flat. C101 is not built. APPROXIMATED.
pub const ULTRA_HI_SLOT: usize = 0;
pub const ULTRA_HI_FARADS: f64 = 120e-12;
pub const ULTRA_HI_OFF_FARADS: f64 = 1.0e-18;

/// The rocker switches, as the contacts they close: each contact is an
/// adjustable resistor, closed at `SWITCH_CLOSED` and open at `SWITCH_OPEN`.
pub const SWITCH_CLOSED: f64 = 1.0;
pub const SWITCH_OPEN: f64 = 1.0e9;
const C: f64 = SWITCH_CLOSED;
const O: f64 = SWITCH_OPEN;

/// SW1 SENSITIVITY, a three-position rocker on V1a's cathode, as its two
/// contacts that carry anything: the cathode to C2 (shorting R3) and C2 to R2's
/// foot (R2 6.8 k across R3). R4's 6.8 k sets the bias in every position.
///
/// - **High**: R3 shorted, the cathode bypassed outright.
/// - **Middle**: R2 across R3, 3.1 k of R4 unbypassed against C2.
/// - **Low**: R3 alone to C2, 5.6 k unbypassed -- the position drawn.
///
/// Built at Middle: V1a turns the A.C. table's 0.3 V at 400 Hz into 9.4 V at
/// its plate there against the table's 10 (-0.5 dB), where Low gives 7.3 V
/// (-2.7 dB) and High 15.6 (+3.9 dB) -- so the table was taken in the middle,
/// whatever the drawing shows (`examples/vt40_op.rs`, `tests/american_vt40.rs`).
/// No panel control.
pub const SENSITIVITY_SLOTS: [usize; 2] = [1, 2];
pub const SENSITIVITY: [[f64; 2]; 3] = [
    [C, O], // High
    [O, C], // Middle, built
    [O, O], // Low, drawn
];
const SENSITIVITY_BUILT: usize = 1;

/// SW5 MIDRANGE SELECT: which section of the toroid VR105's wiper reaches,
/// with which capacitor. 300 Hz, 800 Hz, 3 kHz.
pub const MID_SELECT_SLOTS: [usize; 3] = [3, 4, 5];
pub const MID_SELECT: [[f64; 3]; 3] = [
    [C, O, O], // 1: the whole toroid straight to ground
    [O, C, O], // 2: tap 2 through C107 .15
    [O, O, C], // 3: tap 3 through C106 .033
];

/// L101's three sections, which the drawing does not give (toroid 8910001, not
/// the SVT's 320821-1). ESTIMATED, each from the VT-40's WIDELY REPORTED
/// midrange frequencies, "+/-20 dB at 300, 800 or 3,000 Hz", and the
/// capacitance in series with it -- C203's .33 alone, then with C107's .15,
/// then with C106's .033 -- so tap 1 resonates at 300 Hz, tap 2 at 800 Hz and
/// tap 3 at 3 kHz. Built as three inductors, as the SVT's are.
pub const L101_TAP1: f64 = 0.8529;
pub const L101_TAP2: f64 = 0.3838;
pub const L101_TAP3: f64 = 0.0938;

/// The 393 V node (circled on the drawing), which R40 2.2 k drops to the
/// preamplifier's 354 V rail across C16's first section. Stiff here: what
/// feeds it is the power stage's, and the drawing's own figures for the chain
/// above it do not add up (see the log).
pub const RAIL_UPSTREAM: f64 = 393.0;
pub const RAIL_DROPPER: f64 = 2_200.0;
pub const RAIL_RESERVOIR: f64 = 40e-6;

/// What else hangs on the 354 V rail, as one resistor: V2a (0.38 mA through
/// R12 390 k to its 205 V plate), V3b (0.60 mA through R22 220 k to 223 V, in
/// the power stage here), V202's first unit (1.93 mA through R213 120 k to
/// 122 V) and V203 (0.55 mA through R221 270 k to 205 V), all from the drawing's
/// voltages: 3.46 mA. With the built stages' draw R40 then drops the 393 V node
/// to the drawing's 354 V. DERIVED.
const UNBUILT_DRAW: f64 = 354.0 / 3.46e-3;

/// V203's plate as the reverb pot sees it: R221's 270 k against a 12AX7's plate
/// resistance near 75 k at the 0.55 mA the drawing gives it. APPROXIMATED.
const RECOVERY_SOURCE: f64 = 59_000.0;

/// Where the REVERB pot rests, since the tank is not built: the middle, where
/// the A.C. voltage readings were taken.
const REVERB_REST: f64 = 0.5;

const ECC83: TriodeSpec = TriodeSpec::ECC83;
const ECC82: TriodeSpec = TriodeSpec::ECC82;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("American VT-40");

    net.supply("bp", RAIL_DROPPER, RAIL_UPSTREAM) // R40 from the 393 V node
        .capacitor("bp", "gnd", RAIL_RESERVOIR) // C16
        .resistor("bp", "gnd", UNBUILT_DRAW);

    // --- V1a, with SENSITIVITY on its cathode ------------------------------------
    net.input("in", source)
        .capacitor("in", "g1", 0.005e-6) // C1
        .resistor("g1", "gnd", 220_000.0) // R1
        .resistor("bp", "p1", 390_000.0) // R5
        .triode("p1", "g1", "k1", ECC83)
        .resistor("k1", "gnd", 6_800.0) // R4
        .resistor("k1", "k1c", 33_000.0) // R3
        .capacitor("k1c", "gnd", 10e-6) // C2
        .resistor("k1", "r2b", 6_800.0); // R2

    // --- VOL 1 and ULTRA HI ----------------------------------------------------------
    // `pot(a, wiper, b)` puts `R f(p)` between the wiper and `b`: turned up the
    // wiper is at C3's end.
    net.capacitor("p1", "vol", 0.01e-6) // C3
        .pot("vol", "g1b", "gnd", 1_000_000.0, Taper::Linear, VOLUME); // VR101
    let ultra_hi = net.adjustable("vol", "g1b", Adjust::Capacitor, ULTRA_HI_FARADS);
    debug_assert_eq!(ultra_hi, ULTRA_HI_SLOT);
    let built = SENSITIVITY[SENSITIVITY_BUILT];
    for ((a, b), (slot, value)) in [("k1", "k1c"), ("k1c", "r2b")]
        .into_iter()
        .zip(SENSITIVITY_SLOTS.into_iter().zip(built))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }

    // --- the mixer: V1b and V2b on one plate resistor ----------------------------
    // Channel two's volume at zero puts V2b's grid at ground; it still draws its
    // share of R6 and loads the plate.
    net.resistor("bp", "pm", 68_000.0) // R6
        .triode("pm", "g1b", "k1b", ECC83)
        .resistor("k1b", "gnd", 3_300.0) // R7
        .triode("pm", "gnd", "k2b", ECC83)
        .resistor("k2b", "gnd", 3_300.0) // R13
        .capacitor("pm", "s1", 0.01e-6); // C4

    // --- the P.E.C. 6470000 ---------------------------------------------------------
    super::american_svt::james_stack(&mut net, "s1", "x1", Some((BASS, TREBLE)), "1");

    // --- the 6K11 and its loop ---------------------------------------------------------
    // Unit 3 (pins 11 / 2 / 3), unit 2 (7 / 5 / 6), unit 1 the follower
    // (9 / 10 / 4): the medium-mu unit, by RCA's and GE's sheets the 12AU7.
    net.capacitor("x1", "g3", 0.01e-6) // C201
        .resistor("g3", "k3j", 1_000_000.0) // R201
        .resistor("bp", "p3", 220_000.0) // R204
        .triode("p3", "g3", "k3", ECC83)
        .resistor("k3", "k3j", 560.0) // R202
        .resistor("k3j", "gnd", 7_500.0) // R203
        .capacitor("p3", "g2", 0.01e-6) // C202
        .resistor("g2", "gnd", 1_000_000.0) // R205
        .resistor("bp", "p2", 470_000.0) // R207
        .triode("p2", "g2", "k2", ECC83)
        .resistor("k2", "gnd", 3_300.0) // R206
        .triode("bp", "p2", "kf", ECC82)
        .resistor("kf", "cf", 47_000.0) // R211
        .resistor("cf", "gnd", 6_800.0) // R210
        .resistor("kf", "k3", 56_000.0); // R208

    // --- MIDRANGE --------------------------------------------------------------------
    // As the SVT's: turned up the wiper is at unit 3's cathode end, where the
    // resonance shorts the loop's return and the band rises.
    net.resistor("k3", "r209", 470.0) // R209
        .capacitor("r209", "mid_l", 0.33e-6) // C203
        .resistor("cf", "r212", 620.0) // R212
        .capacitor("r212", "mid_r", 0.33e-6) // C204
        .pot("mid_l", "mid_w", "mid_r", 50_000.0, Taper::Linear, MIDDLE); // VR105
    net.inductor("ml", "gnd", L101_TAP1)
        .inductor("mc", "l1_2", L101_TAP2)
        .capacitor("l1_2", "gnd", 0.15e-6) // C107
        .resistor("l1_2", "gnd", 220_000.0) // R103
        .inductor("mh", "l1_3", L101_TAP3)
        .capacitor("l1_3", "gnd", 0.033e-6) // C106
        .resistor("l1_3", "gnd", 220_000.0); // R102
    let centre = MID_SELECT[1];
    for (to, (slot, value)) in ["ml", "mc", "mh"]
        .into_iter()
        .zip(MID_SELECT_SLOTS.into_iter().zip(centre))
    {
        let made = net.adjustable("mid_w", to, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }

    // --- the mix with the reverb return ------------------------------------------------
    // The dry signal reaches the mix through R14 and returns through R15 to the
    // return's node, where R104 and the REVERB pot (C212 on its wiper, C105 off
    // its top, the bottom grounded) hang. Built as the pot's two halves at its
    // rest, the recovery stage as its plate impedance.
    let below = REVERB_REST * 500_000.0;
    net.capacitor("cf", "c205", 0.1e-6) // C205
        .resistor("c205", "mix", 180_000.0) // R14
        .resistor("mix", "rev", 270_000.0) // R15
        .resistor("rev", "gnd", 150_000.0) // R104
        .capacitor("rev", "rv_t", 0.0022e-6) // C105
        .resistor("rv_t", "rv_w", 500_000.0 - below) // VR106
        .resistor("rv_w", "gnd", below)
        .capacitor("rv_w", "rv_p", 0.005e-6) // C212
        .resistor("rv_p", "gnd", RECOVERY_SOURCE);

    // --- V3a, the 12DW7's medium-mu half: a bootstrapped follower ---------------------
    // Tung-Sol's 12DW7 sheet: section 2 (pins 1 / 2 / 3) is the 12AU7's figure
    // for figure, section 1 (6 / 7 / 8, V3b in the power stage) the 12AX7's.
    net.capacitor("mix", "g3a", 0.01e-6) // C7
        .resistor("g3a", "k3aj", 1_000_000.0) // R16
        .triode("bp", "g3a", "k3a", ECC82)
        .resistor("k3a", "k3aj", 1_000.0) // R17
        .resistor("k3aj", "gnd", 47_000.0) // R18
        .capacitor("k3a", "c8", 0.47e-6) // C8
        .resistor("c8", "out", 10_000.0) // R19
        .resistor("out", "gnd", 100_000.0) // R20
        .resistor("out", "gnd", load);

    net.build(at)
}
