//! What the Mark IIC+ Bass knob does, node by node, small and large signal.
//!
//! `cargo run --release --example mark_bass`
use gainstagefx::circuits::markiic::{self, BASS, LEAD_DRIVE, MIDDLE, TREBLE, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

fn at(node: &str, hz: f64, volts: f64, b: f64) -> f64 {
    let c = markiic::tap(10_000.0, 1_000_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(TREBLE, 0.75);
    sim.set_control(BASS, b);
    sim.set_control(MIDDLE, 0.35);
    sim.set_control(VOLUME, 0.8);
    sim.set_control(LEAD_DRIVE, 0.8);
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 2.0) as usize, |x| sim.process(x)).gain_db()
}

fn main() {
    for node in ["ts_out", "v1b_p", "v3b_p", "to_eq"] {
        for b in [0.0, 0.15, 0.3, 0.65, 1.0] {
            print!("{node:<8} bass {b:.2}:");
            for hz in [60.0, 82.0, 120.0, 250.0, 500.0, 1000.0, 3000.0] {
                print!("{:>8.1}", at(node, hz, 1e-5, b));
            }
            println!();
        }
    }
    println!("--- large signal (0.3 V) at to_eq, fundamental level");
    for b in [0.0, 0.3, 0.65, 1.0] {
        print!("to_eq    bass {b:.2}:");
        for hz in [60.0, 82.0, 120.0, 250.0, 500.0, 1000.0, 3000.0] {
            print!("{:>8.1}", at("to_eq", hz, 0.3, b));
        }
        println!();
    }
}
