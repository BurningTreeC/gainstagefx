//! The Brit AC30's operating point and the idle current of the normal channel's
//! triode, which shares V1's cathode resistor.
//!
//! `cargo run --release --example ac30_op`
use gainstagefx::circuits::{ac30, power};
use gainstagefx::dsp::netlist::{Netlist, TriodeSpec};
use gainstagefx::dsp::time::Simulation;
fn main() {
    // One triode of V1 on its own, to size the unbuilt one's idle current.
    for volts in [250.0, 270.0, 290.0] {
        let mut net = Netlist::new("V1 half");
        net.input("in", 1.0)
            .resistor("in", "g", 68_000.0)
            .resistor("in", "gnd", 1_000_000.0)
            .supply("p", 220_000.0, volts)
            .resistor("k", "gnd", 1_500.0)
            .triode("p", "g", "k", TriodeSpec::ECC83);
        let c = net.build("k").unwrap();
        let k = c.unknown_named("k").unwrap();
        let mut sim = Simulation::new(c, 48_000.0);
        sim.find_operating_point();
        let ik = sim.voltage_at(k) / 1_500.0;
        println!("V1 half at {volts} V: {:.3} mA, equivalent {:.0} ohm", ik * 1e3, volts / ik);
    }
    let c = ac30::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["n4", "n42", "v1_p", "v1_k", "tb_p", "tb_k", "cf"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    println!("preamp settled {}", sim.find_operating_point());
    for (n, i) in names.iter().zip(idx) {
        println!("  {n:<5} {:.2} V", sim.voltage_at(i));
    }
    // The inverter's own idle current through R11 22 k, which sets its supply.
    let spec = &power::PowerSpec::AC30_EL84;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "pi_b", "ok", "ht", "on1", "on2"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    println!("power settled {}", sim.find_operating_point());
    let v: Vec<f64> = idx.iter().map(|i| i.map(|i| sim.voltage_at(i)).unwrap_or(f64::NAN)).collect();
    for (n, x) in names.iter().zip(&v) {
        println!("  {n:<6} {x:.2} V");
    }
    let cathode = v[4];
    let total = cathode / spec.cathode_bias;
    let inverter = (spec.pi_supply - v[0]) / spec.pi_plate_driven + (spec.pi_supply - v[1]) / spec.pi_plate_other;
    println!(
        "  inverter idle {:.2} mA -> {:.0} V behind R11 22k from {HT} V",
        inverter * 1e3,
        ac30::HT - inverter * 22_000.0,
        HT = ac30::HT
    );
    println!(
        "  EL84 bias {:.1} V, {:.1} mA total, {:.1} mA and {:.1} W a valve at {:.0} V",
        cathode,
        total * 1e3,
        total * 1e3 / 4.0,
        total / 4.0 * (v[5] - cathode),
        v[5]
    );
}
