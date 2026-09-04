//! The Neve 73P microphone preamplifier -- **not yet a model of one**.
//!
//! This file does not reproduce the card and is deliberately not offered as a
//! selectable circuit. It is kept because the values are right and only the
//! connections are in doubt, so it is most of the way there once the drawing
//! can be read properly.
//!
//! Two schematics were supplied and they agree on every value: 120k, 68k,
//! 33k/47k, 5k1, 470R, 1k5, 10k, 2k2, 390R, 1k8, 51k, and the 22u/125u/400u
//! electrolytics, with three BC184C/BC109C. What neither resolves at the
//! resolution available is which node each of those joins, and five readings
//! were tried. Each was refuted by measurement rather than by opinion:
//!
//! 1. Feedback from the follower's emitter to the input emitter with R17 2k2,
//!    and the gain leg in series with R19: the gain ran *backwards* -- more
//!    feedback gave more gain -- and topped out at 7 dB.
//! 2. R20 51k as the feedback instead: the direction came right, but the gain
//!    converged on 33 dB whatever the switch said. Feedback cannot produce
//!    more gain than the amplifier has, so that is the card's open loop gain,
//!    and a real BA283 has sixty to eighty.
//! 3. R15 1k5 as a load on the driver's base: 1k5 across a 47k collector load
//!    swamps the input stage, and the whole card measured unity.
//! 4. Base biased from the decoupled rail through R10 68k: that drives
//!    milliamps into the base, puts the input emitter at 22 V on a 24 V rail
//!    and saturates it. Measured, its collector then carried 35 dB *less*
//!    signal than its base and the driver was doing all the gain.
//! 5. A direct coupled servo from the follower's emitter back to the base:
//!    everything cut off and the output fell to -77 dB.
//!
//! What would settle it, in order of usefulness: the **direct voltages at each
//! transistor's base, emitter and collector**, which pin the bias immediately
//! and would have caught readings 4 and 5 without a single measurement; a scan
//! of the card at a resolution where the connections can be traced; or a
//! netlist from the project.
//!
//! The gain switch's own markings are the external check to aim at: 0, 20, 30,
//! 40, 50, 60 and 70 on the front panel, with the two cards cascaded for the
//! higher settings, so one card and the input transformer should cover roughly
//! twenty to fifty decibels.

use crate::dsp::netlist::{BipolarSpec, Circuit, CoreSpec, Fault, Netlist};

/// The gain-setting resistance, as a control. See `gain_leg`.
pub const GAIN: usize = 0;

/// The BC184C/BC109C on the card: a high-beta, low-noise small-signal NPN.
const BC184C: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 500.0,
    reverse_beta: 4.0,
    early: 100.0,
};

/// The rail the card runs from.
pub const SUPPLY: f64 = 24.0;

/// Primary volts per secondary volt on the input transformer. Below one steps
/// up. Not printed on the drawing -- a microphone input transformer of this
/// kind is usually somewhere between 1:5 and 1:10, and the gain switch's own
/// markings are what this has to be reconciled against.
pub const INPUT_RATIO: f64 = 0.1;

/// The switched gain legs, off the gain schematic, smallest resistance first.
///
/// Smaller is less degeneration and so more gain. The front panel numbers
/// these 0 to 70 in tens.
pub const LEGS: [f64; 8] = [68.0, 510.0, 1_000.0, 1_800.0, 2_700.0, 3_900.0, 12_000.0, 1e9];

/// The resistance the gain control selects.
pub fn gain_leg(position: f64) -> f64 {
    let last = LEGS.len() - 1;
    // Turned up means less resistance, so the list is walked backwards.
    let index = ((1.0 - position.clamp(0.0, 1.0)) * last as f64).round() as usize;
    LEGS[index.min(last)]
}

pub fn build(source: f64, load: f64, leg: f64) -> Result<Circuit, Fault> {
    tap(source, load, leg, "out", true)
}

