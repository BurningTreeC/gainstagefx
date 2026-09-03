//! The tone stack, checked by what each control *does* rather than by what it
//! is wired to.
//!
//! This is the test the first version needed and never had. Four topologies
//! were tried there and all four were wrong in different ways -- one gave three
//! controls that all did something and none of which did what its name said,
//! another measured flat at every setting, a third put the notch in but made it
//! deepest with the middle wound up. Every one of those fails an assertion
//! here, and each takes microseconds to find.

use gainstagefx::circuits::stack::{self, Values};
use gainstagefx::dsp::ac;
use gainstagefx::dsp::netlist::Circuit;

/// What a 12AX7 plate looks like to the stack, and what the next grid loads it
/// with.
const SOURCE: f64 = 38_000.0;
const LOAD: f64 = 1_000_000.0;

fn at(circuit: &Circuit, controls: [f64; 3], hz: f64) -> f64 {
    ac::solve(circuit, &controls, hz).db()
}

/// How far the response dips between the bass and treble regions: the scoop,
/// in dB below the lower of the two ends.
fn notch_depth(circuit: &Circuit, controls: [f64; 3]) -> f64 {
    let low = at(circuit, controls, 82.0);
    let high = at(circuit, controls, 6_400.0);
    let mut deepest = f64::INFINITY;
    // Walk the middle of the band rather than trusting one frequency.
    for step in 0..40 {
        let hz = 200.0 * (3_000.0f64 / 200.0).powf(step as f64 / 39.0);
        deepest = deepest.min(at(circuit, controls, hz));
    }
    low.min(high) - deepest
}

/// The scooped sound is the middle wound down, and it has to be a *notch* --
/// the ends stay up. Losing the bass along with the mids is the failure mode
/// that makes a passive stack unable to produce it.
#[test]
fn winding_the_middle_down_scoops_and_keeps_the_ends() {
    for (name, values) in [("American", stack::AMERICAN), ("British", stack::BRITISH)] {
        let circuit = stack::build(&values, SOURCE, LOAD).expect("builds");
        let scooped = [1.0, 0.0, 1.0];
        let filled = [0.5, 1.0, 0.5];

        let deep = notch_depth(&circuit, scooped);
        assert!(
            deep > 10.0,
            "{name}: middle down only scoops {deep:.1} dB"
        );

        let shallow = notch_depth(&circuit, filled);
        assert!(
            shallow < deep - 6.0,
            "{name}: middle up should fill the notch: {shallow:.1} dB against {deep:.1} dB"
        );

        // And the bottom end survives it. If the bass goes with the mids, the
        // network is wired so the bass can only reach the output through the
        // mid pot.
        let bass = at(&circuit, scooped, 82.0);
        let middle = at(&circuit, scooped, 640.0);
        assert!(
            bass > middle + 8.0,
            "{name}: the bass went with the mids: {bass:.1} dB at 82 Hz, {middle:.1} at 640"
        );
    }
}

/// Each control moves its own end of the band, in the direction its label
/// promises.
#[test]
fn every_control_points_the_way_its_name_says() {
    let circuit = stack::build(&stack::BRITISH, SOURCE, LOAD).expect("builds");

    let bass_up = at(&circuit, [1.0, 0.5, 0.5], 82.0);
    let bass_down = at(&circuit, [0.0, 0.5, 0.5], 82.0);
    assert!(
        bass_up > bass_down + 6.0,
        "bass: up {bass_up:.1} dB, down {bass_down:.1} dB"
    );

    let treble_up = at(&circuit, [0.5, 0.5, 1.0], 6_400.0);
    let treble_down = at(&circuit, [0.5, 0.5, 0.0], 6_400.0);
    assert!(
        treble_up > treble_down + 6.0,
        "treble: up {treble_up:.1} dB, down {treble_down:.1} dB"
    );

    let mid_up = at(&circuit, [0.5, 1.0, 0.5], 640.0);
    let mid_down = at(&circuit, [0.5, 0.0, 0.5], 640.0);
    assert!(
        mid_up > mid_down + 6.0,
        "mid: up {mid_up:.1} dB, down {mid_down:.1} dB"
    );
}

/// Every control loads every other one. A stack that behaved as three separate
/// filters would show the same treble whatever the bass was doing, and would
/// not be a passive stack.
#[test]
fn the_controls_are_not_independent() {
    let circuit = stack::build(&stack::BRITISH, SOURCE, LOAD).expect("builds");
    let with_bass_up = at(&circuit, [1.0, 0.5, 0.5], 3_200.0);
    let with_bass_down = at(&circuit, [0.0, 0.5, 0.5], 3_200.0);
    assert!(
        (with_bass_up - with_bass_down).abs() > 0.5,
        "the bass control did not touch the top: {with_bass_up:.2} against {with_bass_down:.2}"
    );
}

/// The chime voicing has a fixed low resistance where the others have a mid
/// pot, so it has nothing to scoop against and sits forward. That is the whole
/// difference between it and the others.
#[test]
fn the_chime_voicing_barely_scoops() {
    let chime = stack::build(&stack::CHIME, SOURCE, LOAD).expect("builds");
    let british = stack::build(&stack::BRITISH, SOURCE, LOAD).expect("builds");
    let flat = [0.5, 0.5, 0.5];
    let chime_depth = notch_depth(&chime, flat);
    let british_depth = notch_depth(&british, flat);
    assert!(
        chime_depth < british_depth,
        "chime scoops {chime_depth:.1} dB, british {british_depth:.1} dB"
    );
}

/// The values are the only difference between the voicings, and they have to
/// produce audibly different curves or naming them separately is a fiction.
#[test]
fn the_voicings_are_actually_different() {
    let curve = |values: Values| {
        let circuit = stack::build(&values, SOURCE, LOAD).expect("builds");
        [82.0, 320.0, 1_000.0, 3_200.0].map(|hz| at(&circuit, [0.5, 0.5, 0.5], hz))
    };
    let american = curve(stack::AMERICAN);
    let british = curve(stack::BRITISH);
    let apart: f64 = american
        .iter()
        .zip(&british)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(apart > 2.0, "the voicings differ by only {apart:.1} dB");
}
