//! The Round Fuzz: two germanium PNP transistors and a feedback resistor.
//!
//! Engineering reference (developer documentation, not panel text): Arbiter Fuzz
//! Face, germanium version (1966-68), per ElectroSmash's "Fuzz Face Analysis"
//! schematic and parts list, cross-checked against fuzzboxes.org's survey of
//! surviving units. Transistor constants from the AC128 SPICE models published with
//! that analysis. See `docs/models/round_fuzz.md`.
//!
//! Q1 is a common-emitter stage with its emitter at ground and 33 k to the
//! negative battery rail; Q2 takes Q1's collector straight on its base, and its
//! emitter comes back to Q1's base through R4 100 k, which is the DC bias of the
//! whole pedal and, through the Fuzz control, its AC feedback. Turning Fuzz up
//! bypasses Q2's emitter resistance with C2 20 uF, which removes the feedback and
//! lets both stages clip. The output is taken from the R2/R3 divider, not from the
//! collector, which is why a Fuzz Face is not especially loud.
//!
//! The circuit is positive-ground: the battery's positive terminal is the circuit's
//! ground here, and the supply is -9 V.

use crate::dsp::netlist::{BipolarSpec, Circuit, Fault, Netlist, Taper};

/// R FUZZ, 1 k linear, from Q2's emitter to ground with C2 on the wiper.
pub const FUZZ: usize = 0;
/// R VOL, 500 k audio.
pub const VOLUME: usize = 1;

/// Where the level control rests: **unity through the pedal**, which is what
/// the panel's Level knob means at noon. Highest of the pedals, because this one
/// has the least gain to give away. See `rodent::VOLUME_REST`.
pub const VOLUME_REST: f64 = 0.747;

/// ESTIMATED: a 9 V PP3 carbon-zinc battery's internal resistance. The analysis
/// notes that the battery resistance adds to R2 and changes the sound; none is
/// given.
pub const BATTERY_RESISTANCE: f64 = 20.0;

/// Q1, a low-gain germanium PNP (β 85): ElectroSmash's GERPNP_LOWGAIN AC128 model,
/// IS 85.8 nA, BF 85, BR 20, VAF 102 V. The ISE/ISC leakage terms, base resistance
/// and junction capacitances are not in the solver's Ebers-Moll device.
pub const Q1: BipolarSpec = BipolarSpec {
    saturation: 85.8e-9,
    forward_beta: 85.0,
    reverse_beta: 20.0,
    early: 102.207,
};

/// Q2, the higher-gain part (β 120): GERPNP_HIGHGAIN, IS 120.8 nA, BF 120.
pub const Q2: BipolarSpec = BipolarSpec {
    saturation: 120.8e-9,
    forward_beta: 120.0,
    reverse_beta: 20.0,
    early: 102.207,
};

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Round Fuzz");
    net.supply("vneg", BATTERY_RESISTANCE, -9.0);

    // --- Q1 ------------------------------------------------------------------------
    // C1: 2.5 uF on the surviving 1966-67 units (fuzzboxes.org), 2.2 uF on
    // ElectroSmash's list. The original value is used.
    net.input("in", source)
        .capacitor("in", "b1", 2.5e-6) // C1
        .resistor("vneg", "c1", 33_000.0) // R1
        .bipolar_pnp("c1", "b1", "gnd", Q1);

    // --- Q2 ---------------------------------------------------------------------------
    net.bipolar_pnp("c2", "c1", "e2", Q2)
        .resistor("vneg", "tap", 470.0) // R2
        .resistor("tap", "c2", 8_200.0) // R3
        .resistor("e2", "b1", 100_000.0) // R4, the feedback
        // R FUZZ: the track from Q2's emitter to ground, the wiper to C2. Turned up,
        // the wiper moves to the emitter and bypasses it.
        .pot("e2", "fuzz", "gnd", 1_000.0, Taper::Linear, FUZZ)
        .capacitor("fuzz", "gnd", 20e-6); // C2

    // --- output ----------------------------------------------------------------------
    net.capacitor("tap", "vol", 0.01e-6) // C3
        .rest(VOLUME, VOLUME_REST)
        .pot("vol", "out", "gnd", 500_000.0, Taper::Audio, VOLUME) // R VOL
        .resistor("out", "gnd", load);

    net.build(at)
}
