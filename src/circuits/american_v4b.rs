//! The American V-4B: the 100 W valve bass head of 1971, the VT-40's sibling,
//! from channel one's input to the EXTERNAL AMP jacks that feed its power
//! amplifier.
//!
//! Engineering reference (developer documentation, not panel text): Ampeg
//! V-4B, schematic DWG 06700 revision A (5-71, sheet 12/71-223). The matched
//! power stage is `power::PowerSpec::V4B_7027A`. See
//! `docs/models/american_vt40.md`, which covers both.
//!
//! ```text
//! J1 -- V1a (R2 5.6M; R6 390k / R5 3k3 unbypassed) -- C3 .1
//!   -- ULTRA LO: straight through, or the C101-C104 ladder -- VOL 1 (1 M lin),
//!      ULTRA HI (C105 500 pF) across it -- V1b (R14 3k3, C106 on ULTRA LO)
//!   V1b and V2b share R13 68k; C22 .03 from their plate on ULTRA LO
//!   -- C7 .1 -- P.E.C. 6470000 (BASS, TREBLE), R109 120k
//!   -- the VT-40's 6K11 loop and MIDRANGE, part for part
//!   -- C205 .1 -- R16 180k -- C8 .01 -- V3a, a bootstrapped follower
//!   -- C9 .47 -- R22 10k -- EXT AMP (R21 100k)
//! ```
//!
//! Against the VT-40: no reverb, no SENSITIVITY switch, V1a's cathode
//! unbypassed, .1 uF couplings where the VT-40 has .01, and **ULTRA LO**, a
//! switch of four poles on channel one's side of it (and its twins on channel
//! two's): off, the volume is fed straight; on, through a ladder of three
//! .0047 uF capacitors with C101 .015 to ground at its head and R102 / R103
//! between them, V1b's cathode bypassed outright by C106 6.8 uF, and C22 .03
//! from the mixer's plate to ground. The ladder takes the bottom octave away,
//! C101 and C22 the top, and the bypass gives back what the two cost in the
//! middle: a band that peaks low.
//!
//! **What is not built.** Channel two's input stage -- its volume is at zero,
//! so V2b's grid is at ground; V2b is built as the load it puts on the shared
//! R13, with its own cathode bypass on ULTRA LO.

use super::american_vt40::{self, tone_section, SWITCH_CLOSED, SWITCH_OPEN};
use crate::dsp::netlist::{Adjust, Circuit, Fault, Netlist, Taper, TriodeSpec};

/// VR101, 1 M linear, channel one's VOL. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// VR105, 1 M log, BASS.
pub const BASS: usize = 1;
/// VR104, 50 k linear, MIDRANGE.
pub const MIDDLE: usize = 2;
/// VR103, 1 M log, TREBLE.
pub const TREBLE: usize = 3;

/// SW2 ULTRA HI, channel one's pole: C105, 500 pF from VR101's top to its
/// wiper, in or out.
pub const ULTRA_HI_SLOT: usize = 0;
pub const ULTRA_HI_FARADS: f64 = 500e-12;
pub const ULTRA_HI_OFF_FARADS: f64 = 1.0e-18;

const C: f64 = SWITCH_CLOSED;
const O: f64 = SWITCH_OPEN;

/// SW1 ULTRA LO, as the six contacts that reach channel one's path: C3 to
/// VOL 1 straight, C3 to the ladder's head, the ladder's tail to VOL 1, V1b's
/// cathode to C106, V2b's cathode to C112, and C22's foot to ground. The
/// drawing shows it off ("OFF (LEFT)"), which is where it is built.
pub const ULTRA_LO_SLOTS: [usize; 6] = [1, 2, 3, 4, 5, 6];
pub const ULTRA_LO: [[f64; 6]; 2] = [
    [O, C, C, C, C, C], // on
    [C, O, O, O, O, O], // off, drawn and built
];

/// SW3 MIDRANGE SELECTOR, the VT-40's: 300 Hz, 800 Hz, 3 kHz.
pub const MID_SELECT_SLOTS: [usize; 3] = [7, 8, 9];
pub use american_vt40::MID_SELECT;

/// The 360 V node (circled on the drawing), which R51 2.2 k drops to the
/// preamplifier's 325 V rail across C17's first section. Stiff here: what
/// feeds it is the power stage's inverter rail.
pub const RAIL_UPSTREAM: f64 = 360.0;
pub const RAIL_DROPPER: f64 = 2_200.0;
pub const RAIL_RESERVOIR: f64 = 40e-6;

/// What else hangs on the 325 V rail, as one resistor: V2a (0.44 mA through
/// R12 390 k to its 155 V plate) and V3b (0.54 mA, its 1.8 V across R25 and
/// R26, in the power stage here), from the drawing's voltages: 0.98 mA.
/// DERIVED.
const UNBUILT_DRAW: f64 = 325.0 / 0.98e-3;

