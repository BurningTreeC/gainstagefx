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

/// Each slider cuts hardest at the frequency the drawing labels it, and the
/// slider two bands away moves that frequency far less.
///
/// This is what says the component values were read correctly and hung
/// together correctly. The branch resonances alone land at 88, 372, 723, 1576
/// and 4823 Hz -- nowhere near the labels -- because a branch is loaded by its
/// own slider and by the four bands beside it. So the labels can only come out
/// right if the whole network is right, which makes this a strong test of a
/// weak-looking claim.
#[test]
fn every_slider_cuts_the_band_it_is_named_for() {
    let flat = [0.5f64; 5];
    for (band, centre) in CENTRES.into_iter().enumerate() {
        let mut cut = flat;
        cut[band] = 0.0;
        let base = at(flat, centre);
        let own = at(cut, centre) - base;
        assert!(
            own < -6.0,
            "the {centre:.0} Hz slider should cut its own band: {own:.1} dB"
        );
        // A band two along has to be left comparatively alone. Not untouched:
        // these are broad, deliberately, and Mesa's own figure is that the
        // equaliser is not constant-Q.
        let far = (band + 2) % 5;
        let mut other = flat;
        other[far] = 0.0;
        let bleed = at(other, centre) - base;
        assert!(
            bleed > own + 4.0,
            "the {:.0} Hz slider moves {centre:.0} Hz by {bleed:.1} dB, nearly as \
             much as {centre:.0} Hz's own slider does at {own:.1} dB -- the bands \
             are not separated",
            CENTRES[far]
        );
    }
}

/// Sliders at their centres is the reference the rest is measured from, and it
/// has to be the same at every frequency to within the network's own tilt --
/// otherwise "flat" is not flat and every setting inherits a shape nobody
/// asked for.
#[test]
fn the_centre_position_is_the_reference() {
    let flat = [0.5f64; 5];
    let levels: Vec<f64> = [40.0, 240.0, 750.0, 2200.0, 10_000.0]
        .into_iter()
        .map(|hz| at(flat, hz))
        .collect();
    let high = levels.iter().cloned().fold(f64::MIN, f64::max);
    let low = levels.iter().cloned().fold(f64::MAX, f64::min);
    assert!(
        high - low < 6.0,
        "with every slider centred the network tilts by {:.1} dB across the band",
        high - low
    );
}
