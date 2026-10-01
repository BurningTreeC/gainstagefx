//! The German 76: two two-valve amplifiers in cascade between a 1:30 and a
//! 9:1 transformer, with a twelve-step gain switch that pads the input or
//! changes the first amplifier's loop.
//!
//! Engineering reference (developer documentation, not panel text): the V76
//! microphone amplifier of Tonographie Apparatebau (TAB), developed at the
//! Institut für Rundfunktechnik, from the IRT's own drawing S 1176 and its
//! Braunbuch-Beschreibung V 76, Ausgabe 1, 16.1.1959. The V76/80 version. See
//! `docs/models/german_76.md`.
//!
//! ```text
//! 200 ohm -- pads (one a leg) -- 44 200 R across -- 85 1:30, the 40 Hz network
//!   between its primary halves -- V1 EF804S -- V2 EF804S on a 400 H choke
//!        \__ V2's plate -- 53 40 k -- the gain switch's string -- V1's cathode
//!   -- 66, 67 40 k ("3 kHz" off their junction) -- the low cut -- 15 kHz LC
//!   -- V3 EF804S -- V4 E83F on a 250 H choke -- 89 9:1 -- 300 ohm
//!        \__ V4's plate -- 75 80 k + 31 -- V3's cathode
//! ```
//!
//! The IRT's own account of the gain switch: from 76 to 34 dB it changes the
//! first amplifier's loop in 6 dB steps; "in the four further positions the
//! gain is reduced to 24, 18, 9 and 3 dB by switching in the attenuation ahead
//! of the input transformer, the loop keeping the value of the 34 dB or 40 dB
//! position" -- so the microphone's own noise sets the noise at every setting.

use crate::dsp::netlist::{steps, Adjust, Circuit, CoreSpec, Fault, Netlist, PentodeSpec, Taper};

/// The gain switch, on the Drive knob's twelve steps: 3, 9, 18, 24, 34, 40,
/// 46, 52, 58, 64, 70 and 76 dB.
pub const GAIN: usize = 0;

/// The pad decks, one in each leg: 43 / 38 3.4 k at 3 dB, 42 / 39 1.6 k at
/// 9 dB, 41 / 40 450 R at 18 dB *and* 24 dB, nothing above. A rheostat of
/// `PAD_TRACK` that leaves `R (1 - f)` in circuit.
pub const PAD_TRACK: f64 = 3_400.0;
pub const PAD: Taper = steps([
    0.0,
    1.0 - 1_600.0 / PAD_TRACK,
    1.0 - 450.0 / PAD_TRACK,
    1.0 - 450.0 / PAD_TRACK,
    1.0,
    1.0,
    1.0,
    1.0,
    1.0,
    1.0,
    1.0,
    1.0,
]);

/// The third deck: 44 200 R across the input in the first four positions,
/// nothing in the rest -- as a rheostat of `TERMINATION_TRACK`, a megohm
/// across a primary of a few kilohms where the drawing has an open contact.
pub const TERMINATION_TRACK: f64 = 1_000_000.0;
pub const TERMINATION: Taper = steps([
    1.0 - 200.0 / TERMINATION_TRACK,
    1.0 - 200.0 / TERMINATION_TRACK,
    1.0 - 200.0 / TERMINATION_TRACK,
    1.0 - 200.0 / TERMINATION_TRACK,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
]);

/// The loop deck: the string 52 19 k, 51 6.7 k, 50 2.4 k, 49 1 k, 48 470 R,
/// 47 224 R, 46 108 R, 45 98 R from its top to 0 V, 30 k in all, and the
/// wiper's tap as the fraction below it. The first three positions take the
/// top (the 34 dB position's loop), the fourth the 19 k / 6.7 k junction (the
/// 40 dB position's), and from the fifth one tap each, down the string.
pub const STRING: f64 = 30_000.0;
pub const LOOP: Taper = steps([
    1.0,
    1.0,
    1.0,
    11_000.0 / STRING,
    1.0,
    11_000.0 / STRING,
    4_300.0 / STRING,
    1_900.0 / STRING,
    900.0 / STRING,
    430.0 / STRING,
    206.0 / STRING,
    98.0 / STRING,
]);

