//! What the valve stage settles at, and what it does to a signal.

use gainstagefx::circuits::valve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn main() {
    let circuit = valve::build(&valve::CLASSIC, 10_000.0, 1_000_000.0).expect("builds");
    let mut sim = Simulation::new(circuit.clone(), RATE);
    sim.set_control(valve::BYPASS, 1.0);

    let settled = sim.find_operating_point();
    println!("operating point, with no signal (settled: {settled}):");
    for name in ["plate", "cathode", "grid"] {
        let at = circuit.names.iter().position(|n| n == name).unwrap();
        println!("  {name:>8}  {:>8.2} V", sim.voltage_at(at));
    }

    let plate = circuit.names.iter().position(|n| n == "plate").unwrap();

    let ip = (valve::CLASSIC.supply - sim.voltage_at(plate)) / valve::CLASSIC.plate_load;
    let rk = 1.0 / (1.0 / valve::CLASSIC.cathode + 1.0 / 100_000.0);
    println!(
        "  plate current {:.3} mA, which through {rk:.0} ohm should put the cathode at {:.2} V",
        ip * 1e3,
        ip * rk
    );

    println!("\ngain and distortion at the grid, bypass wide open:");
    for volts in [0.05, 0.2, 0.5, 1.0, 2.0] {
        let tone = Tone::near(RATE, 16_384, 220.0, volts);
        let mut sim = Simulation::new(circuit.clone(), RATE);
        sim.set_control(valve::BYPASS, 1.0);
        let m = measure::run(tone, (RATE * 0.3) as usize, |x| sim.process(x));
        println!(
            "  {volts:>4.2} V in -> {:>6.1} x, {:>5.2} % total, 2nd {:>5.2} %, 3rd {:>5.2} %",
            10f64.powf(m.gain_db() / 20.0),
            m.thd_percent(),
            m.harmonic_percent(2),
            m.harmonic_percent(3),
        );
    }
}
