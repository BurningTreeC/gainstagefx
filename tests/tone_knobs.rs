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

/// The circuits whose drawings carry tone controls, and what each one has:
/// the Mark IIC+ and the Twin a three-knob stack, the two pedals a single
/// knob.
///
/// This table is the panel's claim and `Gain::own_tone` is the circuit's, and
/// the first test below is what keeps them the same sentence. It caught the
/// Twin: an AB763 has Bass, Middle and Treble on its front, they were wired
/// through `own_tone` when the circuit went in, and this list was not told --
/// so a test asserting the Twin had no tone control of its own was failing
/// against a Twin that has three.
const OWN: [(Circuit, [bool; 3]); 4] = [
    (Circuit::Boogie, [true, true, true]),
    (Circuit::Twin, [true, true, true]),
    (Circuit::Screamer, [false, false, true]),
    (Circuit::Muff, [false, false, true]),
];

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

/// A pedal's single control is not a treble control, and the panel says so --
/// but only while it is the only thing that knob reaches.
#[test]
fn the_pedals_single_control_is_called_tone() {
    for circuit in [Circuit::Screamer, Circuit::Muff] {
        assert!(
            circuit.single_tone(),
            "{} has one tone control",
            circuit.name()
        );
        assert_eq!(
            ToneKnobs::for_state(circuit, ToneStack::Off).names[2],
            "TONE",
            "{} with the stack out turns its own tone control",
            circuit.name()
        );
        assert_eq!(
            ToneKnobs::for_state(circuit, ToneStack::Wide).names[2],
            "TREBLE",
            "{} with the stack in turns the stack's treble as well",
            circuit.name()
        );
    }
    for circuit in Circuit::ALL {
        if circuit == Circuit::Screamer || circuit == Circuit::Muff {
            continue;
        }
        assert_eq!(
            ToneKnobs::for_state(circuit, ToneStack::Off).names[2],
            "TREBLE",
            "{} has no single tone control to name a knob after",
            circuit.name()
        );
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
