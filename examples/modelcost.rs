//! What the modelled circuits cost, and where their level sits.
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::voice::{self, Cabinet, Chain, Settings, Tone as ToneSection, CALIBRATION, NOMINAL_DBFS};
const RATE: f64 = 48_000.0;
fn main() {
    println!("{:<14}{:>10}{:>10}{:>12}{:>10}", "circuit", "load", "passes", "drive V", "peak out");
    for gain in [voice::Gain::Crunch, voice::Gain::Screamer, voice::Gain::Muff, voice::Gain::Boogie] {
        let i = voice::voice_index(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        let mut c = Chain::new(RATE);
        c.apply(&Settings { gain, drive: 0.8, tone: ToneSection::Off,
                            cabinet: Cabinet::Off, oversampling: 2, ..Settings::default() });
        c.settle();
        let amp = 10f64.powf(NOMINAL_DBFS / 20.0);
        let n = (RATE as usize) * 2;
        let start = std::time::Instant::now();
        let mut peak = 0.0f64;
        for k in 0..n {
            let x = amp * (k as f64 * 0.02).sin();
            peak = peak.max(c.process(x).abs());
        }
        let load = start.elapsed().as_secs_f64() / 2.0 * 100.0;
        println!("{:<14}{load:>9.1}%{:>10.2}{:>11.4}V{:>10.2}",
                 gain.name(), c.passes_per_sample(), CALIBRATION[i].drive_volts, peak);
        let _ = measure::run(Tone::near(RATE, 1024, 220.0, amp), 100, |x| x);
    }
}
