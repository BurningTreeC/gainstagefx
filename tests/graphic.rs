//! The Mark IIC+'s five band graphic equaliser.
//!
//! §9.8: this sits late in the preamplifier, after four gain stages and before
//! the power section, and that ordering is why it matters -- a low band lifted
//! here lands on the phase inverter. It is also the part everyone recognises:
//! the scooped middle that the amplifier is known for is this network, not the
//! Fender stack ahead of it.
//!
//! What is checked here is the thing a band-pass network has to get right and
//! the thing that is easiest to get wrong without noticing: that each slider
//! moves its own band and not its neighbour's.

use gainstagefx::circuits::markiic;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// Gain at `hz` with the sliders where `set` puts them, in dB.
fn at(set: [f64; 5], hz: f64) -> f64 {
    let mut sim = Simulation::new(
        markiic::graphic(10_000.0, 1_000_000.0).expect("builds"),
        RATE,
    );
    for (i, &v) in set.iter().enumerate() {
        sim.set_control(i, v);
    }
    sim.find_operating_point();
    let n = (RATE * 0.3) as usize;
    let (mut si, mut so) = (0.0f64, 0.0f64);
    for k in 0..n {
        let t = k as f64 / RATE;
        let x = (std::f64::consts::TAU * hz * t).sin();
        let y = sim.process(x);
        if k > n / 2 {
            si += x * x;
            so += y * y;
        }
    }
    20.0 * (so / si).sqrt().max(1e-12).log10()
}

const CENTRES: [f64; 5] = [60.0, 240.0, 750.0, 2200.0, 6600.0];

/// Each slider cuts *and boosts* the band it is named for, by about the same
/// amount either way, and the slider two bands away moves that band far less.
///
/// The equaliser is a feedback network: every slider's track runs between the
/// amplifier's two inputs, so its branch shunts the input at one end (cut) and
/// the feedback at the other (boost). Built as a network to ground it could cut
/// twelve decibels and boost less than three.
#[test]
fn every_slider_cuts_and_boosts_its_own_band() {
    let flat = [0.5f64; 5];
    for (band, centre) in CENTRES.into_iter().enumerate() {
        let base = at(flat, centre);
        let mut cut = flat;
        cut[band] = 0.0;
        let mut boost = flat;
        boost[band] = 1.0;
        let (down, up) = (at(cut, centre) - base, at(boost, centre) - base);
        assert!(down < -8.0, "{centre:.0} Hz cuts only {down:.1} dB");
        assert!(up > 8.0, "{centre:.0} Hz boosts only {up:.1} dB");
        assert!(
            (up + down).abs() < 2.0,
            "{centre:.0} Hz: {down:.1} / {up:+.1} dB"
        );
        let far = (band + 2) % 5;
        let mut other = flat;
        other[far] = 1.0;
        let bleed = at(other, centre) - base;
        assert!(
            bleed < up - 4.0,
            "the {:.0} Hz slider moves {centre:.0} Hz by {bleed:.1} dB against its own {up:.1}",
            CENTRES[far]
        );
    }
}

/// Every slider centred is flat.
#[test]
fn the_centre_position_is_flat() {
    let flat = [0.5f64; 5];
    let levels: Vec<f64> = [40.0, 240.0, 750.0, 2200.0, 10_000.0]
        .into_iter()
        .map(|hz| at(flat, hz))
        .collect();
    let high = levels.iter().cloned().fold(f64::MIN, f64::max);
    let low = levels.iter().cloned().fold(f64::MAX, f64::min);
    assert!(
        high - low < 1.0,
        "centred, the equaliser tilts by {:.1} dB",
        high - low
    );
    assert!(
        high.abs() < 1.0,
        "centred, the equaliser is not unity: {high:.1} dB"
    );
}

/// A quarter of the travel does a real part of the job, rather than the whole
/// range living in the last few millimetres.
#[test]
fn a_quarter_of_the_travel_is_worth_decibels() {
    let flat = [0.5f64; 5];
    for (band, centre) in CENTRES.into_iter().enumerate() {
        let mut quarter = flat;
        quarter[band] = 0.75;
        let lift = at(quarter, centre) - at(flat, centre);
        assert!(
            (3.0..9.0).contains(&lift),
            "{centre:.0} Hz at three quarters: {lift:.1} dB"
        );
    }
}
