//! The DI: the dry signal taken from the input, after the pedal, or after the
//! circuit, ahead of the power stage. See `docs/DI.md`.
#[path = "support/allocations.rs"]
mod allocations;

use allocations::assert_no_heap;
use gainstagefx::stage_worker::StageWorker;
use gainstagefx::voice::{Chain, DrySource, Gain, Pedal, PedalSettings, PowerAmp, Settings};

fn material(k: usize, rate: f64) -> f64 {
    let t = k as f64 / rate;
    0.2 * ((std::f64::consts::TAU * 110.0 * t).sin()
        + 0.5 * (std::f64::consts::TAU * 1_000.0 * t).sin()
        + 0.25 * (std::f64::consts::TAU * 5_000.0 * t).sin())
}

fn chain(settings: &Settings, rate: f64) -> Chain {
    let mut chain = Chain::new(rate);
    chain.apply(settings);
    chain.settle();
    chain.find_operating_point();
    chain.reset();
    chain
}

/// Input, pedal tap, preamp tap and wet output, sample by sample.
fn play(settings: &Settings, rate: f64, n: usize) -> Vec<(f64, f64, f64)> {
    let mut chain = chain(settings, rate);
    (0..n)
        .map(|k| {
            let x = material(k, rate);
            let delayed = chain.delayed_dry(x);
            let wet = chain.process(x);
            (delayed, chain.dry(delayed), wet)
        })
        .collect()
}