const ECC83: TriodeSpec = TriodeSpec::ECC83;
const ECC82: TriodeSpec = TriodeSpec::ECC82;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("American V-4B");

    net.supply("bp", RAIL_DROPPER, RAIL_UPSTREAM) // R51 from the 360 V node
        .capacitor("bp", "gnd", RAIL_RESERVOIR) // C17
        .resistor("bp", "gnd", UNBUILT_DRAW);

    // --- V1a -----------------------------------------------------------------------
    net.input("in", source)
        .resistor("in", "gnd", 5_600_000.0) // R2
        .resistor("bp", "p1", 390_000.0) // R6
        .triode("p1", "in", "k1", ECC83)
        .resistor("k1", "gnd", 3_300.0) // R5
        .capacitor("p1", "ua", 0.1e-6) // C3
        .resistor("ua", "gnd", 100_000.0); // R101

    // --- the ULTRA LO ladder, VOL 1 and ULTRA HI ----------------------------------------
    // C101 .015 at the ladder's head, then C102, C103 and C104, .0047 each, with
    // R102 1.5 M and R103 1.2 M to ground between them. `pot(a, wiper, b)` puts
    // `R f(p)` between the wiper and `b`: turned up the wiper is at the top.
    net.capacitor("l0", "gnd", 0.015e-6) // C101
        .capacitor("l0", "l1", 0.0047e-6) // C102
        .resistor("l1", "gnd", 1_500_000.0) // R102
        .capacitor("l1", "l2", 0.0047e-6) // C103
        .resistor("l2", "gnd", 1_200_000.0) // R103
        .capacitor("l2", "l3", 0.0047e-6) // C104
        .pot("vol", "g1b", "gnd", 1_000_000.0, Taper::Linear, VOLUME); // VR101
    let ultra_hi = net.adjustable("vol", "g1b", Adjust::Capacitor, ULTRA_HI_FARADS);
    debug_assert_eq!(ultra_hi, ULTRA_HI_SLOT);

    // --- the mixer: V1b and V2b on one plate resistor ----------------------------
    // R107 / R108 47 k hold C106 / C112 charged to their cathodes, so throwing
    // ULTRA LO does not thump.
    net.resistor("bp", "pm", 68_000.0) // R13
        .triode("pm", "g1b", "k1b", ECC83)
        .resistor("k1b", "gnd", 3_300.0) // R14
        .resistor("k1b", "c106", 47_000.0) // R107
        .capacitor("c106", "gnd", 6.8e-6) // C106
        .triode("pm", "gnd", "k2b", ECC83)
        .resistor("k2b", "gnd", 3_300.0) // R15
        .resistor("k2b", "c112", 47_000.0) // R108
        .capacitor("c112", "gnd", 6.8e-6) // C112
        .capacitor("pm", "c22", 0.03e-6) // C22
        .resistor("c22", "gnd", 5_600_000.0) // R59
        .capacitor("pm", "s1", 0.1e-6); // C7
    let off = ULTRA_LO[1];
    for ((a, b), (slot, value)) in [
        ("ua", "vol"),
        ("ua", "l0"),
        ("l3", "vol"),
        ("k1b", "c106"),
        ("k2b", "c112"),
        ("c22", "gnd"),
    ]
    .into_iter()
    .zip(ULTRA_LO_SLOTS.into_iter().zip(off))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }

    // --- the P.E.C. 6470000, the 6K11 and the midrange -----------------------------------
    super::american_svt::james_stack(&mut net, "s1", "x1", Some((BASS, TREBLE)), "1");
    tone_section(&mut net, "x1", MIDDLE, MID_SELECT_SLOTS);

    // --- V3a, the 12DW7's 12AU7 half: a bootstrapped follower ----------------------------
    // C205 .1 and R16 180 k in series from the follower's tap to C8 .01: no
    // mix here, no reverb to mix.
    net.capacitor("cf", "c205", 0.1e-6) // C205
        .resistor("c205", "mix", 180_000.0) // R16
        .capacitor("mix", "g3a", 0.01e-6) // C8
        .resistor("g3a", "k3aj", 1_000_000.0) // R18
        .triode("bp", "g3a", "k3a", ECC82)
        .resistor("k3a", "k3aj", 1_000.0) // R19
        .resistor("k3aj", "gnd", 47_000.0) // R20
        .capacitor("k3a", "c9", 0.47e-6) // C9
        .resistor("c9", "out", 10_000.0) // R22
        .resistor("out", "gnd", 100_000.0) // R21
        .resistor("out", "gnd", load);

    net.build(at)
}
