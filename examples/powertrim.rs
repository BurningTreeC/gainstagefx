//! Measures the make-up correction for power overrides and prints it as Rust.
//!
//! Each voice's calibration was measured on its own path. With a different
//! output stage behind it the level moves by up to twenty decibels. This measures
//! that change with the trim itself disabled (the table below starts at zero) at
//! the calibration drive, on the tone-off, legacy-cabinet-off path, and prints
//! the table `Chain::power_trim` divides out.
//!
//! Run with `cargo run --release --example powertrim > src/power_trim.rs`, starting
//! from an all-zero table (the example refuses to measure through a non-zero one).

use gainstagefx::voice::{Chain, Gain, PowerAmp, Settings, Tone, NOMINAL_DBFS, POWER_TRIM_DB};

pub const RATE: f64 = 48_000.0;
pub const DRIVE: f64 = 0.5;

pub fn level(gain: Gain, power_amp: PowerAmp) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&Settings {
        gain,
        power_amp,
        tone: Tone::Off,
        drive: DRIVE,
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
    (sum / (n / 2) as f64).sqrt()
}

fn main() {
    assert!(
        POWER_TRIM_DB.iter().flatten().all(|v| *v == 0.0),
        "reset src/power_trim.rs to zeros before re-measuring"
    );
    let overrides = [
        PowerAmp::Bypass,
        PowerAmp::Cali6L6,
        PowerAmp::American6L6Clean,
        PowerAmp::American6L6HighGain,
        PowerAmp::BritEL34,
        PowerAmp::BritPlexiEL34,
    ];
    println!("// Measured by `examples/powertrim.rs`. Do not edit by hand.");
    println!("/// Level change, dB, of each power override relative to the voice's own path,");
    println!("/// by `Gain::ALL` row; columns Bypass, Cali 6L6, American 6L6 Clean,");
    println!("/// American 6L6 High-Gain, Brit EL34, Brit Plexi EL34. See `Chain::power_trim`.");
    println!("pub const POWER_TRIM_DB: [[f64; 6]; {}] = [", Gain::ALL.len());
    for gain in Gain::ALL {
        let reference = level(gain, PowerAmp::Matched);
        let row: Vec<String> = overrides
            .iter()
            .map(|p| format!("{:.2}", 20.0 * (level(gain, *p) / reference).log10()))
            .collect();
        println!("    [{}], // {}", row.join(", "), gain.name());
    }
    println!("];");
}
