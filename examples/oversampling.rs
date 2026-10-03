//! What oversampling buys the modelled circuits, and a topology with a power
//! stage behind it, and what it costs.
//!
//! A nonlinear stage makes harmonics, and the ones above half the sample rate
//! fold back down onto frequencies that have nothing to do with the note. This
//! prints that fold-back against what each factor costs, which is the trade
//! `voice::MODELLED_MAX_OVERSAMPLING` is set from: past 2x these circuits cost
//! more than the time there is. A topology with a power stage is capped the
//! same way (`voice::caps_oversampling`), so its rows stop at 2x too.
//!
//! `cargo run --release --example oversampling`
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Gain, PowerAmp, Tone as ToneSection, NOMINAL_DBFS};
use std::time::Instant;

const RATE: f64 = 48_000.0;

fn set_up(gain: Gain, power: PowerAmp, over: usize, drive: f64) -> Chain {
    let mut chain = Chain::new(RATE);
    chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_power_amp(power);
    chain.set_tone_section(ToneSection::Off);
    chain.set_cabinet(Cabinet::Off);
    chain.set_iron(voice::Iron::Off);
    chain.set_oversampling(over);
    chain.set_drive(drive);
    chain.find_operating_point();
    chain
}

fn inharmonic(gain: Gain, power: PowerAmp, over: usize, hz: f64) -> (f64, usize) {
    let mut chain = set_up(gain, power, over, 1.0);
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let tone = Tone::near(RATE, 16_384, hz, amplitude);
    let m = measure::run(tone, (RATE / 4.0) as usize, |x| chain.process(x));
    (m.inharmonic_percent(), chain.effective_oversampling())
}

/// Seconds of CPU per second of audio, one channel.
fn cost(gain: Gain, power: PowerAmp, over: usize) -> f64 {
    let mut chain = set_up(gain, power, over, 0.8);
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
        ("American 5150", Gain::Peavey, PowerAmp::Matched),
        ("Cali IIC+", Gain::Boogie, PowerAmp::Matched),
        ("Cali Rectifier", Gain::Recto, PowerAmp::Matched),
        ("Brit 800", Gain::Brit800, PowerAmp::Matched),
        ("High Gain + 6550", Gain::HighGain, PowerAmp::Svt6550),
        ("Crunch + EL34", Gain::Crunch, PowerAmp::BritEL34),
        (
            "Distortion + 6L6",
            Gain::Distortion,
            PowerAmp::American6L6HighGain,
        ),
    ];
    println!("Inharmonic energy at full drive, % of the fundamental (48 kHz)\n");
    print!("  {:<16}{:>6}", "voice", "over");
    for hz in [220.0, 880.0, 1760.0] {
        print!("{:>10}", format!("{hz:.0} Hz"));
    }
    println!("{:>12}", "cost/ch");
    for (name, gain, power) in voices {
        for over in [1, 2, 4, 8] {
            let (_, active) = inharmonic(gain, power, over, 220.0);
            print!("  {name:<16}{:>6}", format!("{active}x"));
            for hz in [220.0, 880.0, 1760.0] {
                print!("{:>10.1}", inharmonic(gain, power, over, hz).0);
            }
            println!("{:>11.1} %", cost(gain, power, over) * 100.0);
            if active != over {
                println!("    (capped: the panel asked for {over}x and got {active}x)");
                break;
            }
        }
        println!();
    }
}