/// The switches' contacts: adjustable resistors, closed and open.
pub const SWITCH_CLOSED: f64 = 1.0;
pub const SWITCH_OPEN: f64 = 1.0e9;
const C: f64 = SWITCH_CLOSED;
const O: f64 = SWITCH_OPEN;

/// 93, the low cut, as its three contacts that matter: the choke 87 onto
/// 21 / 22's junction, the output to 22 / 24's junction (24 out), and the
/// output to 23 35 nF. In the panel's order, flat second: 80 Hz, flat,
/// 300 Hz, 80 + 300 Hz.
pub const LOW_CUT_SLOTS: [usize; 3] = [0, 1, 2];
pub const LOW_CUT: [[f64; 3]; 4] = [
    [C, C, O], // 80 Hz: 21, 87 to 0 V, 22
    [O, O, C], // flat ("Gerade"): through 23
    [O, O, O], // 300 Hz: 21, 22 and 24 in series
    [C, O, O], // 80 + 300 Hz
];

/// 92, "Gerade / 3 kHz": 20 1 nF and 68 25 k off 66 / 67's junction. In the
/// panel's order: 3 kHz, flat.
pub const TREBLE_CUT_SLOTS: [usize; 1] = [3];
pub const TREBLE_CUT: [[f64; 1]; 2] = [[C], [O]];

/// What the drawing leaves to the bench, and what this model estimates in its
/// place, each set by `examples/german_76_op.rs`.
#[derive(Clone, Copy, Debug)]
pub struct Fit {
    /// 75, the second amplifier's loop resistor, drawn as 80 k "abgegl." --
    /// the Braunbuch's gain trim: "Abgleich der Grundverstärkung ... an Pos.
    /// 72 oder Pos. 75". Selected as it was on the bench, so position 12 is
    /// 76 dB from 200 ohm into 300 ohm, referred to the generator.
    pub r75: f64,
    /// The rail's open-circuit voltage, for the drawing's 300 V at the bridge.
    pub rail_open: f64,
    /// 85's knee, as flux linkage on the secondary. ESTIMATED: the
    /// Braunbuch allows ten times the 1 kHz distortion at 40 Hz with 34 dB of
    /// gain and +22 dB out (1.5 % k3 against 0.2 %), which is the input
    /// transformer at its largest flux; set so k3 there is 1 %.
    pub t1_knee: f64,
}

impl Fit {
    pub const SET: Fit = Fit {
        r75: 88_754.0,
        rail_open: 308.6,
        t1_knee: 0.1506,
    };
}

/// 85, BV Nr. 511, 1:30, each primary half a sixtieth of the secondary: a
/// "Scheibenübertrager mit stark nickelhaltigem Kernblech". Its inductance is
/// not printed. The Braunbuch says what it does: with the parts between its
/// primary halves it forms the 40 Hz high-pass, "so dimensioned that the
/// flat range passes into the low cut evenly, without a resonant rise".
/// ESTIMATED from that and the acceptance sheet together -- 2.5 H primary
/// with its core's loss as 3 k across it (both referred to the primary; the
/// core and the loss are built on the secondary). Nothing lossless meets the
/// sheet: the network's second capacitor and resistor put a zero at 265 Hz,
/// and without a loss across the winding the resonance rises 1.5 dB at
/// 60 Hz whatever the inductance. With it, every point of the sheet is met
/// from 2.25 to 2.75 H and 2.5 to 4 k, and the input impedance with them.
/// Copper ESTIMATED. The secondary's capacitance ESTIMATED between two of the
/// sheet's figures: reflected through 1:30 it is the generator's only load at
/// the top, so flat to 15 kHz within 0.5 dB holds it under about 17 pF, and
/// the 20 dB the sheet wants at 40 kHz wants as much of it as that allows --
/// a disc winding is built for little of it.
const T1_HALF: f64 = 1.0 / 60.0;
const T1_PRIMARY_L: f64 = 2.5;
const T1_CORE_LOSS: f64 = 3_000.0;
const T1_PRIMARY_R: f64 = 15.0;
const T1_SECONDARY_R: f64 = 2_000.0;
const T1_SECONDARY_C: f64 = 15e-12;

