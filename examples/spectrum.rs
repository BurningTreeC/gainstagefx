//! What the drive control does to the *shape* of the wave, not just its total.
//!
//! Total harmonic distortion is one number, and a circuit that is squared off
//! reports much the same number however much harder it is driven. That makes
//! it useless for the question it keeps being asked: is this control still
//! doing something? A wave can go on changing -- second against third, the
//! duty cycle of the clipped part, how far up the orders it reaches -- with
//! its total sitting still.
//!
//! So this prints the orders separately, across the control, at the level the
//! calibration says the circuit is fed. If every column is flat then the
//! control really has stopped working.
//!
//! `cargo run --release --example spectrum`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::CALIBRATION;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 96_000.0;

fn main() {
    for gain in [Gain::Screamer, Gain::Boogie, Gain::Peavey] {
        let index = voice::voice_index(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        let volts = CALIBRATION[index].drive_volts;
        println!("\n=== {} at {volts:.4} V ===", gain.name());
        println!(
            "  {:>6}{:>9}{:>8}{:>8}{:>8}{:>8}{:>8}{:>10}",
            "drive", "THD", "2nd", "3rd", "4th", "5th", "6th", "level"
        );
        let netlist = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        for step in 0..=8 {
            let p = step as f64 / 8.0;
            let mut sim = Simulation::new(netlist.clone(), RATE);
            sim.set_control(gain.drive_control(), p);
            let tone = Tone::near(RATE, 16_384, 220.0, volts);
            let m = measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x));
            print!("  {p:>6.3}{:>8.1} %", m.thd_percent());
            for order in 2..=6 {
                print!("{:>8.2}", m.harmonic_percent(order));
            }
            println!("{:>9.1} dB", m.gain_db());
        }
    }
}
