//! The Round Fuzz, against its schematic's bias points and behaviour.
#[path = "support/micpre.rs"]
mod micpre;
use gainstagefx::circuits::round_fuzz::{self, FUZZ};
use gainstagefx::dsp::time::Simulation;
use micpre::measure;

const RATE: f64 = 96_000.0;

/// With the analysis's own AC128 models (β 85 and 120) the pedal biases where a
/// good Fuzz Face is set: Q1's collector near -0.5 to -0.7 V and Q2's near -4.5 V.
/// Nothing was adjusted to get there.
#[test]
fn it_biases_where_a_good_one_is_set() {
    let c = round_fuzz::build(10_000.0, 470_000.0).unwrap();
    let (c1, c2) = (c.unknown_named("c1").unwrap(), c.unknown_named("c2").unwrap());
    let mut sim = Simulation::new(c, RATE);
    assert!(sim.find_operating_point());
    let (q1, q2) = (sim.voltage_at(c1), sim.voltage_at(c2));
    assert!((-0.9..-0.4).contains(&q1), "Q1 collector {q1}");
    assert!((-5.2..-3.8).contains(&q2), "Q2 collector {q2}");
}

/// Fuzz moves the gain by tens of decibels, and at a guitar's level turned up it
/// is a fuzz.
#[test]
fn fuzz_adds_gain_and_saturation() {
    let c = round_fuzz::build(10_000.0, 470_000.0).unwrap();
    let small = |f: f64| measure(&c, RATE, &[(FUZZ, f)], 1_000.0, 0.002).gain_db();
    assert!(small(1.0) > small(0.0) + 15.0, "{} {}", small(0.0), small(1.0));
    let driven = measure(&c, RATE, &[(FUZZ, 1.0)], 220.0, 0.122).thd_percent();
    assert!(driven > 30.0, "{driven}");
}

/// A small signal clips one side first: the asymmetric soft clipping the pedal is
/// known for, which is even harmonics before odd.
#[test]
fn a_quiet_note_clips_asymmetrically() {
    let c = round_fuzz::build(10_000.0, 470_000.0).unwrap();
    let m = measure(&c, RATE, &[(FUZZ, 0.5)], 220.0, 0.01);
    assert!(m.harmonic_percent(2) > m.harmonic_percent(3), "2nd {} 3rd {}", m.harmonic_percent(2), m.harmonic_percent(3));
}
