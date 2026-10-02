//! Which tone knobs the panel lights, and what it calls them.
//!
//! The rule is that a knob is live when it reaches a control and grey when it
//! does not, and the panel has two ways to reach one: the plugin's own tone
//! stack, and the selected circuit's own controls where the drawing gives it
//! any. Getting that wrong is not cosmetic. A knob that turns and changes
//! nothing reads as a fault, and so does a greyed knob that would have worked
//! -- which is what a TS808 had: a tone control on its drawing, wired up in
//! `voice::set_tone_knobs`, and greyed out on the panel because its preset
//! switches the plugin's stack off.

use gainstagefx::editor::ToneKnobs;
use gainstagefx::params::{Circuit, ToneStack};
use gainstagefx::presets::PRESETS;

/// The circuits whose drawings carry tone controls on the three stack knobs,
/// and which of them: the amplifiers' own stacks and the Metal Zone's three
/// bands. A pedal's single tone control has a knob of its own instead (see
/// below), and the Heavy Metal's Colour Mix a pair.
///
/// This table is the panel's claim and `Gain::own_tone` is the circuit's, and
/// the first test below is what keeps them the same sentence. It caught the
/// Twin: an AB763 has Bass, Middle and Treble on its front, they were wired
/// through `own_tone` when the circuit went in, and this list was not told --
/// so a test asserting the Twin had no tone control of its own was failing
/// against a Twin that has three.
const OWN: [(Circuit, [bool; 3]); 22] = [
    (Circuit::Boogie, [true, true, true]),
    (Circuit::Brit800, [true, true, true]),
    // The boost channel's own three; the muted Normal channel's two are not
    // on the panel.
    (Circuit::Brit2205, [true, true, true]),
    // The original 5150's own stack, built 2026-09-25.
    (Circuit::Peavey, [true, true, true]),
    (Circuit::Plexi, [true, true, true]),
    (Circuit::AC30, [true, false, true]),
    (Circuit::DR103, [true, true, true]),
    (Circuit::Recto, [true, true, true]),
    (Circuit::Twin, [true, true, true]),
    // The other AB763. No Middle: the Deluxe's stack grounds through a fixed
    // 6.8 k where the Twin has a pot, so that knob is dark.
    (Circuit::Deluxe, [true, false, true]),
    // Its Normal channel is the same stack.
    (Circuit::DeluxeNormal, [true, false, true]),
    // All three, and with more authority than any Fender stack here: the
    // Jazz 120's slope resistor feeds two caps into two entry points.
    (Circuit::Jazz120, [true, true, true]),
    // The Heavy Metal's Colour Mix now has two dedicated controls of its own,
    // not the generic Bass/Treble knobs. The Metal Zone still has a three-band
    // equaliser plus its own sweep control.
    (Circuit::Mt2, [true, true, true]),
    // The 1959's bass sibling, the Laney and the Sunn: each a Marshall-family
    // stack of its own, all three knobs.
    (Circuit::PlexiBass, [true, true, true]),
    (Circuit::Brum100, [true, true, true]),
    (Circuit::OregonT, [true, true, true]),
    // The Guv'nor's bass, middle and treble, on the stack knobs as the Metal
    // Zone's three bands are.
    (Circuit::BritDrive, [true, true, true]),
    // The G3's passive bass and treble and its active middle.
    (Circuit::ModernPurple, [true, true, true]),
    // The JTM45's own stack: treble, bass and middle.
    (Circuit::Brit45, [true, true, true]),
    // The SVT's channel 1: BASS, MIDRANGE and TREBLE.
    (Circuit::AmericanSvt, [true, true, true]),
    // The Bass Driver's active BASS, MID and TREBLE.
    (Circuit::BassDriver, [true, true, true]),
    // The 800RB's BASS, LOW MID and TREBLE; HIGH MID is the knob beyond them.
    (Circuit::American800RB, [true, true, true]),
];

