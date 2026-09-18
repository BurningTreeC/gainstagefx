//! Re-voice the two shipped American Twin presets against the completed AB763 model.
//!
//! Run with:
//!
//! ```text
//! cargo run --release --example twin_preset_calibrate
//! ```
//!
//! This deliberately measures the exact shipped `Chain` at 48 kHz, because the
//! American Twin presets run at host rate (`Oversampling::Off`).  It does not
//! change the circuit to make the old preset values work: it finds new preset
//! values for the completed circuit.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::voice::{Chain, Settings, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;
const TONE_HZ: f64 = 220.0;

// A Twin is the catalogue's clean amplifier reference. Keep the core barely
// bending at a nominal guitar level; this matches the intent already written
// in examples/calibrate.rs.
const CLEAN_THD_TARGET: f64 = 2.0;

// Audible spring, but still a clean-combo preset rather than surf wash. Throb
// is allowed a little more return because the moving dry level makes a smaller
// return disappear perceptually at the bottom of the tremolo cycle.
const CLEAN_REVERB_DB: f64 = -12.0;
const THROB_REVERB_DB: f64 = -10.0;

// Deep enough to deserve the name without repeatedly disappearing. Measured as
// the max/min short-window RMS envelope with the reverb disabled so the spring
// tail cannot fill the optical dips.
const THROB_DEPTH_DB: f64 = 8.0;

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

fn thd_at(preset: &Preset, drive: f64) -> f64 {
    let mut settings = preset.settings();
    settings.drive = drive;
    // Calibrate the amplifier's clean operating point without the spring or
    // optical level modulation changing the FFT result.
    settings.reverb = 0.0;
    settings.intensity = 0.0;
    measured_tone(&settings, input_amplitude(preset)).thd_percent()
}

fn nearest_drive(preset: &Preset, target: f64) -> (f64, f64) {
    let mut best = (0.0, thd_at(preset, 0.0));
    let mut best_error = (best.1 - target).abs();

    // Coarse sweep first. Do not assume monotonicity: once a valve stage is
    // deeply clipped, sine THD can stop being a monotonic proxy for "more".
    for step in 1..=10 {
        let drive = step as f64 / 10.0;
        let thd = thd_at(preset, drive);
        let error = (thd - target).abs();
        if error < best_error {
            best = (drive, thd);
            best_error = error;
        }
    }

    // Then resolve the useful tenth to one percentage point of knob travel.
    let lo = (best.0 - 0.10).max(0.0);
    let hi = (best.0 + 0.10).min(1.0);
    let first = (lo * 100.0).round() as usize;
    let last = (hi * 100.0).round() as usize;
    for step in first..=last {
        let drive = step as f64 / 100.0;
        let thd = thd_at(preset, drive);
        let error = (thd - target).abs();
        if error < best_error {
            best = (drive, thd);
            best_error = error;
        }
    }
    best
}

fn paired_reverb_ratio_db(preset: &Preset, drive: f64, reverb: f64) -> f64 {
    let mut dry_settings = preset.settings();
    dry_settings.drive = drive;
    dry_settings.reverb = 0.0;
    dry_settings.intensity = 0.0;

    let mut wet_settings = dry_settings;
    wet_settings.reverb = reverb;

    let mut dry = Chain::new(RATE);
    let mut wet = Chain::new(RATE);
    dry.apply(&dry_settings);
    wet.apply(&wet_settings);
    dry.settle();
    wet.settle();

    let amplitude = input_amplitude(preset);
    let omega = std::f64::consts::TAU * TONE_HZ / RATE;
    let warm = (RATE * 0.75) as usize;
    let measure_len = (RATE * 0.75) as usize;
    let mut dry_sq = 0.0;
    let mut wet_difference_sq = 0.0;

    for n in 0..warm + measure_len {
        let x = amplitude * (omega * n as f64).sin();
        let d = dry.process(x);
        let w = wet.process(x);
        if n >= warm {
            dry_sq += d * d;
            let difference = w - d;
            wet_difference_sq += difference * difference;
        }
    }

    let dry_rms = (dry_sq / measure_len as f64).sqrt();
    let wet_rms = (wet_difference_sq / measure_len as f64).sqrt();
    20.0 * (wet_rms.max(1e-15) / dry_rms.max(1e-15)).log10()
}

