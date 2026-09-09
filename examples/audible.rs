//! Where each voice's output energy actually is.
//!
//! "The meter moves but I hear nothing" has a short list of causes and the
//! first is DC: a peak meter reads a constant offset as level, and a constant
//! offset is inaudible. Sub-audio and ultrasonic content do the same thing to
//! a lesser degree. This measures all three, per voice, through the same
//! `Chain` the plugin uses.
//!
//! `cargo run --release --example audible`

use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};
use std::cell::RefCell;
thread_local! {
    static TONE: RefCell<ToneSection> = const { RefCell::new(ToneSection::Off) };
    static CAB: RefCell<Cabinet> = const { RefCell::new(Cabinet::Off) };
}

const RATE: f64 = 48_000.0;
const N: usize = 1 << 15;

/// Energy in a band, by a plain DFT over the band's bins.
fn band(x: &[f64], lo: f64, hi: f64) -> f64 {
    let n = x.len();
    let from = ((lo / RATE) * n as f64).round().max(0.0) as usize;
    let to = (((hi / RATE) * n as f64).round() as usize).min(n / 2);
    let mut total = 0.0;
    for k in from..=to {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let w = std::f64::consts::TAU * k as f64 / n as f64;
        for (i, &v) in x.iter().enumerate() {
            re += v * (w * i as f64).cos();
            im -= v * (w * i as f64).sin();
        }
        total += re * re + im * im;
    }
    total
}

fn main() {
    for (label, tone, cab) in [
        ("tone and cabinet OUT", ToneSection::Off, Cabinet::Off),
        ("tone and cabinet IN", ToneSection::Scooping, Cabinet::Stack),
    ] {
        TONE.with(|t| *t.borrow_mut() = tone);
        CAB.with(|c| *c.borrow_mut() = cab);
        println!("=== {label} ===");
        println!(
            "  {:<12}{:>10}{:>10}{:>10}{:>12}{:>11}{:>11}",
            "voice", "peak", "rms", "DC", "DC vs peak", "< 20 Hz", "20-20k"
        );
        for gain in [
            Gain::Clean,
            Gain::Crunch,
            Gain::HighGain,
            Gain::Overdrive,
            Gain::Distortion,
            Gain::Screamer,
            Gain::Muff,
            Gain::Boogie,
            Gain::Peavey,
            Gain::Neve,
        ] {
            let mut c = Chain::new(RATE);
            c.apply(&Settings {
                gain,
                drive: 0.7,
                tone: TONE.with(|t| *t.borrow()),
                cabinet: CAB.with(|c| *c.borrow()),
                oversampling: 1,
                ..Settings::default()
            });
            c.settle();
            c.find_operating_point();
            let mut out = Vec::with_capacity(N);
            for k in 0..(N + 4096) {
                let t = k as f64 / RATE;
                let y = c.process(0.5 * (std::f64::consts::TAU * 220.0 * t).sin());
                if k >= 4096 {
                    out.push(y);
                }
            }
            let peak = out.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
            let dc = out.iter().sum::<f64>() / out.len() as f64;
            let rms = (out.iter().map(|v| v * v).sum::<f64>() / out.len() as f64).sqrt();
            let sub = band(&out, 0.0, 20.0);
            let audible = band(&out, 20.0, 20_000.0);
            let all = sub + audible + band(&out, 20_000.0, RATE / 2.0);
            println!(
                "  {:<12}{:>10.4}{:>10.4}{:>10.4}{:>11.1} %{:>10.1} %{:>10.1} %",
                gain.name(),
                peak,
                rms,
                dc,
                if peak > 0.0 {
                    100.0 * dc.abs() / peak
                } else {
                    0.0
                },
                if all > 0.0 { 100.0 * sub / all } else { 0.0 },
                if all > 0.0 {
                    100.0 * audible / all
                } else {
                    0.0
                },
            );
        }
        println!();
    }
}
