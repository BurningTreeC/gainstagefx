//! Moving a knob while audio is playing.
//!
//! A pot is a resistance. Turning it changes a conductance in the matrix and
//! nothing else: every capacitor keeps the charge it had, every core keeps its
//! flux, and the circuit walks to its new operating point through its own time
//! constants. That is what the hardware does.
//!
//! What the model used to do was throw all of it away. A control moving marks
//! the matrix dirty; the next sample rebuilt it, and the rebuild started every
//! reactance and every device from nothing and then hunted the operating point
//! *with no signal in the circuit* to fill them back in. The audio in flight
//! was deleted and replaced with a DC solution, once per block, for as long as
//! the knob was moving.
//!
//! Reported as "when we engage a knob while audio is playing, the audio gets
//! different, washed out, smoothed out, wrong".
//!
//! The test is a knob move so small it cannot change the circuit -- one part
//! in a million, alternating so it ends where it started -- applied at every
//! block boundary, against the same render with the knob left alone. Whatever
//! comes back is not the knob.

use gainstagefx::dsp::measure::Tone;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 48_000.0;
const LEVEL: f64 = 0.1;

/// Every circuit worth asking, including one of each kind that has a state a
/// rebuild could lose: reactances everywhere, an op-amp in the Screamer, a
/// saturating core in the 73P's input transformer.
const CIRCUITS: [Gain; 6] = [
    Gain::Crunch,
    Gain::Distortion,
    Gain::Screamer,
    Gain::Boogie,
    Gain::Peavey,
    Gain::Neve,
];

fn render(gain: Gain, block: usize, nudge: bool) -> Vec<f64> {
    let netlist =
        voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, RATE);
    let which = gain.drive_control();
    sim.set_control(which, 0.5);
    sim.find_operating_point();

    let n = RATE as usize / 4;
    let tone = Tone::near(RATE, 16_384, 220.0, LEVEL);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if nudge && i % block == 0 {
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

/// Eighty decibels down is the bar. Before the fix the Mark IIC+ measured
/// **plus** 4.5 dB -- an error louder than the signal -- and the quietest of
/// the six was 15 dB down.
#[test]
fn a_knob_that_barely_moves_barely_changes_the_sound() {
    for gain in CIRCUITS {
        let still = render(gain, 64, false);
        for block in [64usize, 512] {
            let moved = render(gain, block, true);
            let db = residual_db(&still, &moved);
            println!("{} at {block}: {db:.1} dB", gain.name());
            assert!(
                db < -80.0,
                "{} loses its state when its knob moves: {db:.1} dB of residual \
                 at a block size of {block}",
                gain.name()
            );
        }
    }
}

/// And it must not matter how often the host asks. A residual that grows as
/// the block gets shorter is a per-rebuild discontinuity, which is what this
/// was.
#[test]
fn moving_it_more_often_does_not_make_it_worse() {
    for gain in CIRCUITS {
        let at_64 = residual_db(&render(gain, 64, false), &render(gain, 64, true));
        let at_512 = residual_db(&render(gain, 64, false), &render(gain, 512, true));
        println!(
            "{}: {at_64:.1} dB at 64, {at_512:.1} dB at 512",
            gain.name()
        );
        assert!(
            at_64 - at_512 < 20.0,
            "{} gets {:.1} dB worse when the knob is moved eight times as \
             often, so something is being lost on every rebuild",
            gain.name(),
            at_64 - at_512
        );
    }
}
