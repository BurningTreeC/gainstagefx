//! The Brit DR103's operating point: the one voltage its drawing gives is V2a's
//! plate at 140 V.
//!
//! `cargo run --release --example dr103_op`
use gainstagefx::circuits::{dr103, power};
use gainstagefx::dsp::time::Simulation;
fn main() {
    let c = dr103::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["n3", "n1", "v1_p", "v1_k", "v2_p", "v2_k", "cf1", "v3_p", "out"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    println!("preamp settled {}", sim.find_operating_point());
    for (n, i) in names.iter().zip(idx) {
        println!("  {n:<5} {:.1} V", sim.voltage_at(i));
    }
    let spec = &power::PowerSpec::DR103_EL34;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "pi_a", "ht", "on1"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, 48_000.0);
    println!("power settled {}", sim.find_operating_point());
    let v: Vec<f64> = idx.iter().map(|i| i.map(|i| sim.voltage_at(i)).unwrap_or(f64::NAN)).collect();
    for (n, x) in names.iter().zip(&v) {
        println!("  {n:<6} {x:.1} V");
    }
    let total = (spec.plate_supply - v[4]) / spec.supply_resistance;
    println!(
        "  EL34 {:.1} mA and {:.1} W a valve at {:.0} V; inverter bias {:.2} V",
        total * 1e3 / 4.0,
        total / 4.0 * v[4],
        v[4],
        v[3] - v[2]
    );

    // Small-signal gain of the preamplifier, against the Brit 800's.
    use gainstagefx::dsp::measure::{self, Tone};
    for (name, circuit, drive_control, controls) in [
        ("DR103", gainstagefx::circuits::dr103::build(10_000.0, 1_000_000.0).unwrap(), dr103::VOLUME, vec![(dr103::TREBLE, 0.5), (dr103::BASS, 0.5), (dr103::MIDDLE, 0.5), (dr103::MASTER, 0.5)]),
        ("Brit 800", gainstagefx::circuits::brit800::build(10_000.0, 1_000_000.0).unwrap(), gainstagefx::circuits::brit800::VOLUME, vec![(gainstagefx::circuits::brit800::TREBLE, 0.5), (gainstagefx::circuits::brit800::BASS, 0.5), (gainstagefx::circuits::brit800::MIDDLE, 0.5)]),
    ] {
        for drive in [0.25, 0.5, 1.0] {
            let mut sim = Simulation::new(circuit.clone(), 96_000.0);
            for (c, v) in &controls { sim.set_control(*c, *v); }
            sim.set_control(drive_control, drive);
            sim.find_operating_point();
            let tone = Tone::near(96_000.0, 16_384, 220.0, 1e-4);
            let m = measure::run(tone, 19_200, |x| sim.process(x));
            println!("  {name} at drive {drive}: {:.1} dB small signal", m.gain_db());
        }
    }

    // What the stack actually does, across the band.
    let hz = [40.0, 80.0, 160.0, 320.0, 640.0, 1_280.0, 2_560.0, 5_120.0];
    for (label, bass, mid) in [
        ("bass 0 mid .5", 0.0, 0.5),
        ("bass 1 mid .5", 1.0, 0.5),
        ("bass 0 mid 0", 0.0, 0.0),
        ("bass 1 mid 0", 1.0, 0.0),
    ] {
        print!("  {label:<14}");
        for f in hz {
            let mut sim = Simulation::new(dr103::build(10_000.0, 1_000_000.0).unwrap(), 96_000.0);
            for (c, v) in [(dr103::VOLUME, 0.5), (dr103::TREBLE, 0.5), (dr103::BASS, bass), (dr103::MIDDLE, mid), (dr103::MASTER, 0.5)] {
                sim.set_control(c, v);
            }
            sim.find_operating_point();
            let tone = Tone::near(96_000.0, 16_384, f, 1e-4);
            print!("{:8.1}", measure::run(tone, 19_200, |x| sim.process(x)).gain_db());
        }
        println!();
    }
}
