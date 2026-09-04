//! What each shipped preset costs, set up exactly as it ships.

use gainstagefx::circuits::tone;
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;
const SECONDS: f64 = 1.0;

fn main() {
    println!("{:<24}{:>10}{:>14}", "", "load", "passes/sample");
    let mut worst = ("", 0.0f64);
    for preset in PRESETS {
        let mut chain = Chain::new(RATE);
        chain.set_voice(
            preset.circuit.voice(),
            preset.diode.voice(),
            preset.amplifier.voice(),
        );
        chain.set_iron(preset.iron.voice());
        chain.set_tone_section(preset.tone.voice());
        chain.set_cabinet(preset.cabinet.voice());
        chain.set_oversampling(preset.oversampling.factor());
        chain.set_drive(preset.drive as f64);
        chain.set_tone(tone::BASS, preset.bass as f64);
        chain.set_tone(tone::MID, preset.mid as f64);
        chain.set_tone(tone::TREBLE, preset.treble as f64);
        chain.settle();

        let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
        let n = (RATE * SECONDS) as usize;
        let start = std::time::Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            // Something with a bit of everything in it, at playing level.
            let t = i as f64 / RATE;
            let x = amplitude
                * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7
                    + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3);
            acc += chain.process(x);
        }
        let load = start.elapsed().as_secs_f64() / SECONDS * 100.0;
        if load > worst.1 {
            worst = (preset.name, load);
        }
        println!(
            "{:<24}{load:>9.1}%{:>14.2}{}",
            preset.name,
            chain.passes_per_sample(),
            if acc.is_nan() { "  NaN" } else { "" }
        );
    }
    println!("\nworst: {} at {:.1}% of realtime, one channel", worst.0, worst.1);
}
