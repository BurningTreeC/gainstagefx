//! Stages in a row.

use gainstagefx::circuits::preamp::{self, Preamp};
use gainstagefx::circuits::valve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn run(p: &Preamp, volts: f64, gain: f64) -> measure::Measured {
    let circuit = preamp::build(p, 10_000.0, 1_000_000.0).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(preamp::GAIN, gain);
    sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE * 0.4) as usize, |x| sim.process(x))
}

/// Every stage's plate has to sit somewhere sensible, not just the first.
/// A cascade whose second stage is biased against a rail is a cascade that
/// clips on one side only, and nothing about the output level says so.
#[test]
fn every_stage_finds_its_own_operating_point() {
    let p = preamp::HIGH_GAIN;
    let circuit = preamp::build(&p, 10_000.0, 1_000_000.0).expect("builds");
    let mut sim = Simulation::new(circuit.clone(), RATE);
    sim.set_control(preamp::GAIN, 1.0);
    assert!(sim.find_operating_point(), "the cascade did not settle");

    for index in 0..p.stages {
        let plate = circuit
            .names
            .iter()
            .position(|n| n == &format!("v{index}_plate"))
            .expect("named plate");
        let cathode = circuit
            .names
            .iter()
            .position(|n| n == &format!("v{index}_cathode"))
            .expect("named cathode");
        let vp = sim.voltage_at(plate);
        let vk = sim.voltage_at(cathode);
        assert!(
            (60.0..280.0).contains(&vp),
            "stage {index} has its plate at {vp:.1} V, which is against a rail"
        );
        assert!(
            (0.3..4.0).contains(&vk),
            "stage {index} has its cathode at {vk:.2} V, so it is not biased"
        );
        // And the current adds up, stage by stage.
        let current = (p.values.supply - vp) / p.values.plate_load;
        let implied = current * p.values.cathode;
        assert!(
            (vk - implied).abs() < 0.05,
            "stage {index} loses current: cathode {vk:.3} V against {implied:.3} V implied"
        );
    }
}

/// Another valve is not a louder valve. The stages compound: each one
/// amplifies the last one's distortion along with its signal.
#[test]
fn each_stage_multiplies_the_one_before() {
    let quiet = 0.02;
    let one = run(&preamp::CLEAN, quiet, 1.0).gain_db();
    let two = run(&preamp::CRUNCH, quiet, 1.0).gain_db();
    assert!(
        two > one + 20.0,
        "a second stage added only {:.1} dB",
        two - one
    );
}

/// The whole point of the range: one stage is a preamplifier that colours,
/// three is a wall.
#[test]
fn the_range_runs_from_subtle_to_saturated() {
    let subtle = run(&preamp::CLEAN, 0.1, 1.0);
    assert!(
        subtle.thd_percent() < 2.0,
        "one stage should stay subtle at a tenth of a volt: {:.1} %",
        subtle.thd_percent()
    );
    // ...and be doing something, or it is a wire.
    assert!(
        subtle.thd_percent() > 0.1,
        "one stage should still colour: {:.2} %",
        subtle.thd_percent()
    );

    let saturated = run(&preamp::HIGH_GAIN, 0.1, 1.0);
    assert!(
        saturated.thd_percent() > 30.0,
        "three stages should be well into it: {:.1} %",
        saturated.thd_percent()
    );
}

/// A single ended stage is second-harmonic first; a cascade of them starts
/// making third as the later stages clip both sides.
#[test]
fn the_cascade_grows_odd_harmonics_the_deeper_it_goes() {
    let one = run(&preamp::CLEAN, 0.1, 1.0);
    let three = run(&preamp::HIGH_GAIN, 0.1, 1.0);
    let ratio = |m: &measure::Measured| m.harmonic_percent(3) / m.harmonic_percent(2).max(1e-9);
    assert!(
        ratio(&three) > ratio(&one) * 5.0,
        "third against second: one stage {:.4}, three stages {:.4}",
        ratio(&one),
        ratio(&three)
    );
}

/// The gain control has to reach the whole cascade, not just the first valve.
#[test]
fn the_gain_control_reaches_every_stage() {
    // Measured where the cascade is still linear. At a twentieth of a volt
    // three thousand times over is already clipping, and what a gain control
    // does to a stage that is clipping is compression rather than gain -- two
    // stages then read eight decibels where the arithmetic says eleven, and
    // the number is about the level, not the control.
    let open = run(&preamp::CRUNCH, 0.002, 1.0);
    let shut = run(&preamp::CRUNCH, 0.002, 0.0);
    assert!(
        open.gain_db() > shut.gain_db() + 10.0,
        "two stages of bypass should move more than one: {:.1} dB against {:.1} dB",
        open.gain_db(),
        shut.gain_db()
    );
}

/// Composing a stage has to give the same amplifier as writing one out.
///
/// The one-stage preamplifier is a written-out stage plus its gain control, so
/// it has exactly one node more -- the pot's wiper -- and that is the whole
/// difference. Turned all the way up the wiper sits on the input, and the only
/// thing left between the two circuits is the pot's own million ohms loading
/// the source. Against a ten thousand ohm source that is a small, calculable
/// loss, and it is the tolerance below rather than a fudge factor: a bare
/// divider of 1 M against 10 k gives 20*log10(1M / 1.01M), which is 0.086 dB.
#[test]
fn a_composed_stage_matches_the_written_one() {
    const SOURCE: f64 = 10_000.0;
    const LEAK: f64 = 1_000_000.0;
    let written = valve::build(&valve::CLASSIC, SOURCE, LEAK).expect("builds");
    let composed = preamp::build(&preamp::CLEAN, SOURCE, LEAK).expect("builds");
    assert_eq!(
        composed.nodes,
        written.nodes + 1,
        "the gain control is one node: a wiper"
    );

    // Every control all the way up in both, rather than one by name. The two
    // circuits do not number their controls the same way -- the composed one
    // has a gain control the written one does not -- so naming a single index
    // compared a fully bypassed cathode against a half bypassed one and read
    // the 3.5 dB difference between those as an error in the composition.
    let gain = |circuit: gainstagefx::dsp::netlist::Circuit| {
        let controls = circuit.controls;
        let mut sim = Simulation::new(circuit, RATE);
        for i in 0..controls {
            sim.set_control(i, 1.0);
        }
        sim.find_operating_point();
        let tone = Tone::near(RATE, 16_384, 220.0, 0.05);
        measure::run(tone, (RATE * 0.3) as usize, |x| sim.process(x)).gain_db()
    };
    let a = gain(written);
    let b = gain(composed);
    let expected = 20.0 * (LEAK / (LEAK + SOURCE)).log10();
    assert!(
        (b - a - expected).abs() < 0.05,
        "the volume pot should cost {expected:.3} dB and cost {:.3}: \
         {a:.4} dB written against {b:.4} dB composed",
        b - a
    );
}
