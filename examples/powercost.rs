//! What the power stage costs, and which half of the cost it is.
//!
//! A circuit is expensive for one of two reasons and they want opposite
//! fixes: the matrix is large, which the factorisation pays for as the cube of
//! its size, or the solve is struggling, which is paid for one Newton pass at
//! a time. Nothing about the wall clock says which.
//!
//! `cargo run --release --example powercost`

use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 48_000.0;

fn cost(name: &str, mut sim: Simulation, volts: f64) {
    sim.find_operating_point();
    let n = RATE as usize * 2;
    let start = std::time::Instant::now();
    for k in 0..n {
        let x = volts * (k as f64 * 220.0 * std::f64::consts::TAU / RATE).sin();
        sim.process(x);
    }
    let load = start.elapsed().as_secs_f64() / 2.0 * 100.0;
    let (solves, passes, _, _) = sim.statistics();
    println!(
        "  {name:<22}{:>8} nodes{load:>9.1}%{:>10.2} passes",
        sim.unknowns(),
        passes as f64 / solves.max(1) as f64,
    );
}

fn main() {
    println!("Two seconds of audio at 48 kHz, as a percentage of realtime.\n");
    for (gain, spec) in [
        (Gain::Boogie, &PowerSpec::MARKIIC),
        (Gain::Peavey, &PowerSpec::EVH5150),
    ] {
        println!("{}:", gain.name());
        let pre = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        cost("preamp", Simulation::new(pre, RATE), 0.122);
        let amp = power::build(spec, 10_000.0).expect("builds");
        // Driven where the preamplifier actually leaves it.
        cost("power stage", Simulation::new(amp, RATE), 6.0);
        println!();
    }
}