/// 89, BV Nr. 512, 9:1, fed through 35 so it carries no direct current.
/// Inductance, copper and knee ESTIMATED. The inductance has a floor in the
/// sheet: at +22 dB out the E83F swings 88 V rms, and at 40 Hz 50 H would
/// draw a 10 mA peak of magnetising current from a valve idling at 11.7 mA,
/// cutting it off each cycle (0.7 % k2 against 0.3). 150 H draws 3.3 mA and
/// gives 0.2 %. Its knee sits well clear of that flux.
const T2_RATIO: f64 = 9.0;
const T2_CORE: CoreSpec = CoreSpec {
    henry: 150.0,
    knee: 1.5,
    sharpness: 4.0,
};
const T2_PRIMARY_R: f64 = 500.0;
const T2_SECONDARY_R: f64 = 6.0;

/// The chokes. 86 (400 H) and 88 (250 H): their copper DERIVED from the
/// drawing's voltages -- 86 drops B1 to V2's 175 V at 3.6 mA, 88 drops B2 to
/// V4's 170 V at 11.7 mA. 87 (100 H) and 84 (0.85 H): copper ESTIMATED.
const L86: f64 = 400.0;
const R86: f64 = 18_600.0;
const L88: f64 = 250.0;
const R88: f64 = 9_300.0;
const L87: f64 = 100.0;
const R87: f64 = 5_000.0;
const L84: f64 = 0.85;
const R84: f64 = 300.0;

/// The B450 C80 bridge's 300 V at 21 mA: an open-circuit voltage
/// (`Fit::rail_open`) behind an ESTIMATED 400 ohm.
pub const RAIL_SOURCE: f64 = 400.0;

const EF804S: PentodeSpec = PentodeSpec::EF804S;
const E83F: PentodeSpec = PentodeSpec::E83F;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    tap_with(source, load, Fit::SET, at)
}

