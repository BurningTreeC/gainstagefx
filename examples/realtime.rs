//! Whether the plugin makes its deadline, and by how much.
//!
//! This is the measurement `CLAUDE.md` §5 asks for and BUG-002 has recorded as
//! missing since the first audit. Everything else here measures *throughput* --
//! how long two seconds of audio takes to compute -- and throughput is the
//! wrong number. A callback that is fast on average and occasionally takes four
//! times as long drops out, and its average looks fine.
//!
//! So: real blocks, timed one at a time, reported as a distribution against the
//! deadline that block has to meet. Two channels, because the plugin is dual
//! mono and both of them have to fit inside the same callback.
//!
//! The material is deliberately hostile. A quiet sine is the cheapest thing a
//! nonlinear circuit ever sees: it sits near its operating point and the warm
//! start is nearly right every sample. What costs is a signal that keeps
//! moving the solve somewhere new -- an attack, a chord, a passage that clips
//! and recovers -- so the worst case is measured on transients at full level
//! with the drive control being swept underneath.
//!
//! `cargo run --release --example realtime`

use gainstagefx::voice::{self, Cabinet, Chain, Gain, Settings, Tone as ToneSection};

/// Rates and block sizes from `CLAUDE.md` §5, with the two it singles out --
/// 48 kHz / 64 and 96 kHz / 64 -- always included.
const RATES: [f64; 5] = [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0];
const BLOCKS: [usize; 7] = [16, 32, 64, 128, 256, 512, 1024];

/// A block's worth of the worst thing a circuit sees.
///
/// Not a sine. A plucked note's attack is a step into a nonlinear circuit, and
/// a step is what makes a warm-started solve start cold.
fn material(k: usize, rate: f64) -> f64 {
    let t = k as f64 / rate;
    // A note every eighth of a second, decaying, with a hard edge on each.
    let phase = t * 8.0;
    let since = phase.fract() / 8.0;
    let envelope = (-since * 14.0).exp();
    let note = 110.0 * (1.0 + 0.25 * (phase as usize % 4) as f64);
    let body = (std::f64::consts::TAU * note * t).sin()
        + 0.6 * (std::f64::consts::TAU * note * 2.0 * t).sin()
        + 0.4 * (std::f64::consts::TAU * note * 3.0 * t).sin();
    // Full scale, because that is where the circuits work hardest.
    (body / 2.0) * envelope
}

struct Distribution {
    micros: Vec<f64>,
}

impl Distribution {
    fn at(&mut self, q: f64) -> f64 {
        self.micros
            .sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a duration"));
        let i = ((self.micros.len() - 1) as f64 * q).round() as usize;
        self.micros[i]
    }
    fn worst(&self) -> f64 {
        self.micros.iter().cloned().fold(0.0, f64::max)
    }
}

fn measure(gain: Gain, rate: f64, block: usize, seconds: f64) -> Distribution {
    // Two chains, because the plugin is dual mono and both have to fit in one
    // callback.
    let mut channels: Vec<Chain> = (0..2)
        .map(|_| {
            let mut c = Chain::new(rate);
            c.apply(&Settings {
                gain,
                drive: 0.8,
                tone: ToneSection::Scooping,
                cabinet: Cabinet::Stack,
                oversampling: 1,
                ..Settings::default()
            });
            c.settle();
            c
        })
        .collect();

    let blocks = (rate * seconds / block as f64) as usize;
    let mut micros = Vec::with_capacity(blocks);
    let mut k = 0usize;
    for b in 0..blocks {
        // Sweep the drive underneath, once a block, which is what a host doing
        // automation does and what makes the circuit rebuild its matrix.
        let drive = 0.25 + 0.7 * ((b as f64 * 0.01).sin() * 0.5 + 0.5);
        let start = std::time::Instant::now();
        for chain in channels.iter_mut() {
            chain.set_drive(drive);
        }
        for _ in 0..block {
            let x = material(k, rate);
            for chain in channels.iter_mut() {
                std::hint::black_box(chain.process(x));
            }
            k += 1;
        }
        micros.push(start.elapsed().as_secs_f64() * 1e6);
    }
    Distribution { micros }
}

fn main() {
    println!("Two channels, hostile material, drive swept under it.");
    println!("Every figure is microseconds for one callback, against the");
    println!("deadline that callback has to meet.\n");

    let voices = [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Muff,
        Gain::Screamer,
        Gain::Neve,
        Gain::Crunch,
    ];

    // The two the brief singles out, in full.
    for &(rate, block) in &[(48_000.0, 64usize), (96_000.0, 64usize)] {
        println!("=== {} kHz / {block} samples ===", rate / 1000.0);
        let budget = block as f64 / rate * 1e6;
        println!("  budget {budget:.0} us\n");
        println!(
            "  {:<12}{:>9}{:>9}{:>9}{:>9}{:>10}",
            "voice", "median", "p95", "p99", "worst", "of budget"
        );
        for gain in voices {
            let mut d = measure(gain, rate, block, 4.0);
            let (median, p95, p99, worst) = (d.at(0.5), d.at(0.95), d.at(0.99), d.worst());
            println!(
                "  {:<12}{median:>9.1}{p95:>9.1}{p99:>9.1}{worst:>9.1}{:>9.0}%",
                gain.name(),
                worst / budget * 100.0
            );
        }
        println!();
    }

    // And the whole grid, worst case only, for the voices that cost the most.
    for gain in [Gain::Peavey, Gain::Boogie] {
        println!("=== {} : worst callback as a percentage of its budget ===", gain.name());
        print!("  {:<10}", "rate");
        for b in BLOCKS {
            print!("{:>8}", b);
        }
        println!();
        for rate in RATES {
            print!("  {:<10}", format!("{:.1}k", rate / 1000.0));
            for block in BLOCKS {
                let budget = block as f64 / rate * 1e6;
                let d = measure(gain, rate, block, 1.5);
                print!("{:>7.0}%", d.worst() / budget * 100.0);
            }
            println!();
        }
        println!();
    }
    let _ = voice::VOICES;
}
