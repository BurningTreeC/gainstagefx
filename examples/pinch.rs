//! What the work budget costs when it bites.
//!
//! Layer 5a lowers the Newton ceiling for the tail of a block that is running
//! out of time. That is only defensible if the samples it pinches are
//! *slightly* wrong rather than garbage -- BUG-008 recorded a permanent
//! ceiling of eight turning the 5150's lead channel into 190 per cent
//! distortion, which is not a sound but a solve that never converged.
//!
//! `cargo run --release --example pinch`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Amplifier, Diode, Gain};

const RATE: f64 = 96_000.0;

fn at(gain: Gain, ceiling: usize, volts: f64) -> (f64, f64, u64) {
    let netlist = voice::build_voice(gain, Diode::Silicon, Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, RATE);
    sim.set_control(gain.drive_control(), 1.0);
    sim.find_operating_point();
    sim.set_pass_ceiling(ceiling);
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    let m = measure::run(tone, (RATE / 2.0) as usize, |x| sim.process(x));
    (m.gain_db(), m.thd_percent(), sim.pinched())
}

fn main() {
    println!("Drive at the stop, 220 Hz. The floor is 12 passes.\n");
    println!(
        "  {:<12}{:>9}{:>10}{:>10}{:>10}{:>12}",
        "voice", "volts", "32 THD", "12 THD", "shift", "pinched"
    );
    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Muff,
        Gain::Screamer,
        Gain::Neve,
    ] {
        for volts in [0.122, 0.5, 2.0] {
            let (_, full, _) = at(gain, 32, volts);
            let (_, floored, pinched) = at(gain, 12, volts);
            println!(
                "  {:<12}{volts:>9.3}{full:>9.1} %{floored:>9.1} %{:>9.1} %{pinched:>12}",
                gain.name(),
                floored - full
            );
        }
    }
    println!("\n'pinched' is samples that finished at the ceiling rather than");
    println!("by converging. Zero means the floor never bit at all.");
}
