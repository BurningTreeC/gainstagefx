//! The Brit Plexi Bass (Marshall 1992 Super Bass) stage by stage: the
//! preamplifier's operating point, the inverter node it leaves for the power
//! stage (`plexi_bass::INVERTER_NODE`), the power stage at idle, and the
//! small-signal gain from the jack to each stage beside the Brit Plexi's.
//!
//! `cargo run --release --example plexi_bass_op`

use gainstagefx::circuits::{plexi, plexi_bass, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain(circuit: gainstagefx::dsp::netlist::Circuit, hz: f64) -> f64 {
    let mut s = Simulation::new(circuit, RATE);
    s.set_control(plexi_bass::VOLUME, 1.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-4);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
}

fn main() {
    let c = plexi_bass::build(10_000.0, 1_000_000.0).unwrap();
    let names = [
        "pin", "v2n", "v1n", "v1_p", "v1b_p", "v1_k", "v2_p", "v2_k", "cf",
    ];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    println!("preamplifier operating point:");
    for (n, i) in names.iter().zip(idx) {
        println!("  {n:<6} {:8.2} V", sim.voltage_at(i));
    }

    let spec = &power::PowerSpec::PLEXI_BASS_EL34;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "og1", "og2", "ht", "pl_a", "pl_b"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    let v: Vec<f64> = idx
        .iter()
        .map(|i| i.map(|i| sim.voltage_at(i)).unwrap_or(f64::NAN))
        .collect();
    println!("\npower stage at idle:");
    for (n, x) in names.iter().zip(&v) {
        println!("  {n:<6} {x:8.2} V");
    }
    let i_pi = (spec.pi_supply - v[0]) / spec.pi_plate_driven
        + (spec.pi_supply - v[1]) / spec.pi_plate_other;
    println!(
        "  inverter {:.3} mA at {} V: {:.0} ohm (plexi::INVERTER_IDLE {:.0})",
        i_pi * 1e3,
        spec.pi_supply,
        spec.pi_supply / i_pi,
        plexi::INVERTER_IDLE
    );

    println!("\nsmall-signal gain from the jack, volume full, tone at noon:");
    for hz in [40.0, 100.0, 400.0, 1_000.0, 4_000.0] {
        print!("  {hz:>6.0} Hz:");
        for node in ["v1_p", "v2_g", "v2_p", "cf", "out"] {
            print!(
                " {node} {:+.1}",
                gain(plexi_bass::tap(10_000.0, 1_000_000.0, node).unwrap(), hz)
            );
        }
        println!();
        print!("  {:>6}  1959:", "");
        for node in ["v1_p", "v2_g", "v2_p", "cf", "out"] {
            print!(
                " {node} {:+.1}",
                gain(plexi::tap(10_000.0, 1_000_000.0, node).unwrap(), hz)
            );
        }
        println!();
    }
}
