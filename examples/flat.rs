//! Which stage stops responding to the gain control.
//!
//! A cascade whose output does not change when its gain control moves has one
//! stage that stopped passing the change on, and every stage after it is
//! innocent. This walks the 5150's plates in order and prints what each one
//! does across ULTRA PRE, at the level the calibration feeds it.
//!
//! `cargo run --release --example flat`

use gainstagefx::circuits::evh5150;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const VOLTS: f64 = 0.122;

fn main() {
    println!("5150 at {VOLTS} V, 220 Hz. Output level in dB, then THD.\n");
    let steps = [0.0, 0.125, 0.25, 0.5, 0.75, 1.0];
    print!("  {:<9}", "node");
    for p in steps {
        print!("{:>16}", format!("pre {p:.3}"));
    }
    println!();
    for node in [
        "v1a_p", "v1b_p", "v2a_p", "v2b_p", "v5b_p", "v5a_p", "out", "stack",
    ] {
        print!("  {node:<9}");
        for p in steps {
            match evh5150::tap(10_000.0, evh5150::TONE_STACK_INPUT, node) {
                Ok(c) => {
                    let mut sim = Simulation::new(c, RATE);
                    sim.set_control(evh5150::PRE, p);
                    let tone = Tone::near(RATE, 16_384, 220.0, VOLTS);
                    let m = measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x));
                    print!("{:>10.1} {:>5.1}%", m.gain_db(), m.thd_percent());
                }
                Err(e) => print!("{:>16}", format!("{e:?}")),
            }
        }
        println!();
    }
}
