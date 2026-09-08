//! What a potentiometer's track actually does as it turns.
//!
//! A taper is a component specification, not a curve chosen for feel, and it
//! was wrong here for a long time without anything noticing: `Taper::Audio`
//! was an exponential spanning a thousand to one, which leaves three per cent
//! of the track in circuit at half rotation. An audio pot is specified at its
//! midpoint and the published figure is ten to twenty per cent. The error was
//! worth about sixteen decibels through the middle of every audio control in
//! the catalogue, and the Mark IIC+ has two of them in series.
//!
//! Several tests would have moved if the law changed again -- the Mark IIC+'s
//! gain span, the TS808's drive sweep, every row of the calibration table --
//! but none of them *said* what the law is, so all of them would have been
//! re-measured against whatever the new law happened to be. This one states
//! it.

use gainstagefx::dsp::netlist::Taper;

/// Ten per cent at half rotation, which is the number an audio-taper
/// potentiometer is sold by. Makers publish between ten and twenty; none
/// publishes three.
#[test]
fn an_audio_track_keeps_a_tenth_of_itself_at_half_rotation() {
    let f = Taper::Audio.fraction(0.5);
    assert!(
        (0.09..=0.21).contains(&f),
        "an audio track is specified at ten to twenty per cent at half \
         rotation; this one is {:.2} %",
        f * 100.0
    );
}

/// And it is two straight segments rather than a curve, because that is how a
/// log pot is made: two or three carbon segments of different resistivity laid
/// end to end. The corner at the middle is the thing being modelled, not an
/// artefact to smooth away.
///
/// Checked by measuring the slope either side of the midpoint: within each
/// half it must be constant, and the two halves must differ.
#[test]
fn an_audio_track_is_two_straight_segments_with_a_corner_in_the_middle() {
    let slope = |a: f64, b: f64| (Taper::Audio.fraction(b) - Taper::Audio.fraction(a)) / (b - a);

    let lower = slope(0.05, 0.15);
    let upper = slope(0.85, 0.95);
    for (a, b) in [(0.10, 0.20), (0.25, 0.35), (0.35, 0.45)] {
        let here = slope(a, b);
        assert!(
            (here - lower).abs() < 1e-6,
            "the lower segment bends between {a} and {b}: {here:.4} against {lower:.4}"
        );
    }
    for (a, b) in [(0.55, 0.65), (0.70, 0.80), (0.80, 0.90)] {
        let here = slope(a, b);
        assert!(
            (here - upper).abs() < 1e-6,
            "the upper segment bends between {a} and {b}: {here:.4} against {upper:.4}"
        );
    }
    assert!(
        upper > lower * 3.0,
        "the two segments have the same slope, so this is a linear track \
         wearing an audio name: {lower:.4} then {upper:.4}"
    );
}

/// The bottom of the travel is where a high-gain amplifier's control does all
/// of its work, and it is where the old law was furthest out: nine tenths of a
/// per cent at an eighth of a turn, against the two and a half a real track
/// gives. Nearly three times, which is nine decibels.
#[test]
fn an_audio_track_is_not_shut_at_an_eighth_of_a_turn() {
    let f = Taper::Audio.fraction(0.125);
    assert!(
        f > 0.02,
        "an eighth of a turn leaves {:.2} % of the track in circuit, which is \
         a control that does nothing until it is a quarter open",
        f * 100.0
    );
}

/// Every track runs from nothing to all of itself, and never backwards.
#[test]
fn every_track_runs_from_nothing_to_all_of_itself() {
    let laws = [
        ("linear", Taper::Linear),
        ("audio", Taper::Audio),
        ("log 30", Taper::Log { span: 30.0 }),
    ];
    for (name, law) in laws {
        assert!(
            law.fraction(0.0).abs() < 1e-9,
            "{name} starts at {}",
            law.fraction(0.0)
        );
        assert!(
            (law.fraction(1.0) - 1.0).abs() < 1e-9,
            "{name} ends at {}",
            law.fraction(1.0)
        );
        let mut last = -1.0;
        for i in 0..=200 {
            let p = i as f64 / 200.0;
            let f = law.fraction(p);
            assert!(f >= last - 1e-12, "{name} runs backwards at {p}");
            last = f;
        }
    }
}

/// A reverse taper is the forward one subtracted from the whole track, and
/// both reverses have to mean the same thing. Defining one as `1 - f(p)` and
/// the other as `1 - f(1 - p)` gives two controls that look like a matched
/// pair and turn opposite ways -- which is the sort of thing that is only ever
/// found by a player wondering why one knob is backwards.
#[test]
fn a_reverse_track_is_the_forward_one_taken_from_the_whole() {
    for i in 0..=20 {
        let p = i as f64 / 20.0;
        assert!(
            (Taper::ReverseAudio.fraction(p) - (1.0 - Taper::Audio.fraction(p))).abs() < 1e-12,
            "reverse audio does not mirror audio at {p}"
        );
        assert!(
            (Taper::ReverseLinear.fraction(p) - (1.0 - Taper::Linear.fraction(p))).abs() < 1e-12,
            "reverse linear does not mirror linear at {p}"
        );
        let span = 30.0;
        assert!(
            (Taper::ReverseLog { span }.fraction(p) - (1.0 - Taper::Log { span }.fraction(p)))
                .abs()
                < 1e-12,
            "reverse log does not mirror log at {p}"
        );
    }
}

/// A span at or below one has no range to shape, so the log law falls back to
/// a plain track rather than dividing by zero. A span *below* one is the same
/// law running the other way, which is what a rheostat needs when the thing it
/// is fighting is small.
#[test]
fn a_log_track_survives_a_span_with_no_shape_in_it() {
    for span in [1.0, 0.0, -3.0, f64::NAN] {
        for i in 0..=10 {
            let p = i as f64 / 10.0;
            let f = Taper::Log { span }.fraction(p);
            assert!(f.is_finite(), "a span of {span} gives {f} at {p}");
            assert!(
                (0.0..=1.0).contains(&f),
                "a span of {span} gives {f} at {p}"
            );
        }
    }
    // Below one, the track moves quickly at first and slowly at the end --
    // the opposite way round from a span above one.
    assert!(Taper::Log { span: 0.02 }.fraction(0.5) > 0.5);
    assert!(Taper::Log { span: 50.0 }.fraction(0.5) < 0.5);
}
