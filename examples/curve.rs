//! What the drive knob actually does, across its travel.
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Tone as ToneSection, NOMINAL_DBFS};
const RATE: f64 = 96_000.0;
fn main() {
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    for gain in [voice::Gain::Clean, voice::Gain::Crunch, voice::Gain::HighGain,
                 voice::Gain::Overdrive, voice::Gain::Distortion] {
        print!("{:<12}", gain.name());
        for d in [0.0, 0.2, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0] {
            let mut c = Chain::new(RATE);
            c.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
            c.set_tone_section(ToneSection::Off);
            c.set_cabinet(Cabinet::Off);
            c.set_drive(d);
            c.settle();
            let t = Tone::near(RATE, 16_384, 220.0, amplitude);
            let m = measure::run(t, (RATE / 2.0) as usize, |x| c.process(x));
            print!("{:>7.1}", m.thd_percent());
        }
        println!();
    }
    print!("{:<12}", "drive");
    for d in [0.0, 0.2, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0] { print!("{d:>7.2}"); }
    println!();
}
