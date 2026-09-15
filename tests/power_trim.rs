//! The power-override make-up still describes the circuits, and holds the level of
//! a voice when its output stage is swapped. See `Chain::power_trim`.
use gainstagefx::voice::{Chain, Gain, PowerAmp, Settings, Tone, NOMINAL_DBFS};

fn level(gain: Gain, power_amp: PowerAmp, drive: f64) -> f64 {
    let rate = 48_000.0;
    let mut chain = Chain::new(rate);
    chain.apply(&Settings {
        gain,
        power_amp,
        tone: Tone::Off,
        drive,
        ..Settings::default()
    });
    chain.settle();
    chain.find_operating_point();
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let n = rate as usize;
    let mut sum = 0.0;
    for i in 0..n {
        let t = i as f64 / rate;
        let x = amplitude
            * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7
                + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3);
        let y = chain.process(x);
        if i >= n / 2 {
            sum += y * y;
        }
    }
    (sum / (n / 2) as f64).sqrt()
}

#[test]
fn every_power_override_holds_the_voice_level_at_the_calibration_drive() {
    let overrides = [
        PowerAmp::Bypass,
        PowerAmp::Cali6L6,
        PowerAmp::American6L6Clean,
        PowerAmp::American6L6HighGain,
        PowerAmp::BritEL34,
    ];
    let mut worst = (0.0f64, String::new());
    for gain in Gain::ALL {
        let reference = level(gain, PowerAmp::Matched, 0.5);
        for power_amp in overrides {
            let db = 20.0 * (level(gain, power_amp, 0.5) / reference).log10();
            if db.abs() > worst.0.abs() {
                worst = (db, format!("{gain:?} {power_amp:?}"));
            }
            assert!(db.abs() < 1.0, "{gain:?} into {power_amp:?}: {db:+.2} dB");
        }
    }
    eprintln!("worst residual {:+.2} dB ({})", worst.0, worst.1);
}
