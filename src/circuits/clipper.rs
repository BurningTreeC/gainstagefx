//! Diode clipping, in its two families.
//!
//! The difference between them is not a matter of degree and it is the whole
//! reason both exist.
//!
//! **In the loop.** The diodes sit across the feedback resistor, so when they
//! conduct they lower the stage's *gain*. The output never stops following the
//! input -- it just follows it less enthusiastically -- which is why these are
//! described as transparent, and why one on a hot signal barely does anything:
//! the stage's minimum gain is one, so a large input arrives at the output
//! nearly untouched with the clipped part added on top.
//!
//! **To ground.** The diodes sit across the signal, so they are a ceiling.
//! Past it the output stops going anywhere, the waveform is squared off, and
//! the harmonic series is filled out to the top of the band. That is a wall,
//! and it wants a cabinet after it more than anything else here does.
//!
//! The gain control goes in the feedback path, in series with the fixed
//! resistor, which is where the hardware puts it, and the gain then goes as
//! `1 + (Rf + pot) / Rleg`. That is even in *resistance*, which is not the
//! same as even in decibels: measured, a linear track put eighteen of its
//! thirty decibels into the first quarter of the travel and two into the last.
//! So the track carries the span -- `Taper::ReverseLog` at the ratio the
//! feedback path actually covers -- and the knob then reads evenly.

use crate::dsp::device::OpAmp;
use crate::dsp::netlist::{Circuit, DiodeSpec, Fault, Netlist, Taper};

/// How much gain the stage has before the diodes get a say.
pub const GAIN: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Across the feedback resistor: lowers the gain.
    InTheLoop,
    /// Across the signal to ground: a ceiling.
    ToGround,
}

#[derive(Clone, Copy, Debug)]
pub struct Values {
    pub placement: Placement,
    pub diode: DiodeSpec,
    /// Fixed part of the feedback resistance, and the sweep on top of it.
    pub feedback: f64,
    pub sweep: f64,
    /// The leg that sets the gain, and the capacitor in it.
    pub leg: f64,
    pub leg_c: f64,
    pub rail: f64,
}

/// The overdrive arrangement: silicon in the loop, and a capacitor in the gain
/// leg so the bottom end is amplified and clipped far less than the middle.
/// That is the mid-hump, and it is one capacitor rather than an EQ curve.
pub const OVERDRIVE: Values = Values {
    placement: Placement::InTheLoop,
    diode: DiodeSpec::SILICON,
    feedback: 51_000.0,
    sweep: 500_000.0,
    leg: 4_700.0,
    leg_c: 47e-9,
    rail: OpAmp::NINE_VOLT,
};

/// The distortion arrangement: a great deal of gain into diodes that hold the
/// output flat, and the whole band taken up to them.
///
/// The capacitor in the gain leg is the difference between this and the
/// overdrive above, and it is the difference between fat and thin. At 100 nF
/// against the 4.7 k leg the corner sits at 340 Hz, so everything below it is
/// amplified far less and therefore clipped far less: measured, the stage had
/// 15 dB less gain at 60 Hz than at 3 kHz, and the bottom of the signal went
/// through clean while the middle distorted. That is right for a screamer in
/// front of an amplifier, which is what the overdrive is; it is wrong for a
/// distortion, where the point is that all of it is squared off.
///
/// At 1 uF the corner is 34 Hz and the whole band goes to the diodes together.
pub const DISTORTION: Values = Values {
    placement: Placement::ToGround,
    diode: DiodeSpec::SILICON,
    feedback: 10_000.0,
    sweep: 470_000.0,
    leg: 4_700.0,
    leg_c: 1e-6,
    rail: OpAmp::NINE_VOLT,
};

pub fn build(v: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("clipper");
    net.input("in", source)
        // Non-inverting: the signal goes to the positive input and the loop
        // closes on the negative one.
        .opamp("amp", "in", "minus", v.rail)
        // The drive pot in the feedback path, in series with the fixed part.
        .pot(
            "amp",
            "feedback",
            "feedback",
            v.sweep,
            // The span the feedback path covers, which is what the track has
            // to be shaped for.
            Taper::ReverseLog { span: (v.feedback + v.sweep) / v.feedback },
            GAIN,
        )
        .resistor("feedback", "minus", v.feedback)
        .resistor("minus", "leg", v.leg)
        .capacitor("leg", "gnd", v.leg_c);

    match v.placement {
        Placement::InTheLoop => {
            net.diode("amp", "minus", v.diode)
                .diode("minus", "amp", v.diode)
                .resistor("amp", "out", 1_000.0)
                .resistor("out", "gnd", load);
        }
        Placement::ToGround => {
            // The series resistor is what stops the pair being a short across
            // the output, and with the diodes' own capacitance it is the only
            // thing rounding the corners at all.
            net.resistor("amp", "out", 10_000.0)
                .diode("out", "gnd", v.diode)
                .diode("gnd", "out", v.diode)
                .resistor("out", "gnd", load);
        }
    }
    net.build("out")
}
