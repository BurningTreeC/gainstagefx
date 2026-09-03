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
        .map(thd_of)
        .fold(f64::MAX, f64::min);
    let loudest = PRESETS
        .iter()
        .map(thd_of)
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

// ---------------------------------------------------------------------------
// What a preset means when it is stored
// ---------------------------------------------------------------------------

use gainstagefx::params::GainStageParams;
use gainstagefx::presets::{self, Stored};
use nih_plug::params::Params;
use std::collections::BTreeMap;

/// Every id a shipped preset names has to be a parameter that exists.
///
/// This is the one that would otherwise never be noticed. A misspelled id is
/// not an error anywhere: the conversion simply skips it, the preset loads,
/// and one control silently stays wherever the last preset left it. Renaming a
/// parameter without renaming it here does exactly the same thing.
#[test]
fn every_preset_id_is_a_parameter_that_exists() {
    let params = GainStageParams::default();
    let known: Vec<String> = params
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    for (id, _) in PRESETS[0].dials() {
        assert!(
            known.iter().any(|k| k == id),
            "presets set '{id}', which is not a parameter. It would be dropped \
             silently and the control would keep whatever the last preset left \
             on it. The parameters are: {known:?}"
        );
    }
    // And the other way: a control a preset never mentions is a control that
    // cannot be recalled, which is worth knowing about deliberately.
    let mentioned: Vec<&str> = PRESETS[0].dials().iter().map(|(id, _)| *id).collect();
    let unmentioned: Vec<&String> = known
        .iter()
        .filter(|k| !mentioned.contains(&k.as_str()))
        .collect();
    assert_eq!(
        unmentioned,
        vec!["oversampling"],
        "the only control a preset should leave alone is the one about what \
         the plugin costs rather than what it sounds like"
    );
}

/// No shipped preset may write a value its parameter cannot hold.
///
/// Presets are written in the numbers the panel shows and stored normalised,
/// and normalising clamps. A preset asking for -20 dB from a control that
/// stops at -12 does not fail: it loads quietly as -12, and the sound is not
/// the one that was voiced. Checked against the parameter's real range rather
/// than a constant copied alongside it, which is the check that stays true
/// when a range is narrowed later.
#[test]
fn no_shipped_preset_asks_for_more_than_its_control_allows() {
    let params = GainStageParams::default();
    let pointers: BTreeMap<String, nih_plug::prelude::ParamPtr> = params
        .param_map()
        .into_iter()
        .map(|(id, ptr, _)| (id, ptr))
        .collect();

    for preset in PRESETS {
        for (id, written) in preset.dials() {
            let ptr = pointers.get(id).expect("a parameter that exists");
            // SAFETY: the pointer comes from the parameters above, which
            // outlive this loop.
            let back = unsafe { ptr.preview_plain(ptr.preview_normalized(written)) };
            assert!(
                (back - written).abs() < 0.051,
                "'{}' sets {id} to {written}, which the control cannot hold: it \
                 comes back as {back}",
                preset.name
            );
        }
    }
}

/// Each id has to name the parameter it says it does.
///
/// Not by comparing values: every knob on this panel rests at the middle, so
/// bass and treble hold the same number and swapping their ids would be
/// invisible. Checked against the parameter's own name instead, which is the
/// one thing that differs. Nothing else in the plugin would notice the swap --
/// presets would simply load with two controls exchanged.
#[test]
fn each_id_names_the_parameter_it_claims() {
    let params = GainStageParams::default();
    let named: BTreeMap<String, String> = params
        .param_map()
        .into_iter()
        // SAFETY: the pointers come from the parameters above.
        .map(|(id, ptr, _)| (id, unsafe { ptr.name() }.to_string()))
        .collect();

    for (id, expected) in [
        ("in_trim", "Input"),
        ("circuit", "Circuit"),
        ("diode", "Diode"),
        ("drive", "Drive"),
        ("tone", "Tone"),
        ("bass", "Bass"),
        ("mid", "Mid"),
        ("treble", "Treble"),
        ("cabinet", "Cabinet"),
        ("mix", "Mix"),
        ("out_trim", "Output"),
        ("oversampling", "Oversampling"),
    ] {
        assert_eq!(
            named.get(id).map(String::as_str),
            Some(expected),
            "the id '{id}' reaches {:?}, not the {expected} control",
            named.get(id)
        );
    }

    // And the capture reads through those same ids.
    let live = presets::live_values(&params);
    assert!(
        !live.contains_key("oversampling"),
        "oversampling is about what the plugin costs, not what it sounds like"
    );
    for id in named.keys().filter(|k| *k != "oversampling") {
        assert!(live.contains_key(id), "'{id}' was not captured");
    }
}

/// The edited mark is a comparison, not a flag, and this is the comparison.
///
/// Driven from both sides on purpose. nih-plug keeps every parameter setter
/// private, so a test can read the live values but cannot move them -- which
/// is exactly why `same` takes two maps rather than reaching into the
/// parameters itself. Everything that decides anything is here.
#[test]
fn the_edited_mark_clears_when_a_control_goes_back() {
    let live: BTreeMap<String, f32> = [("drive", 0.5f32), ("bass", 0.25), ("mix", 1.0)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

    assert!(same_as(&live, &live), "a panel matches itself");

    let mut moved = live.clone();
    moved.insert(String::from("drive"), 0.7);
    assert!(!same_as(&live, &moved), "a moved control should show as edited");

    moved.insert(String::from("drive"), 0.5);
    assert!(
        same_as(&live, &moved),
        "putting it back should clear the mark -- this is the whole reason it \
         is compared rather than remembered"
    );

    // Below the tolerance is not a difference: a value that has been through a
    // host as a 32-bit float and back will not return bit-identical, and a
    // preset that always reads as edited is a preset nobody trusts.
    let mut nudged = live.clone();
    nudged.insert(String::from("drive"), 0.5 + 1e-7);
    assert!(same_as(&live, &nudged), "floating point noise is not an edit");

    // An id the preset does not mention is not a difference. A preset written
    // before a control existed should keep loading, with that control left
    // where it is rather than jumping to a default the preset never chose.
    let older: BTreeMap<String, f32> = [("drive".to_string(), 0.5f32)].into_iter().collect();
    assert!(same_as(&live, &older), "a preset need not mention everything");

    // And an id the panel no longer has cannot differ from anything.
    let mut retired = live.clone();
    retired.insert(String::from("gone"), 0.9);
    assert!(same_as(&live, &retired), "a retired control is not an edit");
}

fn same_as(live: &BTreeMap<String, f32>, saved: &BTreeMap<String, f32>) -> bool {
    presets::same(live, saved)
}

/// A captured preset has to match the panel it was captured from, which is the
/// one place the live parameters and the comparison meet.
#[test]
fn a_preset_captured_from_the_panel_matches_it() {
    let params = GainStageParams::default();
    let captured: Stored = presets::capture(&params, "Just Taken");
    assert!(
        presets::matches(&params, &captured.values),
        "a preset taken from the panel should not immediately read as edited"
    );
    let mut differing = captured.values.clone();
    let drive = differing["drive"];
    differing.insert(String::from("drive"), drive + 0.3);
    assert!(
        !presets::matches(&params, &differing),
        "and one that differs should"
    );
}
