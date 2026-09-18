//! How loud each shipped preset is, exactly as it plays.
//!
//! `tests/presets.rs` holds the catalogue's spread under a threshold and names
//! only the two ends of it. This prints the whole list, loudest first, with the
//! pedal each preset carries, which is what a level decision needs: whether one
//! preset is out on its own, and whether the pedal presets sit with the rest.
//!
//! Measured **broadband**, on a low chord, the same as the test. It was one
//! 220 Hz tone, which is not loudness: a preset with a heavily shaped pedal in
//! front read at the mean on that sine while sitting nineteen decibels above it
//! in the room, because 220 Hz fell in the trough between the pedal's bands.
//!
//! `cargo run --release --example presetlevel`

use gainstagefx::params::PedalModel;
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

fn main() {
    let mut levels: Vec<(f64, &str, PedalModel, f32)> = Vec::new();
    for preset in PRESETS {
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        chain.settle();
        let trim = 10f64.powf(preset.output_trim as f64 / 20.0);
        let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
        let n = (RATE / 2.0) as usize;
        let partials = [(82.4, 0.5), (123.5, 0.3), (246.9, 0.2)];
        let at = |k: usize| -> f64 {
            let t = k as f64 / RATE;
            amplitude
                * partials
                    .iter()
                    .map(|(hz, a)| a * (std::f64::consts::TAU * hz * t).sin())
                    .sum::<f64>()
        };
        for k in 0..n {
            chain.process(at(k));
        }
        let mut sum = 0.0;
        for k in n..2 * n {
            let y = chain.process(at(k));
            sum += y * y;
        }
        let out = (sum / n as f64).sqrt() * trim * preset.mix as f64;
        let db = 20.0 * out.max(1e-9).log10() - NOMINAL_DBFS;
        levels.push((db, preset.name, preset.pedal, preset.output_trim));
    }
    levels.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mean = levels.iter().map(|l| l.0).sum::<f64>() / levels.len() as f64;
    println!("{} presets, mean {mean:+.1} dB\n", levels.len());
    for (db, name, pedal, trim) in &levels {
        let pedal = match pedal {
            PedalModel::None => String::new(),
            other => format!("   pedal {other:?}"),
        };
        let trim = if *trim == 0.0 {
            String::new()
        } else {
            format!("   trim {trim:+.0}")
        };
        println!(
            "  {db:+7.1} dB  ({:+5.1} against the mean)  {name}{pedal}{trim}",
            db - mean
        );
    }
    let pedals: Vec<f64> = levels
        .iter()
        .filter(|l| l.2 != PedalModel::None)
        .map(|l| l.0)
        .collect();
    if !pedals.is_empty() {
        let their_mean = pedals.iter().sum::<f64>() / pedals.len() as f64;
        println!(
            "\n{} of them carry a pedal, and average {their_mean:+.1} dB: {:+.1} against the rest",
            pedals.len(),
            their_mean - mean
        );
    }
}
