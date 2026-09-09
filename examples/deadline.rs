//! What the host actually has to fit into a callback.
//!
//! Crackle is a missed deadline, not a high average. `examples/wherecpu.rs`
//! answers "how much of a second does a second of audio cost", which is the
//! average over a whole second and hides the shape of the distribution
//! entirely. A voice can average well inside realtime and still drop out, if
//! the samples that cost the most happen to land in the same callback.
//!
//! So this measures the thing the host measures: the wall time of one
//! `process` callback, over and over, and reports the distribution against
//! the deadline. At 48 kHz and 64 samples that deadline is 1333 us, and a
//! stereo instance has to do both channels inside it.
//!
//! It also runs the transformer, tone stack and cabinet, because the plugin
//! does, and reports the solver's own telemetry beside the timing so a change
//! in cost can be attributed to a change in the solve.
//!
//! `cargo run --release --example deadline`

use gainstagefx::voice::{self, Cabinet, Chain, Gain, Iron, Tone as ToneSection};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
/// The callback budget, in microseconds: BLOCK / RATE.
const DEADLINE: f64 = BLOCK as f64 / RATE * 1e6;
/// Stereo. Both channels are processed inside the one callback.
const CHANNELS: usize = 2;

fn chain(gain: Gain) -> Chain {
    let mut c = Chain::new(RATE);
    c.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    c.set_tone_section(ToneSection::Scooping);
    c.set_cabinet(Cabinet::Stack);
    c.set_iron(Iron::Steel);
    c.set_oversampling(4);
    c.set_drive(0.8);
    c.settle();
    c.find_operating_point();
    c
}

/// Plucked notes at full level: attack, sustain, decay, repeated. The attack
/// is where a nonlinear solve is worked hardest, and a real part has one
/// every few hundred milliseconds.
fn stimulus(k: usize) -> f64 {
    let t = k as f64 / RATE;
    let env = (-((t % 0.5) * 6.0)).exp();
    0.9 * env
        * ((std::f64::consts::TAU * 110.0 * t).sin()
            + 0.6 * (std::f64::consts::TAU * 330.0 * t).sin()
            + 0.3 * (std::f64::consts::TAU * 1757.0 * t).sin())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn main() {
    println!(
        "48 kHz, {BLOCK} samples a callback, {CHANNELS} channels, drive 0.8, \
         transformer + tone + cabinet on."
    );
    println!("Deadline is {DEADLINE:.0} us a callback. Over that is a dropout.\n");
    println!(
        "  {:<12}{:>9}{:>9}{:>9}{:>9}{:>9}{:>8}{:>9}",
        "voice", "mean", "p50", "p95", "p99", "worst", "over", "passes"
    );

    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Neve,
        Gain::Muff,
        Gain::Screamer,
        Gain::Crunch,
        Gain::Overdrive,
        Gain::Distortion,
    ] {
        let mut chains: Vec<Chain> = (0..CHANNELS).map(|_| chain(gain)).collect();
        // Warm, so the operating-point hunt is not in the timing.
        for k in 0..2000 {
            for c in chains.iter_mut() {
                std::hint::black_box(c.process(stimulus(k)));
            }
        }
        let blocks = RATE as usize / BLOCK; // one second of callbacks
        let mut times = Vec::with_capacity(blocks);
        let mut k = 2000usize;
        for _ in 0..blocks {
            let start = std::time::Instant::now();
            for _ in 0..BLOCK {
                for c in chains.iter_mut() {
                    std::hint::black_box(c.process(stimulus(k)));
                }
                k += 1;
            }
            times.push(start.elapsed().as_secs_f64() * 1e6);
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let over = times.iter().filter(|t| **t > DEADLINE).count();
        let mut sorted = times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  {:<12}{:>8.0}u{:>8.0}u{:>8.0}u{:>8.0}u{:>8.0}u{:>7.1}%{:>9.2}",
            gain.name(),
            mean,
            percentile(&sorted, 0.50),
            percentile(&sorted, 0.95),
            percentile(&sorted, 0.99),
            sorted[sorted.len() - 1],
            100.0 * over as f64 / blocks as f64,
            chains[0].passes_per_sample(),
        );
    }
    println!("\n'over' is the share of callbacks that missed the deadline.");
}
