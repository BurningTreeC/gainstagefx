//! What the 73P card does, measured against the drawings.
//!
//! Three questions, in the order they have to be answered:
//!
//! 1. Does each amplifier block have the gain its feedback resistors say? The
//!    drawings' own text states 30 dB for PRE 1 and 18/23/28 dB for PRE 2, and
//!    those are arithmetic on R12, R13, R5, R4, R71, R61 and R62. A model that
//!    misses them has the topology wrong, not the calibration.
//! 2. Where does the whole card sit, and does the Drive control move evenly?
//! 3. Does it stay put when the control is moved after the circuit has been
//!    driven hard -- which is what a player does and what a fresh simulation
//!    per setting would never catch.
//!
//! `cargo run --release --example neve`

use gainstagefx::circuits::neve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// Small enough that every stage is well inside its linear region, so what
/// comes back is the feedback arithmetic and nothing else.
const SMALL: f64 = 1e-5;

fn gain_at(node: &str, drive: f64, volts: f64, hz: f64, iron: bool) -> (f64, f64) {
    let circuit = neve::tap(150.0, 10_000.0, node, iron).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(neve::GAIN, drive);
    sim.set_control(neve::TRIM, 1.0);
    let settled = sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    let m = measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x));
    if !settled {
        eprintln!("warning: {node} at drive {drive} never settled");
    }
    (m.gain_db(), m.thd_percent())
}

fn main() {
    println!("== block gains, no input transformer, 1 kHz at {SMALL} V ==");
    println!("   the figures in brackets are what the drawings' text states\n");

    let (pre1, _) = gain_at("pre1_out", 0.0, SMALL, 1_000.0, false);
    println!("  PRE 1                      {pre1:6.2} dB   (30)");

    for (drive, stated) in [(0.0, 18.0), (0.5, 23.0), (1.0, 28.0)] {
        let (whole, _) = gain_at("pre2_out", drive, SMALL, 1_000.0, false);
        println!(
            "  PRE 1 + PRE 2, drive {drive:.1}     {whole:6.2} dB   ({:.0})   PRE 2 alone {:6.2} dB",
            30.0 + stated,
            whole - pre1
        );
    }

    println!("\n== the whole card, with its input transformer and trim wide open ==");
    for drive in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let (g, thd) = gain_at("out", drive, SMALL, 1_000.0, true);
        println!("  drive {drive:.2}   {g:6.2} dB   {thd:5.2} % distortion");
    }

    println!("\n== level, at drive 1.0 ==");
    for volts in [1e-5, 1e-4, 1e-3, 3e-3, 1e-2, 3e-2] {
        let (g, thd) = gain_at("out", 1.0, volts, 1_000.0, true);
        println!("  {volts:8.5} V in   {g:6.2} dB   {thd:5.2} % distortion");
    }

    println!("\n== response, drive 1.0, trim wide open ==");
    for hz in [20.0, 50.0, 100.0, 1_000.0, 5_000.0, 10_000.0, 20_000.0] {
        let (g, _) = gain_at("out", 1.0, SMALL, hz, true);
        println!("  {hz:8.0} Hz   {g:6.2} dB");
    }

    println!("\n== the control moved after the circuit has been hit hard ==");
    println!("   one simulation throughout, as a plugin has\n");
    let circuit = neve::build(150.0, 10_000.0).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(neve::TRIM, 1.0);
    for drive in [0.0, 1.0, 0.5, 0.0, 1.0] {
        sim.set_control(neve::GAIN, drive);
        // Hammer it, then measure quietly. A fresh simulation for each setting
        // would never see what the last setting left behind.
        let loud = Tone::near(RATE, 16_384, 220.0, 1.0);
        measure::run(loud, (RATE / 20.0) as usize, |x| sim.process(x));
        // Seconds, not milliseconds. C9 and C32 are 22 uF into 120 k, which is
        // a two and a half second corner, and a card knocked off its operating
        // point by a full-scale volt has to charge back through them.
        let quiet = Tone::near(RATE, 16_384, 1_000.0, SMALL);
        for wait in [0.1f64, 0.5, 2.0, 10.0] {
            let m = measure::run(quiet, (RATE * wait) as usize, |x| sim.process(x));
            println!(
                "  drive {drive:.2}, {wait:5.1} s after a full-scale volt   {:6.2} dB   {:6.2} %",
                m.gain_db(),
                m.thd_percent()
            );
        }
    }
}
