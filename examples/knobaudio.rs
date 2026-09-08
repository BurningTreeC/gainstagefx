//! What moving a knob does to audio already in flight, through the whole
//! plugin rather than through one circuit.
//!
//! `examples/knobmove.rs` asks the same question of a `Simulation` and gets a
//! clean answer -- a pot moving is a conductance moving, and the circuit walks
//! to its new operating point through its own time constants. So anything
//! audible has to be above that, in the chain or in the plugin's own per-block
//! work.
//!
//! Two probes, both of which have a right answer that does not need a
//! reference render:
//!
//! * **Block-size independence.** The same knob sweep, rendered at 64 samples
//!   and at 512. A host's buffer setting is not a sound. If the two renders
//!   differ, something is happening once a block that should not be.
//! * **Channel independence.** Two channels fed *different* audio with the
//!   same knob sweep on both, against each channel rendered on its own. A
//!   stereo plugin is dual mono here; one channel must not be able to reach
//!   the other.
//!
//! `cargo run --release --example knobaudio`

use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};

const RATE: f64 = 48_000.0;

fn settings(gain: Gain, drive: f64) -> Settings {
    Settings {
        gain,
        drive,
        tone: ToneSection::Scooping,
        cabinet: Cabinet::Stack,
        oversampling: 1,
        ..Settings::default()
    }
}

/// A knob being turned: the drive swept across its travel, applied once a
/// block the way `Plugin::process` applies it.
///
/// The value is quantised to a grid coarser than any block size compared
/// against it. Without that, the two renders would be turning the knob on
/// different schedules -- 64 steps against 512 -- and would differ for that
/// reason alone, which says nothing about the plugin. On this grid both
/// renders move the knob at exactly the same sample positions, so what is
/// left is the block size and nothing else.
const GRID: usize = 512;

fn sweep_at(k: usize, total: usize) -> f64 {
    let step = (k / GRID) * GRID;
    0.15 + 0.7 * (step as f64 / total as f64)
}

fn left(k: usize) -> f64 {
    0.25 * (k as f64 * 0.031).sin()
}

fn right(k: usize) -> f64 {
    0.25 * (k as f64 * 0.017).sin() + 0.1 * (k as f64 * 0.09).sin()
}

/// Renders `channels` in lockstep the way the plugin does: settings applied to
/// every chain once a block, then the block run a frame at a time. Returns
/// each channel's output.
fn render(gain: Gain, block: usize, seconds: f64, sources: &[fn(usize) -> f64]) -> Vec<Vec<f64>> {
    let total = (RATE * seconds) as usize;
    let mut chains: Vec<Chain> = sources
        .iter()
        .map(|_| {
            let mut c = Chain::new(RATE);
            c.apply(&settings(gain, sweep_at(0, total)));
            c.settle();
            c
        })
        .collect();
    let mut out: Vec<Vec<f64>> = sources.iter().map(|_| Vec::with_capacity(total)).collect();

    let mut k = 0usize;
    while k < total {
        let n = block.min(total - k);
        let set = settings(gain, sweep_at(k, total));
        for chain in &mut chains {
            chain.apply(&set);
        }
        // The plugin's operating-point hand-over, reproduced exactly.
        if chains.len() >= 2 && chains[0].needs_operating_point() {
            let (first, rest) = chains.split_at_mut(1);
            first[0].find_operating_point();
            for chain in rest {
                chain.share_operating_point_from(first[0].operating_point());
            }
        }
        for j in 0..n {
            for (c, chain) in chains.iter_mut().enumerate() {
                let x = sources[c](k + j);
                out[c].push(chain.process(x));
            }
        }
        k += n;
    }
    out
}

fn residual_db(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..n {
        num += (a[i] - b[i]).powi(2);
        den += a[i] * a[i];
    }
    if den <= 0.0 {
        return f64::NEG_INFINITY;
    }
    10.0 * (num / den).log10()
}

fn main() {
    println!("A knob swept across its travel while audio is playing.\n");

    println!("Block-size independence: the same sweep at 64 and at 512 samples.");
    println!("Stereo, because the plugin's operating-point hand-over only runs");
    println!("with more than one channel and that is where the cost was.");
    println!("  {:<12}{:>14}{:>14}", "voice", "left", "right");
    for gain in [Gain::Peavey, Gain::Boogie, Gain::Screamer, Gain::Crunch] {
        let a = render(gain, 64, 0.5, &[left, right]);
        let b = render(gain, 512, 0.5, &[left, right]);
        println!(
            "  {:<12}{:>11.1} dB{:>11.1} dB",
            gain.name(),
            residual_db(&a[0], &b[0]),
            residual_db(&a[1], &b[1]),
        );
    }

    println!("\nChannel independence: two channels of different audio, swept");
    println!("together, against each one rendered alone.");
    println!("  {:<12}{:>14}{:>14}", "voice", "left", "right");
    for gain in [Gain::Peavey, Gain::Boogie, Gain::Screamer, Gain::Crunch] {
        let both = render(gain, 64, 0.5, &[left, right]);
        let only_l = render(gain, 64, 0.5, &[left]);
        let only_r = render(gain, 64, 0.5, &[right]);
        println!(
            "  {:<12}{:>11.1} dB{:>11.1} dB",
            gain.name(),
            residual_db(&both[0], &only_l[0]),
            residual_db(&both[1], &only_r[0]),
        );
    }

    println!("\nBoth should be far below anything audible. A residual up near");
    println!("the signal means the knob is not moving a pot, it is throwing the");
    println!("audio away and starting again.");
}
