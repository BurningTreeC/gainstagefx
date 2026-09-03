//! The passive tone stack every guitar amplifier of the period uses.
//!
//! Three controls interacting through one network, which is why an amplifier's
//! tone controls never behave like three independent filters: the bass pot is
//! part of the treble pot's load, the mid pot is the bottom of both, and with
//! everything down the stack does not go quiet, it goes thin.
//!
//! The famous scoop is not designed in. It is what a divider does when its two
//! paths meet out of phase, and it is present at every setting.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper};

/// Which shaft is which. The panel lays them out in this order.
pub const BASS: usize = 0;
pub const MID: usize = 1;
pub const TREBLE: usize = 2;

/// Component values. Four numbers is the whole difference between an
/// "American" and a "British" reputation.
#[derive(Clone, Copy, Debug)]
pub struct Values {
    pub treble_c: f64,
    pub bass_c: f64,
    pub mid_c: f64,
    pub treble_r: f64,
    pub bass_r: f64,
    pub mid_r: f64,
    /// The slope resistor between the stage and the stack.
    pub slope: f64,
}

/// A Fender: a small treble cap and a big treble pot, and the deepest scoop of
/// the family.
pub const AMERICAN: Values = Values {
    treble_c: 250e-12,
    bass_c: 100e-9,
    mid_c: 100e-9,
    treble_r: 250_000.0,
    bass_r: 1_000_000.0,
    mid_r: 25_000.0,
    slope: 100_000.0,
};

/// A Marshall: twice the treble capacitance and a much smaller bass cap, which
/// moves the scoop up and fills the bottom of it in.
pub const BRITISH: Values = Values {
    treble_c: 500e-12,
    bass_c: 22e-9,
    mid_c: 22e-9,
    treble_r: 250_000.0,
    bass_r: 1_000_000.0,
    mid_r: 25_000.0,
    slope: 33_000.0,
};

/// A Vox: no mid control at all, and a much smaller slope resistor. That
/// missing pot is the whole difference -- with the mid leg a fixed low
/// resistance the divider has nothing to scoop against, so it sits forward.
pub const CHIME: Values = Values {
    treble_c: 250e-12,
    bass_c: 100e-9,
    mid_c: 22e-9,
    treble_r: 1_000_000.0,
    bass_r: 1_000_000.0,
    mid_r: 10_000.0,
    slope: 100_000.0,
};

/// Builds the stack on its own, driven from `source` ohms and working into
/// `load` ohms.
///
/// The topology, in the order the signal meets it:
///
/// * the input reaches the treble pot's **top** only through the treble
///   capacitor -- never directly, or the track passes everything at every
///   frequency and the network measures flat at every setting;
/// * the treble pot's **bottom** lands on the junction the mid pot holds down;
/// * the input also reaches that same junction through the slope resistor and
///   the bass capacitor, by way of the bass pot;
/// * the mid pot ties the junction to ground.
///
/// The output is the treble wiper. Both paths arrive at it, each through its
/// own reactance, and where they meet out of phase is the notch.
pub fn build(values: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("tone stack");
    net.input("in", source)
        // Treble: through the cap onto the top of the track.
        .capacitor("in", "treble_top", values.treble_c)
        .pot("treble_top", "out", "junction", values.treble_r, Taper::Linear, TREBLE)
        // Bass: through the slope resistor and its cap onto the top of the
        // bass track. The bass pot is a divider, not a rheostat -- its wiper
        // feeds the junction, and turning it up moves that wiper toward the
        // capacitor. Wired as a rheostat in series instead, less resistance
        // means more signal and the control runs backwards.
        .resistor("in", "slope", values.slope)
        .capacitor("slope", "bass_top", values.bass_c)
        .pot("bass_top", "junction", "mid_top", values.bass_r, Taper::Linear, BASS)
        // The mid pot holds the bottom of the bass track down to ground.
        .pot("mid_top", "gnd", "gnd", values.mid_r, Taper::ReverseLinear, MID)
        .capacitor("slope", "mid_top", values.mid_c)
        .resistor("out", "gnd", load);
    net.build("out")
}
