//! The TS808, measured against what a Tube Screamer is known to do.
use gainstagefx::circuits::ts808::{self, DRIVE, LEVEL, TONE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

fn at(hz: f64, volts: f64, drive: f64, tone: f64) -> measure::Measured {
    let c = ts808::build(10_000.0, 470_000.0).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(DRIVE, drive);
    sim.set_control(TONE, tone);
    sim.set_control(LEVEL, 1.0);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

fn main() {
    println!("=== the mid hump: small-signal gain across the band, drive full ===");
    print!("{:<10}", "Hz");
    for f in [40.0, 80.0, 160.0, 320.0, 720.0, 1500.0, 3000.0, 6000.0] { print!("{f:>9.0}"); }
    println!();
    for (name, tone) in [("tone 0", 0.0), ("tone 0.5", 0.5), ("tone 1", 1.0)] {
        print!("{name:<10}");
        for f in [40.0, 80.0, 160.0, 320.0, 720.0, 1500.0, 3000.0, 6000.0] {
            print!("{:>9.1}", at(f, 0.0005, 1.0, tone).gain_db());
        }
        println!();
    }

    println!("\n=== drive control, at 1 kHz ===");
    for d in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let m = at(1000.0, 0.05, d, 0.5);
        println!("  drive {d:.2}: {:>6.1} dB, {:>5.1} % distortion, 2nd {:>4.1} % 3rd {:>4.1} %",
                 m.gain_db(), m.thd_percent(), m.harmonic_percent(2), m.harmonic_percent(3));
    }

    println!("\n=== it cleans up when you play softer ===");
    for v in [0.005, 0.02, 0.05, 0.15, 0.4] {
        let m = at(1000.0, v, 1.0, 0.5);
        println!("  {v:>5} V in: {:>5.1} % distortion", m.thd_percent());
    }
}
