//! The American V-4B (Ampeg V-4B, DWG 06700 A) against its drawing's
//! no-signal voltages: the preamplifier's, then the power stage's, after
//! fitting the three open-circuit voltages that put its nodes on the drawing
//! (`power::V4B_PI_OPEN`, `V4B_HT_OPEN`, `V4B_SCREEN_OPEN`). And what ULTRA LO
//! does, from 40 Hz to 5 kHz. Paste the fitted constants into
//! `circuits/power.rs` when they move.
//!
//! `cargo run --release --example v4b_op`

use gainstagefx::circuits::american_v4b as v4b;
use gainstagefx::circuits::power::{self, ParaphaseFront, PowerSpec};
use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::GROUND;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 1_000_000.0;

fn spec_with(pi: f64, ht: f64, scr: f64) -> PowerSpec {
    let front: &'static ParaphaseFront = Box::leak(Box::new(ParaphaseFront {
        rail_upstream: pi,
        ..ParaphaseFront::V4B
    }));
    PowerSpec {
        plate_supply: ht,
        screen_supply: scr,
        paraphase_front: Some(front),
        ..PowerSpec::V4B_7027A
    }
}

fn idle(spec: &PowerSpec, names: &[&str]) -> Vec<f64> {
    let c = power::build(spec, SOURCE).unwrap();
    let idx: Vec<usize> = names
        .iter()
        .map(|n| c.unknown_named(n).unwrap_or_else(|| panic!("{n}")))
        .collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    idx.iter().map(|&i| s.voltage_at(i)).collect()
}

/// The preamplifier's gain at `hz`, small signal, with ULTRA LO in a position.
fn gain(position: usize, hz: f64) -> f64 {
    let mut s = Simulation::new(v4b::build(SOURCE, LOAD).unwrap(), RATE);
    s.set_control(v4b::VOLUME, 0.3);
    for (slot, value) in v4b::ULTRA_LO_SLOTS.into_iter().zip(v4b::ULTRA_LO[position]) {
        s.set_value(slot, value);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 19_200, hz, 1e-3);
    measure::run(t, 48_000, |x| s.process(x)).gain_db()
}

fn main() {
    let c = v4b::build(SOURCE, LOAD).unwrap();
    let pre = [
        ("bp", "rail 325"),
        ("p1", "V1a plate 155"),
        ("k1", "V1a cathode 1.48"),
        ("pm", "V1b plate 230"),
        ("k1b", "V1b cathode 2.1"),
        ("p3", "6K11 pin 2 192"),
        ("k3", "6K11 pin 3 22"),
        ("p2", "6K11 pin 5 178"),
        ("k2", "6K11 pin 6 1.26"),
        ("kf", "6K11 pin 4 140"),
        ("k3a", "V3 pin 3 200"),
    ];
    let idx: Vec<usize> = pre
        .iter()
        .map(|(n, _)| c.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    println!("preamplifier idle (drawing):");
    for ((n, label), i) in pre.iter().zip(idx) {
        println!("  {n:<5} {:8.2} V   {label}", s.voltage_at(i));
    }

    let names = [
        "f_c", "ht", "scr", "f_p1", "f_k1", "pi_p1", "pi_p2", "f_kp", "pl_a", "og1", "os1",
    ];
    let (mut pi, mut ht, mut scr) = (
        power::V4B_PI_OPEN,
        power::V4B_HT_OPEN,
        power::V4B_SCREEN_OPEN,
    );
    for _ in 0..8 {
        let v = idle(&spec_with(pi, ht, scr), &names);
        pi += 360.0 - v[0];
        ht += 545.0 - v[1];
        scr += 540.0 - v[2];
    }
    let spec = spec_with(pi, ht, scr);
    let v = idle(&spec, &names);
    println!("fitted: V4B_PI_OPEN {pi:.1}  V4B_HT_OPEN {ht:.1}  V4B_SCREEN_OPEN {scr:.1}");
    let drawing = [
        "360",
        "545",
        "540",
        "V3b plate 205",
        "V3b cathode 1.8",
        "V4 plate 200",
        "V4 plate 200",
        "V4 cathodes 9.7",
        "plates 540 (ideal at DC: the rail)",
        "grids -64",
        "screens (527 printed)",
    ];
    for ((n, x), label) in names.iter().zip(&v).zip(drawing) {
        println!("  {n:<6} {x:8.2} V   {label}");
    }
    let valve = Pentode::new(0, 1, GROUND, 2, 1.0, spec.tube);
    let (ia, is) = valve.currents(v[8], v[9], v[10]);
    println!(
        "  each 7027A idles at {:.1} mA, {:.1} mA of screen, {:.1} W",
        ia * 1e3,
        is * 1e3,
        ia * v[8]
    );

    println!("\nULTRA LO, on against off, small signal at VOL 1 0.3:");
    for hz in [40.0, 80.0, 150.0, 300.0, 600.0, 1_200.0, 2_500.0, 5_000.0] {
        println!("  {hz:6.0} Hz  {:+6.1} dB", gain(0, hz) - gain(1, hz));
    }
}
