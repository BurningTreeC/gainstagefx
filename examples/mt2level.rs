//! Where the Metal Zone's Level control is unity: `metal_zone::LEVEL_REST`.
//!
//! The pedal alone in volts, Dist and every tone control centred, fed a
//! guitar's low chord -- the probe every level measurement in the project
//! uses -- and the Level pot bisected for 0 dB broadband. Re-run after any
//! change to `metal_zone.rs` and paste the result.
//!
//! `cargo run --release --example mt2level`

use gainstagefx::circuits::metal_zone::{
    self, DIST, HIGH, LEVEL, LEVEL_REST, LOW, MIDDLE, MID_FREQ,
};
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

/// Broadband gain through the pedal, dB, with everything but Level centred.
fn gain_db(level: f64) -> f64 {
    let mut sim = Simulation::new(metal_zone::build(10_000.0, 470_000.0).unwrap(), RATE);
    for control in [DIST, LOW, MIDDLE, MID_FREQ, HIGH] {
        sim.set_control(control, 0.5);
    }
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
        if gain_db(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    println!(
        "unity with everything else centred: Level {:.4}",
        0.5 * (lo + hi)
    );
    println!("LEVEL_REST = {LEVEL_REST}: {:+.2} dB", gain_db(LEVEL_REST));
}
