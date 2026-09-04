//! Which harmonics each voice actually makes.
use gainstagefx::circuits::tone;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Cabinet, Chain, Tone as ToneSection, NOMINAL_DBFS};
const RATE: f64 = 96_000.0;
fn main() {
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    println!("{:<22}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}", "circuit only", "2nd", "3rd", "4th", "5th", "6th", "7th");
    for p in PRESETS.iter().filter(|p| {
        matches!(p.name, "Green Overdrive" | "Classic Distortion" | "Scooped Pedal"
                       | "Scooped Metal" | "Valve Colour" | "Blues Crunch")
    }) {
        let mut c = Chain::new(RATE);
        c.set_voice(p.circuit.voice(), p.diode.voice(), p.amplifier.voice());
        c.set_tone_section(ToneSection::Off);
        c.set_cabinet(Cabinet::Off);
        c.set_oversampling(p.oversampling.factor());
        c.set_drive(p.drive as f64);
        c.set_tone(tone::BASS, p.bass as f64);
        c.settle();
        let t = Tone::near(RATE, 16_384, 220.0, amplitude);
        let m = measure::run(t, (RATE / 2.0) as usize, |x| c.process(x));
        print!("{:<22}", p.name);
        for h in 2..=7 {
            print!("{:>7.1}%", m.harmonic_percent(h));
        }
        println!();
    }
}
