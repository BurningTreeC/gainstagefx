//! The two clipping families, across level.

use gainstagefx::circuits::clipper::{self, Values};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::DiodeSpec;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn main() {
    println!("{:<28}{:>10}{:>9}{:>9}{:>9}", "", "out", "THD", "2nd", "3rd");
    for (name, mut v) in [
        ("overdrive, silicon", clipper::OVERDRIVE),
        ("overdrive, germanium", clipper::OVERDRIVE),
        ("overdrive, LED", clipper::OVERDRIVE),
        ("distortion, silicon", clipper::DISTORTION),
    ] {
        if name.contains("germanium") {
            v.diode = DiodeSpec::GERMANIUM;
        }
        if name.contains("LED") {
            v.diode = DiodeSpec::LED;
        }
        for volts in [0.02, 0.1, 0.4] {
            let m = run(&v, volts, 0.5);
            println!(
                "{:<28}{:>9.3}V{:>8.1}%{:>8.1}%{:>8.1}%",
                format!("{name} {volts}V"),
                m.fundamental().magnitude(),
                m.thd_percent(),
                m.harmonic_percent(2),
                m.harmonic_percent(3),
            );
        }
    }
}

fn run(v: &Values, volts: f64, gain: f64) -> measure::Measured {
    let circuit = clipper::build(v, 10_000.0, 100_000.0).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(clipper::GAIN, gain);
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x))
}
