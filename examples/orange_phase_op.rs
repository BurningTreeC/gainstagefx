//! The Orange Phase (MXR Phase 90) against ElectroSmash's figures for it, and
//! the fit of its bias trimmer.
//!
//! The oscillator's period across the Speed knob ("from tenths of Hz to some
//! Hertz"); the gate line's swing and the trimmer position that puts the
//! bottom of it at the JFETs' cut-off ("adjusts the LFO output to a particular
//! matched set of JFETs for a maximum span"); the notches with the JFETs off
//! (58.5 Hz and 340.8 Hz, from 24 k and 47 nF); and the stage-by-stage levels
//! with the oscillator stopped.
//!
//! `cargo run --release --example orange_phase_op`

use gainstagefx::circuits::orange_phase::{self as p90, tap, J2N5952};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 48_000.0;

/// The gate line and the oscillator's output over `seconds`, after `settle`.
fn run(speed: f64, bias: f64, settle: f64, seconds: f64) -> (Vec<f64>, Vec<f64>) {
    let c = tap(10_000.0, 470_000.0, "out").unwrap();
    let (gate, lfo) = (
        c.unknown_named("gate").unwrap(),
        c.unknown_named("lfo").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    s.set_control(p90::SPEED, speed);
    s.set_control(p90::BIAS, bias);
    s.find_operating_point();
    for _ in 0..(RATE * settle) as usize {
        s.process(0.0);
    }
    let n = (RATE * seconds) as usize;
    let mut g = Vec::with_capacity(n);
    let mut l = Vec::with_capacity(n);
    for _ in 0..n {
        s.process(0.0);
        g.push(s.voltage_at(gate));
        l.push(s.voltage_at(lfo));
    }
    (g, l)
}

/// The oscillator's period from its square wave's rising crossings.
fn period(lfo: &[f64]) -> Option<f64> {
    let mid = 0.5
        * (lfo.iter().cloned().fold(f64::MAX, f64::min)
            + lfo.iter().cloned().fold(f64::MIN, f64::max));
    let rises: Vec<usize> = lfo
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < mid && w[1] >= mid)
        .map(|(i, _)| i)
        .collect();
    (rises.len() >= 2)
        .then(|| (rises[rises.len() - 1] - rises[0]) as f64 / (rises.len() - 1) as f64 / RATE)
}

fn main() {
    println!(
        "the oscillator against Speed (bias at rest {}):",
        p90::BIAS_REST
    );
    for speed in [0.0, 0.25, 0.5, 0.75, 1.0] {
        // Long enough for two periods at the slowest.
        let seconds = if speed < 0.3 { 24.0 } else { 6.0 };
        let (g, l) = run(speed, p90::BIAS_REST, 2.0, seconds);
        let (lo, hi) = (
            g.iter().cloned().fold(f64::MAX, f64::min),
            g.iter().cloned().fold(f64::MIN, f64::max),
        );
        let (llo, lhi) = (
            l.iter().cloned().fold(f64::MAX, f64::min),
            l.iter().cloned().fold(f64::MIN, f64::max),
        );
        match period(&l) {
            Some(t) => println!(
                "  speed {speed:.2}: {:.3} Hz ({t:.3} s); gates {lo:+.3} .. {hi:+.3} V re the zener; U1b {llo:+.2} .. {lhi:+.2}",
                1.0 / t
            ),
            None => println!("  speed {speed:.2}: no oscillation; gates {lo:+.3} .. {hi:+.3}; U1b {llo:+.2} .. {lhi:+.2}"),
        }
    }

    // --- the trimmer: the bottom of the swing at cut-off ------------------------------
    let bottom = |bias: f64| {
        let (g, _) = run(0.5, bias, 2.0, 3.0);
        g.iter().cloned().fold(f64::MAX, f64::min)
    };
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if bottom(mid) > J2N5952.pinch_off {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    println!(
        "\nbias: trimmer at {:.4} puts the swing's bottom at {:.3} V, the cut-off {}; BIAS_REST is {}",
        0.5 * (lo + hi),
        bottom(0.5 * (lo + hi)),
        J2N5952.pinch_off,
        p90::BIAS_REST
    );

    // --- the notches with the JFETs off ----------------------------------------------
    println!("\nresponse with the JFETs off (trimmer down), re 1 kHz, script and block:");
    for block in [0.0, 1.0] {
        let level = |hz: f64| {
            let mut s = Simulation::new(tap(10_000.0, 470_000.0, "out").unwrap(), RATE);
            s.set_control(p90::BIAS, 0.0);
            s.set_control(p90::BLOCK, block);
            s.find_operating_point();
            let t = Tone::near(RATE, 16_384, hz, 0.01);
            let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.4 }) as usize;
            measure::run(t, settle, |x| s.process(x))
                .fundamental()
                .magnitude()
                / 0.01
        };
        let reference = level(1_000.0);
        print!("  {}:", if block > 0.5 { "block " } else { "script" });
        for hz in [30.0, 58.5, 100.0, 200.0, 340.8, 600.0, 2_000.0, 5_000.0] {
            print!(" {hz:.0} {:+.1}", 20.0 * (level(hz) / reference).log10());
        }
        println!("   (1 kHz gain {:+.1} dB)", 20.0 * reference.log10());
    }
}
