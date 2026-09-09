//! What the Mark IIC+'s graphic equaliser actually does.
//!
//! `cargo run --release --example graphic`
use gainstagefx::circuits::markiic;
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

/// Gain at `hz`, in dB, with the sliders where `set` puts them.
fn at(set: &[f64; 5], hz: f64) -> f64 {
    let mut sim = Simulation::new(markiic::graphic(10_000.0, 1_000_000.0).unwrap(), RATE);
    for (i, &v) in set.iter().enumerate() {
        sim.set_control(i, v);
    }
    sim.find_operating_point();
    let n = (RATE * 0.4) as usize;
    let skip = n / 2;
    let (mut si, mut so) = (0.0f64, 0.0f64);
    for k in 0..n {
        let t = k as f64 / RATE;
        let x = (std::f64::consts::TAU * hz * t).sin();
        let y = sim.process(x);
        if k > skip {
            si += x * x;
            so += y * y;
        }
    }
    20.0 * (so / si).sqrt().max(1e-12).log10()
}

fn main() {
    let hzs = [
        40.0, 60.0, 120.0, 240.0, 480.0, 750.0, 1500.0, 2200.0, 4000.0, 6600.0, 10000.0,
    ];
    let flat = [0.5f64; 5];
    println!("  Sliders at centre is the reference. Response relative to it.\n");
    print!("  {:<22}", "hz");
    for hz in hzs {
        print!("{hz:>8.0}");
    }
    println!();
    let base: Vec<f64> = hzs.iter().map(|&h| at(&flat, h)).collect();
    print!("  {:<22}", "flat, absolute dB");
    for b in &base {
        print!("{b:>8.1}");
    }
    println!();
    // Just the two that show the range clearly.
    for (band, name) in [(0usize, "60"), (2, "750")] {
        let _ = name;
        for (pos, label) in [(1.0f64, "up"), (0.0, "down")] {
            let mut set = flat;
            set[band] = pos;
            print!("  {:<22}", format!("{name} Hz {label}"));
            for (i, &hz) in hzs.iter().enumerate() {
                print!("{:>8.1}", at(&set, hz) - base[i]);
            }
            println!();
        }
    }
    println!("\n  A slider's own column should move most, and the others little.");
}
