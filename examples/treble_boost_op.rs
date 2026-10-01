//! The Treble Boost (Dallas Rangemaster) stage by stage: its operating point,
//! its gain against frequency, and the figures ElectroSmash's analysis gives.
//!
//! ElectroSmash: base near 7.8 V above the negative rail (1.2 V below ground),
//! collector "at -7.0 V", about 0.2 mA, a voltage gain of "80 = 38 dB", an
//! input impedance of "10K to 12K", and the input high-pass at "2.6 kHz".
//!
//! `cargo run --release --example treble_boost_op`

use gainstagefx::circuits::treble_boost::{self, BOOST};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain_db(source: f64, node: &str, boost: f64, hz: f64, level: f64) -> (f64, f64) {
    let mut s = Simulation::new(treble_boost::tap(source, 470_000.0, node).unwrap(), RATE);
    s.set_control(BOOST, boost);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, level);
    let m = measure::run(t, (RATE / 4.0) as usize, |x| s.process(x));
    (m.gain_db(), m.thd_percent())
}

fn main() {
    let circuit = treble_boost::build(10_000.0, 470_000.0).unwrap();
    let nodes = ["vneg", "b", "e", "c", "w"];
    let index: Vec<usize> = nodes
        .iter()
        .map(|n| circuit.unknown_named(n).expect("node"))
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    s.set_control(BOOST, 1.0);
    assert!(s.find_operating_point());
    println!("operating point (positive ground, Boost full):");
    let v: Vec<f64> = index.iter().map(|&i| s.voltage_at(i)).collect();
    for (name, x) in nodes.iter().zip(&v) {
        println!("  {name:<5} {x:>8.3} V");
    }
    let ie = -v[2] / 3_900.0;
    println!(
        "  emitter current {:.3} mA (ElectroSmash ~0.2), collector {:.2} V (sources: -7.0)",
        ie * 1e3,
        v[3]
    );

    println!("\nsmall-signal gain, jack to output, Boost full (10 mV):");
    println!(
        "  {:>7}  {:>12}  {:>12}",
        "Hz", "stiff source", "10 k source"
    );
    for hz in [100.0, 300.0, 1_000.0, 2_600.0, 5_000.0, 10_000.0, 20_000.0] {
        let stiff = gain_db(1.0, "out", 1.0, hz, 0.01).0;
        let guitar = gain_db(10_000.0, "out", 1.0, hz, 0.01).0;
        println!("  {hz:>7.0}  {stiff:>+11.1}  {guitar:>+11.1}");
    }

    println!("\nthe collector over the base, 10 kHz, small signal (the stage's own gain):");
    let c = gain_db(1.0, "c", 1.0, 10_000.0, 0.001).0;
    let b = gain_db(1.0, "b", 1.0, 10_000.0, 0.001).0;
    println!("  {:+.1} dB (ElectroSmash: 38 dB)", c - b);

    println!("\ndistortion at 5 kHz from a stiff source, Boost full, by level:");
    for level in [0.01, 0.03, 0.1, 0.2, 0.4] {
        let (g, thd) = gain_db(1.0, "out", 1.0, 5_000.0, level);
        println!("  {:>5.0} mV  gain {g:+.1} dB  THD {thd:.1} %", level * 1e3);
    }

    println!("\nthrough the Boost control at 5 kHz (10 k source):");
    for boost in [0.0, 0.25, 0.5, 0.75, 1.0] {
        println!(
            "  boost {boost:.2}: {:+.1} dB",
            gain_db(10_000.0, "out", boost, 5_000.0, 0.01).0
        );
    }
}
