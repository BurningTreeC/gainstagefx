//! What the distortion voices do across the band, rather than how much.

use gainstagefx::circuits::tone;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Cabinet, Chain, Tone as ToneSection, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;
const BANDS: [f64; 8] = [60.0, 100.0, 160.0, 250.0, 400.0, 700.0, 1500.0, 3000.0];

fn main() {
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);

    println!("=== small signal gain, circuit only: is the bottom amplified at all? ===");
    print!("{:<22}", "");
    for f in BANDS { print!("{:>8.0}", f); }
    println!();
    for p in PRESETS.iter().filter(|p| {
        matches!(p.name, "Classic Distortion" | "Scooped Pedal" | "Green Overdrive" | "Scooped Metal")
    }) {
        print!("{:<22}", p.name);
        for f in BANDS {
            let mut c = Chain::new(RATE);
            c.set_voice(p.circuit.voice(), p.diode.voice(), p.amplifier.voice());
            c.set_tone_section(ToneSection::Off);
            c.set_cabinet(Cabinet::Off);
            c.set_oversampling(p.oversampling.factor());
            c.set_drive(p.drive as f64);
            c.settle();
            let t = Tone::near(RATE, 16_384, f, amplitude * 0.01);
            print!("{:>8.1}", measure::run(t, (RATE / 2.0) as usize, |x| c.process(x)).gain_db());
        }
        println!();
    }

    println!("\n=== distortion by frequency, at playing level ===");
    print!("{:<22}", "");
    for f in BANDS { print!("{:>8.0}", f); }
    println!();
    for p in PRESETS.iter().filter(|p| {
        matches!(p.name, "Classic Distortion" | "Scooped Pedal" | "Green Overdrive" | "Scooped Metal")
    }) {
        print!("{:<22}", p.name);
        for f in BANDS {
            let mut c = Chain::new(RATE);
            c.set_voice(p.circuit.voice(), p.diode.voice(), p.amplifier.voice());
            c.set_iron(p.iron.voice());
            c.set_tone_section(p.tone.voice());
            c.set_cabinet(p.cabinet.voice());
            c.set_oversampling(p.oversampling.factor());
            c.set_drive(p.drive as f64);
            c.set_tone(tone::BASS, p.bass as f64);
            c.set_tone(tone::MID, p.mid as f64);
            c.set_tone(tone::TREBLE, p.treble as f64);
            c.settle();
            let t = Tone::near(RATE, 16_384, f, amplitude);
            print!("{:>7.1}%", measure::run(t, (RATE / 2.0) as usize, |x| c.process(x)).thd_percent());
        }
        println!();
    }
}
