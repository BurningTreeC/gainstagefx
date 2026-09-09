//! What the two amplifiers' drive controls actually change.
//!
//! Total harmonic distortion stops saying anything useful once a circuit is
//! squared off -- it then measures what has happened to the fundamental. The
//! claim on record is that the *spectrum* keeps moving after the total stops.
//! That is testable, and it decides whether the control is doing something or
//! nothing.
//!
//! `cargo run --release --example amps`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 96_000.0;
const GUITAR: f64 = 0.122;

fn main() {
    println!("110 Hz at a guitar's level, drive swept. Harmonics as a share");
    println!("of the fundamental, and the centroid of the harmonic energy.\n");
    for gain in [Gain::Boogie, Gain::Peavey] {
        println!("  {}", gain.name());
        println!(
            "    {:<8}{:>9}{:>8}{:>8}{:>8}{:>8}{:>8}{:>11}",
            "drive", "THD", "2nd", "3rd", "4th", "5th", "7th", "centroid"
        );
        for d in [0.0f64, 0.05, 0.1, 0.2, 0.35, 0.5, 0.65, 0.8, 1.0] {
            let net = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
                .expect("builds");
            let mut sim = Simulation::new(net, RATE);
            sim.set_control(gain.drive_control(), d);
            let t = Tone::near(RATE, 16_384, 110.0, GUITAR);
            let m = measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x));
            // Where the harmonic energy sits, in harmonic number: sum(n*a_n^2)
            // over sum(a_n^2). A circuit whose knob is doing nothing has a
            // centroid that does not move.
            let (mut num, mut den) = (0.0f64, 0.0f64);
            for n in 2..=12 {
                let a = m.harmonic_percent(n);
                num += n as f64 * a * a;
                den += a * a;
            }
            println!(
                "    {:<8.2}{:>8.1} %{:>7.1}{:>8.1}{:>8.1}{:>8.1}{:>8.1}{:>11.2}",
                d,
                m.thd_percent(),
                m.harmonic_percent(2),
                m.harmonic_percent(3),
                m.harmonic_percent(4),
                m.harmonic_percent(5),
                m.harmonic_percent(7),
                if den > 0.0 { num / den } else { 0.0 }
            );
        }
        println!();
    }
}
