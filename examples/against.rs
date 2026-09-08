//! The 5150 against the Big Muff, through everything a player hears.
//!
//! `examples/dirt.rs` measures the circuit with the tone stack, the cabinet
//! and the iron switched out, which is the right way to ask what a circuit
//! does and the wrong way to ask what the plugin sounds like. This runs the
//! whole chain, both circuits, at several playing levels, and prints the
//! orders separately -- because "how distorted does it sound" is not one
//! number, and a wave whose harmonics run to the tenth sounds nothing like a
//! wave with the same total sitting in its third.
//!
//! `cargo run --release --example against`

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

struct Setup {
    tone: ToneSection,
    cabinet: Cabinet,
    iron: voice::Iron,
}

const BARE: Setup = Setup {
    tone: ToneSection::Off,
    cabinet: Cabinet::Off,
    iron: voice::Iron::Off,
};
const PLAYED: Setup = Setup {
    tone: ToneSection::Scooping,
    cabinet: Cabinet::Stack,
    iron: voice::Iron::Off,
};

fn measured(gain: Gain, drive: f64, level_db: f64, s: &Setup, hz: f64) -> measure::Measured {
    let mut chain = Chain::new(RATE);
    chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_tone_section(s.tone);
    chain.set_cabinet(s.cabinet);
    chain.set_iron(s.iron);
    chain.set_oversampling(1);
    chain.set_drive(drive);
    let amplitude = 10f64.powf(level_db / 20.0);
    let tone = Tone::near(RATE, 16_384, hz, amplitude);
    measure::run(tone, (RATE / 4.0) as usize, |x| chain.process(x))
}

const WHO: [Gain; 4] = [Gain::Peavey, Gain::Muff, Gain::Boogie, Gain::HighGain];
const DRIVES: [f64; 7] = [0.0, 0.25, 0.5, 0.625, 0.75, 0.875, 1.0];

fn table(title: &str, s: &Setup, level_db: f64, hz: f64, what: impl Fn(&measure::Measured) -> f64) {
    println!("\n{title}");
    print!("  {:<12}", "circuit");
    for d in DRIVES {
        print!("{:>8}", format!("{:.0}%", d * 100.0));
    }
    println!();
    for gain in WHO {
        print!("  {:<12}", gain.name());
        for d in DRIVES {
            print!("{:>8.1}", what(&measured(gain, d, level_db, s, hz)));
        }
        println!();
    }
}

fn main() {
    println!("=== the circuit alone: no stack, no cabinet, no iron ===");
    table(
        "Total harmonic distortion, nominal signal, 220 Hz.",
        &BARE,
        NOMINAL_DBFS,
        220.0,
        |m| m.thd_percent(),
    );

    println!("\n\n=== the whole chain: scooping stack, stack cabinet ===");
    table(
        "Total harmonic distortion, nominal signal, 220 Hz.",
        &PLAYED,
        NOMINAL_DBFS,
        220.0,
        |m| m.thd_percent(),
    );
    table(
        "Second harmonic, which is the one that sounds like warmth.",
        &PLAYED,
        NOMINAL_DBFS,
        220.0,
        |m| m.harmonic_percent(2),
    );
    table(
        "Third harmonic, which is what squaring off sounds like.",
        &PLAYED,
        NOMINAL_DBFS,
        220.0,
        |m| m.harmonic_percent(3),
    );
    table(
        "Fifth harmonic, which is where a sound starts to buzz.",
        &PLAYED,
        NOMINAL_DBFS,
        220.0,
        |m| m.harmonic_percent(5),
    );

    println!("\n\n=== the whole chain, played quietly: -12 dB under nominal ===");
    table(
        "Total harmonic distortion, 220 Hz.",
        &PLAYED,
        NOMINAL_DBFS - 12.0,
        220.0,
        |m| m.thd_percent(),
    );

    println!("\n\n=== the whole chain, up where a chord lives: 660 Hz ===");
    table(
        "Total harmonic distortion, nominal signal.",
        &PLAYED,
        NOMINAL_DBFS,
        660.0,
        |m| m.thd_percent(),
    );
}
