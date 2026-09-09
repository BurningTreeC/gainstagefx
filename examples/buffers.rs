//! Does a bigger buffer fix a voice that misses its deadline?
//!
//! The instinct is that it must: a 1024-sample callback has sixteen times the
//! budget of a 64-sample one. It does, and it also has sixteen times the work
//! to do in it, so **the ratio does not move**. Buffer size cannot rescue a
//! voice whose *mean* cost is over realtime; all it can do is average out the
//! spikes of a voice whose mean already fits and whose tail does not.
//!
//! Which of those two situations a voice is in is the whole question, and it
//! is not answerable from the mean or the worst case alone. So this reports
//! both, per block size, for the settings a shipped preset actually uses --
//! not a stress configuration -- because that is what the player has loaded.
//!
//! `cargo run --release --example buffers`

use gainstagefx::voice::{Cabinet, Chain, Gain, Iron, Settings, Tone as ToneSection};

const RATE: f64 = 48_000.0;
const BLOCKS: [usize; 7] = [16, 32, 64, 128, 256, 512, 1024];
/// Dual mono: both channels are processed inside the one callback.
const CHANNELS: usize = 2;

/// The material a nonlinear solve is worked hardest by: plucked notes at full
/// level, each one a step that makes a warm-started solve start cold.
fn material(k: usize) -> f64 {
    let t = k as f64 / RATE;
    let since = (t * 8.0).fract() / 8.0;
    let envelope = (-since * 14.0).exp();
    0.9 * envelope
        * ((std::f64::consts::TAU * 110.0 * t).sin()
            + 0.6 * (std::f64::consts::TAU * 330.0 * t).sin()
            + 0.3 * (std::f64::consts::TAU * 1757.0 * t).sin())
        / 1.9
}

fn sweep(name: &str, settings: &Settings) {
    println!("=== {name} ===");
    println!(
        "  {:<8}{:>9}{:>9}{:>9}{:>9}{:>10}{:>9}",
        "block", "budget", "mean", "p50", "p99", "worst", "over"
    );
    for block in BLOCKS {
        let budget = block as f64 / RATE * 1e6;
        let mut channels: Vec<Chain> = (0..CHANNELS)
            .map(|_| {
                let mut c = Chain::new(RATE);
                c.apply(settings);
                c.settle();
                c.find_operating_point();
                c
            })
            .collect();
        // Warm, so the operating-point hunt is not in the timing.
        let mut k = 0usize;
        for _ in 0..2000 {
            for c in channels.iter_mut() {
                std::hint::black_box(c.process(material(k)));
            }
            k += 1;
        }
        // Two seconds of callbacks, so even 1024 gets a useful population.
        let blocks = (RATE * 2.0 / block as f64) as usize;
        let mut times = Vec::with_capacity(blocks);
        for _ in 0..blocks {
            let start = std::time::Instant::now();
            for _ in 0..block {
                let x = material(k);
                for c in channels.iter_mut() {
                    std::hint::black_box(c.process(x));
                }
                k += 1;
            }
            times.push(start.elapsed().as_secs_f64() * 1e6);
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let over = times.iter().filter(|t| **t > budget).count();
        times.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a duration"));
        let at = |q: f64| times[((times.len() - 1) as f64 * q).round() as usize];
        println!(
            "  {block:<8}{budget:>8.0}u{mean:>8.0}u{:>8.0}u{:>8.0}u{:>9.0}u{:>8.1}%",
            at(0.5),
            at(0.99),
            times[times.len() - 1],
            100.0 * over as f64 / blocks as f64,
        );
    }
    let realtime = "mean over budget means no buffer size can help: the work \
                    scales with the block.";
    println!("  {realtime}\n");
}

fn main() {
    println!("48 kHz, {CHANNELS} channels, microseconds per callback.\n");

    // What the shipped "Boutique Lead" preset actually sets: the amplifier's
    // own tone stack, a cabinet, no transformer, no oversampling.
    sweep(
        "Mark IIC+, as the shipped preset has it",
        &Settings {
            gain: Gain::Boogie,
            drive: 0.85,
            tone: ToneSection::Off,
            cabinet: Cabinet::Stack,
            iron: Iron::Off,
            oversampling: 1,
            bass: 0.60,
            mid: 0.25,
            treble: 0.70,
            ..Settings::default()
        },
    );

    // And with the transformer switched in, which is a third nonlinear solve
    // and the difference between the two harnesses' disagreeing figures.
    sweep(
        "Mark IIC+, with the transformer in",
        &Settings {
            gain: Gain::Boogie,
            drive: 0.85,
            tone: ToneSection::Off,
            cabinet: Cabinet::Stack,
            iron: Iron::Steel,
            oversampling: 1,
            bass: 0.60,
            mid: 0.25,
            treble: 0.70,
            ..Settings::default()
        },
    );

    sweep(
        "5150, as the shipped preset has it",
        &Settings {
            gain: Gain::Peavey,
            drive: 0.90,
            tone: ToneSection::Scooping,
            cabinet: Cabinet::Stack,
            iron: Iron::Off,
            oversampling: 1,
            bass: 0.70,
            mid: 0.15,
            treble: 0.75,
            ..Settings::default()
        },
    );
}
