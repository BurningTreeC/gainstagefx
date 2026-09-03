//! The two solvers, checked against each other.
//!
//! A linear network has one right answer, and there are two ways here of
//! arriving at it: solve a complex system once at the frequency of interest,
//! or integrate through time and take the spectrum of what comes out. They
//! share the netlist and nothing else -- different arithmetic, different
//! matrices, different failure modes.
//!
//! So when they agree, both are working, and the agreement is the closest
//! thing to an independent check either can have. The previous version had one
//! way of measuring and no way of knowing whether it was right; three separate
//! measurement artefacts survived in it for a long time because there was
//! nothing to disagree with them.

use gainstagefx::circuits::tone;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{Circuit, Netlist, Taper};
use gainstagefx::dsp::{ac, time::Simulation};

const RATE: f64 = 96_000.0;
const WINDOW: usize = 16_384;

/// Runs a tone through the time domain solver and reads the gain off, then
/// asks the AC solver the same question directly.
fn compare(circuit: &Circuit, controls: &[f64], hz: f64) -> (f64, f64) {
    let tone = Tone::near(RATE, WINDOW, hz, 0.1);
    let mut sim = Simulation::new(circuit.clone(), RATE);
    for (which, &position) in controls.iter().enumerate() {
        sim.set_control(which, position);
    }
    // Long enough for the slowest thing in these networks to settle.
    let settle = (RATE * 0.5) as usize;
    let measured = measure::run(tone, settle, |x| sim.process(x));
    let direct = ac::solve(circuit, controls, tone.hz()).db();
    (measured.gain_db(), direct)
}

fn assert_agree(circuit: &Circuit, controls: &[f64], hz: f64, tolerance: f64) {
    let (stepped, direct) = compare(circuit, controls, hz);
    assert!(
        (stepped - direct).abs() < tolerance,
        "at {hz:.0} Hz the solvers disagree: stepped {stepped:.4} dB, direct {direct:.4} dB"
    );
}

/// Where a digital frequency lands on the analogue axis under the trapezoidal
/// rule.
///
/// The two solvers cannot agree exactly at any real fraction of the sample
/// rate, and it is not either one being wrong. Trapezoidal integration is the
/// bilinear transform, which compresses the infinite analogue axis onto the
/// finite digital one -- so a filter stepped through time behaves like the
/// continuous filter evaluated slightly higher up. Comparing against the
/// warped frequency turns the discrepancy from a tolerance to be swallowed
/// into a prediction to be checked.
fn warp(hz: f64, rate: f64) -> f64 {
    let w = std::f64::consts::TAU * hz;
    let t = 1.0 / rate;
    2.0 / t * (w * t / 2.0).tan() / std::f64::consts::TAU
}

#[test]
fn the_two_solvers_agree_on_a_low_pass() {
    let mut net = Netlist::new("rc");
    net.input("in", 100.0)
        .resistor("in", "out", 10_000.0)
        .capacitor("out", "gnd", 15.915_494e-9);
    let circuit = net.build("out").expect("builds");
    // Well below Nyquist the warping is not worth naming.
    for hz in [100.0, 500.0, 1_000.0, 4_000.0] {
        assert_agree(&circuit, &[], hz, 0.05);
    }
}

/// And where the warping does matter, it is exactly what the bilinear
/// transform says it should be -- which checks the discretisation itself, not
/// just the two solvers against each other.
#[test]
fn the_disagreement_is_exactly_the_frequency_warping() {
    let mut net = Netlist::new("rc");
    net.input("in", 100.0)
        .resistor("in", "out", 10_000.0)
        .capacitor("out", "gnd", 15.915_494e-9);
    let circuit = net.build("out").expect("builds");

    for hz in [8_000.0, 12_000.0, 20_000.0] {
        let tone = Tone::near(RATE, WINDOW, hz, 0.1);
        let mut sim = Simulation::new(circuit.clone(), RATE);
        let measured = measure::run(tone, (RATE * 0.2) as usize, |x| sim.process(x));
        let stepped = measured.gain_db();

        let naive = ac::solve(&circuit, &[], tone.hz()).db();
        let warped = ac::solve(&circuit, &[], warp(tone.hz(), RATE)).db();

        assert!(
            (stepped - warped).abs() < 0.02,
            "at {:.0} Hz: stepped {stepped:.4} dB against {warped:.4} dB warped \
             (unwarped would be {naive:.4})",
            tone.hz()
        );
        // And the warping is a real effect, not a rounding difference.
        if hz >= 12_000.0 {
            assert!(
                (naive - warped).abs() > 0.2,
                "the warping should be visible by {hz:.0} Hz"
            );
        }
    }
}

#[test]
fn the_two_solvers_agree_on_a_resonant_network() {
    let mut net = Netlist::new("lc");
    net.input("in", 100.0)
        .resistor("in", "out", 10_000.0)
        .resistor("out", "leg", 2_200.0)
        .inductor("leg", "cap", 0.5)
        .capacitor("cap", "gnd", 120e-9)
        .resistor("out", "gnd", 100_000.0);
    let circuit = net.build("out").expect("builds");
    // Including right at the notch, where the two are most likely to part.
    for hz in [100.0, 400.0, 650.0, 900.0, 5_000.0] {
        assert_agree(&circuit, &[], hz, 0.1);
    }
}

/// The real thing, with its controls in three different places.
#[test]
fn the_two_solvers_agree_on_the_tone_section() {
    let circuit = tone::build(&tone::WIDE, 38_000.0, 220_000.0).expect("builds");
    for controls in [[1.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.4, 0.6, 0.3]] {
        for hz in [82.0, 550.0, 2_000.0, 8_000.0] {
            assert_agree(&circuit, &controls, hz, 0.15);
        }
    }
}

/// A pot's position has to mean the same thing to both.
#[test]
fn the_two_solvers_agree_about_a_control() {
    let mut net = Netlist::new("divider");
    net.input("in", 100.0)
        .pot("in", "out", "gnd", 100_000.0, Taper::Linear, 0)
        .resistor("out", "gnd", 10_000_000.0);
    let circuit = net.build("out").expect("builds");
    for position in [0.1, 0.5, 0.9] {
        assert_agree(&circuit, &[position], 1_000.0, 0.05);
    }
}
