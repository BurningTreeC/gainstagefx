//! The Mark IIC+ operating point, to set against the voltages on the Mesa drawing.
//!
//! `cargo run --release --example mark_op`
use gainstagefx::circuits::markiic;
use gainstagefx::dsp::time::Simulation;
fn main() {
    let c = markiic::tap(10_000.0, 1_000_000.0, "to_eq").unwrap();
    let names = [
        "v1a_k", "v1a_p", "v1b_k", "v1b_p", "v3b_k", "v3b_p", "v4a_k", "v4a_p", "v2b_k", "v2b_p",
        "v2a_k", "v2a_p",
    ];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    for (n, i) in names.iter().zip(idx) {
        if let Some(i) = i {
            println!("{n:<6} {:.2} V", sim.voltage_at(i));
        }
    }
}
