//! How dirty each circuit is, at the knob positions a player actually uses.
//!
//! `examples/calibrate.rs` sets each voice's input level so that it does what
//! its *name* says with the drive control at the stop. That says nothing about
//! the rest of the travel, and the rest of the travel is where a knob spends
//! its life. This walks the Drive control from shut to open with a nominal
//! signal in front of it and reports what comes out.
//!
//! Reported at the chain's output, after the make-up, because that is what a
//! player hears. Tone stack and cabinet out of circuit so the figure is the
//! circuit's own.
//!
//! `cargo run --release --example dirt`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

/// The circuits `CLAUDE.md` names as under-calibrated, and the two references
/// to read them against: a Big Muff, which the project treats as the standard
/// for strong nonlinear behaviour, and a High Gain topology.
const LOOK_AT: [Gain; 6] = [
    Gain::Screamer,
    Gain::Boogie,
    Gain::Peavey,
    Gain::Muff,
    Gain::HighGain,
    Gain::Distortion,
];

fn measured(gain: Gain, drive: f64, level_db: f64) -> measure::Measured {
    let mut chain = Chain::new(RATE);
    chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_tone_section(ToneSection::Off);
    chain.set_cabinet(Cabinet::Off);
    chain.set_iron(voice::Iron::Off);
    chain.set_oversampling(1);
    chain.set_drive(drive);
    let amplitude = 10f64.powf(level_db / 20.0);
    let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
    measure::run(tone, (RATE / 4.0) as usize, |x| chain.process(x))
}

fn main() {
    let drives = [0.0, 0.25, 0.5, 0.625, 0.75, 0.875, 1.0];

    println!("Total harmonic distortion at the chain's output, nominal signal in.\n");
    print!("  {:<12}", "circuit");
    for d in drives {
        print!("{:>8}", format!("{:.0}%", d * 100.0));
    }
    println!();
    for gain in LOOK_AT {
        print!("  {:<12}", gain.name());
        for d in drives {
            print!("{:>8.1}", measured(gain, d, NOMINAL_DBFS).thd_percent());
        }
        println!();
    }

    println!("\nThird harmonic, which is what squaring off sounds like.\n");
    print!("  {:<12}", "circuit");
    for d in drives {
        print!("{:>8}", format!("{:.0}%", d * 100.0));
    }
    println!();
    for gain in LOOK_AT {
        print!("  {:<12}", gain.name());
        for d in drives {
            print!(
                "{:>8.1}",
                measured(gain, d, NOMINAL_DBFS).harmonic_percent(3)
            );
        }
        println!();
    }

    // What the host's meter does when the Drive knob moves.
    //
    // The make-up curve is measured at one input level, so it cancels the
    // circuit's gain exactly at that level and nowhere else. Where the circuit
    // saturates, more input buys no more output, and a make-up that still cuts
    // by the linear figure takes the level down with it. Reported because it
    // was reported: "setting Gain to 100% the meter on the track goes DOWN,
    // going to 0% Gain the meter goes way UP".
    println!("\nOutput level across the Drive control, at three playing levels.\n");
    print!("  {:<12}{:<10}", "circuit", "level");
    for d in drives {
        print!("{:>8}", format!("{:.0}%", d * 100.0));
    }
    println!();
    for gain in [Gain::Screamer, Gain::Boogie, Gain::Peavey, Gain::Muff] {
        for offset in [-12.0f64, 0.0, 12.0] {
            print!("  {:<12}{:<10}", gain.name(), format!("{offset:+.0} dB"));
            for d in drives {
                let m = measured(gain, d, NOMINAL_DBFS + offset);
                print!("{:>8.1}", m.gain_db() + NOMINAL_DBFS + offset);
            }
            println!();
        }
    }

    println!("\nAnd against input level, with the drive control at the stop.\n");
    let levels = [
        -30.0,
        -24.0,
        NOMINAL_DBFS,
        -12.0,
        -6.0,
        0.0,
        6.0,
        12.0,
        18.0,
    ];
    print!("  {:<12}", "circuit");
    for l in levels {
        print!("{:>9}", format!("{l:.0} dB"));
    }
    println!();
    for gain in LOOK_AT {
        print!("  {:<12}", gain.name());
        for l in levels {
            print!("{:>9.1}", measured(gain, 1.0, l).thd_percent());
        }
        println!();
    }
}