/// With the fitted values stated, for fitting them.
pub fn tap_with(source: f64, load: f64, fit: Fit, at: &str) -> Result<Circuit, Fault> {
    let t1_core = CoreSpec {
        henry: T1_PRIMARY_L * 900.0,
        knee: fit.t1_knee,
        sharpness: 3.0,
    };
    let mut net = Netlist::new("German 76");

    // --- the supply: 36, then 81 to B2 (32, 33), then 76 to B1 (29) ----------
    net.supply("b0", RAIL_SOURCE, fit.rail_open)
        .capacitor("b0", "gnd", 32e-6) // C36
        .resistor("b0", "b2", 1_000.0) // R81
        .capacitor("b2", "gnd", 64e-6) // C32, C33
        .resistor("b2", "b1", 6_000.0) // R76
        .capacitor("b1", "gnd", 32e-6); // C29

    // --- the input: pads, termination, 85 and the 40 Hz network -------------------
    // One leg from the source, the other from its return: the input is
    // floating, and the pads are in both.
    net.input("in", source)
        .pot("in", "ia", "ia", PAD_TRACK, PAD, GAIN) // R43
        .pot("gnd", "ib", "ib", PAD_TRACK, PAD, GAIN) // R38
        .pot("ia", "ib", "ib", TERMINATION_TRACK, TERMINATION, GAIN) // R44
        .resistor("ia", "t1a", T1_PRIMARY_R)
        .transformer("t1a", "x1", "t1s", "t1r", T1_HALF)
        .capacitor("x1", "x2", 8e-6) // C5-C8
        .capacitor("x2", "x3", 2e-6) // C9
        .resistor("x2", "x3", 300.0) // R54, "abgeglichen"
        .transformer("x3", "t1b", "t1s", "t1r", T1_HALF)
        .resistor("t1b", "ib", T1_PRIMARY_R)
        .core("t1s", "t1r", t1_core)
        .resistor("t1s", "t1r", T1_CORE_LOSS * 900.0)
        .resistor("t1s", "g1", T1_SECONDARY_R)
        .capacitor("g1", "t1r", T1_SECONDARY_C)
        .capacitor("t1r", "gnd", 10e-6) // C12
        .resistor("t1r", "k1j", 100_000.0); // R55

    // --- V1 and V2, and the gain switch's loop ----------------------------------------
    net.pentode("p1", "g1", "k1", "s1", 1.0, EF804S)
        .resistor("k1", "k1j", 3_000.0) // R57
        .resistor("k1j", "gnd", 12_000.0) // R56
        .resistor("b1", "s1", 1_000_000.0) // R58
        .capacitor("s1", "gnd", 50e-9) // C14
        .resistor("b1", "d1", 100_000.0) // R60
        .capacitor("d1", "gnd", 4e-6) // C18
        .resistor("d1", "p1", 200_000.0) // R59
        .capacitor("p1", "g2", 25e-9) // C17
        .resistor("g2", "gnd", 1_000_000.0) // R61
        .pentode("p2", "g2", "k2", "s2", 1.0, EF804S)
        .resistor("k2", "gnd", 500.0) // R62, unbypassed
        .resistor("b1", "s2", 100_000.0) // R63
        .capacitor("s2", "gnd", 2e-6) // C19
        .resistor("b1", "c86", R86)
        .inductor("c86", "p2", L86) // 86
        .resistor("p2", "f53", 40_000.0) // R53
        .capacitor("f53", "fbt", 2e-6) // C15
        .pot("fbt", "fbw", "gnd", STRING, LOOP, GAIN) // R52-R45 and the wiper
        .capacitor("fbw", "k1", 50e-6); // C13

    // --- the interstage: 66, 67, "3 kHz", the low cut, the 15 kHz low-pass ---------------
    net.resistor("p2", "m66", 40_000.0) // R66
        .resistor("m66", "n93", 40_000.0) // R67
        .capacitor("x20", "x68", 1e-9) // C20
        .resistor("x68", "gnd", 25_000.0) // R68
        .capacitor("n93", "a93", 25e-9) // C21
        .capacitor("a93", "b93", 25e-9) // C22
        .capacitor("b93", "p93", 4e-9) // C24
        .capacitor("n93", "c23", 35e-9) // C23
        .resistor("c87", "r87", R87)
        .inductor("r87", "gnd", L87) // 87
        .capacitor("p93", "gnd", 100e-12) // C25
        .resistor("p93", "c84", R84)
        .inductor("c84", "g3", L84) // 84
        .capacitor("g3", "gnd", 100e-12); // C26
    let flat = LOW_CUT[1];
    for ((a, b), (slot, value)) in [("a93", "c87"), ("p93", "b93"), ("p93", "c23")]
        .into_iter()
        .zip(LOW_CUT_SLOTS.into_iter().zip(flat))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }
    let made = net.adjustable("m66", "x20", Adjust::Resistor, TREBLE_CUT[1][0]);
    debug_assert_eq!(made, TREBLE_CUT_SLOTS[0]);

    // --- V3 and V4, and their loop -------------------------------------------------------------
    net.resistor("g3", "q3", 80_000.0) // R69
        .capacitor("q3", "gnd", 10e-6) // C27
        .resistor("q3", "k3j", 200_000.0) // R70
        .pentode("p3", "g3", "k3", "s3", 1.0, EF804S)
        .resistor("k3", "k3j", 1_000.0) // R71
        .resistor("k3j", "gnd", 15_000.0) // R72
        .resistor("b1", "s3", 1_000_000.0) // R73
        .capacitor("s3", "k3", 50e-9) // C28, to the cathode
        .resistor("b1", "p3", 200_000.0) // R74
        .capacitor("p3", "g4", 25e-9) // C30
        .resistor("g4", "gnd", 1_000_000.0) // R77
        .pentode("p4", "g4", "k4", "s4", 1.0, E83F)
        .resistor("k4", "gnd", 150.0) // R78
        .resistor("b2", "s4", 50_000.0) // R80
        .capacitor("s4", "gnd", 2e-6) // C34
        .resistor("s4", "k4", 300_000.0) // R79
        .resistor("b2", "c88", R88)
        .inductor("c88", "p4", L88) // 88
        .capacitor("p4", "c31", 2e-6) // C31
        .resistor("c31", "k3", fit.r75); // R75, "abgeglichen"

    // --- 89, and the output ------------------------------------------------------------------------
    net.capacitor("p4", "c35", 4e-6) // C35
        .resistor("c35", "t2p", T2_PRIMARY_R)
        .core("t2p", "gnd", T2_CORE)
        .transformer("t2p", "gnd", "t2s", "gnd", T2_RATIO)
        .resistor("t2s", "out", T2_SECONDARY_R)
        .resistor("out", "gnd", load);

    net.build(at)
}
