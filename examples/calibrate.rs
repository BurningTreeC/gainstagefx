//! Measures the calibration table and prints it as Rust.
//!
//! Two numbers per voice.
//!
//! **The input level.** A circuit works in volts and a plugin is handed
//! numbers near one, and the join between them is a choice rather than a fact.
//! The choice made here is that each circuit does what its *name* says with a
//! nominal signal in front of it and the drive control **all the way up**: a
//! Clean voice barely bending, a Distortion voice thoroughly squared off.
//! Turning the knob down then only ever cleans up, which is what a player
//! expects of it and what the hardware does.
//!
//! Defining it at the middle of the knob instead does not work, and the reason
//! is worth keeping: the drive control now spans about eighty decibels, so its
//! middle sits some thirty decibels down, and the Clean voice could not reach
//! its stated figure there at any input level at all -- it peaked at 0.3 %
//! with four volts going in. The figures below are the intent, and they can be
//! argued with.
//!
//! **The make-up.** A curve across the drive control, which moves the gain of
//! these circuits by around eighty decibels end to end. Without it every
//! comparison between two settings is a comparison of loudness.
//!
//! Run with `cargo run --release --example calibrate > src/calibration.rs`.

use gainstagefx::circuits::clipper::GAIN;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain, NOMINAL_DBFS, POINTS, VOICES};

const RATE: f64 = 96_000.0;

/// What each circuit should be doing, in per cent total harmonic distortion,
/// with a nominal signal and the drive control all the way up.
fn intent(gain: Gain) -> f64 {
    match gain {
        // Barely working. This is the sound of a signal having been through
        // something, not the sound of distortion.
        Gain::Clean => 1.5,
        Gain::Crunch => 8.0,
        // Past this the character stops changing and only the level does.
        Gain::HighGain => 25.0,
        Gain::Overdrive => 12.0,
        Gain::Distortion => 30.0,
    }
}

fn measured(sim: &mut Simulation, volts: f64) -> measure::Measured {
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x))
}

fn main() {
    println!("// Measured by `examples/calibrate.rs`, which states the reasoning.");
    println!("// Do not edit by hand: run the example and paste its output over this");
    println!("// file. `tests/voice.rs` re-measures every entry and fails on drift.");
    println!();
    println!("pub const CALIBRATION: [Calibration; VOICES] = [");

    for index in 0..VOICES {
        let (gain, diode) = voice::voice_at(index);
        let netlist = voice::build_voice(gain, diode).expect("catalogue builds");
        let target = intent(gain);

        // Find the level that gives the intended distortion.
        //
        // Not by bisection from nothing to plenty, which assumes more level
        // always means more distortion. That is true of a clipper to ground,
        // whose ceiling squares the wave off harder the further past it you
        // go, and it is *false* of one in the loop: its minimum gain is one,
        // so a large enough input arrives at the output nearly untouched with
        // the clipped part added on top, and the clipped part is then a
        // shrinking fraction of what comes out. Measured, germanium diodes in
        // a loop peak near 14 % around a tenth of a volt and are back to 6 %
        // by four volts -- so a plain bisection walked off the top of its
        // range and calibrated the voice at four volts.
        //
        // So: sweep first, find the peak, and only then bisect on the rising
        // side of it, where the search is sound. Where a voice cannot reach
        // its intended figure at all, take the peak and say so.
        let thd_at = |volts: f64| {
            let mut sim = Simulation::new(netlist.clone(), RATE);
            sim.set_control(GAIN, 1.0);
            measured(&mut sim, volts).thd_percent()
        };
        const STEPS: usize = 60;
        let (lowest, highest) = (1e-4f64, 4.0f64);
        let level = |i: usize| lowest * (highest / lowest).powf(i as f64 / STEPS as f64);
        let mut peak = (0usize, 0.0f64);
        for i in 0..=STEPS {
            let thd = thd_at(level(i));
            if thd > peak.1 {
                peak = (i, thd);
            }
        }
        let reachable = peak.1 >= target;
        let (mut lo, mut hi) = (lowest, level(peak.0));
        if reachable {
            for _ in 0..30 {
                let mid = (lo * hi).sqrt();
                if thd_at(mid) < target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
        } else {
            lo = level(peak.0);
        }
        let drive_volts = (lo * hi).sqrt();

        // With the level fixed, what does the circuit do to it across the
        // control?
        let mut make_up = [0.0f64; POINTS];
        for (i, slot) in make_up.iter_mut().enumerate() {
            let mut sim = Simulation::new(netlist.clone(), RATE);
            sim.set_control(GAIN, i as f64 / (POINTS - 1) as f64);
            *slot = -measured(&mut sim, drive_volts).gain_db();
        }

        let mut check = Simulation::new(netlist.clone(), RATE);
        check.set_control(GAIN, 1.0);
        let got = measured(&mut check, drive_volts);
        println!(
            "    // {} with {} diodes: {:.4} V in, {:.1} % distortion, {:.1} % third.{}",
            gain.name(),
            diode.name().to_lowercase(),
            drive_volts,
            got.thd_percent(),
            got.harmonic_percent(3),
            if reachable {
                ""
            } else {
                " This voice cannot reach\n    // its intended figure at any level, so this is its peak."
            },
        );
        println!("    Calibration {{");
        println!("        drive_volts: {drive_volts:.6},");
        print!("        make_up_db: [");
        for (i, m) in make_up.iter().enumerate() {
            print!("{}{m:.2}", if i > 0 { ", " } else { "" });
        }
        println!("],");
        println!("    }},");
    }
    println!("];");
    eprintln!("nominal is {NOMINAL_DBFS} dBFS");
}
