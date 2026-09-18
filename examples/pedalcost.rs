//! What each pedal costs, alone and in front of an amplifier.
//!
//! A chain that does not fit inside its deadline is a DAW crackling and
//! dropping out, which is what this is for. HM-2 now partitions its verified
//! linear Colour Mix op-amps out of Newton's boundary. MT-2 deliberately keeps
//! its post-distortion op-amps rail-aware until stage-level partitioning is
//! proven equivalent. The existing 1x safety cap therefore remains for both.
//!
//! `cargo run --release --example pedalcost`
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone, NOMINAL_DBFS};
use std::time::Instant;

const RATE: f64 = 48_000.0;

fn cost(pedal: Pedal, gain: Gain, over: usize) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&Settings {
        gain,
        drive: 0.8,
        tone: Tone::Off,
        oversampling: over,
        pedal: PedalSettings {
            pedal,
            drive: 0.7,
            level: 0.5,
            ..PedalSettings::default()
        },
        ..Settings::default()
    });
    chain.settle();
    chain.find_operating_point();
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    for k in 0..4_800 {
        chain.process(amplitude * (k as f64 * 0.0144).sin());
    }
    let n = RATE as usize;
    let start = Instant::now();
    for k in 0..n {
        let t = k as f64 / RATE;
        chain.process(
            amplitude
                * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.6
                    + (std::f64::consts::TAU * 123.5 * t).sin() * 0.4),
        );
    }
    start.elapsed().as_secs_f64() * 100.0
}

fn main() {
    println!("per cent of one channel's realtime budget at 48 kHz\n");
    println!(
        "{:<14}{:>12}{:>16}{:>16}",
        "pedal", "into Clean", "Cali IIC+ 1x", "Cali IIC+ 2x"
    );
    println!(
        "{:<14}{:>12.1}{:>16.1}{:>16.1}",
        "none",
        cost(Pedal::None, Gain::Clean, 1),
        cost(Pedal::None, Gain::Boogie, 1),
        cost(Pedal::None, Gain::Boogie, 2)
    );
    for pedal in Pedal::ALL.iter().copied().filter(|p| *p != Pedal::None) {
        println!(
            "{:<14}{:>12.1}{:>16.1}{:>16.1}",
            format!("{pedal:?}"),
            cost(pedal, Gain::Clean, 1),
            cost(pedal, Gain::Boogie, 1),
            cost(pedal, Gain::Boogie, 2)
        );
    }
}
