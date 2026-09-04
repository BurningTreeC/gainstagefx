//! The Big Muff Pi, against what its schematics say.

use gainstagefx::circuits::bigmuff::{self, Voicing, SUSTAIN, TONE, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn at(v: &Voicing, node: &str, hz: f64, volts: f64, sus: f64, tone: f64) -> measure::Measured {
    let c = bigmuff::tap(v, 10_000.0, 470_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(SUSTAIN, sus);
    sim.set_control(TONE, tone);
    sim.set_control(VOLUME, 1.0);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

/// The tone control is the mid scoop. Both of its legs are cuts, so with the
/// wiper in the middle neither the bass leg nor the treble leg is doing much
/// and the middle of the band falls between them. That notch is the whole
/// reason a Muff sits underneath a mix rather than on top of it, and it is
/// the control itself rather than an extra network.
///
/// Measured across the network alone. Taken at the output it looks like a
/// plain low pass, because the 470 pF across each clipping stage's feedback
/// has already rolled the top off before the tone control sees it -- which is
/// on the drawing too.
#[test]
fn the_tone_control_is_the_mid_scoop() {
    let v = bigmuff::RAMS_HEAD;
    let network = |hz: f64, tone: f64| {
        at(&v, "wiper", hz, 1e-4, 1.0, tone).gain_db() - at(&v, "q2c", hz, 1e-4, 1.0, tone).gain_db()
    };
    let (low, notch, high) = (network(80.0, 0.5), network(1000.0, 0.5), network(8000.0, 0.5));
    println!("at noon: {low:.1} dB at 80 Hz, {notch:.1} at 1 kHz, {high:.1} at 8 kHz");
    assert!(
        notch < low - 3.0 && notch < high - 3.0,
        "the middle should sit below both ends: {low:.1} / {notch:.1} / {high:.1}"
    );
}

/// And the two ends of the control are a low pass and a high pass.
#[test]
fn the_tone_control_runs_from_dark_to_bright() {
    let v = bigmuff::RAMS_HEAD;
    let network = |hz: f64, tone: f64| {
        at(&v, "wiper", hz, 1e-4, 1.0, tone).gain_db() - at(&v, "q2c", hz, 1e-4, 1.0, tone).gain_db()
    };
    let dark = (network(80.0, 0.0), network(8000.0, 0.0));
    let bright = (network(80.0, 1.0), network(8000.0, 1.0));
    println!("shut: {:.1} / {:.1};  open: {:.1} / {:.1}", dark.0, dark.1, bright.0, bright.1);
    assert!(dark.0 > dark.1 + 10.0, "shut it should be a low pass");
    assert!(bright.1 > bright.0 + 10.0, "open it should be a high pass");
}

/// Four stages, and the gain builds through them. The two in the middle are
/// where the diodes are.
#[test]
fn the_gain_builds_through_four_stages() {
    let v = bigmuff::RAMS_HEAD;
    let g = |node: &str| at(&v, node, 1000.0, 1e-4, 1.0, 0.5).gain_db();
    let (first, second, third) = (g("q4c"), g("q3c"), g("q2c"));
    println!("stages: {first:.1} -> {second:.1} -> {third:.1} dB");
    assert!(second > first + 10.0 && third > second + 10.0, "each stage should add gain");
    assert!(third > 45.0, "three stages should reach real gain: {third:.1} dB");
}

/// Matched pairs of 1N914 across each feedback loop are symmetric, so a Muff
/// makes odd harmonics and no even ones. That is what it is.
#[test]
fn it_clips_symmetrically() {
    let m = at(&bigmuff::RAMS_HEAD, "out", 1000.0, 0.03, 1.0, 0.5);
    println!(
        "{:.1} % distortion, 2nd {:.2} %, 3rd {:.1} %",
        m.thd_percent(),
        m.harmonic_percent(2),
        m.harmonic_percent(3)
    );
    assert!(m.harmonic_percent(2) < 1.5, "symmetric clipping makes no even harmonic");
    assert!(m.thd_percent() > 50.0, "a Muff should be thoroughly squared off");
}

/// The sustain control has to take it from nearly clean to all of it.
#[test]
fn the_sustain_control_covers_the_range() {
    let v = bigmuff::RAMS_HEAD;
    let shut = at(&v, "out", 1000.0, 0.03, 0.0, 0.5).thd_percent();
    let open = at(&v, "out", 1000.0, 0.03, 1.0, 0.5).thd_percent();
    println!("sustain: {shut:.1} % to {open:.1} %");
    assert!(shut < 10.0 && open > 50.0, "{shut:.1} % to {open:.1} %");
}

/// The versions are different pedals, not the same one relabelled. The Supa
/// Tonebender has the first clipping stage's diodes left out altogether,
/// which is one change and audible.
#[test]
fn the_versions_differ() {
    let thd = |v: &Voicing| at(v, "out", 1000.0, 0.03, 1.0, 0.5).thd_percent();
    let (rams, triangle, supa) =
        (thd(&bigmuff::RAMS_HEAD), thd(&bigmuff::TRIANGLE), thd(&bigmuff::SUPA));
    println!("Ram's Head {rams:.1} %, Triangle {triangle:.1} %, Supa {supa:.1} %");
    assert!(
        (rams - triangle).abs() > 2.0 || (rams - supa).abs() > 2.0,
        "three versions that measure the same are one version"
    );
    assert!(supa < rams, "a stage with its diodes removed should clip less");
}

/// Every version builds and settles, and silence in is silence out.
#[test]
fn every_version_settles() {
    for (name, v) in [
        ("Ram's Head", bigmuff::RAMS_HEAD),
        ("Triangle", bigmuff::TRIANGLE),
        ("Supa", bigmuff::SUPA),
    ] {
        let c = bigmuff::build(&v, 10_000.0, 470_000.0).expect("builds");
        let mut sim = Simulation::new(c, RATE);
        assert!(sim.find_operating_point(), "{name} never settled");
        let mut worst: f64 = 0.0;
        for _ in 0..(RATE as usize / 4) {
            worst = worst.max(sim.process(0.0).abs());
        }
        assert!(worst < 0.05, "{name} put out {worst:.4} on silence");
    }
}
