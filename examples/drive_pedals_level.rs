//! Where the Gold Drive's OUTPUT and the Brit Drive's LEVEL are unity:
//! `gold_drive::LEVEL_REST` and `brit_drive::LEVEL_REST`.
//!
//! Each pedal alone in volts, its gain and tone controls at noon, fed a
//! guitar's low chord -- the probe every level measurement in the project uses
//! -- and the level pot bisected for 0 dB broadband. Re-run after a change to
//! either circuit and paste the result.
//!
//! `cargo run --release --example drive_pedals_level`

use gainstagefx::circuits::{brit_drive, gold_drive};
use gainstagefx::dsp::netlist::Circuit;
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

fn gain_db(circuit: Circuit, level: usize, at: f64) -> f64 {
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(level, at);
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

fn unity(name: &str, build: fn() -> Circuit, level: usize, rest: f64) {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if gain_db(build(), level, mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    println!(
        "{name}: unity at {:.4}; the constant {rest} gives {:+.2} dB",
        0.5 * (lo + hi),
        gain_db(build(), level, rest)
    );
}

fn main() {
    unity(
        "Gold Drive OUTPUT",
        || gold_drive::build(10_000.0, 470_000.0).unwrap(),
        gold_drive::LEVEL,
        gold_drive::LEVEL_REST,
    );
    unity(
        "Brit Drive LEVEL",
        || brit_drive::build(10_000.0, 470_000.0).unwrap(),
        brit_drive::LEVEL,
        brit_drive::LEVEL_REST,
    );
}
