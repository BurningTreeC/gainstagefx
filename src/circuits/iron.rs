//! Transformers, with cores that run out of room.
//!
//! Two parts make one transformer here: the ideal ratio, which is a branch in
//! the matrix, and the magnetising branch across the winding, which is the
//! part that saturates. Separating them is what makes the sound come out of
//! the physics -- the ratio alone is perfectly linear and would put iron in
//! the signal path that made no difference whatever.
//!
//! What the iron does is therefore *level and frequency* dependent together,
//! which is the whole character of the thing. Flux is the integral of voltage,
//! so a bass note makes far more of it than a cymbal at the same peak, and the
//! core gives up on the bass note first. Measured on the core alone at twelve
//! volts: 68 per cent distortion at 30 Hz, 4 per cent at 120, nothing at 500.
//!
//! The other half is where the winding is driven from. A transformer's
//! distortion appears as a *voltage* only because the magnetising current has
//! to come through whatever impedance is feeding it -- fed from a perfect
//! source, a core could saturate all it liked and the terminal voltage would
//! never move. So the source resistance is not a detail to be tidied away.

use crate::dsp::netlist::{Circuit, CoreSpec, Fault, Netlist};

#[derive(Clone, Copy, Debug)]
pub struct Values {
    /// Primary volts per secondary volt. Below one steps up, which is what an
    /// input transformer does and where its gain comes from at no noise cost.
    pub ratio: f64,
    pub core: CoreSpec,
    /// Winding resistance, which is part of what turns magnetising current
    /// into a voltage that can be heard.
    pub winding: f64,
    /// The capacitance across the secondary. With the leakage inductance this
    /// is what puts the small lift near the top of the band that iron is
    /// known for -- and, past it, the roll-off.
    pub shunt: f64,
    pub leakage: f64,
}

/// An input transformer, stepping up. Nickel, because that is what the ones
/// people buy for their sound are wound on.
pub const INPUT: Values = Values {
    ratio: 0.1,
    core: CoreSpec::NICKEL,
    winding: 600.0,
    shunt: 220e-12,
    leakage: 8e-3,
};

/// An output transformer, one to one. Steel: out of the way until it is
/// pushed, and then it goes a long way.
pub const OUTPUT: Values = Values {
    ratio: 1.0,
    core: CoreSpec::STEEL,
    winding: 300.0,
    shunt: 470e-12,
    leakage: 4e-3,
};

/// Appends a transformer to a netlist and gives back its secondary node.
///
/// The core goes across the *primary*, which is where the flux is set by what
/// is driving it.
pub fn stage(net: &mut Netlist, prefix: &str, input: &str, v: &Values) -> String {
    let primary = format!("{prefix}_pri");
    let secondary = format!("{prefix}_sec");
    let out = format!("{prefix}_out");

    net.resistor(input, &primary, v.winding)
        .core(&primary, "gnd", v.core)
        .transformer(&primary, "gnd", &secondary, "gnd", v.ratio)
        // Leakage inductance in series and the winding capacitance across it:
        // together a resonance near the top of the band, which is the lift
        // and then the fall that iron puts there.
        .inductor(&secondary, &out, v.leakage)
        .capacitor(&out, "gnd", v.shunt);
    out
}

/// Iron on its own: a signal passed through a transformer and nothing else.
pub fn build(v: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("iron");
    net.input("in", source);
    let out = stage(&mut net, "t", "in", v);
    net.resistor(&out, "gnd", load);
    net.build(&out)
}