#[test]
fn heavy_metal_colour_mix_is_dedicated_not_generic_bass_treble() {
    let off = ToneKnobs::for_state(Circuit::Hm2, ToneStack::Off);
    assert_eq!(off.live, [false; 3]);
    assert_eq!(off.colour_mix, Some(["COLOUR LO", "COLOUR HI"]));

    let stacked = ToneKnobs::for_state(Circuit::Hm2, ToneStack::Wide);
    assert_eq!(stacked.live, [true; 3]);
    assert_eq!(stacked.colour_mix, Some(["COLOUR LO", "COLOUR HI"]));

    for circuit in Circuit::ALL {
        if circuit != Circuit::Hm2 {
            assert_eq!(
                ToneKnobs::for_state(circuit, ToneStack::Off).colour_mix,
                None,
                "{} must not expose the Heavy Metal controls",
                circuit.name()
            );
        }
    }
}

/// The knob beyond the stack's three is shown only for a circuit that has
/// such a control of its own: the Metal Zone's MID FREQ, the Bass Driver's
/// BLEND, the 800RB's HIGH MID and the Modern Purple's AGGRESSION toggle, and
/// nothing anywhere else.
#[test]
fn the_control_beyond_the_stack_is_exposed_only_where_the_circuit_has_one() {
    for circuit in Circuit::ALL {
        let sweep = ToneKnobs::for_state(circuit, ToneStack::Off).sweep;
        match circuit {
            Circuit::Mt2 => assert_eq!(sweep, Some("MID FREQ")),
            Circuit::BassDriver => assert_eq!(sweep, Some("BLEND")),
            Circuit::American800RB => assert_eq!(sweep, Some("HI MID")),
            Circuit::ModernPurple => assert_eq!(sweep, Some("AGGRESSION")),
            _ => assert_eq!(
                sweep,
                None,
                "{} must not expose a control beyond the stack",
                circuit.name()
            ),
        }
    }
}

#[test]
fn a_circuit_with_its_own_tone_control_has_a_live_knob_for_it() {
    for (circuit, expected) in OWN {
        assert_eq!(
            circuit.own_tone_knobs(),
            expected,
            "{} carries different tone controls than the panel thinks",
            circuit.name()
        );
        let knobs = ToneKnobs::for_state(circuit, ToneStack::Off);
        assert_eq!(
            knobs.live,
            expected,
            "{} with the stack out should light exactly its own knobs",
            circuit.name()
        );
    }
}

/// And the other way: a circuit with no tone control of its own, with the
/// stack out of circuit, has no live tone knob at all.
#[test]
fn a_circuit_without_one_has_no_live_knob() {
    for circuit in Circuit::ALL {
        if OWN.iter().any(|(c, _)| *c == circuit) {
            continue;
        }
        assert_eq!(
            ToneKnobs::for_state(circuit, ToneStack::Off).live,
            [false; 3],
            "{} has no tone control, so no knob of its own should be live",
            circuit.name()
        );
    }
}

/// The plugin's own stack reaches every circuit, so switching it in lights all
/// three whatever is selected.
#[test]
fn the_plugin_stack_lights_all_three_whatever_is_selected() {
    for circuit in Circuit::ALL {
        for stack in [ToneStack::Wide, ToneStack::Scooping] {
            assert_eq!(
                ToneKnobs::for_state(circuit, stack).live,
                [true; 3],
                "{} with the {} stack should light all three",
                circuit.name(),
                stack.name()
            );
        }
    }
}