/// With no pedal in the slot, the pedal tap *is* the input: through the same
/// upsampler, its own copy of the decimator and the same padding, it lands on
/// the delayed input -- to rounding at 1x, where there is no filter (the input
/// is scaled into the circuit's volts and back), and inside the half-band
/// filters' ripple above it. That is the tap's level and its alignment both:
/// one sample off would be a comb, a gain error a level step.
#[test]
fn the_pedal_tap_with_no_pedal_is_the_delayed_input() {
    for oversampling in [1, 2, 4, 8] {
        let settings = Settings {
            gain: Gain::Clean,
            dry_source: DrySource::Pedal,
            oversampling,
            ..Settings::default()
        };
        let run = play(&settings, 48_000.0, 4_800);
        let tail = &run[2_400..];
        let signal = (tail.iter().map(|r| r.0 * r.0).sum::<f64>() / tail.len() as f64).sqrt();
        let error =
            (tail.iter().map(|r| (r.1 - r.0).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
        println!(
            "{oversampling}x: tap against delayed input {:.2e} of the signal",
            error / signal
        );
        if oversampling == 1 {
            assert!(error / signal < 1e-12, "1x: {}", error / signal);
        } else {
            assert!(error / signal < 1e-3, "{oversampling}x: {}", error / signal);
        }
    }
}

/// Input is the dry signal as it always was: `dry` hands back what
/// `delayed_dry` gave, whatever else is set.
#[test]
fn the_input_source_is_the_delayed_input_unchanged() {
    let settings = Settings {
        gain: Gain::AmericanSvt,
        drive: 0.7,
        ..Settings::default()
    };
    for (delayed, dry, _) in play(&settings, 48_000.0, 2_000) {
        assert_eq!(delayed.to_bits(), dry.to_bits());
    }
}

/// The circuit tap is an amplifier's direct out: the preamplifier, EQ and all,
/// ahead of the power stage and the Master. Turning the Master or changing the
/// power stage leaves the tap exactly alone while the power stage change moves
/// the amplifier's output; turning the Drive moves the tap, which is where the
/// preamplifier is.
#[test]
fn the_preamp_tap_is_ahead_of_the_master_and_the_power_stage() {
    let base = Settings {
        gain: Gain::AmericanSvt,
        drive: 0.5,
        master: 0.5,
        dry_source: DrySource::Preamp,
        ..Settings::default()
    };
    let rms = |run: &[(f64, f64, f64)], pick: fn(&(f64, f64, f64)) -> f64| {
        let tail = &run[run.len() / 2..];
        (tail.iter().map(|r| pick(r).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    let reference = play(&base, 48_000.0, 4_800);
    let louder = play(
        &Settings {
            master: 0.9,
            ..base
        },
        48_000.0,
        4_800,
    );
    let other_stage = play(
        &Settings {
            power_amp: PowerAmp::BritEL34,
            ..base
        },
        48_000.0,
        4_800,
    );
    let hotter = play(&Settings { drive: 0.8, ..base }, 48_000.0, 4_800);
    let tap = |r: &(f64, f64, f64)| r.1;
    let wet = |r: &(f64, f64, f64)| r.2;
    println!(
        "tap {:.4}; master up: tap {:.4}; Brit EL34: tap {:.4}, wet {:.4} -> {:.4}; drive up: tap {:.4}",
        rms(&reference, tap),
        rms(&louder, tap),
        rms(&other_stage, tap),
        rms(&reference, wet),
        rms(&other_stage, wet),
        rms(&hotter, tap),
    );
    for (a, b) in reference
        .iter()
        .zip(&louder)
        .chain(reference.iter().zip(&other_stage))
    {
        assert_eq!(a.1.to_bits(), b.1.to_bits(), "the tap moved");
    }
    assert!((rms(&other_stage, wet) / rms(&reference, wet) - 1.0).abs() > 0.05);
    assert!(rms(&reference, tap) > 0.0);
    assert!((rms(&hotter, tap) / rms(&reference, tap) - 1.0).abs() > 0.01);
}

/// The tap is levelled to the plugin's units, so a DI blended against the
/// amplifier is neither buried nor towering: the circuit tap carries the
/// make-up that holds the circuit's level at nominal, less the voice's own
/// power stage as the Bypass selection's measured trim takes it out -- within
/// 3 dB of the input for every circuit here, against tens of decibels of
/// circuit gain and the SVT's 40 dB of power stage -- and the pedal tap,
/// through a pedal at rest, the input's own level.
#[test]
fn the_taps_are_at_the_plugins_level() {
    let input_rms = {
        let n = 4_800;
        let x: Vec<f64> = (n / 2..n).map(|k| material(k, 48_000.0)).collect();
        (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
    };
    for gain in [
        Gain::Clean,
        Gain::AmericanSvt,
        Gain::British47,
        Gain::German76,
    ] {
        let run = play(
            &Settings {
                gain,
                drive: 0.4,
                dry_source: DrySource::Preamp,
                ..Settings::default()
            },
            48_000.0,
            4_800,
        );
        let tail = &run[run.len() / 2..];
        let tap = (tail.iter().map(|r| r.1 * r.1).sum::<f64>() / tail.len() as f64).sqrt();
        let db = 20.0 * (tap / input_rms).log10();
        println!("{gain:?} circuit tap {db:+.1} dB re the input");
        assert!(db.abs() < 3.0, "{gain:?}: {db}");
    }
    let run = play(
        &Settings {
            gain: Gain::Clean,
            pedal: PedalSettings {
                pedal: Pedal::CleanBoost,
                drive: 0.0,
                ..PedalSettings::default()
            },
            dry_source: DrySource::Pedal,
            ..Settings::default()
        },
        48_000.0,
        4_800,
    );
    let tail = &run[run.len() / 2..];
    let tap = (tail.iter().map(|r| r.1 * r.1).sum::<f64>() / tail.len() as f64).sqrt();
    let db = 20.0 * (tap / input_rms).log10();
    println!("Clean Boost at rest, gain at minimum: pedal tap {db:+.1} dB re the input");
    assert!(db.abs() < 3.0, "{db}");
}

/// The block path writes the same DI the sample path gives -- serial,
/// pipelined on a live worker and reclaimed from an inert one -- to the bit,
/// because the tap is taken in the first half on the calling thread whichever
/// way the second half runs.
#[test]
fn the_block_path_writes_the_same_di() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    for (source, oversampling) in [
        (DrySource::Preamp, 1),
        (DrySource::Preamp, 4),
        (DrySource::Pedal, 2),
    ] {
        let settings = Settings {
            gain: Gain::Clean,
            drive: 0.6,
            dry_source: source,
            oversampling,
            ..Settings::default()
        };
        let reference: Vec<u64> = play(&settings, 48_000.0, 64 * 12)
            .iter()
            .map(|r| r.1.to_bits())
            .collect();
        for (worker, pipeline) in [(None, false), (Some(&live), true), (Some(&inert), true)] {
            let mut chain = chain(&settings, 48_000.0);
            let mut out = Vec::new();
            for block in 0..12 {
                let input: Vec<f64> = (block * 64..(block + 1) * 64)
                    .map(|k| material(k, 48_000.0))
                    .collect();
                let mut dry: Vec<f64> = input.iter().map(|&x| chain.delayed_dry(x)).collect();
                let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
                chain.process_block_with_dry(
                    &input, &mut l, &mut r, &mut dry, false, worker, pipeline,
                );
                out.extend(dry.iter().map(|v| v.to_bits()));
            }
            assert!(
                out == reference,
                "{source:?} {oversampling}x pipeline {pipeline}"
            );
        }
    }
}

/// Changing the source, and playing every source, allocates nothing, at every
/// rate the plugin runs at.
#[test]
fn the_di_is_realtime_safe_at_every_rate() {
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let base = Settings {
            gain: Gain::AmericanSvt,
            drive: 0.5,
            ..Settings::default()
        };
        let mut chain = chain(&base, rate);
        let sources = [DrySource::Pedal, DrySource::Preamp, DrySource::Input];
        assert_no_heap(|| {
            for (i, source) in sources.iter().enumerate() {
                chain.apply(&Settings {
                    dry_source: *source,
                    ..base
                });
                for k in 0..1_024 {
                    let x = material(i * 1_024 + k, rate);
                    let delayed = chain.delayed_dry(x);
                    let wet = chain.process(x);
                    assert!(wet.is_finite() && chain.dry(delayed).is_finite(), "{rate}");
                }
            }
        });
    }
}
