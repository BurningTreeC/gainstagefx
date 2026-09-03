//! The two clipping families.

use gainstagefx::circuits::clipper::{self, Placement, Values};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::DiodeSpec;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn run(v: &Values, volts: f64, gain: f64) -> measure::Measured {
    let circuit = clipper::build(v, 10_000.0, 100_000.0).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(clipper::GAIN, gain);
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x))
}

/// A pair of matched diodes facing opposite ways clips both halves the same,
/// so the waveform has half wave symmetry and there is no even harmonic to
/// have. Not approximately none: none.
#[test]
fn a_symmetric_pair_makes_no_even_harmonics() {
    for (name, v) in [
        ("in the loop", clipper::OVERDRIVE),
        ("to ground", clipper::DISTORTION),
    ] {
        let m = run(&v, 0.2, 0.6);
        assert!(
            m.harmonic_percent(2) < 0.2,
            "{name} made {:.3} % second harmonic",
            m.harmonic_percent(2)
        );
        assert!(
            m.harmonic_percent(3) > 5.0,
            "{name} should be making third: {:.2} %",
            m.harmonic_percent(3)
        );
    }
}

/// Diodes to ground are a ceiling: past it the output stops going anywhere.
/// Diodes in a loop only lower the gain, so the output keeps following the
/// input. That is the whole difference between the two pedals.
#[test]
fn to_ground_is_a_ceiling_and_in_the_loop_is_not() {
    let ceiling = |v: &Values| {
        let quiet = run(v, 0.1, 0.6).fundamental().magnitude();
        let loud = run(v, 0.8, 0.6).fundamental().magnitude();
        loud / quiet
    };
    let held = ceiling(&clipper::DISTORTION);
    let following = ceiling(&clipper::OVERDRIVE);
    assert!(
        held < 1.3,
        "a clipper to ground should hold its output: it grew {held:.2} times"
    );
    assert!(
        following > held * 1.2,
        "a clipper in the loop should keep following: {following:.2} against {held:.2}"
    );
}

/// The three diodes differ by their forward voltage, which is the whole reason
/// anyone swaps them, and each behaves differently *with level* rather than
/// just being louder or quieter.
#[test]
fn the_diodes_arrive_at_different_levels() {
    let thd = |spec: DiodeSpec, volts: f64| {
        let mut v = clipper::OVERDRIVE;
        v.diode = spec;
        run(&v, volts, 0.5).thd_percent()
    };
    // Quietly: germanium is already bending, an LED has not started.
    assert!(
        thd(DiodeSpec::GERMANIUM, 0.02) > 2.0,
        "germanium should already be working: {:.2} %",
        thd(DiodeSpec::GERMANIUM, 0.02)
    );
    assert!(
        thd(DiodeSpec::LED, 0.02) < 1.0,
        "an LED should still be clean: {:.2} %",
        thd(DiodeSpec::LED, 0.02)
    );
    // Driven hard, the LED arrives all at once and overtakes.
    assert!(
        thd(DiodeSpec::LED, 0.4) > thd(DiodeSpec::GERMANIUM, 0.4),
        "the LED should overtake when driven: {:.1} % against {:.1} %",
        thd(DiodeSpec::LED, 0.4),
        thd(DiodeSpec::GERMANIUM, 0.4)
    );
    // And germanium holds the output lower at every setting.
    let mut ge = clipper::OVERDRIVE;
    ge.diode = DiodeSpec::GERMANIUM;
    assert!(
        run(&ge, 0.2, 0.5).fundamental().magnitude()
            < run(&clipper::OVERDRIVE, 0.2, 0.5).fundamental().magnitude(),
        "germanium should hold the output lower"
    );
}

/// The gain control has to sweep rather than do nothing and then everything.
/// With the pot in the lower leg of the divider instead of the feedback path,
/// the gain goes as 1 + Rf/(pot + Rleg) and the whole useful range sits in the
/// last of the travel.
#[test]
fn the_gain_control_sweeps_evenly() {
    let v = clipper::DISTORTION;
    let at = |g: f64| run(&v, 0.001, g).gain_db();
    // Small enough that nothing is clipping, so this measures the gain and
    // not the ceiling.
    let (shut, half, open) = (at(0.0), at(0.5), at(1.0));
    assert!(open > shut + 12.0, "the control barely moves: {shut:.1} to {open:.1} dB");
    let travelled = (half - shut) / (open - shut);
    assert!(
        (0.3..0.7).contains(&travelled),
        "halfway is {:.0} % of the way. A linear track cannot do better than \
         about {:.0} % here -- in decibels it front-loads -- so the taper has \
         to carry the span.",
        travelled * 100.0,
        {
            let k: f64 = (clipper::DISTORTION.feedback + clipper::DISTORTION.sweep)
                / clipper::DISTORTION.feedback;
            (k / 2.0).ln() / k.ln() * 100.0
        }
    );
}

/// The capacitor in the gain leg means the stage only has its full gain above
/// a corner, so the bottom end is amplified and clipped far less than the
/// middle. That is the mid-hump every one of these pedals is known for, and it
/// is one capacitor rather than an equaliser.
#[test]
fn the_gain_leg_leaves_the_bottom_end_alone() {
    let circuit = clipper::build(&clipper::OVERDRIVE, 10_000.0, 100_000.0).expect("builds");
    let thd_at = |hz: f64| {
        let mut sim = Simulation::new(circuit.clone(), RATE);
        sim.set_control(clipper::GAIN, 0.6);
        // The level matters: driven hard enough, every frequency clips and
        // the difference disappears into a flat 28 % across the band. The
        // claim is about where clipping *starts*, so measure where the bottom
        // end has not started yet.
        let tone = Tone::near(RATE, 16_384, hz, 0.02);
        measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x)).thd_percent()
    };
    let bottom = thd_at(60.0);
    let middle = thd_at(700.0);
    assert!(
        middle > bottom * 5.0,
        "the bottom should be clipped far less: {bottom:.1} % at 60 Hz against \
         {middle:.1} % at 700"
    );
}

/// Both arrangements have to be reachable, and be different circuits.
#[test]
fn the_two_placements_are_not_the_same_circuit() {
    let loop_c = clipper::build(&clipper::OVERDRIVE, 10_000.0, 100_000.0).expect("builds");
    let ground = clipper::build(&clipper::DISTORTION, 10_000.0, 100_000.0).expect("builds");
    assert_eq!(clipper::OVERDRIVE.placement, Placement::InTheLoop);
    assert_eq!(clipper::DISTORTION.placement, Placement::ToGround);
    assert!(!loop_c.is_linear() && !ground.is_linear());
}
