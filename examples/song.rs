//! Ten stereo instances, which is what a song looks like.

use gainstagefx::voice::{self, Chain};

const RATE: f64 = 48_000.0;
const SECONDS: f64 = 4.0;

fn main() {
    let factor: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let voices = [
        voice::Gain::Crunch,
        voice::Gain::Clean,
        voice::Gain::HighGain,
        voice::Gain::Overdrive,
        voice::Gain::Distortion,
    ];
    // Ten instances, two channels each.
    let mut chains: Vec<Chain> = (0..20)
        .map(|i| {
            let mut c = Chain::new(RATE);
            c.set_voice(voices[(i / 2) % voices.len()], voice::Diode::Silicon, voice::Amplifier::Valve);
            // What the presets now ask for: the pedals need four times, the
            // valve cascades do not.
            let per_voice = match voices[(i / 2) % voices.len()] {
                voice::Gain::Overdrive | voice::Gain::Distortion => 4,
                _ => 2,
            };
            c.set_oversampling(if factor == 0 { per_voice } else { factor });
            c.set_drive(0.7);
            c.settle();
            c
        })
        .collect();

    let n = (RATE * SECONDS) as usize;
    let start = std::time::Instant::now();
    let mut acc = 0.0;
    for i in 0..n {
        let x = 0.1 * (i as f64 * 0.01).sin();
        for c in &mut chains {
            acc += c.process(x);
        }
    }
    let took = start.elapsed().as_secs_f64();
    println!(
        "{}: ten stereo instances took {took:.2}s of audio time {SECONDS}s \
         = {:.0}% of one core ({:.1} cores){}",
        if factor == 0 { String::from("as the presets ask") } else { format!("{factor}x") },
        took / SECONDS * 100.0,
        took / SECONDS,
        if acc.is_nan() { " NaN" } else { "" }
    );
}
