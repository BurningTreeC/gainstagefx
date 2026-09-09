//! What the four typology voices do at the level they are given, and at the
//! level a guitar actually puts out.
//!
//! `cargo run --release --example starved`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::CALIBRATION;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 96_000.0;
/// What a guitar puts out, from `examples/calibrate.rs`.
const GUITAR: f64 = 0.122;

fn sweep(gain: Gain, volts: f64) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();
    for &d in &[0.0f64, 0.25, 0.5, 0.75, 1.0] {
        let net = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        let mut sim = Simulation::new(net, RATE);
        sim.set_control(gain.drive_control(), d);
        let t = Tone::near(RATE, 16_384, 1000.0, volts);
        let m = measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x));
        out.push((d, m.thd_percent(), m.harmonic_percent(3)));
    }
    out
}

fn main() {
    println!("1 kHz. THD %, and the third harmonic beside it.\n");
    for gain in [
        Gain::Crunch,
        Gain::Overdrive,
        Gain::Distortion,
        Gain::HighGain,
    ] {
        let i = voice::voice_index(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        let now = CALIBRATION[i].drive_volts;
        println!(
            "  {} -- given {:.4} V, a guitar is {:.3} V ({:+.1} dB)",
            gain.name(),
            now,
            GUITAR,
            20.0 * (now / GUITAR).log10()
        );
        println!(
            "    {:<10}{:>22}{:>24}",
            "drive", "as it is now", "at a guitar's level"
        );
        let a = sweep(gain, now);
        let b = sweep(gain, GUITAR);
        for ((d, t1, h1), (_, t2, h2)) in a.iter().zip(b.iter()) {
            println!(
                "    {:<10.2}{:>12.1} % ({:>4.1} 3rd){:>14.1} % ({:>4.1} 3rd)",
                d, t1, h1, t2, h2
            );
        }
        println!();
    }

    println!("=== where each control is worth something ===");
    println!("  THD at the ends of the Drive knob, against input level.\n");
    for gain in [Gain::Boogie, Gain::Peavey, Gain::Muff, Gain::Screamer] {
        println!("  {}", gain.name());
        println!(
            "    {:<12}{:>9}{:>9}{:>9}{:>9}{:>9}{:>10}",
            "input", "d=0", "d=0.25", "d=0.5", "d=0.75", "d=1", "spread"
        );
        for volts in [0.002, 0.008, 0.02, 0.04, 0.08, 0.122, 0.25, 0.5] {
            let row = sweep(gain, volts);
            print!("    {:<10.3} V", volts);
            for (_, t, _) in &row {
                print!("{t:>8.1} %");
            }
            println!("{:>9.1} pts", row[4].1 - row[0].1);
        }
        println!();
    }
}
