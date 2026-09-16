//! Is the pedal slot level matched the way the circuit list is?
//!
//! Every voice is calibrated so that switching circuits does not change the
//! level (`CALIBRATION`), and every power override is trimmed the same way
//! (`POWER_TRIM_DB`). This asks the same question of the pedals: with the same
//! knobs, into the same amplifier, how much does the level move when the pedal
//! does?
//!
//! `cargo run --release --example pedallevel`
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;

fn level(pedal: Pedal, drive: f64, level: f64, gain: Gain) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&Settings {
        gain,
        pedal: PedalSettings { pedal, drive, tone: 0.5, level },
        tone: Tone::Off,
        drive: 0.5,
        oversampling: 1,
        ..Settings::default()
    });
    chain.settle();
    chain.find_operating_point();
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let n = RATE as usize;
    let mut sum = 0.0;
    for i in 0..n {
        let t = i as f64 / RATE;
        let x = amplitude
            * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7
                + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3);
        let y = chain.process(x);
        if i >= n / 2 {
            sum += y * y;
        }
    }
    20.0 * (sum / (n / 2) as f64).sqrt().max(1e-12).log10()
}

fn main() {
    for gain in [Gain::Clean, Gain::Crunch, Gain::Twin] {
        println!("\ninto {} (drive 0.5), pedal knobs at their middles:", gain.name());
        let bare = level(Pedal::None, 0.5, 0.5, gain);
        println!("  {:<12}{bare:7.1} dB", "no pedal");
        let mut worst: (f64, &str) = (0.0, "");
        for pedal in Pedal::ALL.iter().copied().filter(|p| *p != Pedal::None) {
            let mid = level(pedal, 0.5, 0.5, gain);
            let down = level(pedal, 0.0, 0.5, gain);
            let up = level(pedal, 1.0, 0.5, gain);
            println!(
                "  {:<12}{mid:7.1} dB   ({:+.1} against no pedal)   drive 0: {down:6.1}   drive 1: {up:6.1}",
                format!("{pedal:?}"),
                mid - bare
            );
            if (mid - bare).abs() > worst.0.abs() {
                worst = (mid - bare, "");
            }
        }
        println!("  worst departure from no pedal: {:+.1} dB", worst.0);
    }
}
