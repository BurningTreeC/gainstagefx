//! The Treble Boost: one germanium transistor behind a 5 nF capacitor.
//!
//! Engineering reference (developer documentation, not panel text): Dallas
//! Rangemaster Treble Booster (1965), Mullard OC44, positive ground. No factory
//! drawing exists; the values are recovered from original units and agree
//! across sources, with ElectroSmash's "Dallas Rangemaster Treble Booster
//! Analysis" as the best of them. See `docs/models/treble_boost.md`.
//!
//! A single common-emitter stage. R1 and R2 hold the base about 1.2 V below
//! ground, R3 sets about 0.2 mA through the transistor with C3 bypassing it
//! for signal, and the 10 k Boost pot is the collector load itself, the output
//! taken off its wiper -- so the pot sets how much of a fixed gain reaches
//! the output and never changes the gain. What makes it a *treble* booster is
//! C1: 5 nF into an input impedance of about ten kilohms, which is a high-pass
//! near 2.6 kHz, and which a guitar's pickup and cable then meet as a load.
//!
//! The circuit is positive-ground, as the Round Fuzz is: the battery's
//! positive terminal is this circuit's ground and the supply is -9 V.

use crate::dsp::netlist::{BipolarSpec, Circuit, Fault, Netlist, Taper};

/// The BOOST control: the 10 k collector load, audio (ElectroSmash: "an audio
/// (logarithmic) 10K potentiometer"). The Drive knob turns it.
pub const BOOST: usize = 0;

/// Where the boost control rests: **unity through the pedal**, which is what
/// the panel's Level knob means at noon. Measured by `examples/pedallevel.rs`.
pub const BOOST_REST: f64 = 0.589;

/// ESTIMATED: a 9 V battery's internal resistance, as for the Round Fuzz.
pub const BATTERY_RESISTANCE: f64 = 20.0;

/// The Mullard OC44, from the Gummel-Poon parameters Holmes, Holters and van
/// Walstijn extracted from a vintage OC44 ("Comparison of germanium bipolar
/// junction transistor models for real-time circuit simulation", DAFx-17;
/// published as a SPICE model with the paper's supplementary code): IS 1.423
/// uA, BF 307, BR 20.27, VAF 8.167 V, with ISE 30.54 nA, NE 1.316.
///
/// The solver's transistor is Ebers-Moll, with no low-current recombination
/// term, so `forward_beta` here is the *effective* gain those parameters give
/// at this circuit's own operating point -- 0.2 mA, where the ISE term takes
/// about 1.4 uA of base current beside the ideal term's 0.65 -- which is 97.
/// DERIVED; it agrees with the beta of 100 ElectroSmash's arithmetic assumes
/// and with Mullard's minimum hFE of 100 at 1 mA. The saturation current, the
/// reverse gain and the Early voltage are the paper's.
pub const OC44: BipolarSpec = BipolarSpec {
    saturation: 1.423e-6,
    forward_beta: 97.0,
    reverse_beta: 20.27,
    early: 8.167,
};

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Treble Boost");
    // C4, 47 uF across the battery.
    net.supply("vneg", BATTERY_RESISTANCE, -9.0)
        .capacitor("vneg", "gnd", 47e-6);

    // --- input and bias --------------------------------------------------------
    net.input("in", source)
        .capacitor("in", "b", 5e-9) // C1
        .resistor("b", "vneg", 470_000.0) // R1
        .resistor("b", "gnd", 68_000.0); // R2

    // --- Q1 ----------------------------------------------------------------------
    // The emitter to ground through R3, bypassed by C3; the collector into the
    // Boost pot's track, whose other end is the negative rail.
    net.resistor("e", "gnd", 3_900.0) // R3
        .capacitor("e", "gnd", 47e-6) // C3
        .bipolar_pnp("c", "b", "e", OC44);

    // --- the Boost control and the output -----------------------------------------
    // `pot(a, wiper, b)` puts `R f(p)` between the wiper and `b`: with `a` at
    // the collector and `b` on the rail, turned up the wiper is at the
    // collector and the whole of the stage's swing reaches C2; turned down it
    // is on the rail, which is signal ground.
    net.rest(BOOST, BOOST_REST)
        .pot("c", "w", "vneg", 10_000.0, Taper::Audio, BOOST)
        .capacitor("w", "out", 10e-9) // C2
        .resistor("out", "gnd", load);

    net.build(at)
}
