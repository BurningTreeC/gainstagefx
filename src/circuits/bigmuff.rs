//! The Electro-Harmonix Big Muff Pi, from the schematics.
//!
//! Four transistor stages, and every version ever made uses the same four:
//! an input stage, two identical clipping stages with diodes in their feedback
//! loops, the tone control between them and the output, and an output stage.
//! Eighteen traced variants differ only in values -- collector resistors
//! between 10k and 39k, feedback between 390k and 470k, coupling capacitors
//! between .1 uF and 3 uF -- so "the best of all versions" is not an average
//! of those, which would be a Muff nobody ever built. It is a choice of which
//! one, and the variants worth having are offered as `Voicing`.
//!
//! What makes it a Muff rather than another fuzz is where the clipping sits.
//! The diodes are in the *feedback* of each stage, so they lower its gain
//! rather than stopping its output -- twice, in series -- and the tone control
//! between them is a scoop rather than a tilt. That is why it sustains
//! forever and why it sits under a mix instead of on top of it.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, Netlist, Taper};

pub const SUSTAIN: usize = 0;
pub const TONE: usize = 1;
pub const VOLUME: usize = 2;

/// 1N914, the clipping diodes in every version.
const D1N914: DiodeSpec = DiodeSpec { saturation: 4.352e-9, emission: 1.906 };

/// The transistors. The traced examples measured hFE between 167 and 193, so
/// this is the middle of a real pedal rather than a datasheet maximum.
const FS36999: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 180.0,
    reverse_beta: 4.0,
    early: 90.0,
};

#[derive(Clone, Copy, Debug)]
pub struct Voicing {
    /// Collector load of the input stage, and of the two clipping stages.
    pub input_collector: f64,
    pub stage_collector: f64,
    /// Feedback around each stage, and the capacitor across it that decides
    /// how much top end each one keeps.
    pub feedback: f64,
    pub feedback_cap: f64,
    /// The capacitor in series with the clipping diodes.
    pub clip_cap: f64,
    /// Coupling between stages, and the series resistor into each base.
    pub coupling: f64,
    pub series: f64,
    /// The foot of the sustain control.
    pub sustain_foot: f64,
    /// The tone control's two legs: treble through `treble_cap` and
    /// `treble_leg`, bass through `bass_leg` and `bass_cap`.
    pub treble_cap: f64,
    pub treble_leg: f64,
    pub bass_leg: f64,
    pub bass_cap: f64,
    /// The output stage.
    pub out_bias: f64,
    pub out_collector: f64,
    pub out_emitter: f64,
}

/// The 1973 Ram's Head, the version most often called the best sounding one.
pub const RAMS_HEAD: Voicing = Voicing {
    input_collector: 12_000.0,
    stage_collector: 12_000.0,
    feedback: 470_000.0,
    feedback_cap: 470e-12,
    clip_cap: 0.1e-6,
    coupling: 0.1e-6,
    series: 10_000.0,
    sustain_foot: 560.0,
    treble_cap: 0.004e-6,
    treble_leg: 33_000.0,
    bass_leg: 33_000.0,
    bass_cap: 0.012e-6,
    out_bias: 390_000.0,
    out_collector: 10_000.0,
    out_emitter: 2_700.0,
};

/// The 1971 Triangle: bigger collector loads, so more gain and a rawer sound.
pub const TRIANGLE: Voicing = Voicing {
    input_collector: 33_000.0,
    stage_collector: 10_000.0,
    feedback: 470_000.0,
    feedback_cap: 500e-12,
    clip_cap: 1.0e-6,
    coupling: 1.0e-6,
    series: 10_000.0,
    sustain_foot: 1_000.0,
    treble_cap: 0.004e-6,
    treble_leg: 27_000.0,
    bass_leg: 27_000.0,
    bass_cap: 0.01e-6,
    out_bias: 470_000.0,
    out_collector: 10_000.0,
    out_emitter: 2_200.0,
};

/// The Colorsound Supa Tonebender: a Muff with the first clipping stage's
/// diodes taken out altogether, which is one change and a different pedal.
pub const SUPA: Voicing = Voicing {
    input_collector: 15_000.0,
    stage_collector: 10_000.0,
    feedback: 470_000.0,
    feedback_cap: 470e-12,
    clip_cap: 0.1e-6,
    coupling: 0.1e-6,
    series: 8_200.0,
    sustain_foot: 820.0,
    treble_cap: 0.0047e-6,
    treble_leg: 33_000.0,
    bass_leg: 33_000.0,
    bass_cap: 0.01e-6,
    out_bias: 390_000.0,
    out_collector: 10_000.0,
    out_emitter: 2_700.0,
};

