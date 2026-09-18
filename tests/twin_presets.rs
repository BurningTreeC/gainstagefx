//! Regression guards for the completed AB763 Twin preset voicing.
//!
//! These are measurements of the exact 48 kHz shipped Chain, not isolated
//! control-value assertions.  The completed Twin changed enough electrically
//! that keeping the old knob positions was not a meaningful compatibility
//! target; these measurements are the new target.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::voice::{Chain, Settings, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;
const TONE_HZ: f64 = 220.0;

fn preset(name: &str) -> &'static Preset {
    PRESETS
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("{name} is shipped"))
}

#[inline]
fn input_amplitude(preset: &Preset) -> f64 {
    10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0)
}

fn measured_tone(settings: &Settings, amplitude: f64) -> measure::Measured {
    let mut chain = Chain::new(RATE);
    chain.apply(settings);
    chain.settle();
    let tone = Tone::near(RATE, 16_384, TONE_HZ, amplitude);
    measure::run(tone, (RATE * 0.35) as usize, |x| chain.process(x))
}

fn core_thd(preset: &Preset) -> f64 {
    let mut settings = preset.settings();
    settings.reverb = 0.0;
    settings.intensity = 0.0;
    measured_tone(&settings, input_amplitude(preset)).thd_percent()
}

fn reverb_ratio_db(preset: &Preset) -> f64 {
    let mut dry_settings = preset.settings();
    dry_settings.reverb = 0.0;
    dry_settings.intensity = 0.0;

    let mut wet_settings = dry_settings;
    wet_settings.reverb = preset.reverb as f64;

    let mut dry = Chain::new(RATE);
    let mut wet = Chain::new(RATE);
    dry.apply(&dry_settings);
    wet.apply(&wet_settings);
    dry.settle();
    wet.settle();

    let amplitude = input_amplitude(preset);
    let omega = std::f64::consts::TAU * TONE_HZ / RATE;
    let warm = (RATE * 0.75) as usize;
    let len = (RATE * 0.75) as usize;
    let mut dry_sq = 0.0;
    let mut wet_difference_sq = 0.0;

    for n in 0..warm + len {
        let x = amplitude * (omega * n as f64).sin();
        let d = dry.process(x);
        let w = wet.process(x);
        if n >= warm {
            dry_sq += d * d;
            let spring = w - d;
            wet_difference_sq += spring * spring;
        }
    }

    let dry_rms = (dry_sq / len as f64).sqrt();
    let wet_rms = (wet_difference_sq / len as f64).sqrt();
    20.0 * (wet_rms.max(1e-15) / dry_rms.max(1e-15)).log10()
}

fn tremolo_depth_db(preset: &Preset) -> f64 {
    let mut settings = preset.settings();
    settings.reverb = 0.0;

    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();

    let amplitude = input_amplitude(preset);
    let omega = std::f64::consts::TAU * TONE_HZ / RATE;
    let warm = (RATE * 0.75) as usize;
    let window = (RATE * 0.020) as usize;
    let windows = 60usize;
    let mut min_rms = f64::INFINITY;
    let mut max_rms: f64 = 0.0;
    let mut n = 0usize;

    for _ in 0..warm {
        let x = amplitude * (omega * n as f64).sin();
        chain.process(x);
        n += 1;
    }

    for _ in 0..windows {
        let mut sum = 0.0;
        for _ in 0..window {
            let x = amplitude * (omega * n as f64).sin();
            let y = chain.process(x);
            sum += y * y;
            n += 1;
        }
        let rms = (sum / window as f64).sqrt();
        min_rms = min_rms.min(rms);
        max_rms = max_rms.max(rms);
    }

    20.0 * (max_rms.max(1e-15) / min_rms.max(1e-15)).log10()
}

fn output_rms_before_trim(preset: &Preset) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();

    let amplitude = input_amplitude(preset);
    let omega = std::f64::consts::TAU * TONE_HZ / RATE;
    let warm = (RATE * 0.75) as usize;
    let len = RATE as usize;
    let mut sum = 0.0;

    for n in 0..warm + len {
        let x = amplitude * (omega * n as f64).sin();
        let y = chain.process(x);
        if n >= warm {
            sum += y * y;
        }
    }

    (sum / len as f64).sqrt()
}

#[test]
fn completed_ab763_presets_hold_their_calibrated_voicing() {
    let clean = preset("Blackface Clean");
    let throb = preset("Blackface Throb");

    // First freeze the actual user-facing values chosen by the calibration
    // pass. If these move intentionally, the measured targets below need to be
    // reconsidered at the same time.
    assert!((clean.drive - 0.24).abs() < 1e-6);
    assert!((clean.reverb - 0.30).abs() < 1e-6);
    assert!((clean.intensity - 0.0).abs() < 1e-6);
    assert!((throb.drive - 0.17).abs() < 1e-6);
    assert!((throb.reverb - 0.38).abs() < 1e-6);
    assert!((throb.speed - 0.40).abs() < 1e-6);
    assert!((throb.intensity - 0.94).abs() < 1e-6);
    assert!((throb.output_trim - 2.1).abs() < 1e-6);

    let clean_thd = core_thd(clean);
    let throb_thd = core_thd(throb);
    let clean_reverb = reverb_ratio_db(clean);
    let throb_reverb = reverb_ratio_db(throb);
    let throb_depth = tremolo_depth_db(throb);

    let clean_rms = output_rms_before_trim(clean) * 10f64.powf(clean.output_trim as f64 / 20.0);
    let throb_rms = output_rms_before_trim(throb) * 10f64.powf(throb.output_trim as f64 / 20.0);
    let relative_level_db = 20.0 * (throb_rms.max(1e-15) / clean_rms.max(1e-15)).log10();

    println!(
        "blackface_presets,clean_thd={clean_thd:.3}%,throb_thd={throb_thd:.3}%,clean_reverb={clean_reverb:.2}dB,throb_reverb={throb_reverb:.2}dB,throb_depth={throb_depth:.2}dB,throb_vs_clean={relative_level_db:+.2}dB"
    );

    assert!(
        (1.6..=2.5).contains(&clean_thd),
        "Blackface Clean left its calibrated ~2% core THD region: {clean_thd:.3}%"
    );
    assert!(
        (1.6..=2.5).contains(&throb_thd),
        "Blackface Throb left its calibrated ~2% core THD region: {throb_thd:.3}%"
    );
    assert!(
        (-13.0..=-11.0).contains(&clean_reverb),
        "Blackface Clean spring return moved away from ~-12 dB wet/dry: {clean_reverb:.2} dB"
    );
    assert!(
        (-11.0..=-9.0).contains(&throb_reverb),
        "Blackface Throb spring return moved away from ~-10 dB wet/dry: {throb_reverb:.2} dB"
    );
    assert!(
        (7.0..=9.0).contains(&throb_depth),
        "Blackface Throb optical depth moved away from ~8 dB: {throb_depth:.2} dB"
    );
    assert!(
        relative_level_db.abs() <= 0.5,
        "Blackface Throb no longer level-matches Clean after output trim: {relative_level_db:+.2} dB"
    );
}
