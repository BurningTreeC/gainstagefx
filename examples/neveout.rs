//! The 73P's OUTPUT block against the figures its guide states.
//!
//! Eighteen decibels, of which four are the transformer. Nothing else about
//! this block is published, so those two numbers are the whole test.
//!
//! `cargo run --release --example neveout`

use gainstagefx::circuits::neve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn at(node: &str, volts: f64, hz: f64) -> measure::Measured {
    let c = neve::output_tap(600.0, 10_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    // Two seconds. The emitter and feedback capacitors are a hundred
    // microfarads against a kilohm and a half, which is a seventh of a second
    // each, and half a second of settle left enough drift in the window to
    // read as eight decibels of bass lift and four per cent of distortion at a
    // millivolt. Neither was in the circuit.
    measure::run(tone, (RATE * 2.0) as usize, |x| sim.process(x))
}

fn main() {
    let c = neve::output(600.0, 10_000.0).expect("builds");
    let names = c.names.clone();
    let mut sim = Simulation::new(c, RATE);
    println!(
        "operating point {}\n",
        if sim.find_operating_point() {
            "settled"
        } else {
            "DID NOT SETTLE"
        }
    );
    let v = sim.operating_point();
    for (i, name) in names.iter().enumerate() {
        println!("  {name:<8}{:>10.3} V", v[i]);
    }

    println!("\nSmall-signal gain at 1 kHz, 1 mV in:");
    for node in ["c6", "e7", "out", "spk"] {
        println!("  {node:<8}{:>9.2} dB", at(node, 1e-3, 1_000.0).gain_db());
    }

    println!("\nThe guide: 18 dB for the block, 4 dB of it the transformer.");
    let electronic = at("out", 1e-3, 1_000.0).gain_db();
    let whole = at("spk", 1e-3, 1_000.0).gain_db();
    println!(
        "  electronic {electronic:.2} dB, transformer {:.2} dB, block {whole:.2} dB",
        whole - electronic
    );

    println!("\nAcross the band, at the speaker:");
    for hz in [20.0, 50.0, 200.0, 1_000.0, 5_000.0, 20_000.0] {
        println!("  {hz:>8.0} Hz{:>9.2} dB", at("spk", 1e-3, hz).gain_db());
    }

    println!("\nAgainst level, 1 kHz:");
    for volts in [0.001f64, 0.01, 0.05, 0.1, 0.3, 1.0] {
        let m = at("spk", volts, 1_000.0);
        println!(
            "  {volts:>8.3} V{:>9.2} dB{:>9.2} % THD",
            m.gain_db(),
            m.thd_percent()
        );
    }
}
