//! The Presence knob (2026-09-25): the power stage's presence control -- the
//! AC30's cut -- which existed in every power netlist and which nothing set.
//! See `Chain::set_presence`.
use gainstagefx::dsp::measure::{self, Tone as Probe};
use gainstagefx::params::{Circuit, GainStageParams};
use gainstagefx::presets::{self, Stored, PRESETS};
use gainstagefx::voice::{Chain, Gain, PowerAmp, PowerModel, Settings, Tone, NOMINAL_DBFS};
use nih_plug::params::Params;
use nih_plug::prelude::Enum;
use std::collections::BTreeMap;

const RATE: f64 = 48_000.0;

/// Small-signal level through the whole chain, dB, at one frequency.
fn level(gain: Gain, power_amp: PowerAmp, presence: f64, hz: f64) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&Settings {
        gain,
        power_amp,
        presence,
        drive: 0.3,
        tone: Tone::Off,
        ..Settings::default()
    });
    chain.settle();
    // Far below anything that clips, so this is the stage's response.
    let amplitude = 10f64.powf((NOMINAL_DBFS - 40.0) / 20.0);
    let probe = Probe::near(RATE, 8_192, hz, amplitude);
    let m = measure::run(probe, (RATE / 4.0) as usize, |x| chain.process(x));
    20.0 * m.fundamental().magnitude().max(1e-15).log10()
}

/// How much more the top than the bottom, at one presence setting. The
/// bottom is 200 Hz: a presence lift reaches down to a kilohertz on these
/// stages, so a 500 Hz reference moves with it and understates it.
fn tilt(gain: Gain, presence: f64) -> f64 {
    level(gain, PowerAmp::Matched, presence, 4_000.0)
        - level(gain, PowerAmp::Matched, presence, 200.0)
}

/// Turned up, a presence control takes treble out of the feedback loop and so
/// puts it back into the output. On the 2203's stage (22 k presence, .1 uF),
/// the 2205's and the 5150's, the top rises against the bottom -- measured
/// 2026-09-25: +4.4, +4.0 and +5.9 dB at 4 kHz, rising to +4.6, +3.9 and +8.0
/// at 10 kHz (the 1959's, with more feedback, +9.6 at 4 kHz).
#[test]
fn presence_up_brightens_the_top() {
    for gain in [Gain::Brit800, Gain::Brit2205, Gain::Peavey] {
        let (down, up) = (tilt(gain, 0.0), tilt(gain, 1.0));
        println!(
            "{}: 4 kHz over 200 Hz {down:+.1} dB down, {up:+.1} dB up",
            gain.name()
        );
        assert!(
            up - down > 3.0,
            "{}: presence moves the top by {:.1} dB",
            gain.name(),
            up - down
        );
    }
}

/// The AC30 has no feedback loop and no presence; the same slot is its CUT
/// control, which does the opposite: turned up, it takes treble away.
#[test]
fn the_ac30_calls_it_cut_and_it_darkens() {
    assert_eq!(PowerModel::AC30EL84.presence_name(), Some("CUT"));
    let (down, up) = (tilt(Gain::AC30, 0.0), tilt(Gain::AC30, 1.0));
    println!("AC30: 4 kHz over 200 Hz {down:+.1} dB cut down, {up:+.1} dB cut up");
    assert!(down - up > 3.0, "cut moves the top by {:.1} dB", down - up);
}

/// Where the stage has nothing in the slot the knob does nothing at all, and
/// the panel greys it: the AB763s (no presence on the amplifier), the
/// transistor stages, and no power stage. The DR103 has one since its driver
/// moved into the power stage (see `tests/dr103.rs`).
#[test]
fn no_presence_where_the_stage_has_none() {
    for model in [
        PowerModel::American6L6Clean,
        PowerModel::AmericanDeluxe6V6,
        PowerModel::Jazz120SS,
        PowerModel::British73Out,
    ] {
        assert_eq!(model.presence_name(), None, "{model:?}");
    }
    for model in [
        PowerModel::Cali6L6,
        PowerModel::American6L6HighGain,
        PowerModel::BritEL34,
        PowerModel::BritPlexiEL34,
        PowerModel::Recto6L6,
        PowerModel::Recto6L6Tube,
        PowerModel::Brit2205EL34,
        PowerModel::DR103EL34,
        PowerModel::DR103EL34Return,
    ] {
        assert_eq!(model.presence_name(), Some("PRESENCE"), "{model:?}");
    }
    for (gain, power) in [
        (Gain::Twin, PowerAmp::Matched),
        (Gain::Deluxe, PowerAmp::Matched),
    ] {
        let a = level(gain, power, 0.0, 4_000.0);
        let b = level(gain, power, 1.0, 4_000.0);
        assert_eq!(a, b, "{}: the knob reached something", gain.name());
    }
}

/// Half way is exactly where every stage rested before the knob existed, so
/// nothing that was measured, calibrated or saved moves: the chain at 0.5 is
/// bit-identical to the chain with the knob taken to an end and brought back.
#[test]
fn half_way_is_where_it_always_rested() {
    let render = |detour: Option<f64>| {
        let mut chain = Chain::new(RATE);
        let mut s = Settings {
            gain: Gain::Brit800,
            drive: 0.6,
            tone: Tone::Off,
            ..Settings::default()
        };
        if let Some(p) = detour {
            s.presence = p;
            chain.apply(&s);
            s.presence = 0.5;
        }
        chain.apply(&s);
        chain.settle();
        (0..4_800)
            .map(|k| chain.process(0.1 * (k as f64 * 0.05).sin()))
            .collect::<Vec<f64>>()
    };
    let plain = render(None);
    assert_eq!(plain, render(Some(1.0)));
    assert_eq!(plain, render(Some(0.0)));
}

/// A preset saved before the knob existed comes back with it in the middle.
#[test]
fn an_old_preset_migrates_to_the_middle() {
    let params = GainStageParams::default();
    let mut old = Stored {
        model_ids: BTreeMap::new(),
        name: "Before Presence".into(),
        values: [("drive".into(), 0.7)].into_iter().collect(),
        built_in: false,
        group: "",
    };
    presets::migrate(&mut old, &params);
    assert_eq!(old.values["presence"], 0.5);
    assert!(params.param_map().iter().any(|(id, _, _)| id == "presence"));
    // Every shipped preset carries the knob in its dials.
    for p in PRESETS {
        assert!(
            p.dials().iter().any(|(id, _)| *id == "presence"),
            "{}",
            p.name
        );
    }
    let _ = Circuit::ALL[0].to_index();
}

/// Turned all the way either way it plays at every rate.
#[test]
fn presence_plays_at_every_rate() {
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for presence in [0.0, 1.0] {
            let mut chain = Chain::new(rate);
            chain.apply(&Settings {
                gain: Gain::Brit2205,
                presence,
                drive: 0.9,
                tone: Tone::Off,
                ..Settings::default()
            });
            chain.settle();
            let mut peak = 0.0f64;
            for k in 0..(rate as usize / 10) {
                let y =
                    chain.process(0.3 * (std::f64::consts::TAU * 110.0 * k as f64 / rate).sin());
                assert!(y.is_finite(), "{rate} {presence}");
                peak = peak.max(y.abs());
            }
            assert!(peak > 1e-3, "{rate} {presence}: {peak}");
        }
    }
}
