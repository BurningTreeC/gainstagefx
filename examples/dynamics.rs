//! What the gain control does to an amplifier's *threshold*, not its ceiling.
//!
//! A cascade that is squared off delivers the same ceiling whatever it is fed,
//! so at one input level its gain control looks dead. That is a property of
//! the measurement, not necessarily of the control: what a gain control moves
//! is the level at which the ceiling is *reached*, and a player hears that as
//! how far into a note the distortion holds on and how hard they have to hit
//! it to get there.
//!
//! So: output level against input level, at three positions of the control. If
//! the knee moves, the control works and a sine at one level cannot see it.
//!
//! `cargo run --release --example dynamics`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 96_000.0;

fn main() {
    for gain in [Gain::Peavey, Gain::Boogie] {
        let netlist = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        println!(
            "\n=== {} : output dB / THD, against how hard it is played ===",
            gain.name()
        );
        print!("  {:<12}", "input");
        for p in [0.125f64, 0.25, 0.5, 1.0] {
            print!("{:>18}", format!("drive {p:.3}"));
        }
        println!();
        // A guitar from a whisper to a hard strum: -40 dB to +12 dB about the
        // level the calibration feeds it.
        for db in [-40.0f64, -30.0, -24.0, -18.0, -12.0, -6.0, 0.0, 6.0, 12.0] {
            let volts = 0.122 * 10f64.powf(db / 20.0);
            print!("  {:<12}", format!("{db:+.0} dB"));
            for p in [0.125f64, 0.25, 0.5, 1.0] {
                let mut sim = Simulation::new(netlist.clone(), RATE);
                sim.set_control(gain.drive_control(), p);
                let tone = Tone::near(RATE, 16_384, 220.0, volts);
                let m = measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x));
                let out_db = 20.0 * (volts * 10f64.powf(m.gain_db() / 20.0)).max(1e-12).log10();
                print!("{:>11.1} {:>5.1}%", out_db, m.thd_percent());
            }
            println!();
        }
    }
}
