//! The Mark IIC+ preamp, measured stage by stage.
use gainstagefx::circuits::markiic::{self, BASS, LEAD_DRIVE, MIDDLE, TREBLE, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

/// The five panel controls, so the probe takes a panel rather than a list.
#[derive(Clone, Copy)]
struct Panel { t: f64, b: f64, m: f64, vol: f64, ld: f64 }

fn at(node: &str, hz: f64, volts: f64, p: Panel) -> measure::Measured {
    let (t, b, m, vol, ld) = (p.t, p.b, p.m, p.vol, p.ld);
    let c = markiic::tap(10_000.0, 1_000_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(TREBLE, t);
    sim.set_control(BASS, b);
    sim.set_control(MIDDLE, m);
    sim.set_control(VOLUME, vol);
    sim.set_control(LEAD_DRIVE, ld);
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 2.0) as usize, |x| sim.process(x))
}

fn main() {
    println!("=== gain through the chain, small signal, controls at noon ===");
    for node in ["v1a_p", "ts_out", "v1b_p", "v3b_p", "v4a_p", "lead_out"] {
        println!("  {node:<10}{:>8.1} dB", at(node, 1000.0, 1e-5, Panel { t: 0.5, b: 0.5, m: 0.5, vol: 0.5, ld: 0.5 }).gain_db());
    }

    println!("\n=== the tone stack, measured across it (mid at zero: the scoop) ===");
    print!("{:<22}", "");
    for f in [60.0, 120.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0] { print!("{f:>8.0}"); }
    println!();
    for (name, t, b, m) in [
        ("mid down (scooped)", 0.7, 0.7, 0.0),
        ("all at noon", 0.5, 0.5, 0.5),
        ("mid up", 0.5, 0.5, 1.0),
    ] {
        print!("{name:<22}");
        let mut vals = Vec::new();
        for f in [60.0, 120.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0] {
            vals.push(at("ts_out", f, 1e-5, Panel { t, b, m, vol: 0.5, ld: 0.5 }).gain_db());
        }
        let peak = vals.iter().cloned().fold(f64::MIN, f64::max);
        for v in &vals { print!("{:>8.1}", v - peak); }
        println!();
    }

    println!("\n=== lead drive, at the output ===");
    for ld in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let m = at("lead_out", 1000.0, 0.0003, Panel { t: 0.6, b: 0.6, m: 0.2, vol: 0.7, ld });
        println!("  drive {ld:.2}: {:>7.1} dB, {:>5.1} % distortion, 2nd {:>4.1} 3rd {:>4.1}",
                 m.gain_db(), m.thd_percent(), m.harmonic_percent(2), m.harmonic_percent(3));
    }
}
