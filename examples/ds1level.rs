//! Where the Orange Dist's Level control is unity: `orange_dist::LEVEL_REST`.
//!
//! The pedal alone in volts, Dist and Tone centred, fed a guitar's low chord
//! -- the probe every level measurement in the project uses -- and the Level
//! pot (100 k linear, as Boss's drawing has it) bisected for 0 dB broadband.
//! Re-run after any change to `orange_dist.rs` and paste the result.
//!
//! `cargo run --release --example ds1level`

use gainstagefx::circuits::orange_dist::{self, DIST, LEVEL, LEVEL_REST, TONE};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::GUITAR_VOLTS;

const RATE: f64 = 48_000.0;

fn sample(k: usize) -> f64 {
    let t = k as f64 / RATE;
    GUITAR_VOLTS
        * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.50
            + (std::f64::consts::TAU * 123.5 * t).sin() * 0.30
            + (std::f64::consts::TAU * 246.9 * t).sin() * 0.20)
}

/// Broadband gain through the pedal, dB.
fn gain_db(dist: f64, tone: f64, level: f64) -> f64 {
    let mut sim = Simulation::new(orange_dist::build(10_000.0, 470_000.0).unwrap(), RATE);
    sim.set_control(DIST, dist);
    sim.set_control(TONE, tone);
    sim.set_control(LEVEL, level);
    sim.find_operating_point();
    let warm = (RATE / 2.0) as usize;
    for k in 0..warm {
        sim.process(sample(k));
    }
    let (mut x2, mut y2) = (0.0, 0.0);
    for k in warm..warm + RATE as usize {
        let x = sample(k);
        let y = sim.process(x);
        x2 += x * x;
        y2 += y * y;
    }
    10.0 * (y2 / x2).log10()
}

fn main() {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if gain_db(0.5, 0.5, mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let rest = 0.5 * (lo + hi);
    println!("unity with Dist and Tone centred: Level {rest:.4}");
    println!(
        "LEVEL_REST = {LEVEL_REST}: {:+.2} dB",
        gain_db(0.5, 0.5, LEVEL_REST)
    );
    println!("\nat that rest, across the other two controls:");
    for dist in [0.0, 0.5, 1.0] {
        for tone in [0.0, 0.5, 1.0] {
            println!(
                "  dist {dist:.1} tone {tone:.1}: {:+.1} dB",
                gain_db(dist, tone, LEVEL_REST)
            );
        }
    }
}
