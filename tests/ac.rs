//! The AC solver, checked against networks whose answers can be written down.
//!
//! This is the point of having it: a filter's response becomes something that
//! can be *asserted*, not squinted at. Every case here has a closed form, so a
//! disagreement is the solver's fault and not a matter of opinion.

use gainstagefx::dsp::ac;
use gainstagefx::dsp::netlist::{Fault, Netlist, Taper};

/// A first order RC low pass: `-3.01 dB` at its corner, then six decibels an
/// octave, and `-45` degrees of phase at the corner.
#[test]
fn a_first_order_low_pass_is_exactly_first_order() {
    let r = 1_000.0;
    let c = 159.154_943e-9; // 1 kHz corner
    let corner = 1.0 / (std::f64::consts::TAU * r * c);

    let mut net = Netlist::new("rc low pass");
    // A stiff source, so the source impedance does not join in.
    net.input("in", 0.001)
        .resistor("in", "out", r)
        .capacitor("out", "gnd", c);
    let circuit = net.build("out").expect("builds");

    let at = |hz: f64| ac::solve(&circuit, &[], hz).db();
    let phase = |hz: f64| ac::solve(&circuit, &[], hz).phase();

    assert!((at(corner) - -3.0103).abs() < 0.01, "corner: {}", at(corner));
    assert!(
        (phase(corner) - -45.0).abs() < 0.1,
        "corner phase: {}",
        phase(corner)
    );
    // Twenty decibels per decade, measured where the asymptote holds -- not
    // from the corner, which is already 3.01 dB down and would make the first
    // decade read -17.03.
    // Two decades out, where the asymptote actually holds: at ten times the
    // corner the response is still 0.04 dB off it.
    let slope = at(corner * 1000.0) - at(corner * 100.0);
    assert!((slope - -20.0).abs() < 0.01, "slope per decade: {slope} dB");
    assert!(
        (at(corner * 10.0) - -20.0432).abs() < 0.01,
        "a decade above: {} dB",
        at(corner * 10.0)
    );
    // And it passes everything well below.
    assert!(at(corner / 100.0).abs() < 0.001);
}

/// The same the other way up.
#[test]
fn a_first_order_high_pass_is_its_mirror() {
    let r = 10_000.0;
    let c = 15.915_494e-9; // 1 kHz
    let corner = 1.0 / (std::f64::consts::TAU * r * c);

    let mut net = Netlist::new("rc high pass");
    net.input("in", 0.001)
        .capacitor("in", "out", c)
        .resistor("out", "gnd", r);
    let circuit = net.build("out").expect("builds");

    let at = |hz: f64| ac::solve(&circuit, &[], hz).db();
    assert!((at(corner) - -3.0103).abs() < 0.01, "corner: {}", at(corner));
    assert!(
        (ac::solve(&circuit, &[], corner).phase() - 45.0).abs() < 0.1,
        "corner phase"
    );
    let slope = at(corner / 1000.0) - at(corner / 100.0);
    assert!((slope - -20.0).abs() < 0.01, "slope per decade: {slope} dB");
    assert!(
        (at(corner / 10.0) - -20.0432).abs() < 0.01,
        "a decade below: {} dB",
        at(corner / 10.0)
    );
}

/// A series LC across the signal is a notch: a short at resonance and an open
/// circuit either side of it. This is the section a scooped sound needs, and
/// the first version got it wrong by putting the damping resistor across the
/// capacitor instead of in series -- which makes a treble shelf, not a notch.
/// Here that is one assertion.
#[test]
fn a_series_lc_to_ground_is_a_notch_and_not_a_shelf() {
    let feed = 10_000.0;
    let henry = 0.5;
    let hz = 650.0;
    let farads = 1.0 / ((std::f64::consts::TAU * hz).powi(2) * henry);
    let damping = 2_200.0;

    let mut net = Netlist::new("mid notch");
    net.input("in", 0.001)
        .resistor("in", "out", feed)
        // Damping in series with the pair, which is what makes it a notch.
        .resistor("out", "notch_r", damping)
        .inductor("notch_r", "notch_l", henry)
        .capacitor("notch_l", "gnd", farads)
        .resistor("out", "gnd", 100_000.0);
    let circuit = net.build("out").expect("builds");

    let at = |hz: f64| ac::solve(&circuit, &[], hz).db();
    let deep = at(hz);
    let below = at(hz / 8.0);
    let above = at(hz * 8.0);

    assert!(deep < below - 6.0, "not cut at resonance: {deep} vs {below}");
    assert!(deep < above - 6.0, "not cut at resonance: {deep} vs {above}");
    // A shelf would leave one side low; a notch returns on both.
    assert!(
        (below - above).abs() < 1.5,
        "one side did not come back: {below} vs {above} -- that is a shelf"
    );
}

/// A pot's two halves both load the circuit, and turning it has to move the
/// answer in the direction its taper says.
#[test]
fn a_pot_divides_and_its_taper_points_the_right_way() {
    let mut net = Netlist::new("divider");
    net.input("in", 0.001)
        .pot("in", "out", "gnd", 100_000.0, Taper::Linear, 0)
        .resistor("out", "gnd", 10_000_000.0);
    let circuit = net.build("out").expect("builds");

    let at = |p: f64| ac::solve(&circuit, &[p], 1_000.0).magnitude();
    assert!(at(0.9) > at(0.5), "turning up should pass more");
    assert!(at(0.5) > at(0.1), "turning down should pass less");
    // Half a linear track is half the voltage, near enough with a light load.
    assert!((at(0.5) - 0.5).abs() < 0.01, "midpoint: {}", at(0.5));

    let mut net = Netlist::new("reversed");
    net.input("in", 0.001)
        .pot("in", "out", "gnd", 100_000.0, Taper::ReverseLinear, 0)
        .resistor("out", "gnd", 10_000_000.0);
    let reversed = net.build("out").expect("builds");
    let at = |p: f64| ac::solve(&reversed, &[p], 1_000.0).magnitude();
    assert!(at(0.9) < at(0.5), "a reversed track runs the other way");
}

/// Two sections in a row are not two responses multiplied: the second loads
/// the first. A network that ignored that would read as the product.
#[test]
fn sections_load_each_other() {
    let c = 15.915_494e-9;
    let build = |second: bool| {
        let mut net = Netlist::new("cascade");
        net.input("in", 0.001)
            .resistor("in", "mid", 10_000.0)
            .capacitor("mid", "gnd", c);
        if second {
            net.resistor("mid", "out", 10_000.0).capacitor("out", "gnd", c);
        } else {
            net.resistor("mid", "out", 10_000.0)
                .resistor("out", "gnd", 1_000_000_000.0);
        }
        net.build("out").expect("builds")
    };

    let one = ac::solve(&build(false), &[], 1_000.0).db();
    let two = ac::solve(&build(true), &[], 1_000.0).db();
    let product = one * 2.0;
    assert!(
        (two - product).abs() > 0.5,
        "the sections did not load each other: {two} dB against {product} dB for the product"
    );
}
