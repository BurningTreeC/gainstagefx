//! The Cali Rectifier's operating point against the node voltages on Mesa's sheet,
//! and what its open loop does to the whole voice.
//!
//! In the red channel's Modern mode the sheet's LDR19 lifts the power amp's
//! loop (see `power::PowerSpec::RECTO_6L6`). The last table sets the voice as
//! built beside the same voice with a 100 k loop put back -- what the model had
//! until 2026-09-30 -- at a guitar's 0.122 V, 220 Hz.
//!
//! `cargo run --release --example recto_op`
use gainstagefx::circuits::{power, rectifier};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
fn main() {
    let c = rectifier::build(10_000.0, 1_000_000.0).unwrap();
    let names = [
        "d", "e", "c", "v1_p", "v1_k", "v2_p", "v2b_p", "v2b_k", "v3_p", "cf",
    ];
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
        "  inverter plates {:.0} / {:.0} V (sheet: 280 V), cathodes {:.0} V (sheet: 48 V)",
        v[0], v[1], v[2]
    );
    println!(
        "  6L6 {:.1} mA and {:.1} W a valve at {:.0} V",
        total * 1e3 / 4.0,
        total / 4.0 * v[3],
        v[3]
    );

    println!("\nwhole voice, 0.122 V at 220 Hz, presence at half:");
    for (label, feedback) in [
        ("no loop (as built)", 0.0),
        ("100 k loop put back", 100_000.0),
    ] {
        let mut spec = power::PowerSpec::RECTO_6L6;
        spec.feedback = feedback;
        for drive in [0.3, 0.6, 1.0] {
            let pre = rectifier::build(10_000.0, 1_000_000.0).unwrap();
            let mut pre = Simulation::new(pre, 48_000.0);
            pre.set_control(rectifier::GAIN, drive);
            pre.find_operating_point();
            let mut pow = Simulation::new(power::build(&spec, 10_000.0).unwrap(), 48_000.0);
            pow.find_operating_point();
            let tone = Tone::near(48_000.0, 16_384, 220.0, 0.122);
            let m = measure::run(tone, 4_800, |x| pow.process(pre.process(x)));
            println!(
                "  {label:<20} gain {drive:.1}: {:.1} % distortion, H2 {:.1} H3 {:.1} H5 {:.1} %",
                m.thd_percent(),
                m.harmonic_percent(2),
                m.harmonic_percent(3),
                m.harmonic_percent(5)
            );
        }
    }
}
