//! What a callback is actually made of, stage by stage.
//!
//! `examples/split.rs` measures the three nonlinear solves and adds up to
//! 60.7 % of realtime for the 5150; `examples/deadline.rs` measures the whole
//! callback and reports 107 %. Both are right, and the gap between them is
//! forty-six per cent of a callback that nothing was watching.
//!
//! So this walks the chain: it runs the same voice with parts of it switched
//! off and reports the difference each part makes. Differences, not absolute
//! costs, because the parts are not independent -- the cabinet after a power
//! stage sees a different signal from the cabinet after a preamplifier.
//!
//! `cargo run --release --example breakdown`

use gainstagefx::voice::{self, Cabinet, Chain, Gain, Iron, Tone as ToneSection};

const RATE: f64 = 48_000.0;
const SECONDS: usize = 1;

fn stimulus(k: usize) -> f64 {
    let t = k as f64 / RATE;
    let env = (-((t % 0.5) * 6.0)).exp();
    0.9 * env
        * ((std::f64::consts::TAU * 110.0 * t).sin()
            + 0.6 * (std::f64::consts::TAU * 330.0 * t).sin()
            + 0.3 * (std::f64::consts::TAU * 1757.0 * t).sin())
}

/// Percent of realtime for one second of audio.
fn cost(gain: Gain, tone: ToneSection, cab: Cabinet, iron: Iron, over: usize) -> f64 {
    let mut c = Chain::new(RATE);
    c.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    c.set_tone_section(tone);
    c.set_cabinet(cab);
    c.set_iron(iron);
    c.set_oversampling(over);
    c.set_drive(0.8);
    c.settle();
    c.find_operating_point();
    for k in 0..4000 {
        std::hint::black_box(c.process(stimulus(k)));
    }
    let n = RATE as usize * SECONDS;
    let start = std::time::Instant::now();
    for k in 4000..(4000 + n) {
        std::hint::black_box(c.process(stimulus(k)));
    }
    100.0 * start.elapsed().as_secs_f64() / SECONDS as f64
}

fn main() {
    println!("One second of audio, one chain, drive 0.8, 48 kHz.");
    println!("Each column is the cost of the whole chain with that much of it on.\n");
    println!(
        "  {:<12}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "voice", "bare", "+iron", "+tone", "+cab", "= all"
    );
    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Neve,
        Gain::Twin,
        Gain::Muff,
        Gain::Crunch,
    ] {
        let bare = cost(gain, ToneSection::Off, Cabinet::Off, Iron::Off, 1);
        let iron = cost(gain, ToneSection::Off, Cabinet::Off, Iron::Steel, 1);
        let tone = cost(gain, ToneSection::Scooping, Cabinet::Off, Iron::Steel, 1);
        let cab = cost(gain, ToneSection::Scooping, Cabinet::Stack, Iron::Steel, 1);
        let all = cost(gain, ToneSection::Scooping, Cabinet::Stack, Iron::Steel, 4);
        println!(
            "  {:<12}{:>9.1}%{:>9.1}%{:>9.1}%{:>9.1}%{:>9.1}%",
            gain.name(),
            bare,
            iron,
            tone,
            cab,
            all
        );
    }
    println!("\nThe last column adds the oversampling control at 4x. A circuit");
    println!("modelled voice is pinned to 1x, so it should change nothing.");
}
