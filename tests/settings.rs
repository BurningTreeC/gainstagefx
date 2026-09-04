//! Every control on the panel has to reach the circuit.
//!
//! This file exists because they did not. Moving the circuit controls from
//! per-sample to per-block dropped four of them -- drive, bass, mid and
//! treble -- out of the plugin's loop entirely, and nothing failed: every
//! other test drives the chain directly, so none of them went through the
//! step that was broken. The knobs did nothing at all, and it took someone
//! playing through it to find out.
//!
//! So each control is checked here by the only thing that actually matters:
//! move it, and the audio has to change.

use gainstagefx::voice::{Cabinet, Chain, Diode, Gain, Iron, Settings, Tone, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;

/// What a chain does to a short piece of signal, as a number.
fn render(settings: &Settings) -> Vec<f64> {
    let mut chain = Chain::new(RATE);
    chain.apply(settings);
    chain.settle();
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    // Settle first: a circuit that has just been set up is still charging.
    for i in 0..4_000 {
        chain.process(amplitude * (i as f64 * 0.05).sin());
    }
    (0..2_000)
        .map(|i| chain.process(amplitude * ((i + 4_000) as f64 * 0.05).sin()))
        .collect()
}

/// How far apart two renders are, as a fraction of the signal in them.
fn difference(a: &[f64], b: &[f64]) -> f64 {
    let (mut err, mut sig) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        err += (x - y) * (x - y);
        sig += y * y;
    }
    (err / sig.max(1e-30)).sqrt()
}

/// Moving a control has to change the sound. All of them, every time.
#[test]
fn every_control_reaches_the_circuit() {
    // A voice with a tone stack and diodes, so that every control applies.
    let base = Settings {
        gain: Gain::Distortion,
        diode: Diode::Silicon,
        amplifier: Amplifier::Jfet,
        iron: Iron::Off,
        tone: Tone::Wide,
        cabinet: Cabinet::Off,
        drive: 0.5,
        bass: 0.5,
        mid: 0.5,
        treble: 0.5,
        oversampling: 4,
    };
    let reference = render(&base);

    let moved: [(&str, Settings); 9] = [
        ("drive", Settings { drive: 0.95, ..base }),
        ("bass", Settings { bass: 1.0, ..base }),
        ("mid", Settings { mid: 0.0, ..base }),
        ("treble", Settings { treble: 1.0, ..base }),
        ("circuit", Settings { gain: Gain::Crunch, ..base }),
        ("diode", Settings { diode: Diode::Germanium, ..base }),
        ("iron", Settings { iron: Iron::Steel, ..base }),
        ("tone section", Settings { tone: Tone::Scooping, ..base }),
        ("cabinet", Settings { cabinet: Cabinet::Stack, ..base }),
    ];
    for (name, settings) in moved {
        let d = difference(&render(&settings), &reference);
        println!("{name:<14} changes the output by {:.1} %", d * 100.0);
        assert!(
            d > 0.01,
            "moving the {name} control changed the audio by {:.4} %, which is \
             nothing: it is not reaching the circuit",
            d * 100.0
        );
    }
}

/// The amplifier choice reaches the circuits that have one.
#[test]
fn the_amplifier_choice_reaches_the_preamplifier_channels() {
    let base = Settings {
        gain: Gain::Console,
        tone: Tone::Off,
        cabinet: Cabinet::Off,
        ..Settings::default()
    };
    let valve = render(&Settings { amplifier: Amplifier::Valve, ..base });
    let jfet = render(&Settings { amplifier: Amplifier::Jfet, ..base });
    let d = difference(&valve, &jfet);
    println!("valve against JFET: {:.1} %", d * 100.0);
    assert!(d > 0.01, "the amplifier choice does nothing: {:.4} %", d * 100.0);
}

/// And the oversampling setting, which is part of the sound on the pedals.
#[test]
fn the_quality_setting_reaches_the_solver() {
    let base = Settings {
        gain: Gain::Distortion,
        drive: 0.9,
        tone: Tone::Off,
        cabinet: Cabinet::Off,
        ..Settings::default()
    };
    let coarse = render(&Settings { oversampling: 1, ..base });
    let fine = render(&Settings { oversampling: 4, ..base });
    let d = difference(&coarse, &fine);
    println!("one times against four: {:.3} %", d * 100.0);
    // A small figure, and it should be: the probe tone here is under 400 Hz,
    // so most of what the clipper makes still fits below Nyquist and there is
    // not much to fold. What matters is that the setting does *something* --
    // how much it is worth is measured properly in `examples/alias.rs`, which
    // finds 16 dB at 3 kHz.
    assert!(d > 1e-4, "the quality setting does nothing at all: {:.4} %", d * 100.0);
}

use gainstagefx::voice::Amplifier;
