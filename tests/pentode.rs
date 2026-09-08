//! The power tube against its data sheet.
//!
//! A tube's constants are the one thing in a power amplifier that can be wrong
//! without anything looking wrong. The stage still biases, still amplifies,
//! still clips, still sounds like an amplifier -- and makes a fraction of its
//! rated power, because at full swing the tubes quietly run out of current.
//! That is indistinguishable, from the outside, from a power stage doing
//! exactly what a power stage is meant to do.
//!
//! It happened here. The constants usually published alongside Koren's model
//! land within a couple of milliamps at the idle point and then give a
//! transfer curve about half as steep as the real tube's, and the first
//! working power stage made a sixth of its rated power for that reason alone.
//! So both ends are pinned here, not just the one that is easy to check.
//!
//! Sheet figures are the RCA/GE 6L6GC's.

use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::netlist::PentodeSpec;

fn tube(spec: PentodeSpec) -> Pentode {
    Pentode::new(0, 1, 2, 3, 1.0, spec)
}

/// Where a push-pull pair sits when nobody is playing: 450 V on the plate,
/// 400 V on the screen, 37 volts of negative bias. The sheet says 26 mA of
/// plate current and 2.5 mA of screen.
#[test]
fn the_6l6gc_idles_where_the_sheet_says() {
    let (ip, ig2) = tube(PentodeSpec::T6L6GC).currents(450.0, -37.0, 400.0);
    let (ip, ig2) = (ip * 1000.0, ig2 * 1000.0);
    assert!(
        (22.0..=30.0).contains(&ip),
        "the sheet idles at 26 mA and this tube idles at {ip:.1}"
    );
    assert!(
        (1.5..=4.0).contains(&ig2),
        "the sheet draws 2.5 mA of screen current and this tube draws {ig2:.1}"
    );
}

/// And where it goes when it is asked for everything: grid at zero, plate
/// pulled down towards the knee. The sheet's curves reach something over three
/// hundred milliamps. This is the end that was wrong, and it is the end that
/// sets how much power the amplifier makes.
#[test]
fn the_6l6gc_delivers_its_current_at_full_swing() {
    let t = tube(PentodeSpec::T6L6GC);
    let (wide_open, _) = t.currents(450.0, 0.0, 400.0);
    assert!(
        wide_open > 0.250,
        "at zero bias this tube gives {:.0} mA, where the sheet shows over \
         three hundred. An output stage built on it will bias perfectly and \
         make a fraction of its power",
        wide_open * 1000.0
    );

    // And it must hold that current as the plate is pulled down, because that
    // is what a pentode is for: the long flat shelf is why a power tube is a
    // current source into its transformer rather than a resistor.
    let (at_the_knee, _) = t.currents(50.0, 0.0, 400.0);
    assert!(
        at_the_knee > wide_open * 0.75,
        "pulled down to fifty volts the current collapses from {:.0} mA to \
         {:.0}, which is a triode's plate curve and not a pentode's",
        wide_open * 1000.0,
        at_the_knee * 1000.0
    );
}

/// The transfer curve's *steepness* is the thing that was wrong, so it is
/// checked directly rather than left to follow from the two endpoints.
#[test]
fn the_6l6gc_transfer_is_as_steep_as_the_sheets() {
    let t = tube(PentodeSpec::T6L6GC);
    let (idle, _) = t.currents(450.0, -37.0, 400.0);
    let (open, _) = t.currents(450.0, 0.0, 400.0);
    let steepness = open / idle;
    assert!(
        (9.0..=18.0).contains(&steepness),
        "the sheet goes from 26 mA to something over 300 across the bias, \
         which is a factor of about thirteen; this tube manages {steepness:.1}"
    );
}

/// Every tube in the catalogue conducts more as its grid rises, draws grid
/// current once the grid goes positive, and gives nothing at all with no
/// screen voltage on it. None of that is interesting until one of them stops
/// doing it.
#[test]
fn every_power_tube_behaves_like_one() {
    for (name, spec) in [
        ("6L6GC", PentodeSpec::T6L6GC),
        ("EL34", PentodeSpec::EL34),
        ("EL84", PentodeSpec::EL84),
    ] {
        let t = tube(spec);
        let (dead, _) = t.currents(450.0, -37.0, 0.0);
        assert_eq!(dead, 0.0, "{name} conducts with no screen voltage");

        let mut last = -1.0;
        for step in 0..=20 {
            let vg = -60.0 + 3.0 * step as f64;
            let (ip, ig2) = t.currents(450.0, vg, 400.0);
            assert!(ip.is_finite() && ig2.is_finite(), "{name} is not finite at {vg} V");
            assert!(ip >= last, "{name} conducts less at {vg} V than below it");
            last = ip;
        }
    }
}

/// Two tubes in parallel are one tube passing twice the current. This is the
/// approximation that lets a four-tube amplifier cost the solver two devices
/// rather than four, and it is only true while the parallel tubes see the same
/// voltages -- which, sharing a socket's worth of wiring, they do.
#[test]
fn paralleled_tubes_pass_their_share() {
    let one = Pentode::new(0, 1, 2, 3, 1.0, PentodeSpec::T6L6GC);
    let two = Pentode::new(0, 1, 2, 3, 2.0, PentodeSpec::T6L6GC);
    let (a, ga) = one.currents(450.0, -20.0, 400.0);
    let (b, gb) = two.currents(450.0, -20.0, 400.0);
    assert!((b - 2.0 * a).abs() < 1e-12, "{b} is not twice {a}");
    assert!((gb - 2.0 * ga).abs() < 1e-12, "{gb} is not twice {ga}");
}
