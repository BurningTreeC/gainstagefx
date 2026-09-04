//! The tonal shape of the high gain presets: where the scoop actually is.
use gainstagefx::circuits::tone;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};
const RATE: f64 = 96_000.0;
const BANDS: [f64; 9] = [80.0, 120.0, 200.0, 350.0, 500.0, 800.0, 1200.0, 2500.0, 4000.0];
fn main() {
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    print!("{:<20}", "response, dB");
    for f in BANDS { print!("{:>7.0}", f); }
    println!();
    for p in PRESETS.iter().filter(|p| {
        matches!(p.name, "Scooped Metal" | "Thrash Rhythm" | "Scooped Pedal" | "Tight Low End" | "Modern Rhythm")
    }) {
        print!("{:<20}", p.name);
        let mut out = Vec::new();
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
            out.push(measure::run(t, (RATE / 2.0) as usize, |x| c.process(x)).gain_db());
        }
        let peak = out.iter().cloned().fold(f64::MIN, f64::max);
        for v in &out { print!("{:>7.1}", v - peak); }
        println!();
    }
}