/// The card, optionally with its input transformer in front of it.
pub fn tap(
    source: f64,
    load: f64,
    leg: f64,
    at: &str,
    with_iron: bool,
) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Neve 73P");
    net.supply("n", 100.0, SUPPLY).capacitor("n", "gnd", 470e-6);

    // --- the input transformer --------------------------------------------
    let input = if with_iron {
        net.input("mic", source)
            .resistor("mic", "pri", 100.0)
            .core("pri", "gnd", CoreSpec::NICKEL)
            .transformer("pri", "gnd", "sec", "gnd", INPUT_RATIO)
            .capacitor("sec", "gnd", 180e-12); // C39
        "sec"
    } else {
        net.input("sec", source);
        "sec"
    };

    // --- TR4, the input stage ---------------------------------------------
    // Its collector load is fed from a node decoupled by C12: R11 and R12 in
    // series from the supply with 22 uF to ground between them, so the rail
    // never reaches the signal.
    net.resistor(input, "gnd", 120_000.0) // R9
        .capacitor(input, "t4b", 10e-6) // C8
        .resistor("n", "dec", 33_000.0) // R11
        .capacitor("dec", "gnd", 22e-6) // C12
        .resistor("dec", "t4c", 47_000.0) // R12
        // The base is not biased from the rail. R10 from a 24 V node through
        // 68 k drives milliamps into the base, which puts TR4's emitter at
        // 22 V and saturates it -- measured, its collector then had 35 dB
        // *less* signal on it than its base, and TR5 was doing all the gain.
        // R20 brings the operating point back from the output follower and
        // R10 divides it down: the loop sets its own bias, which is how a
        // card with this few components manages three direct coupled stages.
        .resistor("t4b", "gnd", 68_000.0) // R10
        .resistor("t6e", "t4b", 51_000.0) // R20
        .capacitor("t4b", "t4c", 100e-12) // C9
        .capacitor("t4c", "gnd", 680e-12) // C11
        .bipolar("t4c", "t4b", "t4e", BC184C);

    // The emitter carries the degeneration, and with it the whole gain of the
    // card. R19 is always there; the switch puts its own resistor beside it,
    // each behind an electrolytic so that neither disturbs the bias.
    // No separate bias path here. The feedback resistor from the output
    // follower's emitter is direct coupled, so the loop sets the operating
    // point of the whole card at once -- which is how these cards work and
    // why they have so few components for three stages. Adding a resistor to
    // ground here fights it: 33 k put TR4's emitter at 23 V on a 24 V rail,
    // and a saturated input stage has no gain, so the card measured unity
    // whatever the gain switch said.
    net.resistor("t4e", "r19", 1_800.0) // R19
        .capacitor("r19", "gnd", 400e-6) // C17
        .resistor("t4e", "legc", leg) // whatever the gain switch selected
        .capacitor("legc", "gnd", 470e-6)
        .capacitor("t4e", "gnd", 1500e-12); // C10

    // --- TR5, the driver ---------------------------------------------------
    // R16 in series into the driver's base. R15 is *not* a load to ground
    // here: 1.5 k across a 47 k collector load swamps the input stage and the
    // whole card measured unity whatever the gain switch said.
    net.resistor("t4c", "t5b", 10_000.0) // R16
        .supply("t5c", 5_100.0, SUPPLY) // R13
        .resistor("t5e", "gnd", 470.0) // R14
        .capacitor("t5e", "gnd", 22e-6) // C14
        .bipolar("t5c", "t5b", "t5e", BC184C);

    // --- TR6, the output follower -----------------------------------------
    net.supply("t6c", 100.0, SUPPLY)
        .resistor("t5c", "t6b", 100.0)
        .resistor("t6e", "gnd", 390.0) // R18
        .capacitor("t6e", "gnd", 1000e-12) // C16
        .bipolar("t6c", "t6b", "t6e", BC184C)
        // The feedback, from the follower's emitter back to the input stage's.
        // With the leg above it this sets the whole card's gain as
        // 1 + R17 / Re, which is why a Neve's gain comes in switched steps.
        .resistor("t6e", "t4e", 2_200.0) // R17
        .capacitor("t6e", "out", 22e-6) // C15
        .resistor("out", "gnd", load);

    net.build(at)
}
