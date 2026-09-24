//! The era presets: that each one plays through the rig it claims, and that
//! *Machine Rage '92* distorts where and as much as the amplifier it is built
//! on does. See PRESETS.md and `examples/presetdirt.rs`, which prints the same
//! figures for any preset.
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::params::Circuit;
use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::voice::{Chain, PowerAmp, PowerModel, Settings, SpeakerChoice, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;
/// The groups the era presets live in.
const ERAS: [&str; 5] = [
    "Classic Rock",
    "Psychedelic / Lead",
    "Alternative",
    "Metal / Heavy",
    "Blues",
];

fn preset(name: &str) -> &'static Preset {
    PRESETS
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no preset called {name}"))
}

fn chain_for(settings: &Settings, rate: f64) -> Chain {
    let mut chain = Chain::new(rate);
    chain.apply(settings);
    chain.settle();
    chain
}

/// The speaker as a resistor: the amplifier's own output, no cabinet.
fn di(p: &Preset) -> Settings {
    let mut s = p.settings();
    s.acoustic.speaker = SpeakerChoice::Bypass;
    s
}

/// And with the power stage out as well: the pedal and preamplifier alone.
fn preamp_only(p: &Preset) -> Settings {
    let mut s = di(p);
    s.power_amp = PowerAmp::Bypass;
    s
}

fn nominal(p: &Preset) -> f64 {
    10f64.powf((NOMINAL_DBFS + p.input_trim as f64) / 20.0)
}

fn thd(settings: &Settings, amplitude: f64) -> f64 {
    let mut chain = chain_for(settings, RATE);
    let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
    measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x)).thd_percent()
}

