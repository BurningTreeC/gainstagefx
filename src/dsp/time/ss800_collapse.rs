//! What the American SS 800's power stage is doing when its solves collapse.
//!
//! `cargo test --release --lib ss800_collapse -- --ignored --nocapture`
//! (`GSFX_GAIN_DB` sets the take's gain, 6 by default; `GSFX_FROM` and
//! `GSFX_SAMPLES` the window printed sample by sample.)
//!
//! Plays *American 800RB Clank* over the fixture take and records, for every
//! power-stage solve, its Newton passes and fallbacks and the stage's node
//! voltages. Prints where the hard solves are, a window of them sample by
//! sample, and the nodes' ranges inside hard stretches against the rest.

use super::Simulation;
use crate::presets::PRESETS;
use crate::voice::Chain;

const RATE: f64 = 48_000.0;
const NODES: [&str; 14] = [
    "in", "um", "up", "u1_int", "uo", "e11", "qc", "vas", "bot", "bs", "de1", "eh", "out", "vp",
];

fn take() -> Vec<f64> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/01-260912_1044.wav"
    ))
    .expect("the take");
    let mut at = 12;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if &bytes[at..at + 4] == b"data" {
            data = Some(&bytes[at + 8..(at + 8 + len).min(bytes.len())]);
        }
        at += 8 + len + (len & 1);
    }
    data.unwrap()
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| ((b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64)
        .map(|v| v / 8_388_608.0)
        .collect()
}

fn env(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
#[ignore = "diagnostic: what the SS 800 is doing in its hard solves"]
fn ss800_collapse() {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == "American 800RB Clank")
        .expect("preset");
    let scale = 10f64.powf((preset.input_trim as f64 + env("GSFX_GAIN_DB", 6.0)) / 20.0);
    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();
    let take = take();
    let sim: &Simulation = chain.test_active_power_mut().expect("a power stage");
    let idx: Vec<usize> = NODES
        .iter()
        .map(|n| {
            sim.circuit
                .unknown_named(n)
                .unwrap_or_else(|| panic!("no node {n}"))
        })
        .collect();
    let n = take.len().min(8 * RATE as usize);
    let mut passes = Vec::with_capacity(n);
    let mut fallbacks = Vec::with_capacity(n);
    let mut volts = Vec::with_capacity(n);
    let (mut p0, mut f0) = {
        let s = chain.test_active_power_mut().unwrap();
        (s.newton_passes, s.fallbacks)
    };
    let trace_at = env("GSFX_TRACE", -1.0);
    for (i, &x) in take[..n].iter().enumerate() {
        if trace_at >= 0.0 && i == trace_at as usize {
            let s = chain.test_active_power_mut().unwrap();
            s.set_late_continuation(true);
            s.configure_full_power_trace(0, 1);
        }
        chain.process_stereo(x * scale);
        if trace_at >= 0.0 && i == trace_at as usize {
            chain
                .test_active_power_mut()
                .unwrap()
                .print_full_power_trace();
            return;
        }
        let s = chain.test_active_power_mut().unwrap();
        passes.push(s.newton_passes - p0);
        fallbacks.push(s.fallbacks - f0);
        (p0, f0) = (s.newton_passes, s.fallbacks);
        volts.push(idx.iter().map(|&i| s.voltage_at(i)).collect::<Vec<f64>>());
    }
    let hard: Vec<bool> = passes.iter().map(|&p| p >= 10).collect();
    println!(
        "{} solves, {} hard (>= 10 passes), {} fallbacks",
        n,
        hard.iter().filter(|&&h| h).count(),
        fallbacks.iter().sum::<u64>()
    );
    let first = hard.iter().position(|&h| h).unwrap_or(0);
    let from = env("GSFX_FROM", first.saturating_sub(20) as f64) as usize;
    let count = env("GSFX_SAMPLES", 60.0) as usize;
    print!("{:>7} {:>4} {:>3}", "sample", "pass", "fb");
    for name in NODES {
        print!(" {name:>8}");
    }
    println!();
    for i in from..(from + count).min(n) {
        print!("{i:>7} {:>4} {:>3}", passes[i], fallbacks[i]);
        for v in &volts[i] {
            print!(" {v:>8.3}");
        }
        println!();
    }
    for (label, in_hard) in [("rest", false), ("hard", true)] {
        println!("node ranges in {label} solves:");
        for (k, name) in NODES.iter().enumerate() {
            let vs: Vec<f64> = (0..n)
                .filter(|&i| hard[i] == in_hard)
                .map(|i| volts[i][k])
                .collect();
            if vs.is_empty() {
                continue;
            }
            let lo = vs.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let mean = vs.iter().sum::<f64>() / vs.len() as f64;
            // Sign changes of the sample-to-sample step: a high rate is an
            // oscillation near the Nyquist frequency rather than audio.
            let flips = vs
                .windows(3)
                .filter(|w| (w[1] - w[0]) * (w[2] - w[1]) < 0.0)
                .count() as f64
                / vs.len() as f64;
            println!(
                "  {name:>7} {lo:9.3} .. {hi:9.3}  mean {mean:9.3}  step sign flips {flips:.2}"
            );
        }
    }
}
