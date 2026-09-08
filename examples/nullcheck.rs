//! A render of every voice, for comparing one build against another.
//!
//! Optimising a solver means changing the arithmetic without changing the
//! sound, and the only way to know is to subtract one render from the other.
//! This writes raw f64 samples so a later build can be nulled against it.
//!
//! `cargo run --release --example nullcheck -- out.f64`

use gainstagefx::voice::{self, Cabinet, Chain, Gain, Settings, Tone as ToneSection};
use std::io::Write;

const RATE: f64 = 48_000.0;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "null.f64".into());
    let mut out = Vec::new();
    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Neve,
        Gain::Screamer,
        Gain::Muff,
        Gain::Crunch,
        Gain::Distortion,
    ] {
        for drive in [0.2f64, 0.6, 1.0] {
            let mut c = Chain::new(RATE);
            c.apply(&Settings {
                gain,
                drive,
                tone: ToneSection::Scooping,
                cabinet: Cabinet::Stack,
                oversampling: 1,
                ..Settings::default()
            });
            c.settle();
            // A quarter second of plucked notes at full level: attack,
            // sustain and decay, which is where a solver is worked hardest.
            for k in 0..(RATE as usize / 4) {
                let t = k as f64 / RATE;
                let env = (-(t * 8.0)).exp();
                let x = 0.5
                    * env
                    * ((std::f64::consts::TAU * 110.0 * t).sin()
                        + 0.5 * (std::f64::consts::TAU * 330.0 * t).sin());
                out.push(c.process(x));
            }
        }
    }
    let mut f = std::fs::File::create(&path).expect("writes");
    for y in &out {
        f.write_all(&y.to_le_bytes()).expect("writes");
    }
    let rms = (out.iter().map(|y| y * y).sum::<f64>() / out.len() as f64).sqrt();
    eprintln!("wrote {} samples to {path}, rms {rms:.9}", out.len());
    let _ = voice::VOICES;
}
