//! What moving a knob does to the audio going through it.
//!
//! A pot is a resistance. Turning it changes a conductance in the matrix and
//! changes nothing else: every capacitor in the circuit keeps the charge it
//! had, and the circuit walks to its new operating point through its own time
//! constants. That is what the hardware does and it is what a model has to do.
//!
//! The experiment is a knob move so small it cannot possibly be audible --
//! one part in a million, repeated at every block boundary -- against the same
//! render with the knob left alone. Anything that comes back is not the knob.
//!
//! `cargo run --release --example knobmove`

use gainstagefx::dsp::measure::Tone;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 48_000.0;

/// A tenth of a volt at 220 Hz: a signal, not a whisper.
const LEVEL: f64 = 0.1;

fn render(gain: Gain, block: usize, nudge: bool) -> Vec<f64> {
    let netlist =
        voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, RATE);
    let which = gain.drive_control();
    sim.set_control(which, 0.5);
    sim.find_operating_point();

    let n = RATE as usize / 2;
    let tone = Tone::near(RATE, 16_384, 220.0, LEVEL);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if nudge && i % block == 0 {
            // One part in a million, alternating, so the control ends where it
            // started and cannot have moved the sound by moving the circuit.
            let wobble = if (i / block).is_multiple_of(2) {
                1e-6
            } else {
                -1e-6
            };
            sim.set_control(which, 0.5 + wobble);
        }
        out.push(sim.process(tone.at(i)));
    }
    out
}

fn residual_db(a: &[f64], b: &[f64]) -> f64 {
    let mut energy = 0.0;
    let mut reference = 0.0;
    for (x, y) in a.iter().zip(b) {
        energy += (x - y) * (x - y);
        reference += x * x;
    }
    let rms = (energy / a.len() as f64).sqrt();
    let level = (reference / a.len() as f64).sqrt().max(1e-30);
    20.0 * (rms / level).max(1e-30).log10()
}

fn main() {
    println!("A one-part-in-a-million knob wobble at every block boundary,");
    println!("against the same render with the knob left alone.\n");
    println!(
        "  {:<12}{:>8}{:>14}{:>16}",
        "circuit", "block", "residual", "worst sample"
    );
    for gain in [
        Gain::Crunch,
        Gain::Screamer,
        Gain::Boogie,
        Gain::Peavey,
        Gain::Neve,
    ] {
        let still = render(gain, 64, false);
        for block in [64usize, 256, 512] {
            let moved = render(gain, block, true);
            let worst = still
                .iter()
                .zip(&moved)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f64, f64::max);
            println!(
                "  {:<12}{block:>8}{:>11.1} dB{worst:>16.6}",
                gain.name(),
                residual_db(&still, &moved)
            );
        }
    }
}
