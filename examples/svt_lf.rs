//! The American 6550's low-frequency behaviour: the closed loop's small-signal
//! response from 2 Hz up, and what each node between the inverter and the
//! output grids does after a burst ends -- which is where a stage either
//! recovers or bounces.
//!
//! `cargo run --release --example svt_lf [primary_henries]` -- the argument
//! replaces the ESTIMATED primary inductance, to see how much it matters.

use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 48_000.0;

fn main() {
    let henries: f64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(PowerSpec::SVT_6550.primary_inductance);
    let spec = PowerSpec {
        primary_inductance: henries,
        ..PowerSpec::SVT_6550
    };
    println!("primary {henries} H");
    print!("closed-loop response, dB re 1 kHz:");
    let level = |hz: f64| {
        let mut s = Simulation::new(power::build(&spec, 1_000.0).unwrap(), RATE);
        s.find_operating_point();
        let t = Tone::near(RATE, 65_536, hz, 0.02);
        measure::run(t, (RATE * 2.0) as usize, |x| s.process(x)).gain_db()
    };
    let ref_db = level(1_000.0);
    for hz in [3.0, 5.0, 8.0, 12.0, 20.0, 40.0, 100.0] {
        print!(" {hz:.0} {:+.1}", level(hz) - ref_db);
    }
    println!();
    let c = power::tap(&spec, 1_000.0, "spk").unwrap();
    let names = ["f_g2", "f_dg1", "f_dp1", "f_fg1", "f_fk1", "og1", "spk"];
    let idx: Vec<usize> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let idle: Vec<f64> = idx.iter().map(|&i| s.voltage_at(i)).collect();
    let peak = 0.257 * 3.0 * 2f64.sqrt();
    let (mut vgk_max, mut og_max, mut dg_max) = (f64::MIN, f64::MIN, f64::MIN);
    for k in 0..(RATE * 0.2) as usize {
        s.process(peak * (std::f64::consts::TAU * 110.0 * k as f64 / RATE).sin());
        vgk_max = vgk_max.max(s.voltage_at(idx[3]) - s.voltage_at(idx[4]));
        og_max = og_max.max(s.voltage_at(idx[5]));
        dg_max = dg_max.max(s.voltage_at(idx[1]));
    }
    println!(
        "during the burst: follower grid-cathode at most {vgk_max:+.2} V, output grid at most {og_max:+.2} V, 12BH7 gain-stage grid at most {dg_max:+.2} V"
    );
    println!("after a 200 ms burst at 3x full, each node against its idle:");
    print!("{:>7}", "ms");
    for n in names {
        print!("{n:>9}");
    }
    println!();
    let mut k = 0;
    for ms in [1.0, 3.0, 6.0, 10.0, 15.0, 20.0, 30.0, 50.0, 80.0, 120.0] {
        while (k as f64) < RATE * ms / 1e3 {
            s.process(0.0);
            k += 1;
        }
        print!("{ms:>7.0}");
        for (j, &i) in idx.iter().enumerate() {
            print!("{:>+9.2}", s.voltage_at(i) - idle[j]);
        }
        println!();
    }
}
