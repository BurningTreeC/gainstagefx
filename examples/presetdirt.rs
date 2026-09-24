//! How much each named preset actually distorts, exactly as it ships.
//!
//! `examples/howdirty.rs` and `examples/dirt.rs` measure circuits and the
//! catalogue's generic topologies with everything else out of the way. A
//! preset is more than its circuit: a pedal in front, a power stage behind, an
//! input trim, a speaker and a cabinet. This runs the whole of
//! `Preset::settings()` and reports, at the guitar's nominal level:
//!
//! - **THD** of a 220 Hz tone at the output, as a player hears it -- after the
//!   speaker, cabinet and microphones, which filter the harmonics they are
//!   handed, so the figure understates what the amplifier did;
//! - **DI THD**, the same with the speaker replaced by its resistor, which is
//!   the amplifier's own distortion and the number to compare between presets;
//! - **residual**: a low power chord (82/123/165 Hz) through the DI, and how
//!   much of the output is *not* explained by a level-matched linear copy of
//!   the chord. THD alone reads one tone; a chord is what the presets are for,
//!   and intermodulation is most of what a chord's distortion is.
//!
//! Pass preset names to restrict the list:
//!
//! `cargo run --release --example presetdirt -- "Machine Rage '92" "Brit Crunch"`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::voice::{Chain, Settings, SpeakerChoice, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

fn chain_for(settings: &Settings) -> Chain {
    let mut chain = Chain::new(RATE);
    chain.apply(settings);
    chain.settle();
    chain
}

fn thd(settings: &Settings, amplitude: f64) -> f64 {
    let mut chain = chain_for(settings);
    let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
    measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x)).thd_percent()
}

/// The fraction of the chord's output energy left after removing the
/// least-squares best copy of the input chord at each of its three
/// frequencies -- that is, what the amplifier added.
fn residual(settings: &Settings, amplitude: f64) -> f64 {
    let partials = [(82.41, 0.45), (123.47, 0.35), (164.81, 0.2)];
    let mut chain = chain_for(settings);
    let settle = (RATE / 2.0) as usize;
    let n = (RATE / 2.0) as usize;
    let at = |k: usize| -> f64 {
        let t = k as f64 / RATE;
        amplitude
            * partials
                .iter()
                .map(|(hz, a)| a * (std::f64::consts::TAU * hz * t).sin())
                .sum::<f64>()
    };
    for k in 0..settle {
        chain.process(at(k));
    }
    let out: Vec<f64> = (settle..settle + n).map(|k| chain.process(at(k))).collect();
    let mean = out.iter().sum::<f64>() / n as f64;
    let total: f64 = out.iter().map(|y| (y - mean).powi(2)).sum();
    // Project onto sin/cos at each input frequency: the linear part of the
    // response, whatever gain and phase the chain gave it.
    let mut linear = 0.0;
    for (hz, _) in partials {
        let w = std::f64::consts::TAU * hz / RATE;
        let (mut s, mut c) = (0.0, 0.0);
        for (i, y) in out.iter().enumerate() {
            let k = (settle + i) as f64;
            s += (y - mean) * (w * k).sin();
            c += (y - mean) * (w * k).cos();
        }
        linear += 2.0 * (s * s + c * c) / n as f64;
    }
    (1.0 - linear / total).max(0.0) * 100.0
}

fn di(preset: &Preset) -> Settings {
    let mut settings = preset.settings();
    settings.acoustic.speaker = SpeakerChoice::Bypass;
    settings
}

fn main() {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    println!(
        "{:<22}{:>9}{:>10}{:>11}  chain",
        "preset", "THD", "DI THD", "residual"
    );
    for preset in PRESETS
        .iter()
        .filter(|p| wanted.is_empty() || wanted.iter().any(|w| w == p.name))
    {
        let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
        let settings = preset.settings();
        println!(
            "{:<22}{:>8.1}%{:>9.1}%{:>10.1}%  {:?} -> {:?} drive {:.2} master {:.2} -> {:?}",
            preset.name,
            thd(&settings, amplitude),
            thd(&di(preset), amplitude),
            residual(&di(preset), amplitude),
            preset.pedal,
            preset.circuit,
            preset.drive,
            preset.master,
            preset.power_amp,
        );
    }
}
