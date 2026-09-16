//! The Brit Plexi preamplifier's operating point, and the idle current of the
//! normal channel's unbuilt first triode.
//!
//! `cargo run --release --example plexi_op`
use gainstagefx::circuits::plexi;
use gainstagefx::dsp::netlist::{Netlist, TriodeSpec};
use gainstagefx::dsp::time::Simulation;
fn main() {
    for volts in [270.0, 285.0, 300.0] {
        let mut net = Netlist::new("V1a");
        net.input("in", 1.0)
            .resistor("in", "g", 34_000.0)
            .resistor("in", "gnd", 1_000_000.0)
            .supply("p", 100_000.0, volts)
            .resistor("k", "gnd", 820.0)
            .triode("p", "g", "k", TriodeSpec::ECC83);
        let c = net.build("k").unwrap();
        let k = c.unknown_named("k").unwrap();
        let mut sim = Simulation::new(c, 48_000.0);
        sim.find_operating_point();
        let ik = sim.voltage_at(k) / 820.0;
        println!("V1a at {volts} V: {:.3} mA, equivalent {:.0} ohm", ik * 1e3, volts / ik);
    }
    let c = plexi::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["pin", "v1n", "v2n", "v1_p", "v1_k", "v2_p", "v2_k", "cf"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    for (n, i) in names.iter().zip(idx) {
        println!("{n:<5} {:.2} V", sim.voltage_at(i));
    }
    let spec = &gainstagefx::circuits::power::PowerSpec::PLEXI_EL34;
    let c = gainstagefx::circuits::power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "og1", "og2", "ht", "m", "pl_a", "pl_b"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    let v: Vec<f64> = idx.iter().map(|i| i.map(|i| sim.voltage_at(i)).unwrap_or(f64::NAN)).collect();
    for (n, x) in names.iter().zip(&v) {
        println!("power {n:<6} {x:.2} V");
    }
    let i_pi = (spec.pi_supply - v[0]) / spec.pi_plate_driven + (spec.pi_supply - v[1]) / spec.pi_plate_other;
    let tubes = (spec.plate_supply - v[5]) / spec.supply_resistance;
    println!(
        "output valves idle {:.1} mA total, {:.1} mA and {:.1} W per valve at {:.0} V",
        tubes * 1e3,
        tubes * 1e3 / 4.0,
        tubes / 4.0 * v[7],
        v[7]
    );
    println!("inverter idle {:.3} mA at {} V: {:.0} ohm", i_pi * 1e3, spec.pi_supply, spec.pi_supply / i_pi);
}
