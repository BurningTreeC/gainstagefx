//! The Bass Driver (SansAmp Bass Driver DI V2) against the tracer's own
//! simulation of the traced circuit, stage by stage, and where its LEVEL is
//! unity, for `bass_driver::LEVEL_REST`.
//!
//! The figures to meet are read off kanengomibako's LTspice plots of the same
//! trace -- an independent simulation, so they check this netlist's reading of
//! the drawing, not the hardware: presence up to +27.5 dB near 8 kHz; the
//! drive stages flat from 12.9 to 48.5 dB; the emulation path's 85 Hz peak,
//! 700 Hz notch and 2.8 kHz peak; mid +-18 dB at 450 / 900 Hz; bass +14 dB near
//! 45 Hz (40 Hz shift) and +11 dB near 90 Hz (80 Hz shift); treble +17 dB at
//! 3 kHz. Each is measured between two taps with a signal small enough that
//! neither the zeners nor a rail are reached.
//!
//! `cargo run --release --example bass_driver_op`

use gainstagefx::circuits::bass_driver::{self as bd, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::GUITAR_VOLTS;

const RATE: f64 = 96_000.0;

/// Gain in dB from the input to `node`, at `hz`, with `controls` set and the
/// shifts at `(bass, mid)` (indices into `BASS_SHIFT` and `MID_SHIFT`).
fn gain(node: &str, controls: &[(usize, f64)], shifts: (usize, usize), hz: f64, volts: f64) -> f64 {
    let mut s = Simulation::new(tap(10_000.0, 1_000_000.0, node).unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    for (&slot, &v) in bd::BASS_SHIFT_SLOTS.iter().zip(&bd::BASS_SHIFT[shifts.0]) {
        s.set_value(slot, v);
    }
    for (&slot, &v) in bd::MID_SHIFT_SLOTS.iter().zip(&bd::MID_SHIFT[shifts.1]) {
        s.set_value(slot, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts);
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.25 }) as usize;
    measure::run(t, settle, |x| s.process(x)).gain_db()
}

/// Gain from `from` to `to`: the stage between two taps.
fn stage(from: &str, to: &str, controls: &[(usize, f64)], shifts: (usize, usize), hz: f64) -> f64 {
    let v = 1e-4;
    gain(to, controls, shifts, hz, v) - gain(from, controls, shifts, hz, v)
}

const NOON: [(usize, f64); 7] = [
    (bd::DRIVE, 0.5),
    (bd::PRESENCE, 0.5),
    (bd::BLEND, 0.5),
    (bd::BASS, 0.5),
    (bd::MID, 0.5),
    (bd::TREBLE, 0.5),
    (bd::LEVEL, 0.5),
];

fn with(control: usize, value: f64) -> Vec<(usize, f64)> {
    let mut c = NOON.to_vec();
    c.retain(|&(k, _)| k != control);
    c.push((control, value));
    c
}

fn sample(k: usize) -> f64 {
    let t = k as f64 / 48_000.0;
    GUITAR_VOLTS
        * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.50
            + (std::f64::consts::TAU * 123.5 * t).sin() * 0.30
            + (std::f64::consts::TAU * 246.9 * t).sin() * 0.20)
}

/// Broadband gain through the whole pedal, fed a guitar's low chord, with
/// LEVEL at `level` and everything else at noon, Blend too -- the project's
/// rule for a pedal's level rest, and where the slot's knobs start.
fn through(level: f64) -> f64 {
    let mut s = Simulation::new(bd::build(10_000.0, 470_000.0).unwrap(), 48_000.0);
    for (c, v) in with(bd::LEVEL, level) {
        s.set_control(c, v);
    }
    s.find_operating_point();
    for k in 0..24_000 {
        s.process(sample(k));
    }
    let (mut x2, mut y2) = (0.0, 0.0);
    for k in 24_000..72_000 {
        let x = sample(k);
        let y = s.process(x);
        x2 += x * x;
        y2 += y * y;
    }
    10.0 * (y2 / x2).log10()
}

