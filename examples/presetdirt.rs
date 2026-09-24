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
//! - **pre THD**, the same with the power stage bypassed as well: what the
//!   pedal and preamplifier make on their own. Set against DI THD it says
//!   which stage the distortion comes from -- a preamplifier that grinds, or a
//!   power stage pushed into compression, which reads as loud rather than
//!   dirty;
//! - **residual**: two tones, 110 and 170 Hz, through the DI, and the share of
//!   the output that a level-matched linear copy of them does *not* explain --
//!   harmonics and intermodulation together, which is what a chord's
//!   distortion is. The two are chosen so that no low-order product lands on
//!   either, and both fit a whole number of cycles in the window, so the
//!   linear part is projected out exactly;
//! - **crest**: how much the riff below is compressed, as its crest factor out
//!   of the DI against its crest factor in, in dB. A clean amplifier made
//!   louder keeps its crest factor; a saturated one gives it up. Taken at the
//!   DI because a cabinet and a microphone's phase shifts raise the crest
//!   factor of whatever they are handed.
//!
//! Pass preset names to restrict the list. `--write DIR` also renders the riff
//! through each one to a 24-bit WAV in DIR, to be listened to:
//!
//! `cargo run --release --example presetdirt -- --write /tmp/renders "Machine Rage '92"`
//!
//! The riff is synthetic and deterministic -- an E power chord (E2, B2, E3),
//! Karplus-Strong, struck every half second for four seconds -- so the renders
//! can be regenerated and compared after any change, and carry no one's
//! recording. It peaks 6 dB above the nominal level, a firmly struck chord.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::voice::{Chain, PowerAmp, Settings, SpeakerChoice, NOMINAL_DBFS};
use std::io::Write;

const RATE: f64 = 96_000.0;
/// The renders' rate: a host's.
const WRITE_RATE: f64 = 48_000.0;

fn chain_for(settings: &Settings, rate: f64) -> Chain {
    let mut chain = Chain::new(rate);
    chain.apply(settings);
    chain.settle();
    chain
}

fn thd(settings: &Settings, amplitude: f64) -> f64 {
    let mut chain = chain_for(settings, RATE);
    let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
    measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x)).thd_percent()
}

/// The share of the output energy, in per cent, not explained by the best
/// linear copy of the two input tones.
fn residual(settings: &Settings, amplitude: f64) -> f64 {
    let tones = [110.0, 170.0];
    let mut chain = chain_for(settings, RATE);
    let settle = (RATE / 2.0) as usize;
    // Half a second: 55 and 85 whole cycles.
    let n = (RATE / 2.0) as usize;
    let at = |k: usize| -> f64 {
        let t = k as f64 / RATE;
        amplitude
            * tones
                .iter()
                .map(|hz| 0.5 * (std::f64::consts::TAU * hz * t).sin())
                .sum::<f64>()
    };
    for k in 0..settle {
        chain.process(at(k));
    }
    let out: Vec<f64> = (settle..settle + n).map(|k| chain.process(at(k))).collect();
    let mean = out.iter().sum::<f64>() / n as f64;
    let total: f64 = out.iter().map(|y| (y - mean).powi(2)).sum();
    let mut linear = 0.0;
    for hz in tones {
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

/// The riff, at `rate`, scaled so its peak is `peak`.
fn riff(rate: f64, peak: f64) -> Vec<f64> {
    let n = (rate * 4.0) as usize;
    let mut out = vec![0.0; n];
    let mut seed = 12345u64;
    let mut noise = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) - 1.0
    };
    for hz in [82.41, 123.47, 164.81] {
        let len = (rate / hz) as usize;
        let mut line = vec![0.0; len];
        for (i, v) in out.iter_mut().enumerate() {
            if i % (rate as usize / 2) == 0 {
                for s in line.iter_mut() {
                    *s = noise();
                }
            }
            let j = i % len;
            let next = line[(j + 1) % len];
            let y = line[j];
            line[j] = 0.996 * 0.5 * (y + next);
            *v += y / 3.0;
        }
    }
    let top = out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    out.iter().map(|v| v * peak / top).collect()
}

fn crest_db(x: &[f64]) -> f64 {
    let peak = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let rms = (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
    20.0 * (peak / rms.max(1e-30)).log10()
}

/// The riff through a whole preset, output trim included; through its DI
/// (the speaker as a resistor, no cabinet or microphones) when `direct`.
fn played(preset: &Preset, rate: f64, direct: bool) -> (Vec<f64>, Vec<f64>) {
    let peak = 10f64.powf((NOMINAL_DBFS + 6.0 + preset.input_trim as f64) / 20.0);
    let input = riff(rate, peak);
    let settings = if direct {
        di(preset)
    } else {
        preset.settings()
    };
    let mut chain = chain_for(&settings, rate);
    let trim = 10f64.powf(preset.output_trim as f64 / 20.0);
    let output = input.iter().map(|&x| chain.process(x) * trim).collect();
    (input, output)
}

fn preamp_only(preset: &Preset) -> Settings {
    let mut settings = di(preset);
    settings.power_amp = PowerAmp::Bypass;
    settings
}

fn di(preset: &Preset) -> Settings {
    let mut settings = preset.settings();
    settings.acoustic.speaker = SpeakerChoice::Bypass;
    settings
}

/// 24-bit mono.
fn write_wav(path: &str, rate: f64, x: &[f64]) -> std::io::Result<()> {
    let size = x.len() * 3;
    let mut out = Vec::with_capacity(size + 44);
    out.extend(b"RIFF");
    out.extend(((36 + size) as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend((rate as u32).to_le_bytes());
    out.extend((rate as u32 * 3).to_le_bytes());
    out.extend(3u16.to_le_bytes());
    out.extend(24u16.to_le_bytes());
    out.extend(b"data");
    out.extend((size as u32).to_le_bytes());
    for v in x {
        let s = (v.clamp(-1.0, 1.0) * 8_388_607.0) as i32;
        out.extend(&s.to_le_bytes()[0..3]);
    }
    std::fs::File::create(path)?.write_all(&out)
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().position(|a| a == "--write").map(|i| {
        let dir = args.get(i + 1).cloned().expect("--write needs a directory");
        args.drain(i..=i + 1);
        dir
    });
    println!(
        "{:<22}{:>8}{:>9}{:>9}{:>10}{:>8}  chain",
        "preset", "THD", "DI THD", "pre THD", "residual", "crest"
    );
    for preset in PRESETS
        .iter()
        .filter(|p| args.is_empty() || args.iter().any(|w| w == p.name))
    {
        let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
        let (input, output) = played(preset, RATE, true);
        println!(
            "{:<22}{:>7.1}%{:>8.1}%{:>8.1}%{:>9.1}%{:>+7.1}  {:?} -> {:?} drive {:.2} master {:.2} -> {:?}",
            preset.name,
            thd(&preset.settings(), amplitude),
            thd(&di(preset), amplitude),
            thd(&preamp_only(preset), amplitude),
            residual(&di(preset), amplitude),
            crest_db(&output) - crest_db(&input),
            preset.pedal,
            preset.circuit,
            preset.drive,
            preset.master,
            preset.power_amp,
        );
        if let Some(dir) = &write {
            let (_, output) = played(preset, WRITE_RATE, false);
            let name: String = preset
                .name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            let path = format!("{dir}/{name}.wav");
            write_wav(&path, WRITE_RATE, &output).expect("writes");
            println!("  wrote {path}");
        }
    }
}
