//! The American 800RB: an op-amp input stage with three voicing switches, a
//! volume, a mid contour, a four-band active EQ, an effects loop, and a JFET
//! boost into the crossover and the masters.
//!
//! Engineering reference (developer documentation, not panel text): the
//! Gallien-Krueger 800RB, preamp board 206-0045, drawing 406-0045-C "NEW 800RB
//! PREAMP" of 8/13/91, from GK's own service manual (635 ppi). See
//! `docs/models/american_800rb.md`.
//!
//! ```text
//! in -- U1A x11 (S1 -10 dB, S2 LO CUT in its ground leg) -- VOLUME, S4 HI BOOST
//!    -- S3 MID CONTOUR (U1B's network, or around it) -- U2A x5.7
//!    -- HIGH MID (U2B) -- LOW MID (U3A) -- BASS / TREBLE (U3B)
//!    -- SEND / RETURN (the direct out leaves here) -- BOOST -- Q89 J113
//!    -- the crossover's input and both masters
//! ```
//!
//! The circuit stops at the masters: the LO master is the American SS 800's
//! input (`circuits::american_ss800`). In FULL mode the crossover's outputs go
//! nowhere, but its input is still on the bus, so it is built for its load.

use crate::dsp::netlist::{Adjust, Circuit, Fault, JfetSpec, Netlist, Taper};

/// R17 VOLUME, 50 k A. The Drive knob.
pub const VOLUME: usize = 0;
/// R68 BASS, 50 k B, in U3B's Baxandall.
pub const BASS: usize = 1;
/// R64 LOW MID, 50 k B, U3A's band (250 Hz on the manual).
pub const LO_MID: usize = 2;
/// R53 HIGH MID, 50 k B, U2B's band (1 kHz on the manual).
pub const HI_MID: usize = 3;
/// R36 TREBLE, 50 k B, in U3B's Baxandall.
pub const TREBLE: usize = 4;
/// R82 BOOST, 50 k B, the J113's input level: the Master knob, as the
/// circuit's own level control.
pub const BOOST: usize = 5;
/// R100 FREQ, the crossover's dual 50 k B. Not on the panel: in FULL mode it
/// only moves the crossover's input loading.
pub const FREQ: usize = 6;

/// Where the boost rests when nothing turns it: the middle, the drawing's
/// test runs it at 10 and the manual calls it a preset volume.
pub const BOOST_REST: f64 = 0.5;

/// The direct out's node: U4's input at the return jack, "after the effects
/// loop" and ahead of the boost, the masters and the crossover. The plugin's
/// Preamp DI reads it here rather than at the circuit's output
/// (`Gain::direct_out`).
pub const DIRECT_OUT: &str = "ret";

/// The switches' contacts: adjustable resistors, closed and open.
pub const SWITCH_CLOSED: f64 = 1.0;
pub const SWITCH_OPEN: f64 = 1.0e9;
const C: f64 = SWITCH_CLOSED;
const O: f64 = SWITCH_OPEN;

/// S1, -10 dB: R3 4.7 k across R5 when pressed.
pub const PAD_SLOT: usize = 0;
/// S2, LO CUT: shorts C25 in U1A's ground leg when out. In the panel's order,
/// flat second: Lo Cut, Flat.
pub const LO_CUT_SLOTS: [usize; 1] = [1];
pub const LO_CUT: [[f64; 1]; 2] = [[O], [C]];
/// S3, MID CONTOUR: U2A's input from the volume (out) or from U1B's network
/// (in). Contour, Flat.
pub const CONTOUR_SLOTS: [usize; 2] = [2, 3];
pub const CONTOUR: [[f64; 2]; 2] = [[O, C], [C, O]];
/// S4, HI BOOST: shorts R23 so C26 and R28 bridge the volume.
pub const HI_BOOST_SLOT: usize = 4;

/// The LF353s' swing on the +-14.5 V regulated rails. ESTIMATED.
const RAIL: f64 = 13.5;
/// The regulated rails, as the turn-on procedure checks them.
const RAIL_VOLTS: f64 = 14.5;

