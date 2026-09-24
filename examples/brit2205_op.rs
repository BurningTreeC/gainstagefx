//! The Brit 2205 preamplifier, stage by stage: its operating point, and the
//! small-signal gain from the jack to each stage at the voicing's controls.
//!
//! A stage that reads forty decibels below its neighbour is a mis-wired
//! stage; this is the first thing to run after touching `brit2205.rs`.
//!
//! `cargo run --release --example brit2205_op`

use gainstagefx::circuits::brit2205::{self, GAIN, MASTER};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn main() {
    let circuit = brit2205::build(10_000.0, 250_000.0).unwrap();
    let nodes = [
        "a", "v1a_p", "v1a_k", "v1b_g", "v1b_p", "v1b_k", "x", "v3a_g", "v3a_p", "v3a_k", "g", "m",
        "v4a_p", "v4a_k", "out",
    ];
    let index: Vec<usize> = nodes
        .iter()
        .map(|n| circuit.unknown_named(n).expect("node"))
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    println!("operating point:");
    for (name, i) in nodes.iter().zip(index) {
        println!("  {name:<6} {:>8.2} V", s.voltage_at(i));
    }

    println!("\nsmall-signal gain from the jack (10 uV, 1 kHz), Gain / Master:");
    for gain in [0.3, 0.6, 0.9] {
        print!("  gain {gain:.1}:");
        for node in ["v1a_p", "v1b_p", "x", "v3a_p", "t_w", "g", "m", "out"] {
            let mut s = Simulation::new(brit2205::tap(10_000.0, 250_000.0, node).unwrap(), RATE);
            s.set_control(GAIN, gain);
            s.set_control(MASTER, brit2205::MASTER_REST);
            let tone = Tone::near(RATE, 16_384, 1_000.0, 1e-5);
            let db = measure::run(tone, (RATE / 4.0) as usize, |x| s.process(x)).gain_db();
            print!(" {node} {db:+.1}");
        }
        println!();
    }
    sensitivity();
    power_stage();
    power_sweep();
}

/// The power stage alone into its 4 ohm resistor: how much it makes, against
/// the 50 W it is sold as and the 57 W Ampbooks works out for the 50 W JCM800.
fn power_sweep() {
    use gainstagefx::circuits::power;
    let spec = &power::PowerSpec::BRIT_2205_EL34;
    println!("\npower stage alone, presence 0.5, 1 kHz:");
    for v in [1.0, 2.0, 4.0, 8.0, 16.0] {
        let mut s = Simulation::new(power::build(spec, 10_000.0).unwrap(), RATE);
        let tone = Tone::near(RATE, 16_384, 1_000.0, v);
        let m = measure::run(tone, (RATE / 4.0) as usize, |x| s.process(x));
        let vrms = m.fundamental().magnitude() / 2f64.sqrt();
        println!(
            "  {v:>5.1} V pk in -> {:>5.1} W, THD {:>5.1} %",
            vrms * vrms / spec.speaker,
            m.thd_percent()
        );
    }
}

/// The power stage at idle: the inverter's current, which is part of what
/// `brit2205::RAIL` was derived from, and the output valves' idle current,
/// which is what the bias was set for.
fn power_stage() {
    use gainstagefx::circuits::power;
    let spec = &power::PowerSpec::BRIT_2205_EL34;
    let c = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k", "ht", "pl_a", "pl_b"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n)).collect();
    let mut sim = Simulation::new(c, RATE);
    assert!(sim.find_operating_point());
    let v: Vec<f64> = idx
        .iter()
        .map(|i| i.map(|i| sim.voltage_at(i)).unwrap_or(f64::NAN))
        .collect();
    println!("\npower stage at idle:");
    for (n, x) in names.iter().zip(&v) {
        println!("  {n:<6} {x:>8.2} V");
    }
    let i_pi = (spec.pi_supply - v[0]) / spec.pi_plate_driven
        + (spec.pi_supply - v[1]) / spec.pi_plate_other;
    let valves = (spec.plate_supply - v[3]) / spec.supply_resistance;
    println!("  inverter {:.2} mA", i_pi * 1e3);
    println!(
        "  output valves {:.1} mA each, {:.1} W at {:.0} V",
        valves * 1e3 / 2.0,
        valves / 2.0 * v[4],
        v[4]
    );
}

/// Marshall's manual: boost channel sensitivity 0.12 mV at 1 kHz, all controls
/// full -- the input that gives rated output. 50 W into the 4 ohm tap is
/// 14.1 V rms at the speaker. This finds the input the model needs for that.
fn sensitivity() {
    use brit2205::{BASS, MIDDLE, TREBLE, VOLUME};
    use gainstagefx::circuits::power;
    let rms_out = |input_rms: f64| -> f64 {
        let mut pre = Simulation::new(brit2205::build(10_000.0, 250_000.0).unwrap(), RATE);
        for c in [GAIN, VOLUME, TREBLE, MIDDLE, BASS, MASTER] {
            pre.set_control(c, 1.0);
        }
        let mut pwr = Simulation::new(
            power::build(&power::PowerSpec::BRIT_2205_EL34, 10_000.0).unwrap(),
            RATE,
        );
        pwr.set_control(power::PRESENCE, 1.0);
        let tone = Tone::near(RATE, 16_384, 1_000.0, input_rms * 2f64.sqrt());
        let m = measure::run(tone, (RATE / 4.0) as usize, |x| pwr.process(pre.process(x)));
        m.fundamental().magnitude() / 2f64.sqrt()
    };
    println!("\nsensitivity, all controls full, 1 kHz (rated 50 W = 14.1 V rms into 4 ohm):");
    for uv in [10.0, 30.0, 60.0, 120.0, 250.0, 500.0] {
        let v = rms_out(uv * 1e-6);
        println!(
            "  {uv:>6.0} uV in -> {v:>6.2} V rms, {:>6.1} W",
            v * v / 4.0
        );
    }
}
