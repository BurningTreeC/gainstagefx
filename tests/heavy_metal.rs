//! The Heavy Metal, against Boss's drawing and the published analyses of it.
use gainstagefx::circuits::heavy_metal::{self, DIST, HIGH, LEVEL, LOW};
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
/// being tens of decibels worse off than the moderate one.
#[test]
fn the_coring_gate_holds_small_signals_back() {
    let gain_at = |volts: f64| at(&[(DIST, 0.5)], 440.0, volts).gain_db();
    let under = gain_at(1e-6);
    let over = gain_at(1e-3);
    println!("gate: {under:.1} dB at 1 uV against {over:.1} dB at 1 mV");
    assert!(
        over > under + 20.0,
        "the gate is not holding the quiet one back: {under:.1} vs {over:.1}"
    );
    // And it really is a threshold rather than a slope: past it the pedal is
    // into its clipping, where gain falls again.
    let loud = gain_at(0.122);
    assert!(loud < over, "{over:.1} then {loud:.1}");
}
