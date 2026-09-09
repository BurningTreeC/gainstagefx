//! What `IRON_VOLTS` should be, measured on the iron alone.
//!
//! Flux goes as `V / f`, so a transformer works on the bottom of the band and
//! nowhere else. The question is not whether to make it act at 220 Hz -- no
//! transformer does -- but whether the knee lands inside the instrument's
//! range or below it. It was set at 40 Hz, which is below a guitar's low E.
//!
//! Measured on the iron by itself, so the gain circuit's own distortion is not
//! in the way.
//!
//! `cargo run --release --example ironvolts`

use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{build_iron, Iron, IRON_VOLTS};

const RATE: f64 = 96_000.0;
const N: usize = 1 << 14;
/// What the iron is handed at a nominal signal, before any multiplier.
const NOMINAL: f64 = 0.126;

fn thd(iron: Iron, hz: f64, volts: f64) -> f64 {
    let mut sim = Simulation::new(build_iron(iron).expect("builds"), RATE);
    sim.find_operating_point();
    let warm = (RATE / 8.0) as usize;
    for k in 0..warm {
        sim.process(volts * (std::f64::consts::TAU * hz * k as f64 / RATE).sin());
    }
    let mut out = Vec::with_capacity(N);
    for k in warm..(warm + N) {
        out.push(sim.process(volts * (std::f64::consts::TAU * hz * k as f64 / RATE).sin()));
    }
    let bin = |m: f64| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let w = std::f64::consts::TAU * m * hz / RATE;
        for (i, &v) in out.iter().enumerate() {
            re += v * (w * i as f64).cos();
            im -= v * (w * i as f64).sin();
        }
        (re * re + im * im).sqrt()
    };
    let f = bin(1.0);
    let h: f64 = (2..=8).map(|m| bin(m as f64).powi(2)).sum::<f64>().sqrt();
    100.0 * h / f.max(1e-30)
}

fn main() {
    println!("The iron alone, nominal signal, IRON_VOLTS times a multiplier.");
    println!("Today's IRON_VOLTS is {IRON_VOLTS}, set so the knee lands at 40 Hz.\n");
    println!(
        "  {:<7}{:>8}{:>8}{:>9}{:>9}{:>9}{:>10}",
        "mult", "volts", "Hz", "nickel", "steel", "amorph", "spread"
    );
    for mult in [1.0f64, 2.0, 3.0, 4.0, 6.0] {
        let volts = NOMINAL * IRON_VOLTS * mult;
        for hz in [40.0f64, 82.0, 110.0, 220.0] {
            let row: Vec<f64> = [Iron::Nickel, Iron::Steel, Iron::Amorphous]
                .into_iter()
                .map(|i| thd(i, hz, volts))
                .collect();
            let spread = row.iter().cloned().fold(f64::MIN, f64::max)
                - row.iter().cloned().fold(f64::MAX, f64::min);
            println!(
                "  {mult:<7.1}{volts:>8.2}{hz:>8.0}{:>8.1} %{:>8.1} %{:>8.1} %{spread:>8.1} pts",
                row[0], row[1], row[2]
            );
        }
        println!();
    }
    println!("Wanted: a clear spread at 82 and 110 Hz -- a guitar's bottom two");
    println!("strings -- without 40 Hz turning to mush.");
}
