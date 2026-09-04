//! The 5150 lead preamplifier, against what its schematic says.

use gainstagefx::circuits::evh5150::{self, PRE, TONE_STACK_INPUT};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn at(node: &str, hz: f64, volts: f64, pre: f64) -> measure::Measured {
    let c = evh5150::tap(10_000.0, TONE_STACK_INPUT, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(PRE, pre);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

/// Six triode stages, and the reason six of them do not simply oscillate is
/// that most of what each one makes is thrown away before the next one sees
/// it. R32 and R87 are 1 M in series into 100 k and 1 M to ground.
///
/// This is arithmetic off the drawing, not a matter of taste: 100 k against
/// 1.1 M is a fifth of a decibel over twenty, and 1 M against 2 M is six.
#[test]
fn the_interstage_dividers_throw_away_what_the_drawing_says() {
    let after_v2b = 20.0 * (100_000.0f64 / 1_100_000.0).log10();
    let after_v5b = 20.0 * (1_000_000.0f64 / 2_000_000.0).log10();
    println!("R32/R101 {after_v2b:.1} dB, R87/R91 {after_v5b:.1} dB");
    assert!((after_v2b + 20.8).abs() < 0.5);
    assert!((after_v5b + 6.0).abs() < 0.1);

    // And the measured step from one plate to the next carries that loss plus
    // the stage's own gain, so it must be well short of a bare triode's.
    let v2b = at("v2b_p", 1_000.0, 1e-6, 0.5).gain_db();
    let v5b = at("v5b_p", 1_000.0, 1e-6, 0.5).gain_db();
    println!("V2B plate {v2b:.1} dB, V5B plate {v5b:.1} dB, step {:.1}", v5b - v2b);
    assert!(
        v5b - v2b < 20.0,
        "a stage behind a 21 dB divider cannot gain {:.1} dB",
        v5b - v2b
    );
}

/// R15 is 39 k at V2A's cathode with nothing across it, where every other
/// stage on this drawing has 1.8 k or 2.2 k fully bypassed. That is not a
/// gain stage; it is a valve run cold so it clips one side long before the
/// other. It should contribute almost nothing to the small signal gain.
#[test]
fn the_cold_clipper_makes_almost_no_gain() {
    let before = at("v1b_p", 1_000.0, 1e-6, 0.5).gain_db();
    let after = at("v2a_p", 1_000.0, 1e-6, 0.5).gain_db();
    println!("V1B plate {before:.1} dB, V2A plate {after:.1} dB, step {:.1}", after - before);
    assert!(
        (after - before) < 6.0,
        "the cold clipper gained {:.1} dB, so it is not running cold",
        after - before
    );

    // A bypassed stage next to it gains thirty odd, which is what makes the
    // comparison mean anything.
    let v1a = at("v1a_p", 1_000.0, 1e-6, 0.5).gain_db();
    println!("V1A, fully bypassed: {v1a:.1} dB");
    assert!(v1a > 25.0, "a bypassed 12AX7 should gain more than that: {v1a:.1} dB");
}

/// R88 is 1 M from the output back to V5A's grid, and V5A is driven through
/// R87's 1 M. Shunt feedback between two equal resistors is unity, so the
/// last preamplifier stage gives back none of its gain -- it is there to
/// drive the tone stack, not to amplify into it.
///
/// This is the reading that had to be got right: R88 crosses the plate line
/// on the drawing without joining it. If it *did* join the plate, 1 M from a
/// couple of hundred volts through R91 would sit a hundred of them on the
/// grid, and the stage would not conduct at all.
#[test]
fn the_last_stage_is_a_unity_gain_driver() {
    let v5b = at("v5b_p", 1_000.0, 1e-6, 1.0).gain_db();
    let out = at("out", 1_000.0, 1e-6, 1.0).gain_db();
    println!("V5B plate {v5b:.1} dB, output {out:.1} dB, V5A round trip {:.1}", out - v5b);
    assert!(
        (out - v5b).abs() < 6.0,
        "R87 into R88 is unity, so V5A should come out level: {:.1} dB",
        out - v5b
    );
}

/// R89 470 k into the stack's 33 k is twenty four decibels lost between the
/// preamplifier and the tone controls, before the stack itself cuts anything.
#[test]
fn the_tone_stack_is_handed_a_quiet_signal() {
    let out = at("out", 1_000.0, 1e-6, 1.0).gain_db();
    let stack = at("stack", 1_000.0, 1e-6, 1.0).gain_db();
    let arithmetic =
        20.0 * (TONE_STACK_INPUT / (470_000.0 + TONE_STACK_INPUT)).log10();
    println!("{out:.1} dB to {stack:.1} dB, arithmetic says {arithmetic:.1}");
    assert!(
        ((stack - out) - arithmetic).abs() < 1.5,
        "measured {:.1} dB against {arithmetic:.1} from the values",
        stack - out
    );
}

/// The bottom end is down on the midrange before any tone control is reached.
/// Nothing here is deliberately cutting bass -- the cathode capacitors are 1
/// uF and bypass from a few hundred hertz down -- but six stages of it adds
/// up, and that is where this amplifier's tightness comes from.
#[test]
fn the_bottom_end_is_already_down_before_the_tone_stack() {
    let low = at("stack", 60.0, 1e-6, 1.0).gain_db();
    let mid = at("stack", 1_000.0, 1e-6, 1.0).gain_db();
    println!("{low:.1} dB at 60 Hz against {mid:.1} at 1 kHz");
    assert!(
        mid - low > 10.0,
        "only {:.1} dB between them, which is not a 5150",
        mid - low
    );
}

/// Six stages is a great deal of gain and the pre control has to cover it.
#[test]
fn the_pre_gain_covers_the_whole_range() {
    let shut = at("stack", 1_000.0, 1e-4, 0.0).gain_db();
    let open = at("stack", 1_000.0, 1e-4, 1.0).gain_db();
    println!("pre gain: {shut:.1} dB to {open:.1} dB");
    assert!(open - shut > 40.0, "only {:.1} dB of control", open - shut);
    assert!(open > 80.0, "six triodes should reach some gain: {open:.1} dB");
}

/// Driven, it distorts, and a cold clipper in the middle of the chain means
/// it leads with odd harmonics rather than the second.
#[test]
fn it_distorts_and_leads_with_odd_harmonics() {
    let m = at("stack", 1_000.0, 0.004, 1.0);
    println!(
        "{:.1} % distortion, 2nd {:.1} %, 3rd {:.1} %, 5th {:.1} %",
        m.thd_percent(),
        m.harmonic_percent(2),
        m.harmonic_percent(3),
        m.harmonic_percent(5)
    );
    assert!(m.thd_percent() > 25.0, "only {:.1} % distortion", m.thd_percent());
    assert!(
        m.harmonic_percent(3) > m.harmonic_percent(2),
        "third {:.1} % against second {:.1} %",
        m.harmonic_percent(3),
        m.harmonic_percent(2)
    );
}

/// And every stage has to reach an operating point, which six triodes on one
/// rail is not guaranteed to do.
#[test]
fn every_stage_reaches_an_operating_point() {
    for node in ["v1a_p", "pre_w", "v1b_p", "v2a_p", "v2b_p", "v5b_p", "out", "stack"] {
        let c = evh5150::tap(10_000.0, TONE_STACK_INPUT, node).expect("builds");
        let mut sim = Simulation::new(c, RATE);
        assert!(sim.find_operating_point(), "{node} never settled");
    }

    // Silence in, silence out -- at the jack, not at a plate, which sits some
    // hundreds of volts up by design.
    let c = evh5150::tap(10_000.0, TONE_STACK_INPUT, "stack").expect("builds");
    let mut sim = Simulation::new(c, RATE);
    let mut worst: f64 = 0.0;
    for _ in 0..(RATE as usize / 4) {
        worst = worst.max(sim.process(0.0).abs());
    }
    assert!(worst < 0.05, "the output put out {worst:.4} on silence");
}
