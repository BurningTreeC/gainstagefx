//! The Black Wah and the Chrome Wah (Cry Baby GCB-95, Vox V847) against
//! ElectroSmash's figures for both: a resonant peak at about 750 Hz with the
//! treadle in the middle, swept from 450 Hz to 1.6 kHz, up to 18 dB above the
//! band around it. Prints each build's peak across the treadle.
//!
//! `cargo run --release --example wah_op`

use gainstagefx::circuits::wah::{self, tap, Build};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain(build: Build, treadle: f64, hz: f64) -> f64 {
    let mut s = Simulation::new(tap(build, 10_000.0, 470_000.0, wah::OUTPUT).unwrap(), RATE);
    let (top, bottom) = wah::treadle_halves(treadle);
    s.set_realtime_value(wah::TREADLE_TOP, top);
    s.set_realtime_value(wah::TREADLE_BOTTOM, bottom);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 0.01);
    let m = measure::run(t, (RATE * 0.25) as usize, |x| s.process(x));
    20.0 * (m.fundamental().magnitude() / 0.01).log10()
}

fn main() {
    for build in [Build::CryBaby, Build::V847] {
        println!("{build:?}:");
        for treadle in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut best = (0.0, f64::MIN);
            let mut hz = 200.0;
            while hz < 4_000.0 {
                let g = gain(build, treadle, hz);
                if g > best.1 {
                    best = (hz, g);
                }
                hz *= 1.06;
            }
            let floor = gain(build, treadle, 80.0).max(gain(build, treadle, 8_000.0));
            println!(
                "  treadle {treadle:.2}: peak {:.0} Hz at {:+.1} dB ({:+.1} over the larger of 80 Hz and 8 kHz)",
                best.0,
                best.1,
                best.1 - floor
            );
        }
    }
}
