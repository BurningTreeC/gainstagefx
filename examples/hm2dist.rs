//! Realtime/solver cost of the HM-2 across the entire DIST control.
//!
//! This is deliberately the "Swedish death" stress configuration: both
//! Colour Mix controls and Level are full up. It catches a control mapping or
//! gain-stage topology that is cheap at zero but becomes pathological as soon
//! as DIST is turned.
//!
//! `cargo run --release --example hm2dist`

use gainstagefx::circuits::heavy_metal::{self, DIST, HIGH, LEVEL, LOW};
use gainstagefx::dsp::time::Simulation;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const POSITIONS: &[f64] = &[0.0, 0.01, 0.025, 0.05, 0.10, 0.25, 0.50, 0.75, 1.0];

fn main() {
    println!(
        "{:<8}{:>11}{:>13}{:>12}{:>13}{:>11}{:>12}{:>10}",
        "DIST",
        "% realtime",
        "passes/solve",
        "unsettled",
        "backtracks",
        "fallbacks",
        "nonfinite",
        "peak"
    );

    for &position in POSITIONS {
        let mut sim = Simulation::new(heavy_metal::build(10_000.0, 470_000.0).unwrap(), RATE);
        sim.set_control(DIST, position);
        sim.set_control(LOW, 1.0);
        sim.set_control(HIGH, 1.0);
        sim.set_control(LEVEL, 1.0);
        sim.find_operating_point();

        // Warm all reactive state and the solver's predictor before timing.
        for k in 0..4_800 {
            let t = k as f64 / RATE;
            let x = 0.122
                * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.65
                    + (std::f64::consts::TAU * 123.5 * t).sin() * 0.35);
            sim.process(x);
        }

        let (solves0, passes0, unsettled0, _) = sim.statistics();
        let (backtracks0, fallbacks0, nonfinite0) = sim.health();
        let mut peak = 0.0_f64;
        let start = Instant::now();

        for k in 0..RATE as usize {
            let t = k as f64 / RATE;
            let x = 0.122
                * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.65
                    + (std::f64::consts::TAU * 123.5 * t).sin() * 0.35);
            peak = peak.max(sim.process(x).abs());
        }

        let elapsed = start.elapsed().as_secs_f64();
        let (solves1, passes1, unsettled1, _) = sim.statistics();
        let (backtracks1, fallbacks1, nonfinite1) = sim.health();
        let solves = solves1.saturating_sub(solves0).max(1);
        let passes = passes1.saturating_sub(passes0);

        println!(
            "{:<8.3}{:>11.1}{:>13.3}{:>12}{:>13}{:>11}{:>12}{:>10.4}",
            position,
            elapsed * 100.0,
            passes as f64 / solves as f64,
            unsettled1.saturating_sub(unsettled0),
            backtracks1.saturating_sub(backtracks0),
            fallbacks1.saturating_sub(fallbacks0),
            nonfinite1.saturating_sub(nonfinite0),
            peak,
        );
    }
}
