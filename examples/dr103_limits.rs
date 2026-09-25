//! How hard the Brit DR103's own preamplifier can drive the V3a that now
//! begins its power stage, and whether the whole amplifier settles there.
//!
//! The power-stage abuse test (`tests/speaker_load.rs`) feeds every stage 40 V
//! at 7 kHz, which the DR103 stage now takes at V3a's grid. This measures what
//! the preamplifier can actually put there -- Volume, Master and the treble all
//! the way up, a hot pickup -- and counts the solver's unsettled samples for
//! the whole matched chain at that setting, at the lowest and highest rates.
//!
//! `cargo run --release --example dr103_limits`
use gainstagefx::circuits::dr103::{self, BASS, MASTER, MIDDLE, TREBLE, VOLUME};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, Settings, Tone, GUITAR_VOLTS, NOMINAL_DBFS};

fn main() {
    // The preamplifier alone, everything up, driven 12 dB over a guitar.
    for hz in [82.0, 1_000.0, 7_000.0] {
        let rate = 96_000.0;
        let mut s = Simulation::new(dr103::build(10_000.0, 1_000_000.0).unwrap(), rate);
        for (c, v) in [
            (VOLUME, 1.0),
            (TREBLE, 1.0),
            (BASS, 1.0),
            (MIDDLE, 1.0),
            (MASTER, 1.0),
        ] {
            s.set_control(c, v);
        }
        s.find_operating_point();
        let a = GUITAR_VOLTS * 4.0;
        let mut peak = 0.0f64;
        for k in 0..(rate as usize / 5) {
            let y = s.process(a * (std::f64::consts::TAU * hz * k as f64 / rate).sin());
            if k > rate as usize / 10 {
                peak = peak.max(y.abs());
            }
        }
        println!("preamp at the master's wiper, {hz:>5.0} Hz: {peak:.1} V peak");
    }
    // The whole matched amplifier at that setting.
    for rate in [48_000.0, 192_000.0] {
        for hz in [82.0, 1_000.0, 7_000.0] {
            let mut chain = Chain::new(rate);
            chain.apply(&Settings {
                gain: Gain::DR103,
                drive: 1.0,
                master: 1.0,
                treble: 1.0,
                tone: Tone::Off,
                oversampling: 1,
                ..Settings::default()
            });
            chain.settle();
            let a = 10f64.powf((NOMINAL_DBFS + 12.0) / 20.0);
            let before = chain.solver_breakdown();
            let n = (rate * 0.2) as usize;
            for k in 0..n {
                assert!(chain
                    .process(a * (std::f64::consts::TAU * hz * k as f64 / rate).sin())
                    .is_finite());
            }
            let work = chain.solver_breakdown().saturating_delta(before);
            println!(
                "chain at {rate:.0} Hz, {hz:>5.0} Hz: power {} unsettled of {} solves, gain {} of {}",
                work.power.unsettled, work.power.solves, work.gain.unsettled, work.gain.solves
            );
        }
    }
}
