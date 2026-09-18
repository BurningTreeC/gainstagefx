//! HM-2 output-level calibration after the circuit/topology fixes.
//!
//! This measures the pedal in volts, before any preset `output_trim`, so a
//! preset cannot hide a bad pedal calibration. `LEVEL` below is the physical
//! 10 k log pot position inside the HM-2 netlist, not the normalized plugin
//! knob around `LEVEL_REST`.
//!
//! `cargo run --release --example hm2level`

use gainstagefx::circuits::heavy_metal::{self, DIST, HIGH, LEVEL, LEVEL_REST, LOW};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::GUITAR_VOLTS;

const RATE: f64 = 48_000.0;
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 470_000.0;

#[derive(Clone, Copy)]
struct Reading {
    gain_db: f64,
    peak: f64,
}

fn sample(k: usize) -> f64 {
    let t = k as f64 / RATE;
    GUITAR_VOLTS
        * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.50
            + (std::f64::consts::TAU * 123.5 * t).sin() * 0.30
            + (std::f64::consts::TAU * 246.9 * t).sin() * 0.20)
}

fn measure(dist: f64, low: f64, high: f64, level: f64) -> Reading {
    let mut sim = Simulation::new(heavy_metal::build(SOURCE, LOAD).unwrap(), RATE);
    sim.set_control(DIST, dist);
    sim.set_control(LOW, low);
    sim.set_control(HIGH, high);
    sim.set_control(LEVEL, level);
    sim.find_operating_point();

    let warm = (RATE / 2.0) as usize;
    for k in 0..warm {
        sim.process(sample(k));
    }

    let n = RATE as usize;
    let mut in_sq = 0.0;
    let mut out_sq = 0.0;
    let mut peak = 0.0_f64;
    for k in warm..warm + n {
        let x = sample(k);
        let y = sim.process(x);
        in_sq += x * x;
        out_sq += y * y;
        peak = peak.max(y.abs());
    }

    let in_rms = (in_sq / n as f64).sqrt();
    let out_rms = (out_sq / n as f64).sqrt();
    Reading {
        gain_db: 20.0 * (out_rms / in_rms.max(1e-12)).max(1e-12).log10(),
        peak,
    }
}

fn main() {
    println!("HM-2 physical output-level calibration at {RATE:.0} Hz");
    println!("input = {GUITAR_VOLTS:.3} V nominal low-chord mixture\n");

    // The resting position is supposed to make the pedal level sensible with
    // the sound controls centered. Re-scan it after any topology change.
    let mut best = (
        f64::INFINITY,
        0.0,
        Reading {
            gain_db: 0.0,
            peak: 0.0,
        },
    );
    for step in 0..=100 {
        let level = step as f64 / 100.0;
        let r = measure(0.5, 0.5, 0.5, level);
        if r.gain_db.abs() < best.0 {
            best = (r.gain_db.abs(), level, r);
        }
    }

    let current = measure(0.5, 0.5, 0.5, LEVEL_REST);
    println!("centred DIST/LOW/HIGH:");
    println!(
        "  current LEVEL_REST={LEVEL_REST:.3}: {:+.2} dB, peak {:.4} V",
        current.gain_db, current.peak
    );
    println!(
        "  nearest 1%-step unity position={:.2}: {:+.2} dB, peak {:.4} V",
        best.1, best.2.gain_db, best.2.peak
    );

    println!("\nall-knobs-max HM-2 (Swedish-death setting):");
    println!("  LEVEL    pedal gain    trim to unity    output peak");
    for level in [0.25, 0.50, LEVEL_REST, 0.75, 1.00] {
        let r = measure(1.0, 1.0, 1.0, level);
        println!(
            "  {:>5.2}    {:+8.2} dB    {:+8.2} dB      {:>7.4} V",
            level, r.gain_db, -r.gain_db, r.peak
        );
    }

    println!("\nSlaughter '95 HM-2 settings before the MT-2:");
    // The preset's normalized Pedal Level=0.55 maps around LEVEL_REST in
    // `Chain::master_position`: rest + (1-rest)*0.10 for the upper half.
    let slaughter_level = LEVEL_REST + (1.0 - LEVEL_REST) * 0.10;
    let r = measure(0.85, 0.75, 0.80, slaughter_level);
    println!(
        "  physical LEVEL={slaughter_level:.3}: {:+.2} dB, suggested pedal-only trim {:+.2} dB, peak {:.4} V",
        r.gain_db,
        -r.gain_db,
        r.peak
    );
}
