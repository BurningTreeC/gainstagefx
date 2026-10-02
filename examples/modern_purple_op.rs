//! The Modern Purple (Revv G3) probed stage by stage: the gain of each stage
//! against the arithmetic of the trace's values, what AGGRESSION does at each
//! of its three steps, the tone controls' directions, and where VOLUME is
//! unity through the pedal for `examples/pedallevel.rs`'s guitar-like mix at
//! a guitar's level (`modern_purple::VOLUME_REST`).
//!
//! `cargo run --release --example modern_purple_op`

use gainstagefx::circuits::modern_purple::{self as g3, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn sim(node: &str, controls: &[(usize, f64)]) -> Simulation {
    let mut s = Simulation::new(tap(10_000.0, 470_000.0, node).unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    s.find_operating_point();
    s
}

/// Gain to `node`, dB, at `hz`, a small signal.
fn gain(node: &str, controls: &[(usize, f64)], hz: f64, amplitude: f64) -> f64 {
    let mut s = sim(node, controls);
    let t = Tone::near(RATE, 16_384, hz, amplitude);
    let m = measure::run(t, (RATE * 0.3) as usize, |x| s.process(x));
    20.0 * (m.fundamental().magnitude() / amplitude).log10()
}

fn mix_gain(controls: &[(usize, f64)]) -> f64 {
    let mut s = sim(g3::OUTPUT, controls);
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
    let small = 0.0005;
    println!("small-signal gain to each stage at 1 kHz, knobs at rest:");
    for node in [
        "o22", "n2", "o21", "o24", "o23", "clip", "x", "o12", "o13", "out",
    ] {
        println!("  {node:>5} {:+7.1} dB", gain(node, &[], 1_000.0, small));
    }
    println!("the gain stage (IC2.1 against its input) at full GAIN, by AGGRESSION:");
    for (name, a) in [("off", 0.1), ("Blue", 0.5), ("Red", 0.9)] {
        let c = [(g3::GAIN, 1.0), (g3::AGGRESSION, a)];
        let stage = gain("o21", &c, 1_000.0, small) - gain("p21", &c, 1_000.0, small);
        let low = gain("p21", &c, 200.0, small) - gain("p21", &c, 3_000.0, small);
        println!(
            "  {name:>4}: x{:.0} ({stage:+.1} dB); into it, 200 Hz against 3 kHz {low:+.1} dB",
            10f64.powf(stage / 20.0)
        );
    }
    println!("tone controls at the output, small signal, 0 against 1:");
    for (name, c, hz) in [
        ("BASS", g3::BASS, 100.0),
        ("TREBLE", g3::TREBLE, 5_000.0),
        ("MID", g3::MID, 700.0),
    ] {
        let lo = gain("o12", &[(c, 0.0)], hz, small);
        let hi = gain("o12", &[(c, 1.0)], hz, small);
        let lo13 = gain("o13", &[(c, 0.0)], hz, small);
        let hi13 = gain("o13", &[(c, 1.0)], hz, small);
        println!(
            "  {name:>6} at {hz:.0} Hz: IC1.2 {lo:+.1} -> {hi:+.1} dB, IC1.3 {lo13:+.1} -> {hi13:+.1} dB"
        );
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if mix_gain(&[(g3::VOLUME, mid)]) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    println!(
        "unity through the pedal for the 110 + 330 Hz mix: VOLUME at {:.4}",
        0.5 * (lo + hi)
    );
}
