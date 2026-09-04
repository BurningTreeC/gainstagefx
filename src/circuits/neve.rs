//! The Neve 73P microphone preamplifier, from the schematics.
//!
//! An input transformer into a three transistor amplifier card, and the whole
//! of its gain comes from how much feedback that card is allowed to keep.
//!
//! The card is the classic arrangement: a common emitter input stage, a common
//! emitter driver, and an emitter follower to drive whatever comes next, with
//! series feedback from the follower's emitter back to the input stage's. The
//! gain is then `1 + Rf / Re`, and the switch on the front simply changes
//! `Re` -- which is why a Neve's gain comes in steps rather than on a pot, and
//! why every step is a different amount of feedback and therefore a different
//! amount of distortion. That is not a side effect of the design; it *is* the
//! design.
//!
//! Two drawings of this card were supplied with different designators, and
//! they agree on every value, which is a good deal more confidence than one
//! drawing gives. Where they differ -- the input coupling capacitor is 10 uF
//! on one and 22 uF on the other -- the smaller is used, since it is the one
//! that can be heard.
//!
//! One number is not on either drawing: the input transformer's turns ratio.
//! `INPUT_RATIO` is a stated assumption.

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
    // Its collector load is fed from a node decoupled by C12, which is what
    // keeps the rail out of the signal: R11 and R12 in series from the supply
    // with 22 uF to ground between them.
    net.resistor(input, "gnd", 120_000.0) // R9
        .capacitor(input, "t4b", 10e-6) // C8
        .resistor("n", "dec", 33_000.0) // R11
        .capacitor("dec", "gnd", 22e-6) // C12
        .resistor("dec", "t4c", 47_000.0) // R12
        .resistor("t4b", "dec", 68_000.0) // R10, base bias from the decoupled node
        .capacitor("t4b", "t4c", 100e-12) // C9
        .capacitor("t4c", "gnd", 680e-12) // C11
        .bipolar("t4c", "t4b", "t4e", BC184C);

    // The emitter carries the degeneration and the gain leg, both to ground
    // through their own electrolytics -- so they are in parallel to the
    // signal and the switch shrinks the pair. C13 sets the bias current and
    // takes no part in it.
    net.resistor("t4e", "r19", 1_800.0) // R19
        .capacitor("r19", "gnd", 400e-6) // C17
        .resistor("t4e", "legc", leg) // whatever the gain switch selected
        .capacitor("legc", "gnd", 470e-6)
        .resistor("t4e", "gnd", 150_000.0) // the bias path
        .capacitor("t4e", "gnd", 1500e-12); // C10

    // --- TR5, the driver ---------------------------------------------------
    net.resistor("t4c", "t5b", 15_000.0) // R1 on the other drawing
        .resistor("t5b", "gnd", 10_000.0) // R16
        .supply("t5c", 5_100.0, SUPPLY) // R13
        .resistor("t5e", "gnd", 470.0) // R14
        .capacitor("t5e", "gnd", 22e-6) // C14
        .bipolar("t5c", "t5b", "t5e", BC184C);

    // --- TR6, the output follower -----------------------------------------
    net.supply("t6c", 100.0, SUPPLY)
        .resistor("t5c", "t6b", 1_500.0)
        .resistor("t6e", "gnd", 390.0) // R18
        .capacitor("t6e", "gnd", 1000e-12) // C16
        .bipolar("t6c", "t6b", "t6e", BC184C)
        // The feedback: from the follower's emitter back to the input stage's,
        // which with the gain leg sets the whole card's gain.
        .resistor("t6e", "gnd", 2_200.0) // R17
        .capacitor("t6e", "out", 22e-6) // C15
        // R20 is the feedback, from the output back to the input stage's
        // emitter. Read as a load to ground instead, the card measured seven
        // decibels at most and its gain ran backwards -- a microphone
        // preamplifier gives twenty to seventy and more feedback has to mean
        // less gain, so that reading cannot have been right.
        .resistor("out", "t4e", 51_000.0) // R20
        .resistor("out", "gnd", load);

    net.build(at)
}
