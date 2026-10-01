//! The Brum 100 (Laney Supergroup 100 Mk I) stage by stage: the
//! preamplifier's supply chain and operating point, the inverter node it leaves
//! for the power stage (`brum100::INVERTER_NODE`), the unbuilt BASS channel
//! half's idle current (`brum100::BASS_V1_IDLE`), the power stage at idle on
//! the trace's 600 V and -54 V, and the small-signal gain to each stage.
//!
//! `cargo run --release --example brum100_op`

use gainstagefx::circuits::{brum100, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{Netlist, TriodeSpec};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain(node: &str, hz: f64) -> f64 {
    let mut s = Simulation::new(brum100::tap(10_000.0, 1_000_000.0, node).unwrap(), RATE);
    s.set_control(brum100::VOLUME, 1.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-4);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
}

fn main() {
    let c = brum100::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["ht3", "ht4", "ht5", "v1_p", "v1_k", "v2_p", "v2_k", "cf"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    sim.find_operating_point();
    println!("preamplifier operating point:");
    let mut ht5 = 0.0;
    for (n, i) in names.iter().zip(idx) {
        let v = sim.voltage_at(i);
        if *n == "ht5" {
            ht5 = v;
        }
        println!("  {n:<5} {v:8.2} V");
    }

    // The BASS channel's half, alone on its own 1.5 k and 25 uF.
    let mut net = Netlist::new("U1A");
    net.input("in", 1.0)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "g", 34_000.0)
        .supply("p", 100_000.0, ht5)
        .resistor("k", "gnd", 1_500.0)
        .triode("p", "g", "k", TriodeSpec::ECC83);
    let c = net.build("k").unwrap();
    let k = c.unknown_named("k").unwrap();
    let mut s = Simulation::new(c, 48_000.0);
    s.find_operating_point();
    let ik = s.voltage_at(k) / 1_500.0;
    println!(
        "  BASS channel half at {ht5:.0} V: {:.3} mA, {:.0} ohm (BASS_V1_IDLE {:.0})",
        ik * 1e3,
        ht5 / ik,
        brum100::BASS_V1_IDLE
    );

    let spec = &power::PowerSpec::BRUM_EL34;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "og1", "og2", "ht", "scr", "pl_a"];
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
    let plates = (spec.plate_supply - v[5]) / spec.supply_resistance;
    let screens = (spec.screen_supply - v[6]) / spec.screen_resistance;
    println!(
        "  inverter {:.3} mA at {:.0} V: {:.0} ohm (INVERTER_IDLE {:.0})",
        i_pi * 1e3,
        spec.pi_supply,
        spec.pi_supply / i_pi,
        brum100::INVERTER_IDLE
    );
    println!(
        "  output valves {:.1} mA plate and {:.2} mA screen each, {:.1} W on each plate",
        plates * 1e3 / 4.0,
        screens * 1e3 / 4.0,
        plates / 4.0 * v[7]
    );

    println!("\nsmall-signal gain from the jack, volume full, tone at noon:");
    for hz in [40.0, 100.0, 400.0, 1_000.0, 4_000.0] {
        print!("  {hz:>6.0} Hz:");
        for node in ["v1_p", "v2_g", "v2_p", "cf", "out"] {
            print!(" {node} {:+.1}", gain(node, hz));
        }
        println!();
    }
}
