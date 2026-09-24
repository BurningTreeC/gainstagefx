//! Measures the make-up correction for power overrides and prints it as Rust.
//!
//! Each voice's calibration was measured on its own path. With a different
//! output stage behind it the level can move by tens of decibels. This measures
//! that change at the calibration drive, on the tone-off,
//! legacy-cabinet-off path, and prints the table `Chain::power_trim` divides out.
//!
//! The currently checked-in trim is allowed to be non-zero. For an existing table
//! entry `T`, the measured residual is `raw_delta - T`, because `power_trim()` is a
//! pure post-circuit gain of `-T` dB. Therefore the new calibration is exactly
//! `T + residual`. This lets the generator self-correct after circuit changes and
//! avoids temporarily replacing the source table with zeros.
//!
//! IMPORTANT: never redirect Cargo directly onto `src/power_trim.rs`: the shell
//! truncates the source file before Cargo compiles the example. Generate to a
//! temporary file, validate it, and only then replace the source, for example:
//!
//! `cargo run --release --example powertrim > /tmp/power_trim.rs`
//! `mv /tmp/power_trim.rs src/power_trim.rs`

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
            // The same low chord the calibration and the preset levels use, so
            // every level measurement in the project is made on one signal.
            * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.5
                + (std::f64::consts::TAU * 123.5 * t).sin() * 0.3
                + (std::f64::consts::TAU * 246.9 * t).sin() * 0.2);
        let y = chain.process(x);
        if i >= n / 2 {
            sum += y * y;
        }
    }
    (sum / (n / 2) as f64).sqrt()
}

fn main() {
    // Derived from the enum rather than written out, so appending a power
    // amplifier cannot silently print a row that is one column short. It did:
    // the American Deluxe 6V6 was added, this list was a hand-written literal,
    // and the generator produced ten columns for an eleven-column table.
    let overrides: Vec<PowerAmp> = PowerAmp::ALL
        .into_iter()
        .filter(|amp| *amp != PowerAmp::Matched)
        .collect();
    // Only for the doc comment, and checked against the list above so the two
    // cannot drift apart either.
    let column_names: &[&str] = &[
        "Bypass",
        "Cali 6L6",
        "American 6L6 Clean",
        "American 6L6 High-Gain",
        "Brit EL34",
        "Brit Plexi EL34",
        "AC30 EL84",
        "DR103 EL34",
        "Recto 6L6",
        "Recto 6L6 Tube",
        "American Deluxe 6V6",
        "Brit 2205 EL34",
    ];
    assert_eq!(
        column_names.len(),
        overrides.len(),
        "the column names in powertrim.rs have fallen behind PowerAmp::ALL"
    );
    println!("// Measured by `examples/powertrim.rs`. Do not edit by hand.");
    println!("/// Level change, dB, of each power override relative to the voice's own path,");
    println!(
        "/// by `Gain::ALL` row; columns {}.",
        column_names.join(", ")
    );
    println!("/// See `Chain::power_trim`.");
    // These are measured decibels. A row that happens to contain 3.14 is not a
    // programmer reaching for pi from memory, which is what `approx_constant`
    // is for, and the American Deluxe's Bypass column measured exactly that.
    println!("#[allow(");
    println!("    clippy::approx_constant,");
    println!("    reason = \"measured decibels; a 3.14 here is a measurement, not pi\"");
    println!(")]");
    // Tied to the gain list rather than written as a number, so that appending
    // a circuit fails to compile instead of indexing past the end of this table
    // at runtime -- which is what it did when six were appended at once.
    println!(
        "pub const POWER_TRIM_DB: [[f64; {}]; crate::voice::GAINS] = [",
        overrides.len()
    );
    for (row_index, gain) in Gain::ALL.into_iter().enumerate() {
        let reference = level(gain, PowerAmp::Matched);
        assert!(
            reference.is_finite() && reference > 0.0,
            "{} matched reference level is not finite and positive: {reference}",
            gain.name()
        );
        let row: Vec<String> = overrides
            .iter()
            .enumerate()
            .map(|(column, p)| {
                let overridden = level(gain, *p);
                assert!(
                    overridden.is_finite() && overridden > 0.0,
                    "{} / {p:?} level is not finite and positive: {overridden}",
                    gain.name()
                );

                // `level()` includes the currently checked-in power trim. If D is
                // the raw level change and T is the current table entry, then the
                // measured residual is R = D - T because `power_trim()` applies
                // -T dB after the circuit. Therefore D = T + R. This recovers the
                // raw calibration without ever needing an all-zero source table.
                let old = POWER_TRIM_DB[row_index][column];
                let residual = 20.0 * (overridden / reference).log10();
                let calibrated = old + residual;
                assert!(
                    calibrated.is_finite(),
                    "{} / {p:?} produced a non-finite calibration",
                    gain.name()
                );
                eprintln!(
                    "{:<20} {:<24?} old={old:+7.2} dB residual={residual:+7.2} dB new={calibrated:+7.2} dB",
                    gain.name(),
                    p
                );
                format!("{calibrated:.2}")
            })
            .collect();
        println!("    [{}], // {}", row.join(", "), gain.name());
    }
    println!("];");
}
