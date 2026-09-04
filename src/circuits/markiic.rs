//! The Mesa Boogie Mark IIC+ lead-channel preamplifier, from the schematic.
//!
//! Four triode stages and a Fender tone stack between the first two of them.
//! `INPUT` through V1A, the stack, Volume 1, V1B, then the lead drive control
//! into V3B and V4A, ending where the drawing marks `LEAD OUTPUT`. What comes
//! after that on the real amp is reverb, the graphic equaliser and the power
//! stage, none of which is preamplifier.
//!
//! One number here is not off the drawing: the supply. Only capacitor voltage
//! ratings are printed, never the rail, so `SUPPLY` is a stated assumption
//! rather than a measurement, and it is the first thing to change if this
//! turns out not to bias where the real amp does.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

pub const TREBLE: usize = 0;
pub const BASS: usize = 1;
pub const MIDDLE: usize = 2;
pub const VOLUME: usize = 3;
pub const LEAD_DRIVE: usize = 4;

/// The plate supply. Not printed on the schematic -- see the note above.
pub const SUPPLY: f64 = 410.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "lead_out")
}

/// The same preamplifier brought out at a chosen node, for measuring one
/// stage at a time.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Mark IIC+ preamp");

    // --- V1A --------------------------------------------------------------
    // R1 grid leak, R2 with C1 across it at the cathode. C1 is only .47 uF,
    // which against 1.5 k bypasses from about 220 Hz upwards rather than all
    // the way down -- the bottom of the band keeps its degeneration, and that
    // is where this amplifier's tightness comes from.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R1
        .resistor("v1a_k", "gnd", 1_500.0) // R2
        .capacitor("v1a_k", "gnd", 0.47e-6) // C1
        .supply("v1a_p", 150_000.0, SUPPLY) // R4
        .triode("v1a_p", "in", "v1a_k", ECC83);

    // --- the tone stack ---------------------------------------------------
    // A Fender stack, read off the drawing: C5 and C6 in parallel are the
    // treble capacitor, R6 sits across C6, R5 is the slope resistor, and C4
    // and C3 feed the bass and middle legs. The output is the treble wiper,
    // and the bass reaches it through the lower half of that same pot -- which
    // is what makes the three controls interact the way they famously do.
    net.capacitor("v1a_p", "t_top", 750e-12) // C6
        .capacitor("v1a_p", "t_top", 250e-12) // C5
        .resistor("v1a_p", "t_top", 10_000_000.0) // R6, across C6
        .pot("t_top", "ts_out", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .resistor("v1a_p", "slope", 100_000.0) // R5
        .capacitor("slope", "t_bot", 0.1e-6) // C4
        .capacitor("slope", "b_bot", 0.047e-6) // C3
        // Both of these are wired as variable resistors, and a variable
        // resistor keeps `1 - fraction` of its track -- so a forward taper
        // *shorts them out* at the top of the travel and both controls run
        // backwards. Turned up they have to have more resistance in circuit,
        // not less.
        .pot("t_bot", "b_bot", "b_bot", 250_000.0, Taper::ReverseLinear, BASS)
        .pot("b_bot", "gnd", "gnd", 10_000.0, Taper::ReverseLinear, MIDDLE);

    // --- Volume 1 and V1B --------------------------------------------------
    net.pot("ts_out", "v1b_g", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .resistor("v1b_k", "gnd", 1_500.0) // R7
        .capacitor("v1b_k", "gnd", 22e-6) // C13
        .supply("v1b_p", 100_000.0, SUPPLY) // R8
        .triode("v1b_p", "v1b_g", "v1b_k", ECC83)
        .capacitor("v1b_p", "lead_in", 0.1e-6); // C7

    // --- the lead drive control -------------------------------------------
    // R21 is marked "680K, 1M on ++", and the 330k under the pot is inside
    // the dashed box marked "++ MOD" -- both belong to the modification, not
    // to a stock C+. Left in, the pot's bottom never reaches ground and the
    // control covers only twelve decibels instead of running to silence, so
    // this is the stock arrangement.
    net.resistor("lead_in", "ld_top", 680_000.0) // R21
        .pot("ld_top", "v3b_g", "gnd", 1_000_000.0, Taper::Audio, LEAD_DRIVE);

    // --- V3B ---------------------------------------------------------------
    net.resistor("v3b_g", "gnd", 475_000.0) // R22
        .capacitor("v3b_g", "v3b_k", 120e-12) // C22
        .resistor("v3b_k", "gnd", 1_500.0) // R23
        .capacitor("v3b_k", "gnd", 2.2e-6) // C23
        .supply("v3b_p", 82_000.0, SUPPLY) // R26
        .triode("v3b_p", "v3b_g", "v3b_k", ECC83);

    // --- into V4A ----------------------------------------------------------
    // R25 in series and R24 to ground make a divider between the two stages,
    // with C24 across R24 taking the top off what gets through. Two triodes
    // in a row with an attenuator between them is how a lead channel gets its
    // gain without simply oscillating.
    net.capacitor("v3b_p", "c25", 0.022e-6) // C25
        .resistor("c25", "v4a_g", 270_000.0) // R25
        .resistor("v4a_g", "gnd", 68_000.0) // R24
        .capacitor("v4a_g", "gnd", 1000e-12) // C24
        .resistor("v4a_k", "gnd", 6_800.0) // R29
        .capacitor("v4a_k", "gnd", 0.22e-6) // C29
        .supply("v4a_p", 274_000.0, SUPPLY) // R27
        .capacitor("v4a_p", "gnd", 1000e-12) // C27, across R27 to the rail
        .triode("v4a_p", "v4a_g", "v4a_k", ECC83);

    // --- out ---------------------------------------------------------------
    net.capacitor("v4a_p", "c30", 0.047e-6) // C30
        .resistor("c30", "lead_out", 220_000.0) // R31
        .capacitor("c30", "lead_out", 250e-12) // C31, across R31
        .resistor("lead_out", "gnd", 100_000.0) // R32
        .capacitor("lead_out", "gnd", 500e-12) // C32
        .resistor("lead_out", "gnd", load);

    net.build(at)
}
