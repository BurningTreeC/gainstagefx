//! What the plugin ships knowing how to sound like.

use gainstagefx::circuits::clipper::GAIN;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::params::{Cabinet, Circuit, ToneStack};
use gainstagefx::presets::{GROUPS, PRESETS};
use gainstagefx::voice::{Cabinet as VCabinet, Chain, Tone as VTone, CALIBRATION, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

/// Two presets with the same name are two presets a saved session cannot tell
/// apart, and the one that reloads is whichever happens to be found first.
#[test]
fn every_preset_has_its_own_name() {
    let mut seen = std::collections::HashSet::new();
    for preset in PRESETS {
        assert!(
            seen.insert(preset.name),
            "'{}' appears more than once",
            preset.name
        );
    }
}

/// Every preset has to belong to a group the panel will actually show, or it
/// is shipped and unreachable.
#[test]
fn every_preset_is_in_a_group_the_panel_shows() {
    for preset in PRESETS {
        assert!(
            GROUPS.contains(&preset.group),
            "'{}' is in group '{}', which the panel does not list",
            preset.name,
            preset.group
        );
    }
    for group in GROUPS {
        assert!(
            PRESETS.iter().any(|p| p.group == group),
            "the panel lists '{group}' and nothing is in it"
        );
    }
}

/// Every control a preset sets has to be inside its range. A value outside it
/// is silently clamped, so the preset would load as something other than what
/// is written here and nothing would say so.
#[test]
fn every_preset_is_within_range() {
    for preset in PRESETS {
        for (what, value) in [
            ("drive", preset.drive),
            ("bass", preset.bass),
            ("mid", preset.mid),
            ("treble", preset.treble),
            ("mix", preset.mix),
        ] {
            assert!(
                (0.0..=1.0).contains(&value),
                "'{}' sets {what} to {value}",
                preset.name
            );
        }
        for (what, value) in [
            ("input trim", preset.input_trim),
            ("output trim", preset.output_trim),
        ] {
            assert!(
                (-24.0..=24.0).contains(&value),
                "'{}' sets {what} to {value} dB",
                preset.name
            );
        }
    }
}

/// A preset that selects diodes on a circuit with none in it is not wrong so
/// much as misleading: it records a choice that has no effect, and the panel
/// greys the control out while the preset still claims it. Every one of them
/// should leave it at the default it will actually be given.
#[test]
fn presets_only_choose_diodes_where_there_are_diodes() {
    for preset in PRESETS {
        if !preset.circuit.has_diodes() {
            assert_eq!(
                preset.diode,
                gainstagefx::params::Diode::Silicon,
                "'{}' is a {} and chooses {} diodes, which nothing will use",
                preset.name,
                preset.circuit.name(),
                preset.diode.name(),
            );
        }
    }
}

/// A preamplifier sound with a speaker in front of it is the commonest way
/// one of these ends up sounding muffled, and a high gain sound without one is
/// unlistenable. The catalogue should not do either.
#[test]
fn the_cabinet_is_off_for_preamps_and_on_for_high_gain() {
    for preset in PRESETS {
        match preset.circuit {
            Circuit::Clean => assert_eq!(
                preset.cabinet,
                Cabinet::Off,
                "'{}' is a preamplifier sound with a cabinet on it",
                preset.name
            ),
            Circuit::HighGain => assert_ne!(
                preset.cabinet,
                Cabinet::Off,
                "'{}' is a high gain sound with nothing to come out of",
                preset.name
            ),
            _ => {}
        }
    }
}

/// The catalogue has to actually span the range it claims: from a circuit
/// barely bending a signal to one squaring it off. If the quietest and the
/// loudest preset are the same sound then the list is decoration.
#[test]
fn the_catalogue_spans_from_subtle_to_squared_off() {
    let thd_of = |preset: &gainstagefx::presets::Preset| {
        let gain = preset.circuit.voice();
        let diode = preset.diode.voice();
        let index = gainstagefx::voice::voice_index(gain, diode);
        let netlist = gainstagefx::voice::build_voice(gain, diode).expect("builds");
        let mut sim = gainstagefx::dsp::time::Simulation::new(netlist, RATE);
        sim.set_control(GAIN, preset.drive as f64);
        let tone = Tone::near(RATE, 16_384, 220.0, CALIBRATION[index].drive_volts);
        measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x)).thd_percent()
    };
    let quietest = PRESETS
        .iter()
        .map(|p| thd_of(p))
        .fold(f64::MAX, f64::min);
    let loudest = PRESETS
        .iter()
        .map(|p| thd_of(p))
        .fold(f64::MIN, f64::max);
    println!("the catalogue runs from {quietest:.2} % to {loudest:.1} % distortion");
    assert!(
        quietest < 1.0,
        "the gentlest preset already makes {quietest:.2} % distortion"
    );
    assert!(
        loudest > 25.0,
        "the hardest preset only makes {loudest:.1} % distortion"
    );
}

/// Loading a preset should not change how loud the track is. They are worked
/// examples, and comparing two of them has to be a comparison of sound.
///
/// This is one tone at 220 Hz, not loudness, and the threshold is loose
/// because of it: a scooping voicing into a stack really is ten decibels down
/// *at 220 Hz* while being no quieter overall, and tightening this would only
/// mean forbidding the scoop. It catches a preset that is broadly wrong, which
/// is what it is for.
#[test]
fn the_presets_are_level_matched() {
    let mut levels = Vec::new();
    for preset in PRESETS {
        let mut chain = Chain::new(RATE);
        chain.set_voice(preset.circuit.voice(), preset.diode.voice());
        chain.set_tone_section(match preset.tone {
            ToneStack::Off => VTone::Off,
            ToneStack::Wide => VTone::Wide,
            ToneStack::Scooping => VTone::Scooping,
        });
        chain.set_cabinet(match preset.cabinet {
            Cabinet::Off => VCabinet::Off,
            Cabinet::Combo => VCabinet::Combo,
            Cabinet::Stack => VCabinet::Stack,
        });
        chain.set_drive(preset.drive as f64);
        chain.set_tone(gainstagefx::circuits::tone::BASS, preset.bass as f64);
        chain.set_tone(gainstagefx::circuits::tone::MID, preset.mid as f64);
        chain.set_tone(gainstagefx::circuits::tone::TREBLE, preset.treble as f64);

        let trim = 10f64.powf(preset.output_trim as f64 / 20.0);
        let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
        let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
        let out = measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x))
            .fundamental()
            .magnitude()
            * trim
            * preset.mix as f64;
        levels.push((preset.name, 20.0 * out.max(1e-9).log10() - NOMINAL_DBFS));
    }
    let (loudest, high) = levels
        .iter()
        .fold(("", f64::MIN), |a, b| if b.1 > a.1 { *b } else { a });
    let (quietest, low) = levels
        .iter()
        .fold(("", f64::MAX), |a, b| if b.1 < a.1 { *b } else { a });
    println!("loudest '{loudest}' at {high:+.1} dB, quietest '{quietest}' at {low:+.1} dB");
    assert!(
        high - low < 14.0,
        "the catalogue spans {:.1} dB: '{loudest}' at {high:+.1} against '{quietest}' at {low:+.1}",
        high - low
    );
}
