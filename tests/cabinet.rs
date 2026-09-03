//! The cabinet.
//!
//! Everything upstream of it is flat to within a decibel or two, so the
//! cabinet *is* the tone of a distorted guitar, and its corners being an
//! octave out is the difference between an instrument and a blanket. These
//! run against the AC solver, so the whole set costs microseconds.

use gainstagefx::circuits::cabinet::{self, Voicing};
use gainstagefx::dsp::ac;

fn at(v: Voicing, hz: f64) -> f64 {
    let c = cabinet::build(&v, 600.0).expect("builds");
    ac::solve(&c, &[], hz).db() - ac::solve(&c, &[], 400.0).db()
}

/// The lift between one and three kilohertz is what makes a distorted guitar
/// cut through, and it has to be a lift rather than merely an absence of loss.
/// The previous version's corners were an octave low, which put the 4x12 at
/// -7 dB at 2.5 kHz -- ten decibels missing from exactly there.
#[test]
fn the_presence_region_is_a_lift() {
    for (name, v) in [("1x12", cabinet::COMBO), ("4x12", cabinet::STACK)] {
        let peak = at(v, 2_000.0);
        assert!(peak > 1.5, "{name} has no lift at 2 kHz: {peak:.1} dB");
        assert!(
            at(v, 2_500.0) > -2.0,
            "{name} is already falling at 2.5 kHz: {:.1} dB",
            at(v, 2_500.0)
        );
    }
}

/// It still has to stop. Everything a hard clipper makes above four kilohertz
/// is fizz that in a real rig never reaches the air.
#[test]
fn the_top_goes_away() {
    for (name, v) in [("1x12", cabinet::COMBO), ("4x12", cabinet::STACK)] {
        assert!(at(v, 6_000.0) < -8.0, "{name} at 6 kHz: {:.1} dB", at(v, 6_000.0));
        assert!(at(v, 12_000.0) < -20.0, "{name} at 12 kHz: {:.1} dB", at(v, 12_000.0));
    }
}

/// And so does the bottom, well below the instrument.
#[test]
fn there_is_nothing_below_the_box() {
    for (name, v) in [("1x12", cabinet::COMBO), ("4x12", cabinet::STACK)] {
        assert!(at(v, 40.0) < -8.0, "{name} at 40 Hz: {:.1} dB", at(v, 40.0));
    }
}

/// The two boxes differ the way the hardware does: a sealed 4x12 holds the low
/// E where an open backed 1x12 gives it up, and the 1x12 is brighter on top.
#[test]
fn the_boxes_differ_the_way_they_should() {
    let low_e = 82.0;
    let sealed = at(cabinet::STACK, low_e);
    let open = at(cabinet::COMBO, low_e);
    assert!(
        sealed > open + 3.0,
        "the sealed box should hold the low E: 4x12 {sealed:.1} dB, 1x12 {open:.1} dB"
    );
    assert!(
        at(cabinet::COMBO, 4_000.0) > at(cabinet::STACK, 4_000.0),
        "the open box should be brighter on top"
    );
}

/// The midrange is where the instrument lives, and the cabinet has no business
/// shaping it.
#[test]
fn the_midrange_is_left_alone() {
    for (name, v) in [("1x12", cabinet::COMBO), ("4x12", cabinet::STACK)] {
        for hz in [250.0, 400.0, 700.0, 1_000.0] {
            let db = at(v, hz);
            assert!(db.abs() < 1.5, "{name} is {db:.1} dB at {hz:.0} Hz");
        }
    }
}
