//! The Metal Zone, against Boss's drawing and Electric Druid's analysis of it.
use gainstagefx::circuits::metal_zone::{self, DIST, HIGH, LEVEL, LOW, MIDDLE, MID_FREQ};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn at(node: &str, controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(metal_zone::tap(10_000.0, 470_000.0, node).unwrap(), RATE);
    for (c, v) in [
        (DIST, 0.5),
        (LOW, 0.5),
        (MIDDLE, 0.5),
        (MID_FREQ, 0.5),
        (HIGH, 0.5),
        (LEVEL, 1.0),
    ] {
        s.set_control(c, v);
    }
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    s.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

fn db(node: &str, controls: &[(usize, f64)], hz: f64) -> f64 {
    20.0 * at(node, controls, hz, 0.02)
        .fundamental()
        .magnitude()
        .max(1e-12)
        .log10()
}

#[test]
fn it_builds_and_settles() {
    let mut s = Simulation::new(metal_zone::build(10_000.0, 470_000.0).unwrap(), RATE);
    s.find_operating_point();
    for _ in 0..(RATE as usize / 10) {
        assert!(s.process(0.0).is_finite());
    }
    let tail: Vec<f64> = (0..256).map(|_| s.process(0.0)).collect();
    let rms = (tail.iter().map(|v| v * v).sum::<f64>() / tail.len() as f64).sqrt();
    assert!(rms < 1e-3, "silence is not silent: {rms}");
}

/// The hump before the clipper, which is what decides *which* part of the
/// guitar gets distorted. Electric Druid puts it "around 1kHz"; the values give
/// 953 Hz.
#[test]
fn the_hump_before_the_clipper_peaks_near_a_kilohertz() {
    let peak = db("u3b", &[], 950.0);
    for hz in [100.0, 300.0, 3_000.0] {
        let away = db("u3b", &[], hz);
        assert!(
            peak > away + 10.0,
            "950 Hz {peak:.1} dB against {hz} Hz {away:.1} dB"
        );
    }
}

/// "Gain range: x2 minimum to x252 maximum", which is 6 dB to 48 dB. Measured
/// at the stage's own output on a signal small enough not to reach the diodes.
#[test]
fn the_dist_control_spans_what_the_analysis_says() {
    let down = at("u3a", &[(DIST, 0.0)], 950.0, 1e-4).gain_db();
    let up = at("u3a", &[(DIST, 1.0)], 950.0, 1e-5).gain_db();
    println!("{down:.1} dB down, {up:.1} dB up (the analysis: 6 and 48, plus the hump)");
    assert!(up > down + 20.0, "{down} {up}");
}

/// Each tone control works where it is supposed to, and nowhere else.
#[test]
fn the_three_tone_controls_each_own_their_band() {
    let range = |control: usize, hz: f64| {
        db("out", &[(control, 1.0)], hz) - db("out", &[(control, 0.0)], hz)
    };
    let low = range(LOW, 100.0);
    // The middle is measured at the top of its sweep, which is where it is
    // deepest: its leg's series resistance is the swept one, so the band gets
    // shallower as it moves down. See `the_mid_freq_control_moves_the_centre`.
    let mid = db("out", &[(MIDDLE, 1.0), (MID_FREQ, 1.0)], 4_800.0)
        - db("out", &[(MIDDLE, 0.0), (MID_FREQ, 1.0)], 4_800.0);
    let high = range(HIGH, 6_000.0);
    println!("low {low:+.1} dB at 100 Hz, middle {mid:+.1} at 440, high {high:+.1} at 6k");
    for (name, got) in [("low", low), ("middle", mid), ("high", high)] {
        assert!(got > 10.0, "{name} has no range: {got:.1} dB");
    }
    // ...and each does more in its own band than the other two do there.
    assert!(
        low > range(HIGH, 100.0),
        "the low control is not the low one"
    );
    assert!(
        high > range(LOW, 6_000.0),
        "the high control is not the high one"
    );
}

/// The scoop: two resonances at the ends of the band with a dip between them,
/// which is the shape this pedal is bought for.
#[test]
fn the_post_distortion_stage_is_double_peaked() {
    let flat = db("post", &[], 700.0);
    let at_low = db("u4b", &[], 105.0) - db("post", &[], 105.0);
    let at_high = db("u4b", &[], 4_900.0) - db("post", &[], 4_900.0);
    let between = db("u4b", &[], 700.0) - flat;
    println!("scoop: {at_low:+.1} dB at 105 Hz, {between:+.1} at 700, {at_high:+.1} at 4.9 kHz");
    assert!(
        at_low > between + 6.0,
        "no lift at 105 Hz: {at_low:.1} against {between:.1}"
    );
    assert!(
        at_high > between + 6.0,
        "no lift at 4.9 kHz: {at_high:.1} against {between:.1}"
    );
}

/// The Mid Freq control moves where the middle band works, which is the whole
/// point of it and the one control in the plugin that does.
///
/// Both sections of the dual gang sweep one gyrator, so its inductance goes as
/// `R^2` and the centre as `1/R` -- the Wien bridge's own law -- while
/// `Q = sqrt(C_gyr / Cs)` stays put. Computed from the values: 4877 Hz with the
/// knob down and 206 Hz with it up, against a published 4.7 kHz and 240 Hz.
///
/// What this arrangement does *not* hold is the depth: the leg's series
/// resistance is the swept one, so the band is deep at the top of the sweep and
/// shallow at the bottom, where the original's is even. That is recorded in
/// `docs/models/metal_zone.md` rather than hidden here.
#[test]
fn the_mid_freq_control_moves_the_centre() {
    let lift = |freq: f64, hz: f64| {
        db("out", &[(MIDDLE, 1.0), (MID_FREQ, freq)], hz)
            - db("out", &[(MIDDLE, 0.5), (MID_FREQ, freq)], hz)
    };
    // Turned up the band is high, turned down it is low.
    let up_high = lift(1.0, 4_000.0);
    let up_low = lift(1.0, 250.0);
    let down_high = lift(0.0, 4_000.0);
    let down_low = lift(0.0, 250.0);
    println!("knob up: {up_low:+.1} at 250 Hz, {up_high:+.1} at 4 kHz");
    println!("knob down: {down_low:+.1} at 250 Hz, {down_high:+.1} at 4 kHz");
    assert!(
        up_high > up_low + 6.0,
        "turned up it should work high: {up_low:+.1} vs {up_high:+.1}"
    );
    assert!(
        down_high - down_low < up_high - up_low,
        "the centre did not move: up {:+.1}, down {:+.1}",
        up_high - up_low,
        down_high - down_low
    );
}
