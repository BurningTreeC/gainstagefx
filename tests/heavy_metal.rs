//! The Heavy Metal, against Boss's drawing and the published analyses of it.
use gainstagefx::circuits::heavy_metal::{self, DIST, HIGH, LEVEL, LOW, LEVEL_REST};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn sim() -> Simulation {
    let mut s = Simulation::new(heavy_metal::build(10_000.0, 470_000.0).unwrap(), RATE);
    s.set_control(LEVEL, 1.0);
    s.set_control(DIST, 0.5);
    s.set_control(LOW, 0.5);
    s.set_control(HIGH, 0.5);
    s.find_operating_point();
    s
}

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = sim();
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    s.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// It solves at all, and silence stays silent.
#[test]
fn it_builds_and_settles() {
    let mut s = sim();
    for _ in 0..(RATE as usize / 10) {
        let y = s.process(0.0);
        assert!(y.is_finite());
    }
    let tail: Vec<f64> = (0..256).map(|_| s.process(0.0)).collect();
    let rms = (tail.iter().map(|v| v * v).sum::<f64>() / tail.len() as f64).sqrt();
    assert!(rms < 1e-3, "silence is not silent: {rms}");
}

/// The plugin's centre Level position maps to `LEVEL_REST`, so that physical
/// position must stay close to broadband unity with the sound controls centred.
/// This guards the calibration regression where 0.60 was still being used after
/// the HM-2 topology rewrite and was measured at +6.88 dB.
#[test]
fn level_rest_is_broadband_unity_with_centred_controls() {
    const CAL_RATE: f64 = 48_000.0;
    let mut s = Simulation::new(heavy_metal::build(10_000.0, 470_000.0).unwrap(), CAL_RATE);
    s.set_control(DIST, 0.5);
    s.set_control(LOW, 0.5);
    s.set_control(HIGH, 0.5);
    s.set_control(LEVEL, LEVEL_REST);
    s.find_operating_point();

    let sample = |k: usize| {
        let t = k as f64 / CAL_RATE;
        0.122
            * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.50
                + (std::f64::consts::TAU * 123.5 * t).sin() * 0.30
                + (std::f64::consts::TAU * 246.9 * t).sin() * 0.20)
    };

    let warm = (CAL_RATE / 2.0) as usize;
    for k in 0..warm {
        s.process(sample(k));
    }

    let n = CAL_RATE as usize;
    let mut in_sq = 0.0;
    let mut out_sq = 0.0;
    for k in warm..warm + n {
        let x = sample(k);
        let y = s.process(x);
        in_sq += x * x;
        out_sq += y * y;
    }
    let gain_db = 10.0 * (out_sq / in_sq.max(1e-24)).max(1e-24).log10();
    println!("HM-2 LEVEL_REST={LEVEL_REST:.3}: {gain_db:+.2} dB broadband");
    assert!(
        gain_db.abs() < 1.0,
        "HM-2 LEVEL_REST is not near broadband unity: {gain_db:+.2} dB"
    );
}

/// The Dist control does something, monotonically.
#[test]
fn the_dist_control_turns_the_gain_up() {
    let level = |p: f64| at(&[(DIST, p)], 440.0, 0.05).fundamental().magnitude();
    let (down, mid, up) = (level(0.0), level(0.5), level(1.0));
    println!("dist 0 {down:.4}, 0.5 {mid:.4}, 1 {up:.4}");
    assert!(down < mid && mid < up, "{down} {mid} {up}");
}

/// The two Colour Mix controls pull in opposite parts of the band: the L one at
/// the bottom, the H one above it. The published analysis puts the low gyrator
/// "at around 80Hz" and the mid pair "between about 900Hz and 1.3kHz".
#[test]
fn colour_mix_moves_the_two_ends_separately() {
    let at_hz = |controls: &[(usize, f64)], hz: f64| {
        20.0 * at(controls, hz, 0.02).fundamental().magnitude().max(1e-12).log10()
    };
    let low_up = at_hz(&[(LOW, 1.0), (HIGH, 0.0)], 80.0);
    let low_down = at_hz(&[(LOW, 0.0), (HIGH, 0.0)], 80.0);
    let high_up = at_hz(&[(LOW, 0.0), (HIGH, 1.0)], 1_100.0);
    let high_down = at_hz(&[(LOW, 0.0), (HIGH, 0.0)], 1_100.0);
    println!("80 Hz: {low_down:.1} -> {low_up:.1};  1.1 kHz: {high_down:.1} -> {high_up:.1}");
    assert!(low_up > low_down + 3.0, "L does not lift 80 Hz: {low_down} -> {low_up}");
    assert!(high_up > high_down + 3.0, "H does not lift 1.1 kHz: {high_down} -> {high_up}");
    // ...and each one does more where it lives than where the other does.
    let low_at_high = at_hz(&[(LOW, 1.0), (HIGH, 0.0)], 1_100.0) - high_down;
    assert!(
        low_up - low_down > low_at_high,
        "the L control is not the low one: {:.1} at 80 Hz against {:.1} at 1.1 kHz",
        low_up - low_down,
        low_at_high
    );
}

