//! The tone section, tested by what each control does.
//!
//! Written against behaviour rather than wiring, because wiring was never the
//! problem: four reconstructions of a passive stack each had three controls
//! that all did *something*. What matters is that ten is flat, that each
//! control takes out its own band and nothing else's, and that the middle can
//! be taken out without the bottom going with it.

use gainstagefx::circuits::tone::{self, Voicing};
use gainstagefx::dsp::ac;
use gainstagefx::dsp::netlist::Circuit;

const SOURCE: f64 = 38_000.0;
const LOAD: f64 = 220_000.0;

fn at(c: &Circuit, controls: [f64; 3], hz: f64) -> f64 {
    ac::solve(c, &controls, hz).db()
}

fn built(v: Voicing) -> Circuit {
    tone::build(&v, SOURCE, LOAD).expect("builds")
}

/// A passive network cannot make gain, so the honest zero is "everything up".
/// It has to be flat there, or the controls are fighting a curve they did not
/// ask for.
#[test]
fn everything_up_is_flat() {
    for (name, v) in [("wide", tone::WIDE), ("scooping", tone::SCOOPING)] {
        let c = built(v);
        let flat: Vec<f64> = [82.0, 320.0, 1_000.0, 4_000.0, 8_000.0]
            .iter()
            .map(|&hz| at(&c, [1.0, 1.0, 1.0], hz))
            .collect();
        let spread = flat.iter().cloned().fold(f64::MIN, f64::max)
            - flat.iter().cloned().fold(f64::MAX, f64::min);
        // Not perfectly flat, and honestly so: the resonant mid leg is a
        // finite impedance either side of its band and loads the bus a little
        // at the extremes. Two and a half decibels across the whole band is
        // the network's own tilt, not a control fighting a curve.
        assert!(
            spread < 2.5,
            "{name} is not flat wide open: {spread:.1} dB across the band"
        );
    }
}

/// Each control takes out its own band, and only as it comes down.
#[test]
fn every_control_cuts_its_own_band() {
    let c = built(tone::WIDE);
    let open = [1.0, 1.0, 1.0];

    let bass_cut = at(&c, [0.0, 1.0, 1.0], 82.0) - at(&c, open, 82.0);
    assert!(bass_cut < -4.0, "bass barely cuts: {bass_cut:.1} dB");

    let treble_cut = at(&c, [1.0, 1.0, 0.0], 8_000.0) - at(&c, open, 8_000.0);
    assert!(treble_cut < -10.0, "treble barely cuts: {treble_cut:.1} dB");

    let mid_cut = at(&c, [1.0, 0.0, 1.0], 550.0) - at(&c, open, 550.0);
    assert!(mid_cut < -15.0, "mid barely cuts: {mid_cut:.1} dB");
}

/// A control wound down must not take a neighbour's band with it. The treble
/// control is the clearest case: it should leave the bottom alone entirely.
#[test]
fn a_control_leaves_the_other_bands_alone() {
    let c = built(tone::WIDE);
    let open = [1.0, 1.0, 1.0];

    let bottom = at(&c, [1.0, 1.0, 0.0], 82.0) - at(&c, open, 82.0);
    assert!(
        bottom.abs() < 1.5,
        "the treble control moved the low E by {bottom:.1} dB"
    );

    let top = at(&c, [0.0, 1.0, 1.0], 8_000.0) - at(&c, open, 8_000.0);
    assert!(
        top.abs() < 1.5,
        "the bass control moved the top by {top:.1} dB"
    );
}

/// The scoop. This is the thing a passive stack cannot do: wind its middle
/// down and the bass goes too, because the bass reaches the output through the
/// junction the mid control grounds. A resonant leg takes a band out and
/// leaves both ends where they were.
#[test]
fn the_middle_can_be_taken_out_without_the_bottom() {
    let c = built(tone::SCOOPING);
    let open = [1.0, 1.0, 1.0];
    let scooped = [1.0, 0.0, 1.0];

    let depth = at(&c, scooped, 480.0) - at(&c, open, 480.0);
    assert!(depth < -18.0, "the scoop is only {depth:.1} dB deep");

    let bottom = at(&c, scooped, 82.0) - at(&c, open, 82.0);
    assert!(
        bottom > -3.0,
        "the bottom went with the middle: {bottom:.1} dB at the low E"
    );

    let top = at(&c, scooped, 4_000.0) - at(&c, open, 4_000.0);
    assert!(top > -3.0, "the top went with the middle: {top:.1} dB");
}

/// The two voicings differ in how much of the neighbouring bands the mid
/// control takes with it, which is the whole reason there are two.
#[test]
fn the_narrow_voicing_keeps_more_of_the_bottom() {
    let cost = |v: Voicing| {
        let c = built(v);
        at(&c, [1.0, 0.0, 1.0], 82.0) - at(&c, [1.0, 1.0, 1.0], 82.0)
    };
    let wide = cost(tone::WIDE);
    let narrow = cost(tone::SCOOPING);
    assert!(
        narrow > wide + 1.5,
        "the voicings scoop the same: wide costs {wide:.1} dB, narrow {narrow:.1} dB"
    );
}

/// Halfway is halfway: a control at five should sit between its extremes, not
/// jump at one end of its travel.
#[test]
fn the_controls_move_evenly() {
    let c = built(tone::WIDE);
    for (band, hz) in [(0usize, 82.0), (1, 550.0), (2, 8_000.0)] {
        let open_at = [1.0, 1.0, 1.0];
        let mut half = [1.0, 1.0, 1.0];
        let mut shut = [1.0, 1.0, 1.0];
        half[band] = 0.5;
        shut[band] = 0.0;
        let (open, half, shut) = (at(&c, open_at, hz), at(&c, half, hz), at(&c, shut, hz));
        assert!(
            half < open - 1.0 && half > shut + 1.0,
            "band {band} at halfway is {half:.1} dB, between {open:.1} and {shut:.1} -- \
             a control that does nothing and then everything"
        );
    }
}
