//! A speaker cabinet, as the network its response looks like.
//!
//! The piece that turns a clipper into a guitar sound. A twelve inch speaker
//! is not a full range driver and is not trying to be: it has a resonance down
//! near the bottom of the instrument's range, nothing useful below it, a broad
//! lift where the cone is still pistonic, and it stops somewhere past four
//! kilohertz where the cone breaks up and the voice coil's inductance takes
//! over. Everything a hard clipper generates above that -- which is most of
//! what it generates -- never reaches the air.
//!
//! Three sections: a resonant high pass for the box, a resonant low pass for
//! the cone, and one more pole because a cone past breakup falls faster than
//! twelve decibels an octave.
//!
//! The values are solved from the response wanted rather than nudged towards
//! it. That is the difference the AC solver makes to work like this: the
//! previous version's cabinet took three rounds of moving corners by hand and
//! still ended up an octave low, which cost ten decibels at 2.5 kHz -- exactly
//! where a distorted guitar cuts through.

use crate::dsp::netlist::{Circuit, Fault, Netlist};

#[derive(Clone, Copy, Debug)]
pub struct Voicing {
    /// The box resonance, and how sharply it peaks.
    pub resonance_hz: f64,
    pub resonance_q: f64,
    /// Where the cone gives up, and the lift just before it does.
    pub breakup_hz: f64,
    pub breakup_q: f64,
    /// The last pole, which steepens the fall.
    pub rolloff_hz: f64,
}

/// An open backed combo with a single twelve: less bottom, because the back of
/// the cone is not loaded, and a brighter, higher presence peak.
pub const COMBO: Voicing = Voicing {
    resonance_hz: 110.0,
    resonance_q: 0.9,
    breakup_hz: 8_500.0,
    breakup_q: 0.72,
    rolloff_hz: 14_000.0,
};

/// A closed back four by twelve: the sealed box puts the resonance lower and
/// sharpens it, and four cones beaming together roll the top off sooner.
pub const STACK: Voicing = Voicing {
    resonance_hz: 85.0,
    resonance_q: 1.35,
    breakup_hz: 7_000.0,
    breakup_q: 0.80,
    rolloff_hz: 11_000.0,
};

/// What the network works into.
const LOAD: f64 = 10_000.0;
/// The resistor before the last pole. Small against the load, so it costs
/// under a decibel of level to add.
const SERIES: f64 = 1_000.0;

pub fn build(v: &Voicing, source: f64) -> Result<Circuit, Fault> {
    // For a reactance in series with a load and a reactance across it, the
    // corner is 1 / (2 pi sqrt(LC)) and Q is R sqrt(C / L) = R / (w L). Both
    // sections take the same form, so both use the same rearrangement:
    // L = R / (Q w). Writing it the other way up -- Q R / w -- makes the
    // inductor Q squared too large and lands the corner an octave and a half
    // below where it was asked for.
    let solve = |hz: f64, q: f64| {
        let w = std::f64::consts::TAU * hz;
        let l = LOAD / (q * w);
        let c = 1.0 / (w * w * l);
        (l, c)
    };
    let (l_hp, c_hp) = solve(v.resonance_hz, v.resonance_q);
    let (l_lp, c_lp) = solve(v.breakup_hz, v.breakup_q);
    let parallel = SERIES * LOAD / (SERIES + LOAD);
    let c_rc = 1.0 / (std::f64::consts::TAU * v.rolloff_hz * parallel);

    let mut net = Netlist::new("cabinet");
    net.input("in", source)
        // The box: a series capacitor into a shunt inductor.
        .capacitor("in", "resonance", c_hp)
        .inductor("resonance", "gnd", l_hp)
        // The cone: a series inductor into a shunt capacitor.
        .inductor("resonance", "cone", l_lp)
        .capacitor("cone", "gnd", c_lp)
        // And the last pole.
        .resistor("cone", "out", SERIES)
        .capacitor("out", "gnd", c_rc)
        .resistor("out", "gnd", LOAD);
    net.build("out")
}
