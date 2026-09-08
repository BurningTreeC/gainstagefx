//! How much of what comes out is not the note.
//!
//! A nonlinear stage makes harmonics, and harmonics above half the sample rate
//! fold back down onto frequencies that have nothing to do with what was
//! played. Total harmonic distortion cannot see any of it -- it looks only
//! where harmonics belong -- so a circuit can alias badly and measure exactly
//! the same.
//!
//! It matters most for whichever circuit makes the most harmonics highest up,
//! which here is the 5150: six triodes, one run cold, squaring the wave off
//! completely. And the modelled circuits are pinned to the host rate however
//! the oversampling control is set, so unlike the topologies they have no way
//! to be given more room.
//!
//! Run at 48 kHz, which is the rate a session is actually in.
//!
//! `cargo run --release --example alias`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;

fn measured(gain: Gain, drive: f64, hz: f64, over: usize) -> measure::Measured {
    let mut chain = Chain::new(RATE);
    chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_tone_section(ToneSection::Off);
    chain.set_cabinet(Cabinet::Off);
    chain.set_iron(voice::Iron::Off);
    chain.set_oversampling(over);
    chain.set_drive(drive);
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let tone = Tone::near(RATE, 16_384, hz, amplitude);
    measure::run(tone, (RATE / 4.0) as usize, |x| chain.process(x))
}

fn main() {
    println!("Inharmonic energy -- everything out that is neither the note nor");
    println!("one of its harmonics -- as a percentage of the fundamental.");
    println!("48 kHz, drive at the stop, tone stack and cabinet out.\n");

    let notes = [82.4, 110.0, 220.0, 440.0, 880.0, 1760.0];
    print!("  {:<12}{:>10}", "circuit", "over");
    for hz in notes {
        print!("{:>9}", format!("{hz:.0} Hz"));
    }
    println!();

    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Muff,
        Gain::Screamer,
        Gain::Distortion,
    ] {
        for over in [1usize, 4, 8] {
            let mut chain = Chain::new(RATE);
            chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
            chain.set_oversampling(over);
            let actual = chain.effective_oversampling();
            // A modelled circuit refuses the setting, so printing the request
            // rather than what it did would be a table of fiction.
            print!("  {:<12}{:>10}", gain.name(), format!("{actual}x"));
            for hz in notes {
                print!(
                    "{:>9.1}",
                    measured(gain, 1.0, hz, over).inharmonic_percent()
                );
            }
            println!();
            if actual == 1 && over > 1 {
                // Every row would be identical; say so once and move on.
                println!(
                    "  {:<12}{:>10}  -- pinned to the host rate, so 4x and 8x are the same row",
                    "", ""
                );
                break;
            }
        }
    }

    println!("\nFor comparison, the harmonic total at the same settings.\n");
    print!("  {:<12}{:>10}", "circuit", "over");
    for hz in notes {
        print!("{:>9}", format!("{hz:.0} Hz"));
    }
    println!();
    for gain in [Gain::Peavey, Gain::Boogie, Gain::Muff, Gain::Distortion] {
        let mut chain = Chain::new(RATE);
        chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        chain.set_oversampling(1);
        print!(
            "  {:<12}{:>10}",
            gain.name(),
            format!("{}x", chain.effective_oversampling())
        );
        for hz in notes {
            print!("{:>9.1}", measured(gain, 1.0, hz, 1).thd_percent());
        }
        println!();
    }
}
