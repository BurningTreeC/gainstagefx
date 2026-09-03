//! What the plugin costs, as a fraction of the time it has.
//!
//! A plugin gets one second of processor time per second of audio per core,
//! and that budget is shared with the host and everything else in the project.
//! Anything over about a tenth of it for a single instance is unusable.

use gainstagefx::voice::{self, Chain, Iron, Tone as ToneSection, Cabinet, VOICES};

const RATE: f64 = 48_000.0;
const SECONDS: f64 = 2.0;

fn measure(label: &str, mut chain: Chain, moving: bool) {
    let n = (RATE * SECONDS) as usize;
    let start = std::time::Instant::now();
    let mut acc = 0.0;
    for i in 0..n {
        // What the plugin does while a control is being turned. Once a block
        // now, which is where a circuit control belongs -- the plugin used to
        // do this once a sample.
        if moving && i % 128 == 0 {
            chain.set_drive(0.5 + 0.2 * (i as f64 / n as f64));
        }
        let x = 0.1 * (i as f64 * 0.01).sin();
        acc += chain.process(x);
    }
    let took = start.elapsed().as_secs_f64();
    let load = took / SECONDS * 100.0;
    println!(
        "{label:<34}{load:>8.1}% of realtime{}",
        if acc.is_nan() { "  (NaN!)" } else { "" }
    );
}

fn main() {
    println!("one channel at {} kHz, 4x oversampling\n", RATE / 1000.0);
    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice::voice_at(index);
        let mut chain = Chain::new(RATE);
        chain.set_voice(gain, diode, amplifier);
        chain.set_drive(0.7);
        let part = if gain.has_diodes() {
            diode.name()
        } else if gain.has_amplifier() {
            amplifier.name()
        } else {
            "valve"
        };
        measure(&format!("{} / {part}", gain.name()), chain, false);
    }

    println!();
    let mut chain = Chain::new(RATE);
    chain.set_voice(voice::Gain::Crunch, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_iron(Iron::Steel);
    measure("Crunch + iron", chain, false);

    let mut chain = Chain::new(RATE);
    chain.set_voice(voice::Gain::Crunch, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_tone_section(ToneSection::Scooping);
    chain.set_cabinet(Cabinet::Stack);
    measure("Crunch + tone + cabinet", chain, false);

    println!();
    let mut chain = Chain::new(RATE);
    chain.set_voice(voice::Gain::Crunch, voice::Diode::Silicon, voice::Amplifier::Valve);
    measure("Crunch, drive moving", chain, true);
}
