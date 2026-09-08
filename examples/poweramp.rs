//! Does the power stage bias where a power stage should, and what does it do?
//!
//! The first questions to ask of a new circuit, in order: does it find an
//! operating point at all, does that operating point put the right current
//! through the tubes, does it amplify, and does it clip where its supply says
//! it must rather than wherever the solver gave up.
//!
//! `cargo run --release --example poweramp`

use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// Every node's operating point, by name, straight after the DC solve.
///
/// Printing one node at a time through the output tap was a false economy: it
/// says nothing about whether the *solve* worked, and a stage that has not
/// converged reports a number with just as straight a face as one that has.
fn operating_point(spec: &PowerSpec) {
    let circuit = power::build(spec, 1_000.0).expect("builds");
    let names = circuit.names.clone();
    let mut sim = Simulation::new(circuit, RATE);
    let settled = sim.find_operating_point();
    println!(
        "  the operating point {}",
        if settled { "settled" } else { "DID NOT SETTLE" }
    );
    let v = sim.operating_point();
    for (i, name) in names.iter().enumerate() {
        println!("  {name:<8}{:>14.2} V", v[i]);
    }
}

fn gain_at(spec: &PowerSpec, at: &str, volts: f64, hz: f64) -> measure::Measured {
    let circuit = power::tap(spec, 1_000.0, at).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 8.0) as usize, |x| sim.process(x))
}

fn main() {
    for spec in [&PowerSpec::EVH5150, &PowerSpec::MARKIIC] {
        println!("\n=== {} ===", spec.name);
        println!("Quiescent volts at each node:");
        operating_point(spec);

        println!("\nSmall-signal gain to each node, 220 Hz at 10 mV:");
        for at in ["pi_p1", "pi_p2", "on1", "og1", "pl_a", "sec", "spk"] {
            let m = gain_at(spec, at, 0.01, 220.0);
            println!("  {at:<8}{:>10.1} dB", m.gain_db());
        }
        // Two things could be holding the plate down: the loop, or the screen
        // sagging under its own current. Take each away in turn.
        for (what, altered) in [
            ("the feedback loop opened", {
                let mut a = *spec;
                a.feedback = 100_000_000.0;
                a
            }),
            ("the screen supply made stiff", {
                let mut a = *spec;
                a.screen_resistance = 1.0;
                a.screen_resistor = 1.0;
                a
            }),
            ("both", {
                let mut a = *spec;
                a.feedback = 100_000_000.0;
                a.screen_resistance = 1.0;
                a.screen_resistor = 1.0;
                a
            }),
        ] {
            println!("\n  with {what}:");
            for at in ["pl_a", "spk"] {
                let m = gain_at(&altered, at, 0.01, 220.0);
                println!("  {at:<8}{:>10.1} dB", m.gain_db());
            }
            let m = gain_at(&altered, "spk", 3.0, 220.0);
            let rms = 3.0 * 10f64.powf(m.gain_db() / 20.0) / 2f64.sqrt();
            println!(
                "  {:<8}{rms:>10.2} V rms at 3 V in, {:.0} W, {:.1} % THD",
                "driven",
                rms * rms / spec.speaker,
                m.thd_percent()
            );
        }

        // What load the tubes actually want. The turns ratio sets the
        // impedance the plates work into, and an output stage makes its rated
        // power only when that matches what the tubes can deliver: too low and
        // they run out of current before they run out of voltage, too high and
        // the other way about.
        println!("\nPower against turns ratio, driven hard:");
        println!("  {:>7}{:>10}{:>10}{:>9}", "ratio", "Raa", "watts", "THD");
        for ratio in [11.0f64, 14.0, 15.4, 18.0, 22.0, 26.0, 30.0, 36.0] {
            let mut a = *spec;
            a.ratio = ratio;
            let mut best = (0.0f64, 0.0f64);
            for drive in [3.0f64, 6.0, 10.0, 16.0, 25.0] {
                let m = gain_at(&a, "spk", drive, 220.0);
                let rms = drive * 10f64.powf(m.gain_db() / 20.0) / 2f64.sqrt();
                let w = rms * rms / spec.speaker;
                if w > best.0 {
                    best = (w, m.thd_percent());
                }
            }
            println!(
                "  {ratio:>7.1}{:>10.0}{:>10.1}{:>8.1} %",
                ratio * ratio * spec.speaker,
                best.0,
                best.1
            );
        }

        // How stiff the supply is. Sag is wanted -- it is most of what a big
        // amplifier does under the hand -- but a supply impedance is also the
        // easiest thing to guess wrong, and guessing it high costs power that
        // then gets blamed on the tubes.
        println!("\nPower and sag against supply impedance:");
        println!("  {:>8}{:>10}{:>9}", "ohms", "watts", "THD");
        for r in [40.0f64, 80.0, 120.0, 150.0, 220.0] {
            let mut a = *spec;
            a.supply_resistance = r;
            let mut best = (0.0f64, 0.0f64);
            for drive in [6.0f64, 10.0, 16.0] {
                let m = gain_at(&a, "spk", drive, 220.0);
                let rms = drive * 10f64.powf(m.gain_db() / 20.0) / 2f64.sqrt();
                let w = rms * rms / spec.speaker;
                if w > best.0 {
                    best = (w, m.thd_percent());
                }
            }
            println!("  {r:>8.0}{:>10.1}{:>8.1} %", best.0, best.1);
        }

        println!("\nOutput across the speaker, against how hard the inverter is driven:");
        println!(
            "  {:>10}{:>12}{:>10}{:>10}{:>12}{:>10}",
            "in (V)", "out (V rms)", "watts", "THD", "plate pk", "grid pk"
        );
        for volts in [0.1f64, 0.3, 1.0, 3.0, 4.5, 6.0, 8.0, 10.0, 20.0, 40.0] {
            let m = gain_at(spec, "spk", volts, 220.0);
            let out = volts * 10f64.powf(m.gain_db() / 20.0);
            let rms = out / 2f64.sqrt();
            let plate = gain_at(spec, "pl_a", volts, 220.0);
            let grid = gain_at(spec, "og1", volts, 220.0);
            println!(
                "  {volts:>10.1}{:>12.2}{:>10.1}{:>9.1} %{:>12.0}{:>10.0}",
                rms,
                rms * rms / spec.speaker,
                m.thd_percent(),
                volts * 10f64.powf(plate.gain_db() / 20.0),
                volts * 10f64.powf(grid.gain_db() / 20.0),
            );
        }
    }
}