/// The coring gate: D6 and D7 sit *in series* with the signal, so a small
/// signal does not get through at all. That is the pedal's gated decay, and it
/// is the one thing in the catalogue that behaves this way -- every other
/// clipper here passes small signals untouched.
///
/// What a gate looks like from outside is **gain that rises with level**, which
/// is the opposite of what every other pedal here does: a clipper's gain always
/// falls as it runs out of room. So the two levels compared are both well below
/// where this one starts squaring off, and the gate shows up as the quiet one
/// being materially worse off than the moderate one. The exact amount is not
/// specified by Boss and depends strongly on the germanium parts, source level,
/// and the rest of the gain structure, so this test deliberately checks the
/// *shape* of the gate rather than asserting an invented 20 dB hardware spec.
#[test]
fn the_coring_gate_holds_small_signals_back() {
    let gain_at = |volts: f64| at(&[(DIST, 0.5)], 440.0, volts).gain_db();
    let under = gain_at(1e-6);
    let over = gain_at(1e-3);
    println!("gate: {under:.1} dB at 1 uV against {over:.1} dB at 1 mV");
    assert!(
        over > under + 12.0,
        "the gate is not holding the quiet one back: {under:.1} vs {over:.1}"
    );
    // And it really is a threshold rather than a slope: past it the pedal is
    // into its clipping, where gain falls again.
    let loud = gain_at(0.122);
    assert!(loud < over, "{over:.1} then {loud:.1}");
}

/// The DIST control must remain a realtime-safe operating control across its
/// whole physical travel. This specifically guards the regression where only
/// roughly the first five percent sounded sane and every larger setting drove
/// Newton into repeated fallbacks/dropouts.
#[test]
fn distortion_sweep_stays_finite_and_settled() {
    const POSITIONS: &[f64] = &[0.0, 0.01, 0.025, 0.05, 0.10, 0.25, 0.50, 0.75, 1.0];
    const SAMPLES: usize = 12_000;

    for &position in POSITIONS {
        let mut s = sim();
        s.set_control(DIST, position);
        // The Swedish-death use case has both Colour Mix controls dimed, so
        // exercise that load instead of hiding behind the neutral EQ setting.
        s.set_control(LOW, 1.0);
        s.set_control(HIGH, 1.0);
        s.find_operating_point();

        let (solves0, passes0, unsettled0, _) = s.statistics();
        let (backtracks0, fallbacks0, nonfinite0) = s.health();
        let mut peak = 0.0_f64;

        for k in 0..SAMPLES {
            let t = k as f64 / RATE;
            // A deliberately hot low-guitar waveform. The old broken DIST
            // topology became pathological here as soon as the knob moved
            // beyond its tiny usable region.
            let x = 0.122
                * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.65
                    + (std::f64::consts::TAU * 123.5 * t).sin() * 0.35);
            let y = s.process(x);
            assert!(y.is_finite(), "DIST {position:.3}: non-finite output at sample {k}");
            peak = peak.max(y.abs());
        }

        let (solves1, passes1, unsettled1, _) = s.statistics();
        let (backtracks1, fallbacks1, nonfinite1) = s.health();
        let solves = solves1.saturating_sub(solves0).max(1);
        let passes = passes1.saturating_sub(passes0);
        let unsettled = unsettled1.saturating_sub(unsettled0);
        let backtracks = backtracks1.saturating_sub(backtracks0);
        let fallbacks = fallbacks1.saturating_sub(fallbacks0);
        let nonfinite = nonfinite1.saturating_sub(nonfinite0);

        println!(
            "HM-2 DIST={position:>5.3} passes/solve={:.3} unsettled={unsettled} backtracks={backtracks} fallbacks={fallbacks} nonfinite={nonfinite} peak={peak:.4}",
            passes as f64 / solves as f64,
        );

        assert_eq!(unsettled, 0, "DIST {position:.3}: unsettled nonlinear solves");
        assert_eq!(nonfinite, 0, "DIST {position:.3}: non-finite Newton corrections");
        assert!(peak < 20.0, "DIST {position:.3}: implausible runaway output {peak}");
    }
}
