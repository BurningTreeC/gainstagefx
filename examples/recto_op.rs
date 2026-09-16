//! The Cali Rectifier's operating point against the node voltages on Mesa's sheet.
//!
//! `cargo run --release --example recto_op`
use gainstagefx::circuits::{power, rectifier};
use gainstagefx::dsp::time::Simulation;
fn main() {
    let c = rectifier::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["d", "e", "c", "v1_p", "v1_k", "v2_p", "v2b_p", "v2b_k", "v3_p", "cf"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    println!("preamp settled {}", sim.find_operating_point());
    for (n, i) in names.iter().zip(idx) {
        println!("  {n:<5} {:.1} V", sim.voltage_at(i));
    }
    let spec = &power::PowerSpec::RECTO_6L6;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "ht"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    println!("power settled {}", sim.find_operating_point());
    let v: Vec<f64> = idx.iter().map(|i| sim.voltage_at(*i)).collect();
    for (n, x) in names.iter().zip(&v) {
        println!("  {n:<6} {x:.1} V");
    }
    let total = (spec.plate_supply - v[3]) / spec.supply_resistance;
    println!(
        "  inverter plates {:.0} / {:.0} V (sheet: 280 V), cathodes {:.0} V (sheet: 30 V)",
        v[0], v[1], v[2]
    );
    println!("  6L6 {:.1} mA and {:.1} W a valve at {:.0} V", total * 1e3 / 4.0, total / 4.0 * v[3], v[3]);
}
