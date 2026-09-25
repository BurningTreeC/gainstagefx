//! The Orange Dist (Boss DS-1, TA7136P) stage by stage: its operating point,
//! and the small-signal gain from the jack to each stage.
//!
//! A stage that reads forty decibels below its neighbour is a mis-wired
//! stage; this is the first thing to run after touching `orange_dist.rs`.
//!
//! `cargo run --release --example ds1_op`

use gainstagefx::circuits::orange_dist::{self, DIST, LEVEL, TONE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain_db(node: &str, dist: f64, tone: f64, hz: f64) -> f64 {
    let mut s = Simulation::new(orange_dist::tap(10_000.0, 470_000.0, node).unwrap(), RATE);
    s.set_control(DIST, dist);
    s.set_control(TONE, tone);
    s.set_control(LEVEL, 1.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-5);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
}

fn main() {
    let circuit = orange_dist::build(10_000.0, 470_000.0).unwrap();
    let nodes = [
        "v9", "vref", "b1", "e1", "b2", "c2", "e2", "plus", "minus", "amp", "clip", "b3", "e3",
    ];
    let index: Vec<usize> = nodes
        .iter()
        .map(|n| circuit.unknown_named(n).expect("node"))
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    println!("operating point:");
    for (name, i) in nodes.iter().zip(index) {
        println!("  {name:<6} {:>8.3} V", s.voltage_at(i));
    }

    println!("\nsmall-signal gain from the jack (10 uV), Dist 0.5, Tone 0.5, Level 1:");
    let stages = ["e1", "b2", "c2", "amp", "clip", "lp", "hp", "tone", "out"];
    for hz in [50.0, 100.0, 300.0, 1_000.0, 3_000.0, 10_000.0] {
        print!("  {hz:>6.0} Hz:");
        for node in stages {
            print!(" {node} {:+.1}", gain_db(node, 0.5, 0.5, hz));
        }
        println!();
    }

    println!("\nthe booster alone (Q2's collector over Q2's base drive), by frequency:");
    for hz in [33.0, 100.0, 300.0, 1_000.0, 3_300.0, 10_000.0] {
        let booster = gain_db("c2", 0.5, 0.5, hz) - gain_db("e1", 0.5, 0.5, hz);
        println!("  {hz:>6.0} Hz  {booster:+.1} dB");
    }

    println!("\nthe gain stage (IC output over its input) against Dist, 1 kHz and 100 Hz:");
    for dist in [0.0, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let at = |hz| gain_db("amp", dist, 0.5, hz) - gain_db("plus", dist, 0.5, hz);
        println!(
            "  dist {dist:.2}: {:+.1} dB at 1 kHz, {:+.1} dB at 100 Hz, {:+.1} dB at 10 kHz",
            at(1_000.0),
            at(100.0),
            at(10_000.0)
        );
    }

    println!("\nthe tone section (Level's wiper over the clipper), Tone 0 / 0.5 / 1:");
    for hz in [100.0, 234.0, 400.0, 500.0, 700.0, 1_000.0, 2_000.0, 5_000.0] {
        print!("  {hz:>6.0} Hz:");
        for tone in [0.0, 0.5, 1.0] {
            let db = gain_db("tone", 0.0, tone, hz) - gain_db("clip", 0.0, tone, hz);
            print!(" {db:+6.1}");
        }
        println!();
    }
}
