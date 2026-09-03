//! A triode gain stage, complete.
//!
//! Input coupling, the valve, its cathode bias and bypass, and the capacitor
//! that takes the audio off the plate. That last one is not tidying up: a
//! plate sits a couple of hundred volts above ground and the hardware sheds
//! that exactly this way, into the next stage's grid leak. The pair of them is
//! a high pass whose corner is part of how the amplifier sounds, and removing
//! the operating point arithmetically instead would give a different bottom
//! end.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// How much of the cathode resistor the bypass capacitor covers.
///
/// Backed off, the cathode follows the signal and fights the grid, which is
/// local feedback: less gain, and a stage that stays clean much longer. This
/// is the stage's own gain control and the reason a preamplifier can be
/// subtle rather than only loud.
pub const BYPASS: usize = 0;

#[derive(Clone, Copy, Debug)]
pub struct Values {
    pub plate_load: f64,
    pub cathode: f64,
    pub supply: f64,
    pub triode: TriodeSpec,
}

/// The ordinary arrangement: a hundred kilohm plate load and 1.5k at the
/// cathode, which is what most of the amplifiers of the period use.
pub const CLASSIC: Values = Values {
    plate_load: 100_000.0,
    cathode: 1_500.0,
    supply: 300.0,
    triode: TriodeSpec::ECC83,
};

/// Run harder: a bigger plate load and less cathode resistance put the stage
/// further into its own curvature before the signal even arrives.
pub const HOT: Values = Values {
    plate_load: 220_000.0,
    cathode: 820.0,
    supply: 300.0,
    triode: TriodeSpec::ECC83,
};

pub fn build(v: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("valve stage");
    net.input("in", source)
        .capacitor("in", "grid", 22e-9)
        .resistor("grid", "gnd", 1_000_000.0)
        .supply("plate", v.plate_load, v.supply)
        .resistor("cathode", "gnd", v.cathode)
        // The control has to be *in series* with the bypass capacitor. Put it
        // across the cathode resistor instead, in parallel with the capacitor,
        // and the cathode is fully bypassed at every setting: the control then
        // shifts the bias a little and does nothing whatever to the gain.
        //
        // Twenty-five kilohms, not a hundred: what the leg is competing with
        // is the cathode resistor itself, so a track that spends most of its
        // travel far above 1.5k is a control that does nothing until the very
        // end of it.
        .pot("cathode", "bypass_leg", "bypass_leg", 25_000.0, Taper::Linear, BYPASS)
        .capacitor("bypass_leg", "gnd", 22e-6)
        .triode("plate", "grid", "cathode", v.triode)
        // Off the plate and into the next stage's grid leak.
        .capacitor("plate", "out", 22e-9)
        .resistor("out", "gnd", load);
    net.build("out")
}
