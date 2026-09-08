//! Two channels, and the one thing that has to be true of them.
//!
//! `Plugin::process` hands channel 0's operating point to every other channel
//! so only one of them has to hunt it. That is a real saving and it is sound
//! exactly once: while there is an operating point to hand over. After the
//! first sample the vector it copies is the *running* state, and
//! `Simulation::apply_operating_point` sets every capacitor's stored charge
//! from what it is given -- so handing it over again does not seed a solve, it
//! throws the receiving channel's audio away.
//!
//! Done at the top of every block, as it was, the right channel of a TS808
//! measured 5.9 dB of residual against the same channel run on its own, and
//! its output changed with the host's buffer size. `examples/sharing.rs` is
//! that measurement written out; this is the assertion.

use gainstagefx::voice::{self, Chain, Settings};

const RATE: f64 = 48_000.0;

/// Two different signals, because two identical ones would agree however
/// wrongly the channels were coupled.
fn signals(n: usize) -> (Vec<f64>, Vec<f64>) {
    let w = std::f64::consts::TAU / RATE;
    (
        (0..n).map(|i| 0.1 * (w * 220.0 * i as f64).sin()).collect(),
        (0..n).map(|i| 0.1 * (w * 330.0 * i as f64).sin()).collect(),
    )
}

/// The right channel, run the way the plugin runs it: settings applied once,
/// the operating point handed over only while there is one to hand over, and
/// the block boundary crossed every `block` samples.
fn right_channel(gain: voice::Gain, block: usize, left: &[f64], right: &[f64]) -> Vec<f64> {
    let settings = Settings {
        gain,
        ..Settings::default()
    };
    let mut a = Chain::new(RATE);
    let mut b = Chain::new(RATE);
    a.apply(&settings);
    b.apply(&settings);

    let mut out = Vec::with_capacity(left.len());
    let mut at = 0;
    while at < left.len() {
        let end = (at + block).min(left.len());
        if a.needs_operating_point() {
            a.find_operating_point();
            let op = a.operating_point().to_vec();
            b.share_operating_point_from(&op);
        }
        for i in at..end {
            a.process(left[i]);
            out.push(b.process(right[i]));
        }
        at = end;
    }
    out
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

/// The question that matters: a plugin whose output depends on the host's
/// buffer size cannot be bounced, because the bounce will not match what was
/// heard.
#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let n = RATE as usize / 4;
    let (left, right) = signals(n);
    for gain in [
        voice::Gain::Crunch,
        voice::Gain::Screamer,
        voice::Gain::Neve,
    ] {
        let at_64 = right_channel(gain, 64, &left, &right);
        let at_512 = right_channel(gain, 512, &left, &right);
        let db = residual_db(&at_64, &at_512);
        println!(
            "{}: 64 against 512 samples, {db:.1} dB residual",
            gain.name()
        );
        assert!(
            db < -120.0,
            "{}'s right channel changes with the block size: {db:.1} dB of residual",
            gain.name()
        );
    }
}

/// And the other half: the second channel has to be running its own circuit,
/// not being re-seeded from the first one's.
#[test]
fn the_second_channel_runs_its_own_circuit() {
    let n = RATE as usize / 4;
    let (left, right) = signals(n);
    for gain in [
        voice::Gain::Crunch,
        voice::Gain::Screamer,
        voice::Gain::Neve,
    ] {
        // One block for the whole render, so nothing is handed over after the
        // start: this is the channel on its own.
        let alone = right_channel(gain, n, &left, &right);
        let blocked = right_channel(gain, 64, &left, &right);
        let db = residual_db(&alone, &blocked);
        println!(
            "{}: against the same channel run alone, {db:.1} dB residual",
            gain.name()
        );
        assert!(
            db < -120.0,
            "{}'s right channel is not running its own circuit: {db:.1} dB of residual",
            gain.name()
        );
    }
}
