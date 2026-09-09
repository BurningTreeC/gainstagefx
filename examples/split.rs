//! Which of a voice's three simulations the time actually goes into.
//!
//! `Chain::process` runs up to three nonlinear solves a sample: the gain
//! circuit, the power amplifier, and the transformer core. `passes_per_sample`
//! reports only the first of them, so a voice with a power stage was being
//! judged on a third of its solver work.
//!
//! **The power stage is fed the preamplifier's output, not a tone.** It used
//! not to be, and the difference is not small: handed a clean sine the 5150's
//! power amplifier converged in 2.78 passes and cost 33 % of realtime, and
//! handed what the lead channel actually sends it -- a slammed square with
//! edges -- it costs what the whole callback says it does. A stage measured
//! on a signal it never sees is not measured.
//!
//! `cargo run --release --example split`

use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain, Iron};

const RATE: f64 = 48_000.0;

fn stimulus(k: usize) -> f64 {
    let t = k as f64 / RATE;
    let env = (-((t % 0.5) * 6.0)).exp();
    0.9 * env
        * ((std::f64::consts::TAU * 110.0 * t).sin()
            + 0.6 * (std::f64::consts::TAU * 330.0 * t).sin()
            + 0.3 * (std::f64::consts::TAU * 1757.0 * t).sin())
}

/// Runs one second through a simulation and reports realtime percentage,
/// unknowns and mean passes, driven by whatever `drive` hands it.
///
/// Returns what came out, so the next stage can be measured on it.
fn run(label: &str, sim: &mut Simulation, drive: &mut dyn FnMut(usize) -> f64) -> Vec<f64> {
    for k in 0..2000 {
        std::hint::black_box(sim.process(drive(k)));
    }
    let (s0, p0, u0, _) = sim.statistics();
    let (b0, f0, nf0) = sim.health();
    let n = RATE as usize;
    let mut out = Vec::with_capacity(n);
    let start = std::time::Instant::now();
    for k in 2000..(2000 + n) {
        out.push(sim.process(drive(k)));
    }
    let realtime = start.elapsed().as_secs_f64() * 100.0;
    let (s1, p1, u1, _) = sim.statistics();
    let (b1, f1, nf1) = sim.health();
    println!(
        "    {:<10}{:>7.1} %{:>5} unk{:>7.2} passes{:>7} unsettled{:>8} backtr{:>7} fallb{:>5} nan",
        label,
        realtime,
        sim.unknowns(),
        (p1 - p0) as f64 / (s1 - s0).max(1) as f64,
        u1 - u0,
        b1 - b0,
        f1 - f0,
        nf1 - nf0
    );
    out
}

fn main() {
    println!("One second, one channel, 48 kHz. Percentages are of realtime.\n");
    for gain in [Gain::Peavey, Gain::Boogie, Gain::Neve, Gain::Muff] {
        println!("  {}", gain.name());
        let netlist = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        let mut sim = Simulation::new(netlist, RATE);
        sim.set_control(0, 0.8);
        let preamp = run("gain", &mut sim, &mut |k| 12.0 * stimulus(k));

        match voice::build_power(gain) {
            Some(Ok(netlist)) => {
                let mut sim = Simulation::new(netlist, RATE);
                // What the preamplifier actually sends it. The warm-up runs
                // off the front of the capture, so it repeats the first
                // sample rather than reading out of bounds.
                run("power", &mut sim, &mut |k| {
                    preamp[k.saturating_sub(2000).min(preamp.len() - 1)]
                });
            }
            _ => println!("    {:<16}   none", "power"),
        }
        println!();
    }
    let mut sim = Simulation::new(voice::build_iron(Iron::Steel).expect("builds"), RATE);
    println!("  transformer (shared)");
    run("iron", &mut sim, &mut |k| 12.0 * stimulus(k));
}
