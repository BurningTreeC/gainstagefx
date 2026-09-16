//! What comes out of the plugin, per voice, across the Drive control.
//!
//! The make-up is supposed to hold the output level while Drive changes the
//! character. If it does not, some voices are loud, some are quiet and some
//! seem to do nothing -- which is what a player hears before anything else.
//!
//! `cargo run --release --example loudness`

use gainstagefx::voice::{Cabinet, Chain, Gain, Iron, Settings, Tone as ToneSection, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;

fn level(gain: Gain, drive: f64) -> (f64, f64) {
    let mut c = Chain::new(RATE);
    c.apply(&Settings {
        gain,
        drive,
        tone: ToneSection::Scooping,
        cabinet: Cabinet::Stack,
        iron: Iron::Steel,
        oversampling: 4,
        ..Settings::default()
    });
    c.settle();
    c.find_operating_point();
    let amp = 10f64.powf(NOMINAL_DBFS / 20.0) * std::f64::consts::SQRT_2;
    let mut peak = 0.0f64;
    let mut sum = 0.0f64;
    let n = RATE as usize / 2;
    // A low chord, not one tone. The make-up normalises the level of the whole
    // output, so measuring it with a single sine reads a shaped circuit
    // wherever that one frequency happens to fall -- which for the Boss HM-2
    // was in the trough between its two bands, twenty decibels from the truth.
    let partials = [(82.4, 0.5), (123.5, 0.3), (246.9, 0.2)];
    for k in 0..(n + 4096) {
        let t = k as f64 / RATE;
        let x: f64 = partials
            .iter()
            .map(|(hz, a)| a * (std::f64::consts::TAU * hz * t).sin())
            .sum();
        let y = c.process(amp * x);
        if k >= 4096 {
            peak = peak.max(y.abs());
            sum += y * y;
        }
    }
    (peak, (sum / n as f64).sqrt())
}

fn main() {
    println!("A low chord at nominal (-18 dBFS RMS) in, tone + cabinet + iron on, 4x.");
    println!("Output RMS in dBFS. A working make-up holds this roughly level.\n");
    print!("  {:<12}", "voice");
    for d in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
        print!("{d:>10.2}");
    }
    println!("{:>12}", "spread");
    // The whole list, so a voice cannot be added without its level being seen.
    for gain in Gain::ALL {
        print!("  {:<12}", gain.name());
        let mut db = Vec::new();
        for d in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
            let (_, rms) = level(gain, d);
            let v = 20.0 * rms.max(1e-9).log10();
            db.push(v);
            print!("{v:>10.1}");
        }
        let hi = db.iter().cloned().fold(f64::MIN, f64::max);
        let lo = db.iter().cloned().fold(f64::MAX, f64::min);
        println!("{:>10.1} dB", hi - lo);
    }
}
