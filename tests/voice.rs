//! A circuit made playable, and the two joins that has to get right.

use gainstagefx::circuits::clipper::GAIN;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection, CALIBRATION};
use gainstagefx::voice::{NOMINAL_DBFS, POINTS, VOICES};

const RATE: f64 = 96_000.0;

fn nominal() -> f64 {
    10f64.powf(NOMINAL_DBFS / 20.0)
}

/// Peak level in dBFS of a settled tone at a chain's output.
fn level_through(chain: &mut Chain, amplitude: f64) -> f64 {
    let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
    measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x))
        .fundamental()
        .magnitude()
        .max(1e-12)
        .log10()
        * 20.0
}

/// Everything the plugin can select has to build. A `Fault` at this point is
/// a panic in a host, so it is worth one cheap test.
#[test]
fn the_whole_catalogue_builds() {
    for index in 0..VOICES {
        let (gain, diode) = voice::voice_at(index);
        voice::build_voice(gain, diode)
            .unwrap_or_else(|f| panic!("{} with {} diodes: {f:?}", gain.name(), diode.name()));
    }
    for t in ToneSection::ALL {
        if let Some(built) = t.build() {
            built.unwrap_or_else(|f| panic!("tone {}: {f:?}", t.name()));
        }
    }
    for c in Cabinet::ALL {
        if let Some(built) = c.build() {
            built.unwrap_or_else(|f| panic!("cabinet {}: {f:?}", c.name()));
        }
    }
}

/// The table in `src/calibration.rs` is measured, and a circuit that has
/// changed underneath it makes it a set of stale numbers that still compile.
/// This is the test that notices.
#[test]
fn the_calibration_table_still_describes_the_circuits() {
    for (index, c) in CALIBRATION.iter().enumerate() {
        let (gain, diode) = voice::voice_at(index);
        let netlist = voice::build_voice(gain, diode).expect("builds");
        for (i, expected) in c.make_up_db.iter().enumerate() {
            let mut sim = Simulation::new(netlist.clone(), RATE);
            sim.set_control(GAIN, i as f64 / (POINTS - 1) as f64);
            let tone = Tone::near(RATE, 16_384, 220.0, c.drive_volts);
            let got = -measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x)).gain_db();
            assert!(
                (got - expected).abs() < 0.5,
                "{} with {} diodes at drive {}/{}: the table says {expected:.2} dB \
                 and the circuit now says {got:.2}. Re-run `cargo run --release \
                 --example calibrate > src/calibration.rs`.",
                gain.name(),
                diode.name(),
                i,
                POINTS - 1,
            );
        }
    }
}

/// The make-up is nine measured points with straight lines between them, and
/// what matters is the level *between* the points, not at them. If the curve
/// is too coarse the level sags in the middle of every segment and the drive
/// control is once again partly a volume control.
#[test]
fn the_make_up_holds_the_level_between_the_measured_points() {
    for index in 0..VOICES {
        let (gain, diode) = voice::voice_at(index);
        let mut worst: f64 = 0.0;
        let mut worst_at = 0.0;
        // Deliberately off the measured grid.
        for step in 0..17 {
            let drive = (step as f64 + 0.5) / 17.0;
            let mut chain = Chain::new(gain, diode, ToneSection::Off, Cabinet::Off, RATE);
            chain.set_drive(drive);
            let out = level_through(&mut chain, nominal());
            if (out - NOMINAL_DBFS).abs() > worst {
                worst = (out - NOMINAL_DBFS).abs();
                worst_at = drive;
            }
        }
        assert!(
            worst < 3.0,
            "{} with {} diodes drifts {worst:.2} dB at drive {worst_at:.2}, \
             so the drive control is partly a volume control",
            gain.name(),
            diode.name(),
        );
    }
}

/// A plate sits at a couple of hundred volts and the coupling capacitor is
/// what stops that reaching the output. If it does not, the first thing anyone
/// hears when the plugin loads is an enormous thump.
#[test]
fn silence_in_is_silence_out() {
    for index in 0..VOICES {
        let (gain, diode) = voice::voice_at(index);
        let mut chain = Chain::new(gain, diode, ToneSection::Off, Cabinet::Off, RATE);
        chain.set_drive(0.7);
        let mut worst: f64 = 0.0;
        for _ in 0..(RATE as usize / 4) {
            worst = worst.max(chain.process(0.0).abs());
        }
        assert!(
            worst < 0.02,
            "{} with {} diodes put out {worst:.4} on silence, which is a thump",
            gain.name(),
            diode.name(),
        );
    }
}

/// Switching a passive section in must not drop the plugin twenty decibels.
/// A tone stack can only cut -- that is what passive means -- so the loss has
/// to be given back, and the AC solver knows exactly how much.
#[test]
fn the_passive_sections_do_not_cost_the_level() {
    let reference = {
        let mut chain = Chain::new(Gain::Crunch, voice::Diode::Silicon, ToneSection::Off, Cabinet::Off, RATE);
        chain.set_drive(0.8);
        level_through(&mut chain, nominal())
    };
    for t in [ToneSection::Wide, ToneSection::Scooping] {
        for c in [Cabinet::Off, Cabinet::Combo, Cabinet::Stack] {
            let mut chain = Chain::new(Gain::Crunch, voice::Diode::Silicon, t, c, RATE);
            chain.set_drive(0.8);
            let out = level_through(&mut chain, nominal());
            assert!(
                (out - reference).abs() < 12.0,
                "tone {} into cabinet {} moved the level {:.1} dB",
                t.name(),
                c.name(),
                out - reference,
            );
        }
    }
}

/// A voice named Clean should be clean and a voice named Distortion should
/// not be, at the same knob setting. If the names do not order that way the
/// selection is decoration.
#[test]
fn the_voices_are_in_the_order_their_names_claim() {
    let thd = |gain: Gain| {
        let netlist = voice::build_voice(gain, voice::Diode::Silicon).expect("builds");
        let c = CALIBRATION[voice::voice_index(gain, voice::Diode::Silicon)];
        let mut sim = Simulation::new(netlist, RATE);
        sim.set_control(GAIN, 1.0);
        let tone = Tone::near(RATE, 16_384, 220.0, c.drive_volts);
        measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x)).thd_percent()
    };
    let (clean, crunch, high) = (thd(Gain::Clean), thd(Gain::Crunch), thd(Gain::HighGain));
    assert!(
        clean < crunch && crunch < high,
        "clean {clean:.1} %, crunch {crunch:.1} %, high gain {high:.1} %"
    );
    assert!(thd(Gain::Overdrive) < thd(Gain::Distortion));
}
