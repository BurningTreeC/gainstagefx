//! The Round Fuzz and Rodent netlists alone: operating points, gain and distortion.
//!
//! `cargo run --release --example pedals`
use gainstagefx::circuits::{rodent, round_fuzz};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;
fn meas(c: &Circuit, controls: &[(usize, f64)], hz: f64, peak: f64) -> measure::Measured {
    let mut sim = Simulation::new(c.clone(), RATE);
    for &(k, v) in controls { sim.set_control(k, v); }
    sim.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, peak);
    measure::run(t, (RATE * 0.5) as usize, |x| sim.process(x))
}
fn main() {
    let ff = round_fuzz::build(10_000.0, 470_000.0).unwrap();
    let names = ["b1", "c1", "e2", "c2", "tap", "vneg"];
    let idx: Vec<_> = names.iter().map(|n| ff.unknown_named(n).unwrap()).collect();
    for fuzz in [0.0, 0.5, 1.0] {
        let mut sim = Simulation::new(ff.clone(), RATE);
        sim.set_control(round_fuzz::FUZZ, fuzz);
        print!("fuzz {fuzz}: settled {} ", sim.find_operating_point());
        for (n, i) in names.iter().zip(&idx) { print!("{n} {:.3} ", sim.voltage_at(*i)); }
        println!();
    }
    for fuzz in [0.0, 0.5, 1.0] {
        let small = meas(&ff, &[(round_fuzz::FUZZ, fuzz)], 1000.0, 0.005);
        let big = meas(&ff, &[(round_fuzz::FUZZ, fuzz)], 220.0, 0.122);
        println!("  fuzz {fuzz}: gain {:.1} dB | 0.122 V: THD {:.1}% 2nd {:.1}% 3rd {:.1}% level {:.1} dB", small.gain_db(), big.thd_percent(), big.harmonic_percent(2), big.harmonic_percent(3), big.gain_db());
    }
    let rat = rodent::build(10_000.0, 470_000.0).unwrap();
    let names = ["vref", "amp", "clip", "source", "u1_int"];
    let idx: Vec<_> = names.iter().map(|n| rat.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(rat.clone(), RATE);
    print!("rat settled {} ", sim.find_operating_point());
    for (n, i) in names.iter().zip(&idx) { print!("{n} {:.3} ", sim.voltage_at(*i)); } println!();
    for d in [0.0, 0.5, 1.0] {
        let lo = meas(&rat, &[(rodent::DISTORTION, d), (rodent::FILTER, 0.0)], 1000.0, 1e-4);
        let hi = meas(&rat, &[(rodent::DISTORTION, d), (rodent::FILTER, 0.0)], 8000.0, 1e-4);
        let big = meas(&rat, &[(rodent::DISTORTION, d), (rodent::FILTER, 0.3)], 220.0, 0.122);
        println!("  dist {d}: 1k gain {:.1} dB, 8k {:.1} dB | 0.122 V THD {:.1}% level {:.1} dB", lo.gain_db(), hi.gain_db(), big.thd_percent(), big.gain_db());
    }
    let tap = rodent::tap(10_000.0, 470_000.0, "amp").unwrap();
    for hz in [200.0, 1000.0, 3000.0, 10000.0] {
        let m = meas(&tap, &[(rodent::DISTORTION, 1.0)], hz, 1e-5);
        println!("  op-amp out, full distortion, {hz} Hz: {:.1} dB (ideal 67)", m.gain_db());
    }
}
