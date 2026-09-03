//! What another valve buys you.

use gainstagefx::circuits::preamp::{self, Preamp};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn main() {
    println!(
        "{:<12}{:>8}{:>10}{:>9}{:>9}{:>9}",
        "", "stages", "gain", "THD", "2nd", "3rd"
    );
    for (name, p) in [
        ("clean", preamp::CLEAN),
        ("crunch", preamp::CRUNCH),
        ("high gain", preamp::HIGH_GAIN),
    ] {
        for volts in [0.02, 0.1, 0.3] {
            let m = run(&p, volts, 1.0);
            println!(
                "{:<12}{:>8}{:>9.0}x{:>8.1}%{:>8.1}%{:>8.1}%",
                format!("{name} {volts}V"),
                p.stages,
                10f64.powf(m.gain_db() / 20.0),
                m.thd_percent(),
                m.harmonic_percent(2),
                m.harmonic_percent(3),
            );
        }
    }
}

fn run(p: &Preamp, volts: f64, gain: f64) -> measure::Measured {
    let circuit = preamp::build(p, 10_000.0, 1_000_000.0).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(preamp::GAIN, gain);
    sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE * 0.4) as usize, |x| sim.process(x))
}
