//! What sharing an operating point between channels does to the audio.
//!
//! `Plugin::process` copies channel 0's voltage vector into every other
//! channel at the top of each block, to save them the DC hunt. The saving is
//! real. The question this asks is what else it does, because
//! `Simulation::apply_operating_point` does not only set the node voltages --
//! it also sets every capacitor's stored charge from them.
//!
//! Run at two block sizes. If the right channel's output depends on the block
//! size, the copy is reaching into the running state and not only into the
//! operating point.
//!
//! `cargo run --release --example sharing`

use gainstagefx::voice::{self, Chain, Settings};

const RATE: f64 = 48_000.0;

/// A second of two different signals: 220 Hz on the left, 330 Hz on the right,
/// at a level the circuit is voiced around.
fn signals(n: usize) -> (Vec<f64>, Vec<f64>) {
    let w = std::f64::consts::TAU / RATE;
    (
        (0..n).map(|i| 0.1 * (w * 220.0 * i as f64).sin()).collect(),
        (0..n).map(|i| 0.1 * (w * 330.0 * i as f64).sin()).collect(),
    )
}

/// When the second channel is handed the first one's solution.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Share {
    /// Never: each channel hunts its own operating point and keeps its own
    /// state. This is the reference.
    Never,
    /// At the top of every block, which is what `Plugin::process` used to do.
    EveryBlock,
    /// Only while there is an operating point to hand over rather than a
    /// running state, which is what it does now.
    WhenDirty,
}

/// Runs both channels the way the plugin does. Returns the right channel.
fn run(gain: voice::Gain, block: usize, share: Share, left: &[f64], right: &[f64]) -> Vec<f64> {
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
        let hand_over = match share {
            Share::Never => false,
            Share::EveryBlock => true,
            Share::WhenDirty => a.needs_operating_point(),
        };
        if hand_over {
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

fn difference(a: &[f64], b: &[f64]) -> (f64, f64) {
    let mut worst: f64 = 0.0;
    let mut energy = 0.0;
    let mut reference = 0.0;
    for (x, y) in a.iter().zip(b) {
        worst = worst.max((x - y).abs());
        energy += (x - y) * (x - y);
        reference += x * x;
    }
    let rms = (energy / a.len() as f64).sqrt();
    let level = (reference / a.len() as f64).sqrt().max(1e-30);
    (worst, 20.0 * (rms / level).log10())
}

fn main() {
    let n = RATE as usize;
    let (left, right) = signals(n);

    println!("The right channel, against the same channel run on its own.\n");
    println!("  circuit        sharing       block   worst sample     residual");
    for gain in [
        voice::Gain::Crunch,
        voice::Gain::Screamer,
        voice::Gain::Neve,
    ] {
        let alone = run(gain, n, Share::Never, &left, &right);
        for (name, policy) in [
            ("every block", Share::EveryBlock),
            ("when dirty", Share::WhenDirty),
        ] {
            for block in [64usize, 512] {
                let shared = run(gain, block, policy, &left, &right);
                let (worst, db) = difference(&alone, &shared);
                println!(
                    "  {:<13}  {name:<12}  {block:>5}   {worst:12.6}   {db:8.1} dB",
                    gain.name()
                );
            }
            // And the two block sizes against each other, which is the question
            // that matters: a plugin whose output depends on the host's buffer
            // size is a plugin that cannot be bounced.
            let at_64 = run(gain, 64, policy, &left, &right);
            let at_512 = run(gain, 512, policy, &left, &right);
            let (worst, db) = difference(&at_64, &at_512);
            println!(
                "  {:<13}  {name:<12}  64:512   {worst:12.6}   {db:8.1} dB",
                gain.name()
            );
        }
    }
}
