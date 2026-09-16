//! What oversampling buys the modelled circuits, and what it costs.
//!
//! A nonlinear stage makes harmonics, and the ones above half the sample rate
//! fold back down onto frequencies that have nothing to do with the note. This
//! prints that fold-back against what each factor costs, which is the trade
//! `voice::MODELLED_MAX_OVERSAMPLING` is set from: past 2x these circuits cost
//! more than the time there is.
//!
//! `cargo run --release --example oversampling`
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection, NOMINAL_DBFS};
use std::time::Instant;

const RATE: f64 = 48_000.0;

fn set_up(gain: Gain, over: usize, drive: f64) -> Chain {
    let mut chain = Chain::new(RATE);
    chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_tone_section(ToneSection::Off);
    chain.set_cabinet(Cabinet::Off);
    chain.set_iron(voice::Iron::Off);
    chain.set_oversampling(over);
    chain.set_drive(drive);
    chain.find_operating_point();
    chain
}

fn inharmonic(gain: Gain, over: usize, hz: f64) -> (f64, usize) {
    let mut chain = set_up(gain, over, 1.0);
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let tone = Tone::near(RATE, 16_384, hz, amplitude);
    let m = measure::run(tone, (RATE / 4.0) as usize, |x| chain.process(x));
    (m.inharmonic_percent(), chain.effective_oversampling())
}

/// Seconds of CPU per second of audio, one channel.
fn cost(gain: Gain, over: usize) -> f64 {
    let mut chain = set_up(gain, over, 0.8);
    let n = RATE as usize;
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    for k in 0..n / 10 {
        chain.process(amplitude * (k as f64 * 0.0144).sin());
    }
    let start = Instant::now();
    for k in 0..n {
        let t = k as f64 / RATE;
        chain.process(
            amplitude
                * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7
                    + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3),
        );
    }
    start.elapsed().as_secs_f64()
}

fn main() {
    let voices = [
        ("American 5150", Gain::Peavey),
        ("Cali IIC+", Gain::Boogie),
        ("Cali Rectifier", Gain::Recto),
        ("Brit 800", Gain::Brit800),
    ];
    println!("Inharmonic energy at full drive, % of the fundamental (48 kHz)\n");
    print!("  {:<16}{:>6}", "voice", "over");
    for hz in [220.0, 880.0, 1760.0] {
        print!("{:>10}", format!("{hz:.0} Hz"));
    }
    println!("{:>12}", "cost/ch");
    for (name, gain) in voices {
        for over in [1, 2, 4, 8] {
            let (_, active) = inharmonic(gain, over, 220.0);
            print!("  {name:<16}{:>6}", format!("{active}x"));
            for hz in [220.0, 880.0, 1760.0] {
                print!("{:>10.1}", inharmonic(gain, over, hz).0);
            }
            println!("{:>11.1} %", cost(gain, over) * 100.0);
            if active != over {
                println!("    (capped: the panel asked for {over}x and got {active}x)");
                break;
            }
        }
        println!();
    }
}
