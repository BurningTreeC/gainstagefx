//! What a candidate calibration level does to the drive control.
//!
//! `examples/calibrate.rs` picks the input level from a distortion figure, and
//! that figure has a consequence nothing else measures: past the point where a
//! circuit is squared off at every drive setting, the make-up curve goes flat.
//! This prints both, so the figure can be chosen on evidence rather than on
//! which number is largest.
//!
//! A flat make-up curve is not by itself a verdict, and reading it as one was
//! an error worth naming. The make-up is the *level* the circuit delivers, and
//! an amplifier that is saturated does not get louder when its gain control is
//! opened further -- it gets dirtier. So the distortion across the control is
//! printed alongside it. The control has stopped being a control only when
//! **both** are flat.
//!
//! `cargo run --release --example drivecheck`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain, POINTS};

const RATE: f64 = 96_000.0;

fn measured(gain: Gain, volts: f64, drive: f64) -> measure::Measured {
    let netlist =
        voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, RATE);
    sim.set_control(gain.drive_control(), drive);
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x))
}

fn main() {
    for gain in [Gain::Boogie, Gain::Peavey] {
        println!("\n=== {} ===", gain.name());
        println!(
            "  {:>9}  {:>7}   {:>9}   THD across the control  /  make-up across it",
            "level", "THD 1.0", "make-up"
        );
        let mut volts = 1e-4f64;
        while volts < 8.0 {
            let thd = measured(gain, volts, 1.0).thd_percent();
            let mut make_up = [0.0f64; POINTS];
            let mut dirt = [0.0f64; POINTS];
            for i in 0..POINTS {
                let m = measured(gain, volts, gainstagefx::voice::knot_position(i));
                make_up[i] = -m.gain_db();
                dirt[i] = m.thd_percent();
            }
            let lo = make_up.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = make_up.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            println!(
                "  {volts:8.5} V  {thd:6.1} %   {:6.1} dB   [{}]  [{}]",
                hi - lo,
                dirt.iter()
                    .map(|d| format!("{d:.0}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                make_up
                    .iter()
                    .map(|m| format!("{m:.0}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            volts *= 2.0;
        }
    }
}
