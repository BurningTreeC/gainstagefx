//! How loud each shipped preset is, exactly as it plays.
//!
//! `tests/presets.rs` holds the catalogue's spread under a threshold and names
//! only the two ends of it. This prints the whole list, loudest first, with the
//! pedal each preset carries, which is what a level decision needs: whether one
//! preset is out on its own, and whether the pedal presets sit with the rest.
//!
//! One tone at 220 Hz rather than loudness, the same as the test: a scooping
//! voicing really is ten decibels down *there* while being no quieter overall,
//! so read a few decibels as nothing and a dozen as something.
//!
//! `cargo run --release --example presetlevel`

use gainstagefx::dsp::measure::{self, Tone};
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
        let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
        let out = measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x))
            .fundamental()
            .magnitude()
            * trim
            * preset.mix as f64;
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
        let trim = if *trim == 0.0 { String::new() } else { format!("   trim {trim:+.0}") };
        println!("  {db:+7.1} dB  ({:+5.1} against the mean)  {name}{pedal}{trim}", db - mean);
    }
    let pedals: Vec<f64> =
        levels.iter().filter(|l| l.2 != PedalModel::None).map(|l| l.0).collect();
    if !pedals.is_empty() {
        let their_mean = pedals.iter().sum::<f64>() / pedals.len() as f64;
        println!(
            "\n{} of them carry a pedal, and average {their_mean:+.1} dB: {:+.1} against the rest",
            pedals.len(),
            their_mean - mean
        );
    }
}
