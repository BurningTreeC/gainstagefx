//! The half-step twin carries the circuit's state between the two rates
//! faithfully.
//!
//! A rescued sample hands the state the sample started from to a copy of the
//! circuit built at twice the rate, and may take the state back after two half
//! steps. Every reactance is re-expressed on the way: a capacitor's companion
//! term is `G v + i` with `G = 2C/T`, an inductor's is `-(i + G v)` with
//! `G = T/2L`, and a core keeps its own half-step. Get any of those wrong and
//! the error is not subtle -- a capacitor recurrence carrying the wrong term
//! reads 24 dB down -- but it only shows on the samples that are rescued, which
//! are the rare ones. So here the twin's answer is committed on *every* sample,
//! and the result has to be the same circuit simply run at twice the rate.

use super::*;
use crate::dsp::netlist::{BipolarSpec, CoreSpec, DiodeSpec, Netlist};

const RATES: [f64; 5] = [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0];

/// A transistor stage into a series inductor, a saturating core and a pair of
/// clipping diodes: every kind of state the twin has to carry.
fn circuit() -> Circuit {
    let mut net = Netlist::new("half-step twin");
    net.input("in", 10_000.0)
        .capacitor("in", "base", 1e-6)
        .supply("base", 100_000.0, 9.0)
        .resistor("base", "gnd", 22_000.0)
        .supply("collector", 4_700.0, 9.0)
        .resistor("emitter", "gnd", 1_000.0)
        .capacitor("emitter", "gnd", 10e-6)
        .bipolar("collector", "base", "emitter", BipolarSpec::NPN)
        .capacitor("collector", "mid", 1e-6)
        .inductor("mid", "out", 0.05)
        .core("out", "gnd", CoreSpec::STEEL)
        .diode("out", "gnd", DiodeSpec::SILICON)
        .diode("gnd", "out", DiodeSpec::SILICON)
        .resistor("out", "gnd", 10_000.0);
    net.build("out").expect("builds")
}

fn signal(k: usize, rate: f64) -> f64 {
    let t = k as f64 / rate;
    // Loud enough to clip the diodes and push the transistor about.
    0.8 * (std::f64::consts::TAU * 220.0 * t).sin()
        + 0.4 * (std::f64::consts::TAU * 1_370.0 * t).sin()
}

#[test]
fn committing_the_twin_every_sample_is_the_circuit_at_twice_the_rate() {
    for rate in RATES {
        let mut rescued = Simulation::new(circuit(), rate);
        rescued.enable_half_step_rescue();
        rescued.test_half_step_every_sample = true;
        let mut doubled = Simulation::new(circuit(), 2.0 * rate);
        let mut plain = Simulation::new(circuit(), rate);
        // All start from the same operating point, which does not depend on
        // the rate, so only the stepping differs.
        doubled.apply_operating_point(rescued.operating_point());
        plain.apply_operating_point(rescued.operating_point());

        let samples = (rate / 20.0) as usize;
        let mut previous = 0.0;
        let (mut peak, mut worst, mut step) = (0.0f64, 0.0f64, 0.0f64);
        for k in 0..samples {
            let x = signal(k, rate);
            let a = rescued.process(x);
            doubled.process(0.5 * (previous + x));
            let b = doubled.process(x);
            let c = plain.process(x);
            previous = x;
            assert!(a.is_finite() && b.is_finite());
            peak = peak.max(b.abs());
            worst = worst.max((a - b).abs());
            step = step.max((c - b).abs());
        }
        let (attempts, bridged, _) = rescued.half_step_health();
        assert_eq!(
            (attempts, bridged),
            (samples as u64, samples as u64),
            "at {rate} Hz every sample should have gone through the twin"
        );
        assert!(peak > 0.1, "at {rate} Hz the stage should be driven hard");
        // The two runs converge independently, each to the solver's tolerance,
        // and on a clipping stage with a slow inductor-and-core loop those
        // differences add up over time: measured, 1.6e-8 V at the start
        // growing to 2.4e-6 V over the 50 ms. A state carried wrongly is
        // tens of decibels, not that. The sharper test is against the step
        // size itself: the committed answer has to be the doubled-rate
        // circuit, at least 80 dB closer to it than the 1x one is.
        let db = 20.0 * (worst / peak).log10();
        let versus_step = 20.0 * (worst / step).log10();
        assert!(
            db < -90.0 && versus_step < -80.0,
            "at {rate} Hz the twin's committed answer is {db:.1} dB from the \
             circuit run at twice the rate, and only {versus_step:.1} dB closer \
             to it than the circuit at 1x; the state is being carried wrongly"
        );
    }
}

/// And an ordinary sample never reaches the twin at all: with nothing failing,
/// enabling the rescue changes nothing, to the bit.
#[test]
fn a_circuit_that_settles_is_untouched_by_the_rescue() {
    for rate in RATES {
        let mut plain = Simulation::new(circuit(), rate);
        let mut rescued = Simulation::new(circuit(), rate);
        rescued.enable_half_step_rescue();
        for k in 0..(rate / 20.0) as usize {
            let x = signal(k, rate);
            assert_eq!(
                plain.process(x).to_bits(),
                rescued.process(x).to_bits(),
                "at {rate} Hz, sample {k}"
            );
        }
        assert_eq!(plain.statistics().2, 0, "the test circuit should settle");
        assert_eq!(rescued.half_step_health().0, 0);
    }
}
