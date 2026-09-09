//! The Twin Reverb channel, stage by stage.
//!
//! `cargo run --release --example twin`

use gainstagefx::circuits::twin::{self, BASS, MIDDLE, REVERB, TREBLE, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const GUITAR: f64 = 0.122;

fn at(node: &str, hz: f64, volts: f64, vol: f64) -> measure::Measured {
    let c = twin::tap(10_000.0, 1_000_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(VOLUME, vol);
    sim.set_control(TREBLE, 0.5);
    sim.set_control(BASS, 0.5);
    sim.set_control(MIDDLE, 0.5);
    sim.set_control(REVERB, 0.0);
    sim.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

/// What the drawing prints beside each tube, against what the model settles
/// at. A stage biased somewhere other than where the schematic says it sits
/// will distort at the wrong level however right its component values are,
/// and this is the only check that catches that.
fn operating_point() {
    let c = twin::build(10_000.0, 1_000_000.0).expect("builds");
    let names = c.names.clone();
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(VOLUME, 1.0);
    sim.set_control(TREBLE, 0.5);
    sim.set_control(BASS, 0.5);
    sim.set_control(MIDDLE, 0.5);
    sim.set_control(REVERB, 0.0);
    sim.find_operating_point();
    println!("=== operating point, against what the drawing prints ===");
    println!("  {:<10}{:>12}{:>14}", "node", "modelled", "on the drawing");
    for (node, printed) in [
        ("v1_p", Some(250.0)),
        ("v1_k", Some(2.0)),
        ("v2_p", Some(280.0)),
        ("v2_k", Some(2.0)),
        ("rv_p", Some(440.0)),
        ("rv_k", Some(8.6)),
        ("rc_p", Some(215.0)),
        ("rc_k", Some(2.0)),
    ] {
        let Some(i) = names.iter().position(|n| n == node) else {
            continue;
        };
        let v = sim.voltage_at(i);
        match printed {
            Some(p) => println!("  {node:<10}{v:>10.1} V{p:>12.1} V"),
            None => println!("  {node:<10}{v:>10.1} V{:>14}", "--"),
        }
    }
    println!();
}

fn main() {
    operating_point();
    let c = twin::build(10_000.0, 1_000_000.0).expect("builds");
    println!(
        "Twin Reverb AB763 vibrato channel: {} unknowns.\n",
        c.unknowns()
    );

    println!("=== stage by stage, small signal (1 mV at 1 kHz) ===");
    println!("  {:<10}{:>10}", "node", "gain");
    for node in ["v1_p", "ts_out", "vol", "out"] {
        println!(
            "  {:<10}{:>9.1} dB",
            node,
            at(node, 1000.0, 0.001, 1.0).gain_db()
        );
    }

    println!("\n=== the volume control, at a guitar's level ===");
    println!(
        "  {:<8}{:>10}{:>10}{:>9}{:>9}",
        "volume", "gain", "THD", "2nd", "3rd"
    );
    for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let m = at("out", 220.0, GUITAR, v);
        println!(
            "  {v:<8.2}{:>9.1} dB{:>9.2} %{:>8.2} %{:>8.2} %",
            m.gain_db(),
            m.thd_percent(),
            m.harmonic_percent(2),
            m.harmonic_percent(3)
        );
    }

    println!("\n=== headroom: what a Twin is for ===");
    println!("  {:<10}{:>10}{:>10}", "volts in", "gain", "THD");
    for volts in [0.05, 0.122, 0.25, 0.5, 1.0, 2.0] {
        let m = at("out", 220.0, volts, 1.0);
        println!(
            "  {volts:<10.3}{:>9.1} dB{:>9.2} %",
            m.gain_db(),
            m.thd_percent()
        );
    }

    println!("\n=== the tone stack, at the extremes (1 kHz reference) ===");
    println!("  {:<22}{:>10}", "setting", "gain");
    for (name, t, b, mid) in [
        ("flat (all at half)", 0.5, 0.5, 0.5),
        ("treble up", 1.0, 0.5, 0.5),
        ("treble down", 0.0, 0.5, 0.5),
        ("bass up", 0.5, 1.0, 0.5),
        ("middle down (scoop)", 0.5, 0.5, 0.0),
        ("middle up", 0.5, 0.5, 1.0),
    ] {
        let c = twin::tap(10_000.0, 1_000_000.0, "out").expect("builds");
        let mut sim = Simulation::new(c, RATE);
        sim.set_control(VOLUME, 1.0);
        sim.set_control(TREBLE, t);
        sim.set_control(BASS, b);
        sim.set_control(MIDDLE, mid);
        sim.set_control(REVERB, 0.0);
        sim.find_operating_point();
        let tone = Tone::near(RATE, 16_384, 1000.0, 0.001);
        let g = measure::run(tone, (RATE / 2.0) as usize, |x| sim.process(x)).gain_db();
        println!("  {name:<22}{g:>9.1} dB");
    }
}
