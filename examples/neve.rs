//! The Neve card, measured against what its feedback says it must do.
use gainstagefx::circuits::neve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

fn run(leg: f64, node: &str, iron: bool, hz: f64, volts: f64) -> measure::Measured {
    let c = neve::tap(150.0, 10_000.0, leg, node, iron).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

fn main() {
    println!("=== the card alone: gain against the leg the switch selects ===");
    println!("{:<10}{:>12}{:>12}{:>10}", "leg", "predicted", "measured", "THD");
    for leg in neve::LEGS {
        // Series feedback: 1 + Rf/Re, with Rf the 2.2k from the follower's
        // emitter back to the input stage's and Re what the switch selects
        // in series with the 1.8k that is always there.
        // The leg and the 1.8k are both to ground, so they are in parallel,
        // and the feedback is the 51k from the output.
        let re = 1.0 / (1.0 / leg + 1.0 / 1_800.0);
        let predicted = 20.0 * (1.0 + 51_000.0 / re).log10();
        let m = run(leg, "out", false, 1000.0, 1e-4);
        println!("{:<10}{:>11.1} dB{:>11.1} dB{:>9.2} %",
                 if leg > 1e8 { String::from("open") } else { format!("{leg:.0}") },
                 predicted, m.gain_db(), m.thd_percent());
    }

    println!("\n=== with the input transformer in front ===");
    for leg in [68.0, 1_000.0, 12_000.0] {
        let m = run(leg, "out", true, 1000.0, 1e-4);
        println!("  leg {leg:>6.0}: {:>6.1} dB, {:.2} % distortion", m.gain_db(), m.thd_percent());
    }

    println!("\n=== driven hard, at the highest gain ===");
    for v in [1e-4, 1e-3, 5e-3, 2e-2] {
        let m = run(68.0, "out", true, 1000.0, v);
        println!("  {v:>8} V: {:>6.1} dB, {:>5.2} % distortion, 2nd {:>4.2} 3rd {:>4.2}",
                 m.gain_db(), m.thd_percent(), m.harmonic_percent(2), m.harmonic_percent(3));
    }
}
