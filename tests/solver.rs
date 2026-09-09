//! That the nonlinear solve actually converges, on every voice, every sample.
//!
//! A solve that runs out of passes does not fail loudly. It leaves the solver
//! holding a partly converged answer, which goes out as audio, and it costs
//! the full iteration budget to produce -- so it is at once the least accurate
//! sample and the most expensive one. That combination is what a deadline miss
//! is made of, and nothing here was watching for it: the 5150 was failing to
//! converge 3304 times a second and the only visible symptom was that players
//! said it crackled.
//!
//! See `docs/experiments/crackle-root-cause.md`.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 48_000.0;

/// A second of plucked notes at full level. The attacks are where a nonlinear
/// solve is worked hardest, and a real part has one every few hundred
/// milliseconds.
fn hard(k: usize) -> f64 {
    let t = k as f64 / RATE;
    let env = (-((t % 0.5) * 6.0)).exp();
    12.0 * 0.9
        * env
        * ((std::f64::consts::TAU * 110.0 * t).sin()
            + 0.6 * (std::f64::consts::TAU * 330.0 * t).sin()
            + 0.3 * (std::f64::consts::TAU * 1757.0 * t).sin())
}

const VOICES: [Gain; 6] = [
    Gain::Peavey,
    Gain::Boogie,
    Gain::Neve,
    Gain::Muff,
    Gain::Screamer,
    Gain::Crunch,
];

fn run(gain: Gain) -> (u64, u64, u64, u64) {
    let netlist =
        voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, RATE);
    sim.set_control(0, 0.8);
    for k in 0..(RATE as usize) {
        sim.process(hard(k));
    }
    let (solves, _, unsettled, _) = sim.statistics();
    let (_, fallbacks, nonfinite) = sim.health();
    (solves, unsettled, fallbacks, nonfinite)
}

#[test]
fn every_voice_converges_on_every_sample() {
    for gain in VOICES {
        let (solves, unsettled, _, _) = run(gain);
        // The 73P has long had two, on the first samples after the operating
        // point; anything above a handful is the solver giving up on audio.
        assert!(
            unsettled <= 4,
            "{}: {unsettled} of {solves} solves ran out of passes",
            gain.name()
        );
    }
}

#[test]
fn no_voice_produces_a_non_finite_correction() {
    for gain in VOICES {
        let (_, _, _, nonfinite) = run(gain);
        assert_eq!(
            nonfinite,
            0,
            "{}: {nonfinite} Newton corrections were not numbers",
            gain.name()
        );
    }
}

#[test]
fn the_line_search_is_not_carrying_the_solve() {
    // It is there for the samples the plain step cannot manage, and those
    // should be rare. If a voice needs it constantly, the circuit or its
    // Jacobian wants looking at rather than more backtracking -- DAMPING.md
    // section 20.
    for gain in VOICES {
        let (solves, _, fallbacks, _) = run(gain);
        assert!(
            fallbacks * 20 < solves,
            "{}: the search failed to improve on {fallbacks} of {solves} solves",
            gain.name()
        );
    }
}

/// The work budget's floor is enforced by the solver, not by its caller.
///
/// BUG-008 recorded what a permanent ceiling of eight did to the 5150's lead
/// channel: 190, 107 and 428 per cent distortion at three input levels, which
/// is not a sound but a solve that never converged. Layer 5a lowers the
/// ceiling deliberately when a block is running out of time, so the number it
/// can reach has to be bounded somewhere that no caller can get under.
#[test]
fn no_caller_can_starve_the_solve_past_the_floor() {
    use gainstagefx::voice::{Amplifier, Diode};
    let netlist =
        voice::build_voice(Gain::Peavey, Diode::Silicon, Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, 96_000.0);
    sim.set_control(Gain::Peavey.drive_control(), 1.0);
    sim.find_operating_point();

    // Ask for one pass a sample, which is what a bug in the budget would do.
    sim.set_pass_ceiling(1);
    let tone = Tone::near(96_000.0, 16_384, 220.0, 2.0);
    let pinched = measure::run(tone, 9_600, |x| sim.process(x));

    sim.set_pass_ceiling(32);
    let netlist =
        voice::build_voice(Gain::Peavey, Diode::Silicon, Amplifier::Valve).expect("builds");
    let mut full = Simulation::new(netlist, 96_000.0);
    full.set_control(Gain::Peavey.drive_control(), 1.0);
    full.find_operating_point();
    let tone = Tone::near(96_000.0, 16_384, 220.0, 2.0);
    let whole = measure::run(tone, 9_600, |x| full.process(x));

    assert!(
        (pinched.thd_percent() - whole.thd_percent()).abs() < 2.0,
        "asking for one pass a sample should be clamped to the floor and sound \
         all but identical: {:.1} % against {:.1} %",
        pinched.thd_percent(),
        whole.thd_percent()
    );
    assert!(
        pinched.thd_percent() < 100.0,
        "a floored solve must still converge: {:.1} %",
        pinched.thd_percent()
    );
}