/// Q89, a J113, inside its data sheet's ranges (IDSS at least 2 mA, VGS(off)
/// -0.5 to -3 V) and fitted to the drawing's own figures: 93 mV at the boost's
/// top, 1.1 V at the drain, x11.8, needs about 5 mS at its 1.25 mA. ESTIMATED.
/// The drawing's 6 V at the drain is not reachable by a J113 within those
/// ranges (it would need VGS -3.4 V behind R88); this one sits at 7.4 V.
pub const J113: JfetSpec = JfetSpec {
    idss: 5.0e-3,
    pinch_off: -1.0,
};

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "bus")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("American 800RB");

    net.supply("vp", 1.0, RAIL_VOLTS)
        .supply("vn", 1.0, -RAIL_VOLTS);

    // --- U1A, its three switches, VOLUME ------------------------------------------------
    // D8 / D9 (1N759, 12 V) clamp the input only past twelve volts; not built.
    net.input("in", source)
        .resistor("in", "n1", 12_000.0) // R1
        .capacitor("n1", "p1", 0.1e-6) // C2
        .capacitor("p1", "gnd", 100e-12) // C6
        .resistor("p1", "gnd", 1_000_000.0) // R4
        .opamp("u1a", "p1", "m1", RAIL) // U1A
        .resistor("u1a", "m1", 33_000.0) // R5
        .capacitor("u1a", "m1", 560e-12) // C7
        .resistor("u1a", "s1", 4_700.0) // R3
        .capacitor("m1", "l1", 10e-6) // C11
        .capacitor("l1", "l2", 0.22e-6) // C25
        .resistor("l2", "gnd", 3_300.0) // R24
        .capacitor("u1a", "hb1", 10e-9) // C26
        .resistor("hb1", "hb2", 470_000.0) // R23
        .resistor("hb2", "vw", 5_600.0) // R28
        .pot("u1a", "vw", "gnd", 50_000.0, Taper::Audio, VOLUME) // R17
        .capacitor("vw", "v", 3.3e-6); // C21
    let pad = net.adjustable("s1", "m1", Adjust::Resistor, O); // S1
    debug_assert_eq!(pad, PAD_SLOT);
    let lo_cut = net.adjustable("l1", "l2", Adjust::Resistor, LO_CUT[1][0]); // S2
    debug_assert_eq!(lo_cut, LO_CUT_SLOTS[0]);

    // --- MID CONTOUR: U1B and S3 ----------------------------------------------------------
    net.resistor("v", "cp", 12_000.0) // R12
        .resistor("cp", "cq", 120_000.0) // R13
        .capacitor("cp", "cr", 4.7e-9) // C14
        .capacitor("cr", "cq", 4.7e-9) // C15
        .resistor("cr", "co", 33_000.0) // R16
        .resistor("co", "cq", 220_000.0) // R19
        .capacitor("co", "cq", 560e-12) // C20
        .opamp("co", "gnd", "cq", RAIL) // U1B
        .capacitor("co", "ct", 0.22e-6) // C23
        .resistor("ct", "gnd", 220_000.0) // R22
        .resistor("u2p", "gnd", 33_000.0); // R27
    for ((a, b), (slot, value)) in [("v", "u2p"), ("ct", "u2p")]
        .into_iter()
        .zip(CONTOUR_SLOTS.into_iter().zip(CONTOUR[1]))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }
    let hi_boost = net.adjustable("hb1", "hb2", Adjust::Resistor, O); // S4
    debug_assert_eq!(hi_boost, HI_BOOST_SLOT);

    // --- U2A, and the four bands -------------------------------------------------------------
    // Each band is a unity inverting stage with a band in its loop: 12 k in,
    // 47 k and the pot, a capacitor across them, a second from the wiper to
    // -in. Turned up, the wiper is at the input side: `pot(a, wiper, b)` puts
    // `R f(p)` between the wiper and `b`.
    net.opamp("u2a", "u2p", "m2a", RAIL) // U2A
        .resistor("u2a", "m2a", 22_000.0) // R32
        .capacitor("u2a", "m2a", 100e-12) // C31
        .resistor("m2a", "gnd", 4_700.0) // R30
        // HIGH MID, U2B
        .resistor("u2a", "h", 470_000.0) // R33
        .opamp("o3", "gnd", "h", RAIL) // U2B
        .resistor("o3", "h", 470_000.0) // R38
        .resistor("u2a", "ha", 12_000.0) // R35
        .resistor("ha", "hpa", 47_000.0) // R41
        .pot("hpa", "hw", "hb", 50_000.0, Taper::Linear, HI_MID) // R53
        .resistor("hb", "o3", 12_000.0) // R39
        .capacitor("ha", "hb", 2.2e-9) // C37
        .capacitor("hw", "h", 2.2e-9) // C40
        // LOW MID, U3A
        .capacitor("o3", "lin", 0.1e-6) // C42
        .resistor("lin", "l", 470_000.0) // R44
        .opamp("o4", "gnd", "l", RAIL) // U3A
        .resistor("o4", "l", 470_000.0) // R50
        .capacitor("o4", "l", 47e-12) // C49
        .resistor("lin", "la", 12_000.0) // R43
        .resistor("la", "lpa", 47_000.0) // R48
        .pot("lpa", "lw", "lb", 50_000.0, Taper::Linear, LO_MID) // R64
        .resistor("lb", "o4", 12_000.0) // R47
        .capacitor("la", "lb", 10e-9) // C46
        .capacitor("lw", "l", 10e-9) // C54
        // BASS and TREBLE, U3B
        .resistor("o4", "ba", 5_600.0) // R51
        .resistor("ba", "bpa", 22_000.0) // R60
        .pot("bpa", "bw", "bb", 50_000.0, Taper::Linear, BASS) // R68
        .resistor("bb", "o5", 5_600.0) // R59
        .capacitor("ba", "bb", 0.1e-6) // C62
        .resistor("bw", "b", 47_000.0) // R57
        .resistor("o4", "ta", 5_600.0) // R45
        .pot("ta", "tw", "tb", 50_000.0, Taper::Linear, TREBLE) // R36
        .resistor("tb", "o5", 5_600.0) // R58
        .capacitor("tw", "b", 2.2e-9) // C56
        .opamp("o5", "gnd", "b", RAIL); // U3B

    // --- the loop and the boost ------------------------------------------------------------------
    // SEND straight to RETURN through the return jack's switch; the direct out
    // (U4) is taken here and loads nothing. U5 holds the boost on: its wiper
    // reaches the gate.
    net.capacitor("o5", "c61", 0.22e-6) // C61
        .resistor("c61", "ret", 4_700.0) // R63
        .capacitor("ret", "c70", 0.1e-6) // C70
        .resistor("c70", "bi", 12_000.0) // R73
        .rest(BOOST, BOOST_REST)
        .pot("bi", "g", "bl", 50_000.0, Taper::Linear, BOOST) // R82
        .resistor("bl", "gnd", 5_600.0) // R81
        .resistor("g", "gnd", 220_000.0) // R42
        .jfet("jd", "g", "js", J113) // Q89
        .resistor("jd", "jdd", 4_700.0) // R91
        .resistor("jdd", "vp", 1_000.0) // R90
        .capacitor("jdd", "gnd", 10e-6) // C32
        .resistor("js", "vn", 12_000.0) // R88
        .capacitor("js", "js2", 10e-6) // C93
        .resistor("js2", "gnd", 100.0) // R94
        .capacitor("jd", "bus", 0.22e-6); // C95

    // --- what the bus drives in FULL mode -------------------------------------------------------
    // Both masters' tracks, and the crossover's input: a state-variable filter
    // whose difference stage U6A takes R101 and the band-pass (R106) on +in
    // and the low-pass (R108) on -in, which is all that reaches the bus while
    // S5 is on FULL.
    net.resistor("bus", "gnd", 50_000.0) // R118, LO master
        .resistor("bus", "gnd", 50_000.0) // R115, HI master
        .resistor("bus", "x6p", 22_000.0) // R101
        .linear_opamp("x6o", "x6p", "x6m") // U6A
        .resistor("x6o", "x6m", 22_000.0) // R97
        .rest(FREQ, 0.5)
        .pot("x6o", "xf1", "xf1b", 50_000.0, Taper::Linear, FREQ) // R100a
        .resistor("xf1b", "gnd", 4_700.0) // R98
        .resistor("xf1", "x6bm", 33_000.0) // R103
        .linear_opamp("x6b", "gnd", "x6bm") // U6B
        .capacitor("x6b", "x6bm", 4.7e-9) // C104
        .pot("x6b", "xf2", "xf2b", 50_000.0, Taper::Linear, FREQ) // R100b
        .resistor("xf2b", "gnd", 4_700.0) // R96
        .resistor("xf2", "x7m", 33_000.0) // R111
        .linear_opamp("x7", "gnd", "x7m") // U7
        .capacitor("x7", "x7m", 4.7e-9) // C113
        .resistor("x6b", "x6p", 22_000.0) // R106
        .resistor("x7", "x6m", 22_000.0) // R108
        .resistor("bus", "gnd", load);

    net.build(at)
}