fn main() {
    let built = (1, 1);
    println!("presence stage, nw -> u1a (tracer: 0 dB low, up to +27.5 dB near 8 kHz at the top):");
    for p in [0.0, 0.25, 0.5, 0.75, 1.0] {
        print!("  {p:.2}:");
        for hz in [100.0, 1_000.0, 3_000.0, 8_000.0] {
            print!(
                " {hz:.0} {:+.1}",
                stage("nw", "u1a", &with(bd::PRESENCE, p), built, hz)
            );
        }
        println!();
    }
    println!("\ndrive stages, u1a -> o2 at 1 kHz (tracer: 12.9, 21.5, 28.5, 35.5, 48.5 dB):");
    for d in [0.0, 0.25, 0.5, 0.75, 1.0] {
        print!(
            "  {d:.2}: {:+.1} dB",
            stage("u1a", "o2", &with(bd::DRIVE, d), built, 1_000.0)
        );
        println!(
            ", 10 kHz {:+.1}",
            stage("u1a", "o2", &with(bd::DRIVE, d), built, 10_000.0)
        );
    }
    println!("\nemulation, in -> u2a at minimum drive and presence (tracer: peak ~85 Hz, notch ~700 Hz, peak ~2.8 kHz 7 dB under the first):");
    let quiet = vec![
        (bd::DRIVE, 0.0),
        (bd::PRESENCE, 0.0),
        (bd::BLEND, 1.0),
        (bd::BASS, 0.5),
        (bd::MID, 0.5),
        (bd::TREBLE, 0.5),
        (bd::LEVEL, 0.5),
    ];
    for hz in [
        40.0, 85.0, 200.0, 500.0, 700.0, 1_000.0, 2_000.0, 2_800.0, 5_000.0, 10_000.0,
    ] {
        print!(" {hz:.0} {:+.1}", gain("u2a", &quiet, built, hz, 1e-4));
    }
    println!();
    println!("\nmid, u3b -> u3a (tracer: +-18 dB at ~450 Hz with 500, ~900 Hz with 1000):");
    for (shift, name) in [(1, "500 Hz"), (0, "1 kHz")] {
        for m in [0.0, 1.0] {
            print!("  {name} mid {m:.0}:");
            for hz in [200.0, 450.0, 900.0, 2_000.0] {
                print!(
                    " {hz:.0} {:+.1}",
                    stage("u3b", "u3a", &with(bd::MID, m), (1, shift), hz)
                );
            }
            println!();
        }
    }
    println!("\nbass, u3a -> u6b (tracer: +14 dB near 45 Hz at 40, +11 near 90 Hz at 80; cuts -17 / -13):");
    for (shift, name) in [(0, "40 Hz"), (1, "80 Hz")] {
        for b in [0.0, 1.0] {
            print!("  {name} bass {b:.0}:");
            for hz in [30.0, 45.0, 90.0, 200.0, 1_000.0] {
                print!(
                    " {hz:.0} {:+.1}",
                    stage("u3a", "u6b", &with(bd::BASS, b), (shift, 1), hz)
                );
            }
            println!();
        }
    }
    println!("\ntreble, u3a -> u6b (tracer: +17 dB at 3 kHz at the top):");
    for tr in [0.0, 0.5, 1.0] {
        print!("  {tr:.1}:");
        for hz in [1_000.0, 3_000.0, 10_000.0, 20_000.0] {
            print!(
                " {hz:.0} {:+.1}",
                stage("u3a", "u6b", &with(bd::TREBLE, tr), built, hz)
            );
        }
        println!();
    }
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..18 {
        let mid = 0.5 * (lo + hi);
        if through(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    println!(
        "\nLEVEL unity through the pedal at {:.4}; the constant {} gives {:+.2} dB",
        0.5 * (lo + hi),
        bd::LEVEL_REST,
        through(bd::LEVEL_REST)
    );
}
