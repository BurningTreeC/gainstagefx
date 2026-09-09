//! One jack, one signal.
//!
//! An amplifier has a single input and a guitar has a single output, so
//! `Plugin::process` sums the host's channels to mono *before* the circuit
//! rather than running two copies of it. This is the file that says what that
//! must mean, because two of the three things it has to get right are silent
//! when they are wrong.
//!
//! It is also what makes the catalogue meet its deadline live. A dual-mono
//! plugin fed a guitar track was solving the same circuit twice for the same
//! answer: half the cost of the plugin, spent computing numbers it already
//! had. The Mark IIC+ missed 87.3 % of its callbacks with two chains and none
//! with one.

use gainstagefx::voice::{self, Chain, Settings};

const RATE: f64 = 48_000.0;

/// Two different signals, because two identical ones would agree however
/// wrongly the channels were combined.
fn signals(n: usize) -> (Vec<f64>, Vec<f64>) {
    let w = std::f64::consts::TAU / RATE;
    (
        (0..n).map(|i| 0.1 * (w * 220.0 * i as f64).sin()).collect(),
        (0..n).map(|i| 0.1 * (w * 330.0 * i as f64).sin()).collect(),
    )
}

fn chain(gain: voice::Gain) -> Chain {
    let mut c = Chain::new(RATE);
    c.apply(&Settings {
        gain,
        ..Settings::default()
    });
    c.settle();
    c.find_operating_point();
    c
}

/// `Plugin::process`, reproduced: the frame averaged, solved once, and the one
/// answer written to every channel, with the block boundary crossed every
/// `block` samples.
fn render(gain: voice::Gain, block: usize, left: &[f64], right: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let settings = Settings {
        gain,
        ..Settings::default()
    };
    let mut c = chain(gain);
    let (mut l, mut r) = (
        Vec::with_capacity(left.len()),
        Vec::with_capacity(left.len()),
    );
    let mut at = 0;
    while at < left.len() {
        let end = (at + block).min(left.len());
        c.apply(&settings);
        if c.needs_operating_point() {
            c.find_operating_point();
        }
        for i in at..end {
            let out = c.process((left[i] + right[i]) / 2.0);
            l.push(out);
            r.push(out);
        }
        at = end;
    }
    (l, r)
}

fn residual_db(a: &[f64], b: &[f64]) -> f64 {
    let mut energy = 0.0;
    let mut reference = 0.0;
    for (x, y) in a.iter().zip(b) {
        energy += (x - y) * (x - y);
        reference += x * x;
    }
    let rms = (energy / a.len() as f64).sqrt();
    let level = (reference / a.len() as f64).sqrt().max(1e-30);
    20.0 * (rms / level).max(1e-30).log10()
}

const VOICES: [voice::Gain; 4] = [
    voice::Gain::Crunch,
    voice::Gain::Screamer,
    voice::Gain::Neve,
    voice::Gain::Twin,
];

/// The average, not the sum.
///
/// A mono track arrives at a stereo plugin as two identical channels. Adding
/// them would hand the circuit twice the voltage it was calibrated for, which
/// on a clean voice is six decibels and on a clipping one is a different
/// pedal. Averaging leaves it exactly as it was, which is the whole point:
/// dropping a mono guitar into this must not change how hard it is driven.
#[test]
fn a_mono_track_arrives_at_the_circuit_unchanged() {
    for gain in VOICES {
        let (signal, _) = signals(4_000);
        let (both, _) = render(gain, 64, &signal, &signal);

        // The same samples fed to one channel, which is what the mono layout
        // does and what the voice was calibrated against.
        let mut alone = chain(gain);
        let one: Vec<f64> = signal.iter().map(|&x| alone.process(x)).collect();

        let db = residual_db(&one, &both);
        println!("{}: {db:.0} dB", gain.name());
        assert!(
            db < -200.0,
            "{}: a mono track sent to both channels must reach the circuit at \
             the level it was calibrated for, and differed by {db:.0} dB -- \
             which is a sum where an average belongs",
            gain.name()
        );
    }
}

/// A stereo source is summed. That is what plugging one into a single
/// amplifier does, and it is the honest behaviour for a model of one -- but it
/// has to be stated, because it is the one thing this design costs.
#[test]
fn a_stereo_source_is_summed_before_the_circuit() {
    for gain in VOICES {
        let (left, right) = signals(4_000);
        let (out, _) = render(gain, 64, &left, &right);

        let mixed: Vec<f64> = left
            .iter()
            .zip(right.iter())
            .map(|(l, r)| (l + r) / 2.0)
            .collect();
        let mut alone = chain(gain);
        let direct: Vec<f64> = mixed.iter().map(|&x| alone.process(x)).collect();

        let db = residual_db(&direct, &out);
        assert!(
            db < -200.0,
            "{}: the circuit must see the mono sum and nothing else: {db:.0} dB",
            gain.name()
        );
    }
}

/// And the consequence, written down rather than discovered: two channels in
/// antiphase cancel, and the plugin goes quiet. A real amplifier fed the same
/// pair down one cable does the same thing.
#[test]
fn antiphase_channels_cancel() {
    let (signal, _) = signals(4_000);
    let inverted: Vec<f64> = signal.iter().map(|x| -x).collect();
    let (out, _) = render(voice::Gain::Crunch, 64, &signal, &inverted);

    let mut quiet = chain(voice::Gain::Crunch);
    let silence: Vec<f64> = signal.iter().map(|_| quiet.process(0.0)).collect();
    let worst = out
        .iter()
        .zip(silence.iter())
        .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
    assert!(
        worst < 1e-12,
        "channels in antiphase must cancel exactly, and left {worst:.3e}"
    );
}

/// Both outputs carry the same answer. There is one circuit; there is nothing
/// for them to differ about.
#[test]
fn both_outputs_are_the_same_signal() {
    for gain in VOICES {
        let (left, right) = signals(2_000);
        let (l, r) = render(gain, 64, &left, &right);
        assert_eq!(l, r, "{}: the two outputs differ", gain.name());
    }
}

/// A plugin whose output depends on the host's buffer size cannot be bounced,
/// because the bounce will not match what was heard.
#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let n = RATE as usize / 4;
    let (left, right) = signals(n);
    for gain in VOICES {
        let (at_64, _) = render(gain, 64, &left, &right);
        let (at_512, _) = render(gain, 512, &left, &right);
        let db = residual_db(&at_64, &at_512);
        println!("{}: 64 against 512, {db:.1} dB", gain.name());
        assert!(
            db < -120.0,
            "{} changes with the block size: {db:.1} dB of residual",
            gain.name()
        );
    }
}