/// The share of the output, in per cent, that a level-matched linear copy of
/// two input tones does not explain. 110 and 170 Hz: no low-order product lands
/// on either, and both fit a whole number of cycles in half a second, so the
/// linear part projects out exactly. A clean amplifier made louder scores
/// nothing here however loud it is.
fn residual(settings: &Settings, amplitude: f64) -> f64 {
    let tones = [110.0, 170.0];
    let mut chain = chain_for(settings, RATE);
    let (settle, n) = ((RATE / 2.0) as usize, (RATE / 2.0) as usize);
    let at = |k: usize| {
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

/// An E power chord struck every half second, Karplus-Strong and
/// deterministic, peaking 6 dB above the nominal level. The same riff
/// `examples/presetdirt.rs` renders.
fn riff(rate: f64, seconds: f64, peak: f64) -> Vec<f64> {
    let n = (rate * seconds) as usize;
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

// ---------------------------------------------------------------------------
// Machine Rage '92
// ---------------------------------------------------------------------------

/// Reported as "much too clean", and it was: it stood on the single-channel
/// 2203, whose preamplifier made 19 % distortion at a guitar's level while the
/// power stage behind it made up the rest as compression. Morello's amplifier
/// is a 2205's boost channel -- a diode-biased stage and a diode clipper -- and
/// on it the **preamplifier** does the distorting. This holds that: the
/// preamplifier alone past 30 %, and the whole amplifier not far off it.
#[test]
fn machine_rage_distorts_in_its_preamplifier_as_a_2205_does() {
    let p = preset("Machine Rage '92");
    assert_eq!(p.circuit, Circuit::Brit2205);
    assert_eq!(
        p.power_amp.voice().resolved(p.circuit.voice()),
        Some(PowerModel::Brit2205EL34)
    );
    let a = nominal(p);
    let (pre, whole) = (thd(&preamp_only(p), a), thd(&di(p), a));
    println!("preamplifier alone {pre:.1} %, whole amplifier {whole:.1} %");
    assert!(pre > 30.0, "the preamplifier makes only {pre:.1} %");
    assert!(whole > 30.0, "the amplifier makes only {whole:.1} %");
    // Distorted rock, not a wall: below the most saturated amplifier presets.
    assert!(whole < 60.0, "{whole:.1} % is past what this rig does");
}

/// THD reads one tone. A chord's distortion is mostly intermodulation, and a
/// clean signal made louder scores nothing here at any level -- which is the
/// failure this preset was reported for.
#[test]
fn machine_rage_is_not_a_clean_signal_made_louder() {
    let p = preset("Machine Rage '92");
    let r = residual(&di(p), nominal(p));
    println!("two-tone residual {r:.1} %");
    assert!(r > 12.0, "{r:.1} % of the output is distortion");
}

/// At a host's rate and block size the chain plays the riff with no
/// non-finite sample, settles nearly every solve, and works no harder than
/// the other amplifier presets do.
///
/// A fallback is a Newton pass on which no step length improved the residual,
/// so the best measured trial is kept and the solve goes on; it is the solver
/// working hard, not failing. On this riff (2026-09-24) this preset took 1,862
/// in 384,000 solves, all in the preamplifier, against 1,519 for Brit Lead,
/// 5,568 for Recto Lead and 6,158 for Plexi Cranked, at 8.9 passes a sample
/// across both circuits against their 9.1, 10.2 and 10.0.
#[test]
fn machine_rage_plays_a_riff_without_the_solver_struggling() {
    let p = preset("Machine Rage '92");
    let rate = 48_000.0;
    let mut chain = chain_for(&p.settings(), rate);
    let input = riff(rate, 4.0, nominal(p) * 2.0);
    let before = chain.solver_health();
    let mut peak = 0.0f64;
    for block in input.chunks(64) {
        for &x in block {
            let y = chain.process(x);
            assert!(y.is_finite());
            peak = peak.max(y.abs());
        }
    }
    let h = chain.solver_health();
    let (solves, unsettled) = (h.solves - before.solves, h.unsettled - before.unsettled);
    println!(
        "{solves} solves, {} passes, {unsettled} unsettled, {} backtracks, {} fallbacks, {} non-finite; peak {peak:.3}",
        h.passes - before.passes,
        h.backtracks - before.backtracks,
        h.fallbacks - before.fallbacks,
        h.nonfinite - before.nonfinite
    );
    let (passes, fallbacks) = (h.passes - before.passes, h.fallbacks - before.fallbacks);
    assert_eq!(h.nonfinite - before.nonfinite, 0);
    assert!(
        unsettled * 1_000 < solves,
        "{unsettled} of {solves} unsettled"
    );
    assert!(
        fallbacks * 100 < solves,
        "{fallbacks} fallbacks in {solves} solves"
    );
    assert!(passes < 6 * solves, "{passes} passes for {solves} solves");
    assert!(peak > 1e-3, "the riff came out silent");
}

// ---------------------------------------------------------------------------
// Every era preset
// ---------------------------------------------------------------------------

/// Each era preset builds, plays a riff finite, comes out of a physical
/// cabinet, and a modelled amplifier's Matched power stage resolves to one --
/// so a preset cannot be left pointing at a rig the catalogue no longer
/// builds.
#[test]
fn every_era_preset_plays_through_the_rig_it_claims() {
    let rate = 48_000.0;
    let eras: Vec<&Preset> = PRESETS.iter().filter(|p| ERAS.contains(&p.group)).collect();
    assert!(eras.len() >= 12, "only {} era presets", eras.len());
    for p in eras {
        let settings = p.settings();
        if p.circuit.voice().power_stage().is_some()
            && p.power_amp == gainstagefx::params::PowerAmp::Matched
        {
            assert!(
                p.power_amp.voice().resolved(p.circuit.voice()).is_some(),
                "{}: Matched resolves to nothing",
                p.name
            );
        }
        assert!(
            !matches!(
                settings.acoustic.cabinet,
                gainstagefx::voice::CabinetChoice::Legacy
            ),
            "{}: an era preset plays through a physical cabinet",
            p.name
        );
        let mut chain = chain_for(&settings, rate);
        let input = riff(rate, 1.0, nominal(p) * 2.0);
        let mut peak = 0.0f64;
        for x in input {
            let y = chain.process(x);
            assert!(y.is_finite(), "{}", p.name);
            peak = peak.max(y.abs());
        }
        assert!(peak > 1e-4, "{}: silent", p.name);
    }
}
