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

/// The pedals alias audibly below four times oversampling and the valve
/// cascades do not, so each preset has to ask for what its own sound needs.
/// Measured against an eight times reference at 3 kHz: a clipper to ground is
/// 31 dB down at two times and 71 at four, while a three stage valve cascade
/// is already 55 dB down at two -- and it is the cascade that costs anything.
#[test]
fn the_pedals_ask_for_the_oversampling_they_need() {
    use gainstagefx::params::Oversampling;
    for preset in PRESETS {
        let wanted = match preset.circuit {
            Circuit::Overdrive | Circuit::Distortion => Oversampling::Four,
            _ => Oversampling::Two,
        };
        assert_eq!(
            preset.oversampling, wanted,
            "'{}' is a {} and asks for {}",
            preset.name,
            preset.circuit.name(),
            preset.oversampling.name(),
        );
    }
}

/// A preset that chooses a part its circuit does not contain is not wrong so
/// much as misleading: it records a choice that has no effect, the panel greys
/// the control out, and the preset still claims it. Every one should leave
/// such a control at the value it will actually be given.
#[test]
fn presets_only_choose_parts_their_circuit_contains() {
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
        if !preset.circuit.has_amplifier() {
            assert_eq!(
                preset.amplifier,
                gainstagefx::params::Amplifier::Jfet,
                "'{}' is a {} and chooses a {}, which nothing will use",
                preset.name,
                preset.circuit.name(),
                preset.amplifier.name(),
            );
        }
    }
}

/// The studio channels are not guitar amplifiers, and a speaker in front of a
/// microphone preamplifier makes no sense at all.
#[test]
fn the_studio_presets_have_no_speaker_on_them() {
    for preset in PRESETS.iter().filter(|p| p.group == "Studio") {
        assert_eq!(
            preset.cabinet,
            Cabinet::Off,
            "'{}' is a microphone preamplifier with a guitar speaker after it",
            preset.name
        );
    }
}

/// Every combination the presets use has to be reachable from the panel, and
/// every combination the panel offers has to build. This is the check that a
/// selection row is not offering something that does not exist.
#[test]
fn every_combination_on_the_panel_builds() {
    use gainstagefx::params::{Amplifier, Iron};
    for circuit in Circuit::ALL {
        for diode in gainstagefx::params::Diode::ALL {
            for amplifier in Amplifier::ALL {
                gainstagefx::voice::build_voice(
                    circuit.voice(),
                    diode.voice(),
                    amplifier.voice(),
                )
                .unwrap_or_else(|f| {
                    panic!(
                        "{} / {} / {} does not build: {f:?}",
                        circuit.name(),
                        diode.name(),
                        amplifier.name()
                    )
                });
            }
        }
    }
    for iron in Iron::ALL.into_iter().filter(|i| *i != Iron::Off) {
        gainstagefx::voice::build_iron(iron.voice())
            .unwrap_or_else(|f| panic!("{} iron does not build: {f:?}", iron.name()));
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
        let amplifier = preset.amplifier.voice();
        let index = gainstagefx::voice::voice_index(gain, diode, amplifier);
        let netlist =
            gainstagefx::voice::build_voice(gain, diode, amplifier).expect("builds");
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
        chain.set_voice(
            preset.circuit.voice(),
            preset.diode.voice(),
            preset.amplifier.voice(),
        );
        chain.set_iron(preset.iron.voice());
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
    let empty: Vec<&String> = Vec::new();
    assert_eq!(
        unmentioned, empty,
        "a control a preset never mentions cannot be recalled, so this list \
         should stay empty unless something is deliberately left out"
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
    for id in named.keys() {
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

/// A plugin that has just been added has not loaded a preset, and the strip
/// has to say so rather than name one.
///
/// It used to open on a preset called "Init", which claims a sound has been
/// loaded when none has -- and invites the reasonable belief that turning a
/// knob has departed from something.
#[test]
fn nothing_loaded_is_not_a_preset() {
    let params = GainStageParams::default();
    let shown = params.preset_name.lock().expect("readable").clone();
    assert_eq!(shown, gainstagefx::presets::NONE);
    assert!(
        PRESETS.iter().all(|p| p.name != gainstagefx::presets::NONE),
        "the empty marker must not also be the name of a preset"
    );
    assert!(
        !PRESETS.iter().any(|p| p.name == "Init"),
        "Init is not a sound, it is the absence of one"
    );
    // And it cannot be deleted, since it is not a file.
    assert!(gainstagefx::presets::index_of(gainstagefx::presets::NONE).is_none());
}

/// A circuit's description has to fit the line it is drawn on.
///
/// A label wider than its box in vizia is neither wrapped nor clipped -- it
/// spills out across whatever is beside it. The description sits on a line of
/// its own that is `body_w()` wide, which at the size it is set holds a little
/// over a hundred characters, so this is the length that fits rather than a
/// style preference.
#[test]
fn every_circuit_description_fits_its_row() {
    for circuit in Circuit::ALL {
        let text = gainstagefx::editor::describe(circuit);
        assert!(
            text.len() <= 105,
            "the {} description is {} characters and the row holds about 105:\n  {text}",
            circuit.name(),
            text.len()
        );
    }
}