/// Whether a stage carries clipping diodes. The Supa Tonebender leaves them
/// off the first one.
fn clips(v: &Voicing, first: bool) -> bool {
    !(first && v.input_collector == SUPA.input_collector && v.series == SUPA.series)
}

pub fn build(v: &Voicing, source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(v, source, load, "out")
}

pub fn tap(v: &Voicing, source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Big Muff Pi");
    net.supply("v9", 100.0, 9.0).capacitor("v9", "gnd", 100e-6);

    // --- input stage, Q4 --------------------------------------------------
    // Not a buffer: a full gain stage, biased by its own collector through
    // the feedback resistor, which is how all four of these are biased.
    net.input("in", source)
        .resistor("in", "c1", 33_000.0)
        .capacitor("c1", "q4b", v.coupling)
        .resistor("q4b", "gnd", 100_000.0)
        .resistor("q4b", "q4c", v.feedback)
        .capacitor("q4b", "q4c", v.feedback_cap)
        .supply("q4c", v.input_collector, 9.0)
        .resistor("q4e", "gnd", 100.0)
        .bipolar("q4c", "q4b", "q4e", FS36999)
        .capacitor("q4c", "sus_top", v.coupling)
        .pot("sus_top", "sus", "sus_foot", 100_000.0, Taper::Linear, SUSTAIN)
        .resistor("sus_foot", "gnd", v.sustain_foot);

    // --- the two clipping stages, Q3 and Q2 -------------------------------
    // Identical but for what is in front of them. The diodes go across the
    // feedback, so they lower each stage's gain rather than stopping its
    // output -- which is why a Muff sustains rather than gates.
    let mut node = String::from("sus");
    for (index, first) in [(3usize, true), (2usize, false)] {
        let b = format!("q{index}b");
        let c = format!("q{index}c");
        let e = format!("q{index}e");
        let cap = format!("q{index}cap");
        let clip = format!("q{index}clip");

        net.capacitor(&node, &cap, v.coupling)
            .resistor(&cap, &b, v.series)
            .resistor(&b, "gnd", 100_000.0)
            .resistor(&b, &c, v.feedback)
            .capacitor(&b, &c, v.feedback_cap)
            .supply(&c, v.stage_collector, 9.0)
            .resistor(&e, "gnd", 100.0)
            .bipolar(&c, &b, &e, FS36999);
        if clips(v, first) {
            net.capacitor(&b, &clip, v.clip_cap)
                .diode(&clip, &c, D1N914)
                .diode(&c, &clip, D1N914);
        }
        node = c;
    }

    // --- the tone control --------------------------------------------------
    // A treble leg through a small capacitor and a bass leg through a large
    // one, meeting at the wiper. Both are cuts, so the middle of the pot is a
    // scoop rather than flat -- the Muff's mid dip is the tone control itself
    // and not an extra network.
    // The last collector feeds the network directly -- there is no coupling
    // capacitor in front of it. C9 blocks the direct voltage on the treble
    // side and C8 blocks it on the bass side, so the pot sits at the
    // collector's own potential and the coupling capacitor comes *after* the
    // wiper. Putting it in front instead flattens the network into a plain
    // low pass and the mid dip disappears.
    net.capacitor(&node, "treble", v.treble_cap)
        .resistor("treble", "gnd", v.treble_leg)
        .resistor(&node, "bass", v.bass_leg)
        .capacitor("bass", "gnd", v.bass_cap)
        .pot("treble", "wiper", "bass", 100_000.0, Taper::Linear, TONE);

    // --- output stage, Q1 --------------------------------------------------
    net.capacitor("wiper", "q1b", v.coupling)
        .supply("q1b", v.out_bias, 9.0)
        .resistor("q1b", "gnd", 100_000.0)
        .supply("q1c", v.out_collector, 9.0)
        .resistor("q1e", "gnd", v.out_emitter)
        .bipolar("q1c", "q1b", "q1e", FS36999)
        // Off the collector: R6 above it and R4 below make a gain stage, not
        // a follower.
        .capacitor("q1c", "vol_top", v.coupling)
        .pot("vol_top", "out", "gnd", 100_000.0, Taper::Audio, VOLUME)
        .resistor("out", "gnd", load);

    net.build(at)
}
