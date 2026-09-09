//! What the three cores actually do, and where.
//!
//! Flux is the integral of voltage, so it goes as `V / f`: a transformer
//! saturates on a low note and is linear on a high one. `IRON_VOLTS` was
//! chosen so that a nominal signal at **40 Hz** lands about at steel's knee --
//! which says nothing about what happens at 220 Hz, where a guitar mostly
//! lives.
//!
//! `cargo run --release --example ironcheck`

use gainstagefx::voice::{Cabinet, Chain, Gain, Iron, Settings, Tone, IRON_VOLTS};

const RATE: f64 = 96_000.0;
const N: usize = 1 << 14;

/// Harmonic distortion of a chain's output on a pure tone.
fn thd(chain: &mut Chain, hz: f64, amp: f64) -> (f64, f64) {
    let warm = (RATE / 4.0) as usize;
    for k in 0..warm {
        chain.process(amp * (std::f64::consts::TAU * hz * k as f64 / RATE).sin());
    }
    let mut out = Vec::with_capacity(N);
    for k in warm..(warm + N) {
        out.push(chain.process(amp * (std::f64::consts::TAU * hz * k as f64 / RATE).sin()));
    }
    let bin = |m: f64| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let w = std::f64::consts::TAU * m * hz / RATE;
        for (i, &v) in out.iter().enumerate() {
            re += v * (w * i as f64).cos();
            im -= v * (w * i as f64).sin();
        }
        (re * re + im * im).sqrt()
    };
    let f = bin(1.0);
    let harmonics: f64 = (2..=8).map(|m| bin(m as f64).powi(2)).sum::<f64>().sqrt();
    let rms = (out.iter().map(|v| v * v).sum::<f64>() / out.len() as f64).sqrt();
    (100.0 * harmonics / f.max(1e-30), rms)
}

fn chain_with(iron: Iron, drive: f64) -> Chain {
    let mut c = Chain::new(RATE);
    c.apply(&Settings {
        gain: Gain::Clean,
        iron,
        tone: Tone::Off,
        cabinet: Cabinet::Off,
        drive,
        oversampling: 1,
        ..Settings::default()
    });
    c.settle();
    c.find_operating_point();
    c
}

fn main() {
    println!("Clean voice, tone and cabinet out, so only the iron differs.");
    println!("IRON_VOLTS is {IRON_VOLTS}, and flux goes as V/f.\n");

    // Does the Drive control reach the iron at all? It is the question the
    // whole arrangement turns on: the make-up holds the output level constant,
    // so anything behind it never gets driven any harder.
    println!("=== does Drive reach the iron? (Clean voice, 82 Hz, nominal) ===");
    println!(
        "  {:<8}{:>10}{:>10}{:>10}{:>12}",
        "drive", "off", "nickel", "steel", "spread"
    );
    for drive in [0.0f64, 0.25, 0.5, 0.75, 1.0] {
        let mut row = Vec::new();
        for iron in [Iron::Off, Iron::Nickel, Iron::Steel] {
            let mut c = chain_with(iron, drive);
            row.push(thd(&mut c, 82.0, 0.126).0);
        }
        println!(
            "  {drive:<8.2}{:>9.2} %{:>9.2} %{:>9.2} %{:>10.2} pts",
            row[0],
            row[1],
            row[2],
            (row[1] - row[2]).abs()
        );
    }
    println!();

    for amp in [0.126f64, 0.5, 1.0] {
        println!("=== input {amp:.3} ({:.0} dBFS) ===", 20.0 * amp.log10());
        println!(
            "  {:<8}{:>10}{:>10}{:>10}{:>10}{:>12}",
            "Hz", "off", "nickel", "steel", "amorph", "spread"
        );
        for hz in [40.0f64, 82.0, 110.0, 220.0, 440.0, 1000.0] {
            let mut row = Vec::new();
            for iron in [Iron::Off, Iron::Nickel, Iron::Steel, Iron::Amorphous] {
                let mut c = chain_with(iron, 0.5);
                row.push(thd(&mut c, hz, amp).0);
            }
            let spread = row[1..].iter().cloned().fold(f64::MIN, f64::max)
                - row[1..].iter().cloned().fold(f64::MAX, f64::min);
            println!(
                "  {hz:<8.0}{:>9.2} %{:>9.2} %{:>9.2} %{:>9.2} %{spread:>10.2} pts",
                row[0], row[1], row[2], row[3]
            );
        }
        println!();
    }
    println!("'spread' is the difference between the three cores. If it is small");
    println!("the control does nothing audible at that frequency and level.");
}
