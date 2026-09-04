//! The TS808, checked against what its own schematic says it must do.
//!
//! Every figure here is worked out from the drawing rather than chosen to fit
//! what the model happens to produce. A model of a specific circuit is either
//! that circuit or it is nothing, and the only way to tell is arithmetic done
//! independently of it.

use gainstagefx::circuits::ts808::{self, DRIVE, LEVEL, TONE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

// Straight off the schematic.
const R4: f64 = 4_700.0;
const R6: f64 = 51_000.0;
const RV1: f64 = 500_000.0;
const C3: f64 = 47e-9;

fn at(node: &str, hz: f64, volts: f64, drive: f64, tone: f64) -> measure::Measured {
    let c = ts808::tap(10_000.0, 470_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(DRIVE, drive);
    sim.set_control(TONE, tone);
    sim.set_control(LEVEL, 1.0);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

/// The clipping stage is a non-inverting amplifier, so above the corner where
/// C3 stops mattering its gain is `1 + (R6 + RV1) / R4`. With the pot shut it
/// is `1 + R6 / R4`. Both are arithmetic from the drawing.
#[test]
fn the_clipping_stage_has_the_gain_its_resistors_say() {
    let wide = 20.0 * (1.0 + (R6 + RV1) / R4).log10();
    let shut = 20.0 * (1.0 + R6 / R4).log10();
    // Measured at 2 kHz: above the 720 Hz corner, below where C4 starts to
    // roll the top off.
    let got_wide = at("u1a", 2_000.0, 0.0002, 1.0, 0.5).gain_db();
    let got_shut = at("u1a", 2_000.0, 0.0002, 0.0, 0.5).gain_db();
    println!("drive full: {got_wide:.1} dB against {wide:.1} predicted");
    println!("drive shut: {got_shut:.1} dB against {shut:.1} predicted");
    assert!(
        (got_wide - wide).abs() < 3.0,
        "full drive should be near {wide:.1} dB and is {got_wide:.1}"
    );
    assert!(
        (got_shut - shut).abs() < 3.0,
        "shut it should be near {shut:.1} dB and is {got_shut:.1}"
    );
}

/// The famous hump. The gain leg is `R4` in series with `C3`, so the stage
/// only reaches its full gain above `1 / (2 pi R4 C3)` -- 720 Hz on these
/// values -- and below that it falls away at six decibels an octave. That
/// rising slope is what leaves a Screamer's bottom end alone, and it is the
/// whole reason one sits in front of an amplifier rather than replacing it.
#[test]
fn the_gain_leg_puts_the_corner_where_the_arithmetic_says() {
    let corner = 1.0 / (std::f64::consts::TAU * R4 * C3);
    println!("corner from the values: {corner:.0} Hz");
    assert!((corner - 720.0).abs() < 40.0, "the arithmetic itself has moved");

    let plateau = at("u1a", 4.0 * corner, 0.0002, 1.0, 0.5).gain_db();
    let at_corner = at("u1a", corner, 0.0002, 1.0, 0.5).gain_db();
    let octave_below = at("u1a", corner / 2.0, 0.0002, 1.0, 0.5).gain_db();
    let two_below = at("u1a", corner / 4.0, 0.0002, 1.0, 0.5).gain_db();
    println!(
        "plateau {plateau:.1}, corner {at_corner:.1}, -1 oct {octave_below:.1}, \
         -2 oct {two_below:.1}"
    );
    // A first order slope: six decibels for each halving, once clear of the
    // corner itself.
    let slope = octave_below - two_below;
    assert!(
        (slope - 6.0).abs() < 1.5,
        "below the corner it should fall six decibels an octave and falls {slope:.1}"
    );
    assert!(
        at_corner < plateau - 1.5 && at_corner > plateau - 5.0,
        "at the corner it should be a few decibels down from the plateau: \
         {at_corner:.1} against {plateau:.1}"
    );
}

/// A stock Tube Screamer clips with two 1N4148 facing opposite ways, and that
/// is symmetric -- so it makes odd harmonics and no even ones. This is not a
/// shortcoming to be fixed: it is what the pedal is. The asymmetric one is a
/// different pedal.
#[test]
fn the_stock_pedal_clips_symmetrically() {
    let m = at("out", 1_000.0, 0.05, 1.0, 0.5);
    println!(
        "{:.1} % distortion, 2nd {:.2} %, 3rd {:.1} %",
        m.thd_percent(),
        m.harmonic_percent(2),
        m.harmonic_percent(3)
    );
    assert!(
        m.harmonic_percent(2) < 1.0,
        "two matched diodes facing opposite ways should make no even harmonic: \
         {:.2} %",
        m.harmonic_percent(2)
    );
    assert!(m.harmonic_percent(3) > 4.0, "and should be making third");
}

/// The tone control works on the top of the band and leaves the bottom where
/// it is -- C6 and R8 only come into it once the capacitor is worth something.
#[test]
fn the_tone_control_works_on_the_top_of_the_band() {
    let bottom = (
        at("out", 100.0, 0.0005, 1.0, 0.0).gain_db(),
        at("out", 100.0, 0.0005, 1.0, 1.0).gain_db(),
    );
    let top = (
        at("out", 4_000.0, 0.0005, 1.0, 0.0).gain_db(),
        at("out", 4_000.0, 0.0005, 1.0, 1.0).gain_db(),
    );
    println!(
        "100 Hz: {:.1} to {:.1};  4 kHz: {:.1} to {:.1}",
        bottom.0, bottom.1, top.0, top.1
    );
    assert!(
        (bottom.1 - bottom.0).abs() < 2.0,
        "the bottom should barely move: {:.1} against {:.1}",
        bottom.0,
        bottom.1
    );
    assert!(
        (top.0 - top.1) > 6.0,
        "the top should move a great deal: {:.1} against {:.1}",
        top.0,
        top.1
    );
}

/// Both buffers are emitter followers, which is to say they pass the signal
/// and do not amplify it. If either has gain, it is wired wrong.
#[test]
fn the_buffers_are_followers() {
    let buf = at("buf", 1_000.0, 0.01, 1.0, 0.5).gain_db();
    println!("input buffer {buf:.2} dB");
    assert!(
        buf.abs() < 1.5,
        "an emitter follower should be about unity and is {buf:.2} dB"
    );
    // And the output buffer likewise: the level control is the only thing
    // between U1B and the jack that changes anything.
    let before = at("lvl", 1_000.0, 0.01, 1.0, 0.5).gain_db();
    let after = at("out", 1_000.0, 0.01, 1.0, 0.5).gain_db();
    println!("across the output buffer: {before:.2} to {after:.2} dB");
    assert!((before - after).abs() < 1.5);
}

/// And the level control has to reach silence, since it is the only volume on
/// the pedal.
#[test]
fn the_level_control_reaches_both_ends() {
    let c = ts808::build(10_000.0, 470_000.0).expect("builds");
    let gain_at = |level: f64| {
        let mut sim = Simulation::new(c.clone(), RATE);
        sim.set_control(DRIVE, 1.0);
        sim.set_control(TONE, 0.5);
        sim.set_control(LEVEL, level);
        let t = Tone::near(RATE, 16_384, 1_000.0, 0.005);
        measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x)).gain_db()
    };
    let (shut, open) = (gain_at(0.0), gain_at(1.0));
    println!("level: {shut:.1} dB to {open:.1} dB");
    assert!(open - shut > 40.0, "only {:.1} dB of level", open - shut);
}
