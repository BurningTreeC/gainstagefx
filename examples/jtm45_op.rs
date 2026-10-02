//! The Brit 45 (Marshall JTM45) against the valve voltage chart on Marshall's
//! period drawing: V1's plates 220 V and cathodes 1.6 V, the gain stage's plate
//! 190 V and cathode 1.1 V, the follower's cathode 190 V on the 310 V node, the
//! inverter's plates 250 V and cathodes 40 V, the KT66s' plates 430 V and
//! screens 440 V, the rectifier 450 V; and the KT66s' idle.
//!
//! `cargo run --release --example jtm45_op`
use gainstagefx::circuits::{jtm45, power};
use gainstagefx::dsp::time::Simulation;

fn main() {
    let c = jtm45::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["pin", "v1n", "v1_p", "v1_k", "v2_p", "v2_k", "cf"];
    let chart = ["380", "310", "220", "1.6", "190", "1.1", "190"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    println!("preamp (chart in brackets):");
    for ((n, i), want) in names.iter().zip(idx).zip(chart) {
        println!("  {n:<5} {:7.2} V  ({want})", sim.voltage_at(i));
    }
    let spec = &power::PowerSpec::JTM45_KT66;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = [
        "pi_p1", "pi_p2", "pi_k", "og1", "og2", "ht", "scr", "pl_a", "pl_b",
    ];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    let v: Vec<f64> = idx
        .iter()
        .map(|i| i.map(|i| sim.voltage_at(i)).unwrap_or(f64::NAN))
        .collect();
    println!("power stage (chart: inverter plates 250, cathodes 40; KT66 plates 430, screens 440; rectifier 450):");
    for (n, x) in names.iter().zip(&v) {
        println!("  {n:<6} {x:8.2} V");
    }
    let i_pi = (spec.pi_supply - v[0]) / spec.pi_plate_driven
        + (spec.pi_supply - v[1]) / spec.pi_plate_other;
    println!(
        "  inverter draws {:.2} mA at {} V: {:.0} ohm (jtm45::INVERTER_IDLE {:.0})",
        i_pi * 1e3,
        spec.pi_supply,
        spec.pi_supply / i_pi,
        jtm45::INVERTER_IDLE
    );
}
