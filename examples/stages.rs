//! Where the gain is, stage by stage, in the two amplifier models.
//!
//! A cascade that under-distorts can be under-distorting for two different
//! reasons, and they need opposite fixes. Either it is being handed too little
//! signal -- a calibration matter -- or one of its stages is not amplifying
//! what its own components say it should, which is a circuit matter.
//!
//! This tells them apart by bringing each plate out in turn and measuring the
//! small-signal gain from the input to it, at a level low enough that nothing
//! is bending. What comes back is the model's stage-by-stage gain structure,
//! which can then be read against the schematic's arithmetic.
//!
//! `cargo run --release --example stages`

use gainstagefx::circuits::{evh5150, markiic};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
/// Ten microvolts. Every stage is well inside its linear region here, so what
/// comes back is the gain the components give and not the clipping.
const SMALL: f64 = 1e-5;

fn gain_at(circuit: Circuit, control: usize, position: f64, hz: f64) -> f64 {
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(control, position);
    let tone = Tone::near(RATE, 16_384, hz, SMALL);
    measure::run(tone, (RATE / 8.0) as usize, |x| sim.process(x)).gain_db()
}

fn main() {
    println!("Small-signal gain from the input to each plate, 1 kHz at {SMALL} V.\n");

    println!("=== 5150 lead channel, ULTRA PRE at three positions ===");
    println!(
        "  {:<10}{:>12}{:>12}{:>12}",
        "node", "pre 0.0", "pre 0.5", "pre 1.0"
    );
    for node in ["v1a_p", "v1b_p", "v2a_p", "v2b_p", "v5a_p", "v5b_p", "out"] {
        print!("  {node:<10}");
        for p in [0.0, 0.5, 1.0] {
            match evh5150::tap(10_000.0, evh5150::TONE_STACK_INPUT, node) {
                Ok(c) => print!("{:>12.1}", gain_at(c, evh5150::PRE, p, 1_000.0)),
                Err(e) => print!("{:>12}", format!("{e:?}")),
            }
        }
        println!();
    }

    println!("\n=== Mark IIC+ lead channel, Lead Drive at three positions ===");
    println!(
        "  {:<10}{:>12}{:>12}{:>12}",
        "node", "drive 0.0", "drive 0.5", "drive 1.0"
    );
    for node in [
        "v1a_p", "ts_out", "v1b_p", "lead_in", "v3b_p", "v4a_p", "lead_out", "out",
    ] {
        print!("  {node:<10}");
        for p in [0.0, 0.5, 1.0] {
            match markiic::tap(10_000.0, 1_000_000.0, node) {
                Ok(c) => print!("{:>12.1}", gain_at(c, markiic::LEAD_DRIVE, p, 1_000.0)),
                Err(e) => print!("{:>12}", format!("{e:?}")),
            }
        }
        println!();
    }

    println!("\nA triode stage of this kind gives about 30 to 35 dB unbypassed");
    println!("and rather more with its cathode bypassed. A stage that shows a");
    println!("few decibels is being loaded by what follows it, or is not");
    println!("amplifying; a step that shows none at all is not a stage.");
}
