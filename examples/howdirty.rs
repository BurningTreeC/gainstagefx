//! How much a preset actually distorts, across input level.

use gainstagefx::circuits::tone;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

fn main() {
    let levels = [-30.0, -24.0, NOMINAL_DBFS, -12.0, -6.0];
    print!("{:<24}", "preset (drive)");
    for l in levels {
        print!("{:>10}", format!("{l:.0} dBFS"));
    }
    println!();
    for preset in PRESETS.iter().filter(|p| {
        matches!(p.group, "Distortion" | "Overdrive") || p.name == "Scooped Metal"
    }) {
        print!("{:<24}", format!("{} ({:.2})", preset.name, preset.drive));
        for level in levels {
            let mut chain = Chain::new(RATE);
            chain.set_voice(preset.circuit.voice(), preset.diode.voice(), preset.amplifier.voice());
            chain.set_iron(preset.iron.voice());
            chain.set_tone_section(preset.tone.voice());
            chain.set_cabinet(preset.cabinet.voice());
            chain.set_oversampling(preset.oversampling.factor());
            chain.set_drive(preset.drive as f64);
            chain.set_tone(tone::BASS, preset.bass as f64);
            chain.set_tone(tone::MID, preset.mid as f64);
            chain.set_tone(tone::TREBLE, preset.treble as f64);
            chain.settle();
            let amplitude = 10f64.powf((level + preset.input_trim as f64) / 20.0);
            let t = Tone::near(RATE, 16_384, 220.0, amplitude);
            let m = measure::run(t, (RATE / 2.0) as usize, |x| chain.process(x));
            print!("{:>9.1}%", m.thd_percent());
        }
        println!();
    }
}
