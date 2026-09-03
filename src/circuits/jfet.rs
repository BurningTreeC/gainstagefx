//! A junction FET gain stage, complete.
//!
//! The discrete alternative to a valve, and the reason a great deal of studio
//! equipment sounds the way it does. Where a triode follows a three-halves
//! power and a bipolar transistor an exponential, a JFET follows a square --
//! and a square law has only a second-order term, so a stage kept inside its
//! saturated region makes second harmonic and essentially nothing else.
//!
//! Self-biased, because an n-channel JFET conducts with no bias at all and is
//! turned *off* by pulling the gate negative. There is no divider from the
//! rail: the current the device passes develops a voltage across the source
//! resistor, and that voltage is the bias. A stage that biases itself is also
//! a stage whose bias moves when it is driven hard, which is part of how it
//! behaves at the edge.

use crate::dsp::netlist::{Circuit, Fault, JfetSpec, Netlist, Taper};

/// How much of the source resistor the bypass capacitor covers. The same idea
/// as a cathode bypass and the same shaped track -- see `valve::BYPASS`.
pub const BYPASS: usize = 0;

/// Below one, so the resistance falls quickly and then finely.
pub const BYPASS_SPAN: f64 = 0.004;

#[derive(Clone, Copy, Debug)]
pub struct Values {
    pub drain_load: f64,
    pub source: f64,
    pub supply: f64,
    pub jfet: JfetSpec,
}

/// A small-signal stage on a modest rail, which is what sits at the front of
/// most discrete equipment.
pub const CLASSIC: Values = Values {
    drain_load: 22_000.0,
    source: 1_000.0,
    supply: 24.0,
    jfet: JfetSpec::J2N5457,
};

/// Run hotter, on a rail with room to swing: a bigger drain load and less
/// source resistance, so the device sits further into its own curvature.
pub const HOT: Values = Values {
    drain_load: 47_000.0,
    source: 470.0,
    supply: 30.0,
    jfet: JfetSpec::J201,
};

/// Appends one stage to a netlist and gives back the node its output is on.
///
/// Named nodes, so a second stage asks for its own and nothing has to be
/// offset by hand.
pub fn stage(net: &mut Netlist, prefix: &str, input: &str, v: &Values, control: usize) -> String {
    let gate = format!("{prefix}_gate");
    let drain = format!("{prefix}_drain");
    let source = format!("{prefix}_source");
    let leg = format!("{prefix}_bypass");
    let out = format!("{prefix}_out");

    net.capacitor(input, &gate, 100e-9)
        .resistor(&gate, "gnd", 1_000_000.0)
        .supply(&drain, v.drain_load, v.supply)
        .resistor(&source, "gnd", v.source)
        // In series with the capacitor, not across the resistor. Across it,
        // the source is bypassed at every setting and the control does nothing
        // to the gain at all.
        .pot(&source, &leg, &leg, 10_000.0, Taper::Log { span: BYPASS_SPAN }, control)
        .capacitor(&leg, "gnd", 100e-6)
        .jfet(&drain, &gate, &source, v.jfet)
        // The drain sits well above ground and this is how the hardware sheds
        // it, into whatever follows.
        .capacitor(&drain, &out, 100e-9);
    out
}

pub fn build(v: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("jfet stage");
    net.input("in", source);
    let out = stage(&mut net, "j", "in", v, BYPASS);
    net.resistor(&out, "gnd", load);
    net.build(&out)
}
