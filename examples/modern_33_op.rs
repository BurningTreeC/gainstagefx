//! The Modern 33 (Fortin 33) probed: its bias against the arithmetic of the
//! TC Integrated Preamp's DC analysis, and its boost across the band against
//! Fortin's "+22 dB of clean gain"; and where LEVEL is unity through the
//! pedal for `examples/pedallevel.rs`'s guitar-like 110 Hz + 330 Hz mix at a
//! guitar's level, which is `modern_33::LEVEL_REST`.
//!
//! `cargo run --release --example modern_33_op`

use gainstagefx::circuits::modern_33::{self as m33, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain(level: f64, hz: f64, amplitude: f64) -> (f64, f64) {
    let mut s = Simulation::new(tap(10_000.0, 1_000_000.0, m33::OUTPUT).unwrap(), RATE);
    s.set_control(m33::LEVEL, level);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, amplitude);
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.3 }) as usize;
    let m = measure::run(t, settle, |x| s.process(x));
    (
        20.0 * (m.fundamental().magnitude() / amplitude).log10(),
        m.harmonic_percent(3),
    )
}

/// The pedal's output over its input, dB, for the 110 Hz + 330 Hz mix at a
/// guitar's 0.122 V, LEVEL at `level`.
fn mix_gain(level: f64) -> f64 {
    let mut s = Simulation::new(tap(10_000.0, 470_000.0, m33::OUTPUT).unwrap(), RATE);
    s.set_control(m33::LEVEL, level);
    s.find_operating_point();
    let n = RATE as usize;
    let (mut sx, mut sy) = (0.0, 0.0);
    for i in 0..n {
        let t = i as f64 / RATE;
        let x = 0.122
            * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7
                + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3);
        let y = s.process(x);
        if i >= n / 2 {
            sx += x * x;
            sy += y * y;
        }
    }
    10.0 * (sy / sx).log10()
}

fn main() {
    let c = tap(10_000.0, 1_000_000.0, m33::OUTPUT).unwrap();
    let nodes: Vec<(&str, usize)> = ["vcc", "vref", "b1", "e1", "c1", "o1", "e", "b", "o"]
        .iter()
        .map(|n| (*n, c.unknown_named(n).unwrap()))
        .collect();
    let mut s = Simulation::new(c, RATE);
    s.set_control(m33::LEVEL, 1.0);
    s.find_operating_point();
    println!("operating point:");
    for (name, i) in &nodes {
        println!("  {name:>5} {:8.3} V", s.voltage_at(*i));
    }
    println!("boost, LEVEL full, 50 mV in:");
    for hz in [
        30.0, 41.0, 60.0, 82.0, 110.0, 150.0, 220.0, 330.0, 500.0, 750.0, 1_000.0, 1_500.0,
        2_000.0, 3_000.0, 5_000.0, 8_000.0, 12_000.0,
    ] {
        let (g, h3) = gain(1.0, hz, 0.05);
        println!("  {hz:7.0} Hz {g:+6.1} dB  (third harmonic {h3:.3} %)");
    }
    println!("headroom at 1 kHz, LEVEL full:");
    for a in [0.1, 0.5, 1.0, 1.5, 2.0] {
        let (g, h3) = gain(1.0, 1_000.0, a);
        println!("  {a:4.1} V in: {g:+6.1} dB, third harmonic {h3:.2} %");
    }
    // Bisect for unity: the gain rises with the pot.
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if mix_gain(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    println!(
        "unity through the pedal for the 110 + 330 Hz mix: LEVEL at {:.4} ({:+.2} dB there; full LEVEL {:+.1} dB)",
        0.5 * (lo + hi),
        mix_gain(0.5 * (lo + hi)),
        mix_gain(1.0)
    );
}
