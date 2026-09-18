//! The Tube 610, against the 610-A drawing and its parts' data.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/micpre.rs"]
mod micpre;
use gainstagefx::circuits::tube610::{self as t610, LEVEL};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::Gain;
use micpre::*;

const RATE: f64 = 96_000.0;

/// UTC's O-1 on its 50 ohm winding: ±1 dB from 30 Hz to 20 kHz, and about 1 % at
/// its +8 dBm maximum level at 30 Hz.
#[test]
fn the_input_transformer_meets_utcs_rating() {
    let t = transformer(
        50.0,
        16.0,
        t610::INPUT_CORE,
        t610::INPUT_RATIO,
        3_900.0,
        40e-12,
        1_200_000.0,
    );
    let mid = measure(&t, RATE, &[], 1_000.0, 1e-3).gain_db();
    for hz in [30.0, 20_000.0] {
        let rel = measure(&t, RATE, &[], hz, 1e-3).gain_db() - mid;
        assert!(rel.abs() < 1.0, "{hz} Hz: {rel:+.2}");
    }
    let plus8 = (6.3e-3f64 * 50.0).sqrt() * 2f64.sqrt() * 2.0;
    let thd = measure(&t, 48_000.0, &[], 30.0, plus8).thd_percent();
    assert!((0.5..2.5).contains(&thd), "{thd}");
}

/// Every stage settles class A with its plate well inside the supply, and the
/// output valve's current is what a 12AY7 on 560 ohm draws.
#[test]
fn every_stage_biases_class_a() {
    let c = t610::build(50.0, 600.0).unwrap();
    let node = |n: &str| c.unknown_named(n).unwrap();
    let (p1, p2, p3, p4, k4) = (node("p1"), node("p2"), node("p3"), node("p4"), node("k4"));
    let mut sim = Simulation::new(c, RATE);
    assert!(sim.find_operating_point());
    for (name, at) in [("V1-A", p1), ("V1-B", p2), ("V2-A", p3)] {
        let v = sim.voltage_at(at);
        assert!((40.0..240.0).contains(&v), "{name} plate at {v}");
    }
    let output_ma = sim.voltage_at(k4) / 560.0 * 1e3;
    assert!((2.0..10.0).contains(&output_ma), "{output_ma} mA");
    assert!(sim.voltage_at(p4) > 250.0);
}

/// Level sweeps the gain up to about 70 dB, the response is flat within a
/// decibel and a half across the band, and pushed it bends with second harmonic
/// first, as a valve channel does.
#[test]
fn level_gain_response_and_character() {
    let c = t610::build(50.0, 600.0).unwrap();
    let full = measure(&c, RATE, &[(LEVEL, 1.0)], 1_000.0, 1e-5).gain_db();
    assert!((63.0..74.0).contains(&full), "{full}");
    assert!(measure(&c, RATE, &[(LEVEL, 0.5)], 1_000.0, 1e-5).gain_db() < full - 10.0);
    for hz in [30.0, 100.0, 10_000.0, 20_000.0] {
        let rel = measure(&c, RATE, &[(LEVEL, 1.0)], hz, 1e-5).gain_db() - full;
        assert!(rel.abs() < 1.6, "{hz} Hz: {rel:+.2}");
    }
    let pushed = measure(&c, RATE, &[(LEVEL, 1.0)], 220.0, 3e-3);
    assert!(pushed.thd_percent() > 0.2, "{}", pushed.thd_percent());
    assert!(pushed.harmonic_percent(2) > pushed.harmonic_percent(3));
}

#[test]
fn it_runs_in_the_chain_without_allocating() {
    realtime_safe(Gain::Tube610, |f| allocations::assert_no_heap(f));
}
