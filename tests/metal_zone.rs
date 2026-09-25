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
    // The middle at the top of its sweep; it is as deep everywhere else (see
    // `the_mid_freq_control_moves_the_centre`).
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
/// point of it and the one control in the plugin that does -- and since the
/// factory U2a/U2b stage replaced the swept gyrator (2026-09-25), the band is
/// as deep at the bottom of the sweep as at the top, as the original's is.
/// The gyrator managed +12 dB at 4 kHz with the knob up and +1.3 dB at 250 Hz
/// with it down.
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
    // Even depth: the published +-15 dB at both ends of the sweep.
    assert!(
        up_high > 12.0,
        "only {up_high:+.1} dB at the top of the sweep"
    );
    assert!(
        down_low > 12.0,
        "only {down_low:+.1} dB at the bottom of the sweep"
    );
    assert!(
        down_low > down_high + 10.0,
        "turned down it should work low"
    );
}

/// AC analysis uses the zero-signal limit of the *same* two rail-aware devices;
/// the production/reference time-domain builder always keeps their headroom.
fn original_mid_ac() -> gainstagefx::dsp::netlist::Circuit {
    use gainstagefx::dsp::netlist::Part;
    let mut circuit = metal_zone::mid_eq_reference(1.0, 50_000.0).unwrap();
    let mut amps = 0;
    for part in &mut circuit.parts {
        if let Part::OpAmp {
            out, plus, minus, ..
        } = *part
        {
            *part = Part::LinearOpAmp { out, plus, minus };
            amps += 1;
        }
    }
    assert_eq!(amps, 2, "the factory middle stage has U2a and U2b");
    circuit
}

fn original_mid_controls(middle: f64, frequency: f64) -> [f64; 4] {
    let mut controls = [0.5; 4];
    controls[MIDDLE] = middle;
    controls[MID_FREQ] = frequency;
    controls
}

#[test]
fn original_wien_mid_preserves_depth_across_the_sweep() {
    use gainstagefx::dsp::ac;
    let circuit = original_mid_ac();
    let frequencies: Vec<f64> = (0..241)
        .map(|i| 100.0 * 100.0_f64.powf(i as f64 / 240.0))
        .collect();
    for frequency in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut boost = (0.0, f64::NEG_INFINITY);
        let mut cut = (0.0, f64::INFINITY);
        for &hz in &frequencies {
            let flat = ac::solve(&circuit, &original_mid_controls(0.5, frequency), hz).db();
            let up = ac::solve(&circuit, &original_mid_controls(1.0, frequency), hz).db() - flat;
            let down = ac::solve(&circuit, &original_mid_controls(0.0, frequency), hz).db() - flat;
            if up > boost.1 {
                boost = (hz, up);
            }
            if down < cut.1 {
                cut = (hz, down);
            }
        }
        println!(
            "original MT-2 middle: gang={frequency:.2}, boost={:+.2} dB at {:.1} Hz, cut={:+.2} dB at {:.1} Hz",
            boost.1, boost.0, cut.1, cut.0
        );
        // The published circuit analysis gives roughly +/-15 dB throughout
        // the sweep. The old substitute loses most of its depth at the low end.
        assert!((12.0..18.0).contains(&boost.1), "boost {boost:?}");
        assert!((-18.0..-12.0).contains(&cut.1), "cut {cut:?}");
        if frequency == 0.0 {
            assert!((190.0..300.0).contains(&boost.0), "low boost {boost:?}");
            assert!((190.0..300.0).contains(&cut.0), "low cut {cut:?}");
        } else if frequency == 1.0 {
            assert!(
                (4_000.0..5_500.0).contains(&boost.0),
                "high boost {boost:?}"
            );
            assert!((5_000.0..7_500.0).contains(&cut.0), "high cut {cut:?}");
        }
    }
}

#[test]
fn original_wien_mid_at_center_leaves_the_sweep_band_nearly_flat() {
    use gainstagefx::dsp::ac;
    let circuit = original_mid_ac();
    for frequency in [0.0, 0.5, 1.0] {
        // The published middle-frequency range ends at about 4.7 kHz on
        // boost and 6.3 kHz on cut. Inside that actual control range, the
        // centered Middle control should remain close to unity. Do not demand
        // the same bound at 10 kHz: the factory U2a/U2b network still contains
        // C026 and the Wien bridge, and its fixed out-of-band response is not
        // mathematically flat when the frequency gang is at an endpoint.
        for hz in [100.0, 240.0, 500.0, 1_000.0, 4_700.0, 6_300.0] {
            let response = ac::solve(&circuit, &original_mid_controls(0.5, frequency), hz);
            assert!(
                response.db().abs() < 0.65,
                "gang={frequency}, {hz} Hz: {response:?}"
            );
        }

        // Keep a looser broadband guard above the published sweep so a wiring
        // mistake cannot turn the neutral setting into a large HF shelf. The
        // factory topology is about -1.31 dB at 10 kHz at the high-frequency
        // endpoint with the documented values.
        let response = ac::solve(&circuit, &original_mid_controls(0.5, frequency), 10_000.0);
        assert!(
            response.db().abs() < 1.5,
            "gang={frequency}, 10000 Hz: {response:?}"
        );
    }
}

#[test]
fn original_wien_mid_small_signal_matches_ac_at_all_solver_rates() {
    use gainstagefx::dsp::ac;
    let circuit = original_mid_ac();
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for (middle, frequency, hz) in [(0.0, 0.0, 240.0), (1.0, 0.5, 500.0)] {
            let controls = original_mid_controls(middle, frequency);
            let mut sim =
                Simulation::new(metal_zone::mid_eq_reference(1.0, 50_000.0).unwrap(), rate);
            sim.set_control(MIDDLE, middle);
            sim.set_control(MID_FREQ, frequency);
            assert!(sim.find_operating_point());
            let tone = Tone::near(rate, 8_192, hz, 1e-3);
            let expected = ac::solve(&circuit, &controls, tone.hz()).db();
            let measured = measure::run(tone, (rate * 0.15) as usize, |x| sim.process(x));
            assert!(
                (measured.gain_db() - expected).abs() < 0.1,
                "rate={rate}, middle={middle}, gang={frequency}: {} dB vs {expected}",
                measured.gain_db()
            );
        }
    }
}
