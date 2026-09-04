//! A bipolar transistor, by Ebers-Moll.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{BipolarSpec, Circuit, Netlist};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// A textbook common-emitter stage: divider bias, collector load, emitter
/// degeneration, optionally bypassed.
///
/// The bias has to leave the collector somewhere it can swing. A 1 k emitter
/// against this divider puts about a milliamp through it, which drops half
/// the supply across a 4.7 k collector load and leaves the other half to move
/// in. Get that wrong -- 470 ohms here, which asks for two milliamps -- and
/// the collector is on the floor before any signal arrives, the stage has no
/// gain at all, and *more* collector load makes it worse rather than better.
fn common_emitter(collector: f64, emitter: f64, bypass: bool, spec: BipolarSpec) -> Circuit {
    let mut net = Netlist::new("common emitter");
    net.input("in", 10_000.0)
        .capacitor("in", "base", 1e-6)
        .supply("base", 100_000.0, 9.0)
        .resistor("base", "gnd", 22_000.0)
        .supply("collector", collector, 9.0)
        .resistor("emitter", "gnd", emitter)
        .bipolar("collector", "base", "emitter", spec)
        .capacitor("collector", "out", 1e-6)
        .resistor("out", "gnd", 470_000.0);
    if bypass {
        net.capacitor("emitter", "gnd", 100e-6);
    }
    net.build("out").expect("builds")
}

fn run(c: &Circuit, volts: f64) -> measure::Measured {
    let mut sim = Simulation::new(c.clone(), RATE);
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 4.0) as usize, |x| sim.process(x))
}

/// With the emitter resistor left in circuit, a common-emitter stage
/// amplifies by the ratio of its two resistors. That is the first thing
/// anyone checks about one, and it holds only while the emitter is *not*
/// bypassed -- bypassed, the gain is set by the transistor's own internal
/// emitter resistance instead and is far higher.
#[test]
fn a_common_emitter_stage_amplifies_by_its_resistor_ratio() {
    let (rc, re) = (4_700.0, 1_000.0);
    let m = run(&common_emitter(rc, re, false, BipolarSpec::NPN_2SC3378), 0.001);
    let gain = m.gain_db();
    let expected = 20.0 * (rc / re).log10();
    println!("gain {gain:.1} dB, resistor ratio says {expected:.1} dB");
    assert!(
        (gain - expected).abs() < 6.0,
        "a stage with {rc} over {re} should be near {expected:.1} dB and is \
         {gain:.1}"
    );
}

/// More collector load, more gain.
#[test]
fn more_collector_load_is_more_gain() {
    let small = run(&common_emitter(2_200.0, 1_000.0, false, BipolarSpec::NPN_2SC3378), 0.001).gain_db();
    let large = run(&common_emitter(4_700.0, 1_000.0, false, BipolarSpec::NPN_2SC3378), 0.001).gain_db();
    println!("2.2k {small:.1} dB, 4.7k {large:.1} dB");
    let expected = 20.0 * (4_700.0f64 / 2_200.0).log10();
    assert!(
        (large - small - expected).abs() < 2.0,
        "{large:.1} dB against {small:.1} for {expected:.1} dB more load"
    );
}

/// A single-ended stage is asymmetric, so it makes even harmonics -- and a
/// bipolar's exponential curve makes them arrive rather more suddenly than a
/// valve's three-halves power does.
#[test]
fn it_makes_even_harmonics_and_bends_further_when_driven() {
    let c = common_emitter(4_700.0, 1_000.0, true, BipolarSpec::NPN_2SC3378);
    let quiet = run(&c, 0.0002);
    let loud = run(&c, 0.005);
    println!(
        "quiet {:.2} % ({:.2} second), loud {:.1} % ({:.1} second)",
        quiet.thd_percent(),
        quiet.harmonic_percent(2),
        loud.thd_percent(),
        loud.harmonic_percent(2)
    );
    assert!(
        quiet.harmonic_percent(2) > quiet.harmonic_percent(3),
        "a single-ended stage should lead with second harmonic"
    );
    assert!(
        loud.thd_percent() > quiet.thd_percent() * 3.0,
        "driven harder it should bend further: {:.1} % against {:.2} %",
        loud.thd_percent(),
        quiet.thd_percent()
    );
}

/// Bypassing the emitter takes the resistor out of the signal path and leaves
/// the transistor's own internal emitter resistance setting the gain, which is
/// a great deal smaller. That difference is the whole reason the capacitor is
/// there.
#[test]
fn bypassing_the_emitter_is_where_the_gain_comes_from() {
    let plain = run(&common_emitter(4_700.0, 1_000.0, false, BipolarSpec::NPN_2SC3378), 0.001).gain_db();
    let bypassed = run(&common_emitter(4_700.0, 1_000.0, true, BipolarSpec::NPN_2SC3378), 0.0002).gain_db();
    println!("unbypassed {plain:.1} dB, bypassed {bypassed:.1} dB");
    assert!(
        bypassed > plain + 15.0,
        "bypassing should buy a great deal of gain: {bypassed:.1} dB against \
         {plain:.1}"
    );
}

/// Driven hard enough the collector runs down to the emitter and stops, which
/// is saturation -- and a model with only the forward junction in it would
/// amplify forever instead.
#[test]
fn the_collector_runs_out_of_room() {
    let c = common_emitter(4_700.0, 1_000.0, true, BipolarSpec::NPN_2SC3378);
    // Far enough apart that the second one is genuinely against the supply:
    // the stage has to be driven past what its collector can swing before
    // there is anything to see.
    let modest = run(&c, 0.001).fundamental().magnitude();
    let slammed = run(&c, 0.5).fundamental().magnitude();
    let ratio = slammed / modest;
    println!("five hundred times the input gave {ratio:.1} times the output");
    assert!(
        ratio < 150.0,
        "five hundred times in gave {ratio:.1} times out, so nothing is limiting"
    );
}
