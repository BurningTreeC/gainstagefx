//! What switching a voice does to the audio, measured rather than described.
//!
//! Reported as a loud spike when switching to the Mark IIC+ or the 5150 from
//! another model. Those two are the only voices with a power stage, which is
//! the first thing worth noticing about the report.
//!
//! The experiment plays a steady tone, switches the voice mid-stream the way
//! the plugin does, and reports the largest sample in the second after the
//! switch against the largest in the second before it. A switch is allowed to
//! change the level -- a 5150 is not a Crunch -- so the number that matters is
//! not the ratio itself but whether there is a single sample far above what
//! the voice settles to.
//!
//! `cargo run --release --example switching`

use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;

fn settings(gain: Gain) -> Settings {
    Settings {
        gain,
        drive: 0.7,
        tone: ToneSection::Scooping,
        cabinet: Cabinet::Stack,
        oversampling: 1,
        ..Settings::default()
    }
}

fn tone(k: usize) -> f64 {
    0.2 * (k as f64 * 0.028).sin()
}

/// Runs `channels` chains the way `Plugin::process` does, switching from
/// `from` to `to` half way through. Returns each channel's output.
fn render(from: Gain, to: Gain, channels: usize, seconds: f64) -> Vec<Vec<f64>> {
    let total = (RATE * seconds) as usize;
    let switch = total / 2;

    let mut chains: Vec<Chain> = (0..channels)
        .map(|_| {
            let mut c = Chain::new(RATE);
            c.apply(&settings(from));
            c.settle();
            c
        })
        .collect();
    let mut out: Vec<Vec<f64>> = (0..channels).map(|_| Vec::with_capacity(total)).collect();

    let mut k = 0usize;
    while k < total {
        let n = BLOCK.min(total - k);
        let set = settings(if k < switch { from } else { to });
        for chain in &mut chains {
            chain.apply(&set);
        }
        // The plugin's operating-point hand-over, exactly as it stands.
        if chains.len() >= 2 && chains[0].needs_operating_point() {
            let (first, rest) = chains.split_at_mut(1);
            first[0].find_operating_point();
            for chain in rest {
                chain.share_operating_point_from(first[0].operating_point());
                if let Some(op) = first[0].iron_operating_point() {
                    chain.share_iron_operating_point_from(op);
                }
            }
        }
        for j in 0..n {
            for (c, chain) in chains.iter_mut().enumerate() {
                out[c].push(chain.process(tone(k + j)));
            }
        }
        k += n;
    }
    out
}

/// The largest sample in a window, and where the run settles: the largest in
/// the last tenth of a second, long after any switching transient.
fn spike(y: &[f64], switch: usize) -> (f64, f64) {
    let after = &y[switch..];
    let peak = after.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let tail = &y[y.len().saturating_sub(RATE as usize / 10)..];
    let settled = tail.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    (peak, settled)
}

fn main() {
    println!("Switching to a voice while a tone is playing.\n");
    println!("`peak` is the largest sample after the switch, `settled` the");
    println!("largest once it has run for a while. Their ratio is the spike.\n");
    println!(
        "  {:<26}{:>10}{:>11}{:>10}",
        "switch", "channel", "peak", "settled"
    );

    let seconds = 1.0;
    let switch = (RATE * seconds) as usize / 2;

    // Where the transient sits, and how wide it is, for the pairing that
    // shows it. One number cannot tell a click from a settling ramp.
    {
        let out = render(Gain::Muff, Gain::Peavey, 1, seconds);
        let y = &out[0];
        let (at, peak) =
            y.iter()
                .enumerate()
                .skip(switch)
                .fold((0usize, 0.0f64), |(ai, av), (i, v)| {
                    if v.abs() > av {
                        (i, v.abs())
                    } else {
                        (ai, av)
                    }
                });
        println!(
            "  Big Muff -> 5150: peak {peak:.3} at sample {} after the switch",
            at - switch
        );
        let wide = y[switch..].iter().filter(|v| v.abs() > 1.0).count();
        println!("  samples above 1.0 after the switch: {wide}");
        print!("  first 12 samples after the switch:");
        for v in y[switch..switch + 12].iter() {
            print!(" {v:.3}");
        }
        println!("\n  last 6 before it:");
        print!("   ");
        for v in y[switch - 6..switch].iter() {
            print!(" {v:.3}");
        }
        println!("\n");
    }

    for (from, to) in [
        (Gain::Crunch, Gain::Boogie),
        (Gain::Crunch, Gain::Peavey),
        (Gain::Muff, Gain::Peavey),
        (Gain::Crunch, Gain::Neve),
        (Gain::Crunch, Gain::Screamer),
    ] {
        let out = render(from, to, 2, seconds);
        for (c, y) in out.iter().enumerate() {
            let (peak, settled) = spike(y, switch);
            let ratio = if settled > 0.0 { peak / settled } else { 0.0 };
            let flag = if ratio > 4.0 { "  <-- spike" } else { "" };
            println!(
                "  {:<26}{:>10}{peak:>11.3}{settled:>10.3}   {ratio:>6.1}x{flag}",
                format!("{} -> {}", from.name(), to.name()),
                if c == 0 { "left" } else { "right" },
            );
        }
    }
}