/// A pedal's single tone control, with the pedal selected as the circuit, is
/// a knob of its own named as its box names it -- TONE, and FILTER on the
/// Rodent -- whatever the plugin's stack is doing, and the stack's third knob
/// is its Treble and nothing else.
///
/// Until 2026-09-30 the pedal's tone rode on that Treble knob, which the
/// panel called TREBLE whenever the stack was in circuit, the default. So a
/// TS808 picked as the circuit showed no Tone knob anywhere: reported, and
/// this is the test for it.
#[test]
fn a_pedal_circuits_tone_has_its_own_knob() {
    let expected = [
        (Circuit::Screamer, "TONE"),
        (Circuit::Green9, "TONE"),
        (Circuit::Muff, "TONE"),
        (Circuit::Rat, "FILTER"),
        (Circuit::Ds1, "TONE"),
        (Circuit::GoldDrive, "TREBLE"),
        // Not tone controls, but each pedal's one knob beyond its drive: the
        // Orange Phase's R28 switch and the Blue Chorus's depth.
        (Circuit::OrangePhase, "BLOCK"),
        (Circuit::BlueChorus, "DEPTH"),
    ];
    for circuit in Circuit::ALL {
        let name = expected
            .iter()
            .find(|(c, _)| *c == circuit)
            .map(|(_, name)| *name);
        assert_eq!(circuit.single_tone(), name.is_some(), "{}", circuit.name());
        for stack in [ToneStack::Off, ToneStack::Wide, ToneStack::Scooping] {
            let knobs = ToneKnobs::for_state(circuit, stack);
            assert_eq!(knobs.single, name, "{} {}", circuit.name(), stack.name());
            assert_eq!(knobs.names, ["BASS", "MID", "TREBLE"], "{}", circuit.name());
            // No circuit has its own knob in the fourth column twice over.
            assert!(
                knobs.single.is_none() || (knobs.sweep.is_none() && knobs.colour_mix.is_none()),
                "{}",
                circuit.name()
            );
        }
        // With the stack out, its three knobs reach nothing on these pedals.
        if name.is_some() {
            assert_eq!(
                ToneKnobs::for_state(circuit, ToneStack::Off).live,
                [false; 3],
                "{}",
                circuit.name()
            );
        }
    }
}

/// The bug in its original form: every shipped preset, asked whether its tone
/// knobs are live, has to agree with whether they reach anything.
#[test]
fn no_shipped_preset_greys_a_knob_that_works() {
    for preset in PRESETS {
        let knobs = ToneKnobs::for_state(preset.circuit, preset.tone);
        let own = preset.circuit.own_tone_knobs();
        for (i, name) in ["bass", "mid", "treble"].into_iter().enumerate() {
            let reaches = preset.tone != ToneStack::Off || own[i];
            assert_eq!(
                knobs.live[i],
                reaches,
                "'{}' would draw its {name} knob {} while it {} a control",
                preset.name,
                if knobs.live[i] { "live" } else { "grey" },
                if reaches { "reaches" } else { "reaches no" },
            );
        }
    }
}

/// A pedal's knobs are called the same in the slot and as the circuit: the
/// names its box prints, lower case in the slot. One pedal, one set of names.
#[test]
fn a_pedal_is_named_the_same_in_the_slot_and_as_the_circuit() {
    use gainstagefx::voice::Pedal;
    for pedal in Pedal::ALL {
        let Some(gain) = pedal.as_circuit() else {
            continue;
        };
        let (drive, level) = pedal.drive_and_level_labels();
        if pedal.has_drive() && !pedal.has_level() {
            // One knob and it is the gain (the Clean Boost): the slot's drive,
            // the circuit's Drive, the same name; no level anywhere.
            assert_eq!(drive, gain.drive_name().to_lowercase(), "{pedal:?}");
            assert!(gain.level_control().is_none(), "{pedal:?}");
        } else if pedal.has_drive() {
            assert_eq!(drive, gain.drive_name().to_lowercase(), "{pedal:?}");
            assert_eq!(level, gain.level_name().to_lowercase(), "{pedal:?}");
        } else {
            // One pot and no drive (the Treble Boost): in the slot it is the
            // level knob, as the circuit it is the Drive knob, and it has the
            // same name in both. The circuit has no level control besides it.
            assert_eq!(level, gain.drive_name().to_lowercase(), "{pedal:?}");
            assert!(gain.level_control().is_none(), "{pedal:?}");
        }
        if let Some((_, name, _)) = gain.own_single_tone() {
            let lower = name.to_lowercase();
            assert_eq!(
                pedal.tone_labels(),
                [Some(lower.as_str()), None, None, None, None],
                "{pedal:?}"
            );
        }
    }
}
