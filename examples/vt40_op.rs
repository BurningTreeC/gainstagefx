//! The American VT-40 (Ampeg VT-40, DWG 06500 B) against its own drawing and
//! the service section's A.C. voltage readings.
//!
//! The drawing gives no-signal DC voltages on every stage; the readings page
//! (12/71-198) gives the A.C. volts at every valve pin with 0.3 V at 400 Hz in
//! and the treble, bass, midrange and reverb controls at their middle, at
//! three-quarter power: 17.9 V across 8 ohm.
//!
//! This prints the preamplifier's idle beside the drawing's; fits the three
//! open-circuit voltages that put the power stage's nodes on it
//! (`power::VT40_PI_OPEN`, `VT40_HT_OPEN`, `VT40_SCREEN_OPEN`) and prints its
//! idle; then sets channel one's volume so that V1b's grid reads the table's
//! 0.5 V, and follows the signal pin by pin through both netlists, for each
//! SENSITIVITY position. Paste the fitted constants into `circuits/power.rs`
//! when they move.
//!
//! `cargo run --release --example vt40_op`

use gainstagefx::circuits::american_vt40 as vt40;
use gainstagefx::circuits::power::{self, ParaphaseFront, PowerSpec};
use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::GROUND;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 1_000_000.0;
const HZ: f64 = 400.0;
const IN_RMS: f64 = 0.3;

fn spec_with(pi: f64, ht: f64, scr: f64) -> PowerSpec {
    let front: &'static ParaphaseFront = Box::leak(Box::new(ParaphaseFront {
        rail_upstream: pi,
        ..ParaphaseFront::VT40
    }));
    PowerSpec {
        plate_supply: ht,
        screen_supply: scr,
        paraphase_front: Some(front),
        ..PowerSpec::VT40_7027A
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

/// The rms at a preamplifier node with `IN_RMS` at the input.
fn pre_rms(at: &str, volume: f64, sensitivity: usize) -> f64 {
    let mut s = Simulation::new(vt40::tap(SOURCE, LOAD, at).unwrap(), RATE);
    s.set_control(vt40::VOLUME, volume);
    for (slot, value) in vt40::SENSITIVITY_SLOTS
        .into_iter()
        .zip(vt40::SENSITIVITY[sensitivity])
    {
        s.set_realtime_value(slot, value);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 9_600, HZ, IN_RMS * 2f64.sqrt());
    measure::run(t, 19_200, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 2f64.sqrt()
}

/// The rms at a power-stage node with `rms` at its input.
fn power_rms(spec: &PowerSpec, at: &str, rms: f64) -> f64 {
    let mut s = Simulation::new(power::tap(spec, SOURCE, at).unwrap(), RATE);
    s.find_operating_point();
    let t = Tone::near(RATE, 9_600, HZ, rms * 2f64.sqrt());
    measure::run(t, 38_400, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 2f64.sqrt()
}

fn main() {
    // --- the preamplifier's idle ---------------------------------------------
    let c = vt40::build(SOURCE, LOAD).unwrap();
    let pre = [
        ("bp", "rail 354"),
        ("p1", "V1a plate 205"),
        ("k1", "V1a cathode 2.2"),
        ("pm", "V1b plate 250"),
        ("k1b", "V1b cathode 2.9"),
        ("p3", "6K11 pin 2 209"),
        ("k3", "6K11 pin 3 24"),
        ("p2", "6K11 pin 5 194"),
        ("k2", "6K11 pin 6 1.37"),
        ("kf", "6K11 pin 4 196"),
        ("k3a", "V3 pin 3 224"),
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

    // --- the power stage: fit, then idle ---------------------------------------
    let names = [
        "f_c", "ht", "scr", "f_p1", "f_k1", "pi_p1", "pi_p2", "f_kp", "pl_a", "og1", "os1",
    ];
    let (mut pi, mut ht, mut scr) = (
        power::VT40_PI_OPEN,
        power::VT40_HT_OPEN,
        power::VT40_SCREEN_OPEN,
    );
    for _ in 0..8 {
        let v = idle(&spec_with(pi, ht, scr), &names);
        pi += 393.0 - v[0];
        ht += 594.0 - v[1];
        scr += 589.0 - v[2];
    }
    let spec = spec_with(pi, ht, scr);
    let v = idle(&spec, &names);
    println!("fitted: VT40_PI_OPEN {pi:.1}  VT40_HT_OPEN {ht:.1}  VT40_SCREEN_OPEN {scr:.1}");
    let drawing = [
        "393",
        "594",
        "589",
        "V3b plate 223",
        "V3b cathode 1.96",
        "V4 plate 218",
        "V4 plate 218",
        "V4 cathodes 10.5",
        "plates 586 (ideal at DC: the rail)",
        "grids -65",
        "screens",
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

    // --- the A.C. readings -----------------------------------------------------
    for (sensitivity, label) in [(1usize, "Middle (built)"), (2, "Low (drawn)"), (0, "High")] {
        println!("\nSENSITIVITY {label}, 0.3 V at 400 Hz in:");
        println!(
            "  V1a plate     {:6.2} V  (10)",
            pre_rms("p1", 1.0, sensitivity)
        );
        // The volume that puts the table's 0.5 V on V1b's grid.
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            if pre_rms("g1b", mid, sensitivity) < 0.5 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let volume = 0.5 * (lo + hi);
        println!("  VOL 1 at {volume:.3} for 0.5 V on V1b's grid");
        if sensitivity != 1 {
            continue;
        }
        for (node, label) in [
            ("pm", "V1b plate (5.8)"),
            ("g3", "6K11 pin 11 (.47)"),
            ("k3", "6K11 pin 3 (.5)"),
            ("p3", "6K11 pin 2 (.1)"),
            ("p2", "6K11 pin 5 (5.2)"),
            ("kf", "6K11 pin 4 (5)"),
            ("d1p", "V202 pin 1 (4.2)"),
            ("d2g", "V202 pin 7 (2)"),
            ("d2p", "V202 pin 6 (2.3)"),
            ("g3a", "V3 pin 2 (.32)"),
            ("k3a", "V3 pin 3 (.3)"),
            ("out", "EXT AMP"),
        ] {
            println!("  {label:<20} {:7.3} V", pre_rms(node, volume, sensitivity));
        }
        let ext = pre_rms("out", volume, sensitivity);
        for (node, label) in [
            ("f_g1", "V3 pin 7 (.28)"),
            ("f_p1", "V3 pin 6 (3.5)"),
            ("f_g2", "V4 pin 2 (3.4)"),
            ("f_g3", "V4 pin 7 (3.4)"),
            ("pi_p2", "V4 pin 1 (38)"),
            ("pi_p1", "V4 pin 6 (38)"),
            ("og1", "V5 pin 5 (38)"),
            ("pl_a", "V5 pin 3 (250)"),
            ("spk", "8 ohm (17.9)"),
        ] {
            println!("  {label:<20} {:7.2} V", power_rms(&spec, node, ext));
        }
    }
}
