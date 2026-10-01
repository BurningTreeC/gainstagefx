//! Regenerates `src/direct_out.rs`: for every circuit with its own direct out
//! ahead of its last stage (`Gain::direct_out`), the small-signal gain from the
//! circuit's input to that node at each of the make-up's drive positions.
//!
//! The Preamp DI is levelled by the make-up, which is measured through the
//! whole circuit; a node ahead of a stage that clips at the calibration level
//! -- the 800RB's boost -- is not, so it is levelled by its own gain instead,
//! as the pedal tap is levelled by what the pedal is fed. 1 kHz, 1 mV, every
//! control but the drive at its rest. Floored where the volume is all the way
//! down, so the DI is not lifted into nothing.
//!
//! `cargo run --release --example directout > src/direct_out.rs`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{build_voice, knot_position, Amplifier, Diode, Gain, POINTS};

const RATE: f64 = 96_000.0;
const HZ: f64 = 1_000.0;
const VOLTS: f64 = 1e-3;
/// The least gain a table holds: below it the drive is off its stop and the
/// direct out carries nothing worth levelling.
const FLOOR_DB: f64 = -60.0;

fn gain_db(gain: Gain, node: &str, drive: f64) -> f64 {
    let circuit = build_voice(gain, Diode::Silicon, Amplifier::Valve).expect("catalogue builds");
    let at = circuit
        .unknown_named(node)
        .expect("a direct out names a node");
    let mut s = Simulation::new(circuit, RATE);
    s.set_control(gain.drive_control(), drive);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, HZ, VOLTS);
    let m = measure::run(t, (RATE * 0.3) as usize, |x| {
        s.process(x);
        s.voltage_at(at)
    });
    (20.0 * (m.fundamental().magnitude() / VOLTS).log10()).max(FLOOR_DB)
}

fn main() {
    println!("// Measured by `examples/directout.rs`. Do not edit by hand.");
    println!("/// The small-signal gain, dB, from each circuit's input to its own direct out");
    println!("/// (`Gain::direct_out`), at 1 kHz with every other control at its rest, at the");
    println!("/// drive positions `knot_position` gives. What levels the Preamp DI of a");
    println!("/// circuit whose direct out is ahead of its last stage; see `Chain` and");
    println!("/// `docs/DI.md`.");
    println!("pub const DIRECT_OUT_DB: &[(Gain, [f64; POINTS])] = &[");
    for gain in Gain::ALL {
        let Some(node) = gain.direct_out() else {
            continue;
        };
        let row: Vec<String> = (0..POINTS)
            .map(|i| format!("{:.2}", gain_db(gain, node, knot_position(i))))
            .collect();
        println!("    (");
        println!("        Gain::{gain:?},");
        println!("        [{}],", row.join(", "));
        println!("    ),");
    }
    println!("];");
}