fn nearest_reverb(preset: &Preset, drive: f64, target_db: f64) -> (f64, f64) {
    let mut best = (0.0, paired_reverb_ratio_db(preset, drive, 0.0));
    let mut best_error = (best.1 - target_db).abs();

    for step in 1..=10 {
        let reverb = step as f64 / 10.0;
        let db = paired_reverb_ratio_db(preset, drive, reverb);
        let error = (db - target_db).abs();
        if error < best_error {
            best = (reverb, db);
            best_error = error;
        }
    }

    let lo = (best.0 - 0.10).max(0.0);
    let hi = (best.0 + 0.10).min(1.0);
    let first = (lo * 50.0).round() as usize;
    let last = (hi * 50.0).round() as usize;
    for step in first..=last {
        let reverb = step as f64 / 50.0; // 0.02 increments
        let db = paired_reverb_ratio_db(preset, drive, reverb);
        let error = (db - target_db).abs();
        if error < best_error {
            best = (reverb, db);
            best_error = error;
        }
    }
    best
}

fn tremolo_depth_db(preset: &Preset, drive: f64, intensity: f64) -> f64 {
    let mut settings = preset.settings();
    settings.drive = drive;
    settings.reverb = 0.0;
    settings.intensity = intensity;

    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();

    let amplitude = input_amplitude(preset);
    let omega = std::f64::consts::TAU * TONE_HZ / RATE;
    let warm = (RATE * 0.75) as usize;
    let window = (RATE * 0.020) as usize; // 20 ms: several 220 Hz cycles, << tremolo period.
    let windows = 60usize; // 1.2 s, several cycles at the shipped speed.
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

fn nearest_intensity(preset: &Preset, drive: f64, target_db: f64) -> (f64, f64) {
    let mut best = (0.0, tremolo_depth_db(preset, drive, 0.0));
    let mut best_error = (best.1 - target_db).abs();

    for step in 1..=10 {
        let intensity = step as f64 / 10.0;
        let depth = tremolo_depth_db(preset, drive, intensity);
        let error = (depth - target_db).abs();
        if error < best_error {
            best = (intensity, depth);
            best_error = error;
        }
    }

    let lo = (best.0 - 0.10).max(0.0);
    let hi = (best.0 + 0.10).min(1.0);
    let first = (lo * 50.0).round() as usize;
    let last = (hi * 50.0).round() as usize;
    for step in first..=last {
        let intensity = step as f64 / 50.0;
        let depth = tremolo_depth_db(preset, drive, intensity);
        let error = (depth - target_db).abs();
        if error < best_error {
            best = (intensity, depth);
            best_error = error;
        }
    }
    best
}

fn output_rms(preset: &Preset, drive: f64, reverb: f64, intensity: f64) -> (f64, f64) {
    let mut settings = preset.settings();
    settings.drive = drive;
    settings.reverb = reverb;
    settings.intensity = intensity;

    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();

    let amplitude = input_amplitude(preset);
    let omega = std::f64::consts::TAU * TONE_HZ / RATE;
    let warm = (RATE * 0.75) as usize;
    let len = (RATE * 1.0) as usize;
    let mut sum = 0.0;
    let mut peak: f64 = 0.0;
    for n in 0..warm + len {
        let x = amplitude * (omega * n as f64).sin();
        let y = chain.process(x);
        if n >= warm {
            sum += y * y;
            peak = peak.max(y.abs());
        }
    }
    ((sum / len as f64).sqrt(), peak)
}

fn main() {
    let clean = preset("Blackface Clean");
    let throb = preset("Blackface Throb");

    println!("Twin preset calibration against the completed AB763 circuit");
    println!("rate={RATE:.0} Hz, tone={TONE_HZ:.0} Hz, nominal={NOMINAL_DBFS:.1} dBFS");
    println!();

    println!("=== current shipped values ===");
    for p in [clean, throb] {
        let thd = thd_at(p, p.drive as f64);
        println!(
            "{:<16} drive={:.2} bass={:.2} mid={:.2} treble={:.2} reverb={:.2} speed={:.2} intensity={:.2} core_thd={:.3}%",
            p.name, p.drive, p.bass, p.mid, p.treble, p.reverb, p.speed, p.intensity, thd
        );
    }

    println!();
    println!("=== clean-core Volume search ({CLEAN_THD_TARGET:.1}% THD target) ===");
    let (clean_drive, clean_thd) = nearest_drive(clean, CLEAN_THD_TARGET);
    let (throb_drive, throb_thd) = nearest_drive(throb, CLEAN_THD_TARGET);
    println!("Blackface Clean: drive={clean_drive:.2}, THD={clean_thd:.3}%");
    println!("Blackface Throb: drive={throb_drive:.2}, THD={throb_thd:.3}%");

    println!();
    println!("=== spring return search ===");
    let (clean_reverb, clean_reverb_db) = nearest_reverb(clean, clean_drive, CLEAN_REVERB_DB);
    let (throb_reverb, throb_reverb_db) = nearest_reverb(throb, throb_drive, THROB_REVERB_DB);
    println!("Blackface Clean: reverb={clean_reverb:.2}, wet/dry={clean_reverb_db:.2} dB (target {CLEAN_REVERB_DB:.1})");
    println!("Blackface Throb: reverb={throb_reverb:.2}, wet/dry={throb_reverb_db:.2} dB (target {THROB_REVERB_DB:.1})");

    println!();
    println!("=== optical tremolo search ===");
    let (throb_intensity, throb_depth) = nearest_intensity(throb, throb_drive, THROB_DEPTH_DB);
    let speed_hz = 1.8 * (11.0_f64 / 1.8).powf(throb.speed as f64);
    println!(
        "Blackface Throb: speed={:.2} (~{speed_hz:.2} Hz), intensity={throb_intensity:.2}, depth={throb_depth:.2} dB (target {THROB_DEPTH_DB:.1})",
        throb.speed
    );

    println!();
    println!("=== resulting level ===");
    let (clean_rms, clean_peak) = output_rms(clean, clean_drive, clean_reverb, 0.0);
    let (throb_rms, throb_peak) = output_rms(throb, throb_drive, throb_reverb, throb_intensity);
    let throb_compensation_db = 20.0 * (clean_rms.max(1e-15) / throb_rms.max(1e-15)).log10();
    println!("Blackface Clean: rms={:.6}, peak={:.6}", clean_rms, clean_peak);
    println!("Blackface Throb: rms={:.6}, peak={:.6}, suggested output_trim relative to Clean={:+.2} dB", throb_rms, throb_peak, throb_compensation_db);

    println!();
    println!("=== recommended preset fields ===");
    println!("Blackface Clean: drive={clean_drive:.2}, reverb={clean_reverb:.2}, intensity=0.00, output_trim={:+.2}", clean.output_trim);
    // `output_rms()` measures the DSP chain before the plugin-level Output
    // parameter.  The absolute trim that makes Throb match Clean is therefore
    // Clean's existing trim plus the measured RMS compensation; adding the
    // current Throb trim here would double-count it on subsequent calibration
    // runs after the recommendation has been applied.
    let recommended_throb_trim = clean.output_trim as f64 + throb_compensation_db;
    println!("Blackface Throb: drive={throb_drive:.2}, reverb={throb_reverb:.2}, speed={:.2}, intensity={throb_intensity:.2}, output_trim={recommended_throb_trim:+.2}", throb.speed);
    println!();
    println!("Tone, input jack/Bright, cabinet, speaker and microphones are intentionally left unchanged by this calibration pass.");
}
