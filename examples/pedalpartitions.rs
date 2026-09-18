//! Print the full MNA size and exact nonlinear Schur boundary of the two large
//! pedals after filter partitioning.
//!
//! `cargo run --release --example pedalpartitions`
use gainstagefx::circuits::{heavy_metal, metal_zone};
use gainstagefx::dsp::time::Simulation;

fn print(name: &str, circuit: gainstagefx::dsp::netlist::Circuit) {
    let sim = Simulation::new(circuit, 48_000.0);
    match sim.nonlinear_reduction() {
        Some((boundary, internal)) => println!(
            "{name:<12} full={}  newton_boundary={}  schur_internal={}",
            sim.unknowns(),
            boundary,
            internal
        ),
        None => println!("{name:<12} full={}  no nonlinear reduction", sim.unknowns()),
    }
}

fn main() {
    print(
        "HM-2",
        heavy_metal::build(10_000.0, 470_000.0).expect("HM-2 builds"),
    );
    print(
        "MT-2",
        metal_zone::build(10_000.0, 470_000.0).expect("MT-2 builds"),
    );
}
