//! The Brit Plexi measured stage by stage: the preamplifier's gain and
//! distortion across the Volume, then the power stage behind it.
//!
//! `cargo run --release --example plexi_stages`
use gainstagefx::circuits::{plexi, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

fn run(sim: &mut Simulation, volts: f64) -> measure::Measured {
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 2.0) as usize, |x| sim.process(x))
}

fn main() {
    println!("preamp at 0.122 V, treble/bass/middle at 0.5:");
    for node in ["v1_p", "v2_g", "v2_p", "out"] {
        for vol in [0.25, 0.5, 0.75, 1.0] {
            let c = plexi::tap(10_000.0, 1_000_000.0, node).unwrap();
            let mut sim = Simulation::new(c, RATE);
            sim.set_control(plexi::VOLUME, vol);
            sim.find_operating_point();
            let m = run(&mut sim, 0.122);
            println!(
                "  {node:<5} vol {vol:.2}: {:6.1} dB  THD {:5.1} %  2nd {:5.1} %  3rd {:5.1} %  peak out {:.2} V",
                m.gain_db(),
                m.thd_percent(),
                m.harmonic_percent(2),
                m.harmonic_percent(3),
                0.122 * 10f64.powf(m.gain_db() / 20.0)
            );
        }
    }
    let brit_open = power::PowerSpec { master_rest: 1.0, ..power::PowerSpec::BRIT_EL34 };
    for (label, spec) in [("Plexi EL34", power::PowerSpec::PLEXI_EL34), ("2203 EL34, master open", brit_open)] {
    println!("{label}, alone (resistive load), 220 Hz:");
    for volts in [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 40.0] {
        let c = power::build(&spec, 10_000.0).unwrap();
        let mut sim = Simulation::new(c, RATE);
        sim.find_operating_point();
        let m = run(&mut sim, volts);
        let out = volts * 10f64.powf(m.gain_db() / 20.0);
        println!(
            "  {volts:5.1} V in: {:5.1} V peak at 4 ohm ({:5.1} W)  THD {:5.1} %  2nd {:5.1} %  3rd {:5.1} %",
            out,
            out * out / 8.0,
            m.thd_percent(),
            m.harmonic_percent(2),
            m.harmonic_percent(3)
        );
    }
    }
}
