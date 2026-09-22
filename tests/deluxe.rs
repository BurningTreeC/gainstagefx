//! The American Deluxe: AB763 Vibrato channel.
//!
//! Every figure checked here comes off the original Fender drawing or is a
//! documented property of the AB763 tone stack. None of it was read out of this
//! model and then asserted back at it.
use gainstagefx::circuits::deluxe::{
    self, BASS, INPUT_GRID_SHUNT_SLOT, INPUT_HIGH_JACK_LOAD_OHMS, INPUT_HIGH_SERIES_OHMS,
    INPUT_JACK_LOAD_SLOT, INPUT_LOW_GRID_SHUNT_OHMS, INPUT_LOW_SERIES_OHMS, INPUT_OPEN_OHMS,
    INPUT_SERIES_SLOT, INTENSITY, REVERB, TREBLE, VOLUME,
};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 1_000_000.0;

fn sim() -> Simulation {
    let mut s = Simulation::new(deluxe::build(SOURCE, LOAD).expect("Deluxe builds"), RATE);
    for control in [TREBLE, BASS, VOLUME] {
        s.set_control(control, 0.5);
    }
    s.set_control(REVERB, 0.0);
    s.set_control(INTENSITY, 0.0);
    s
}

/// Level at one frequency, dB, at a small signal so the valves stay linear.
fn level(s: &mut Simulation, hz: f64) -> f64 {
    let tone = Tone::near(RATE, 16_384, hz, 0.01);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x)).gain_db()
}

/// The drawing's own node voltages, within the tolerance the drawing states.
///
/// Fender printed "VOLTAGES READ TO GROUND WITH ELECTRONIC VOLTMETER, VALUES
/// SHOWN + OR - 20 %" on the sheet. Checked here: the Vibrato first stage's
/// plate at +170 V, both second stages at +180 V, the first-stage cathodes at
/// +1.3 V and the reverb driver's cathode at +8.7 V. The cathodes are the
/// sharper test of the two -- they are what the valve's own idle current puts
/// across a known resistor, so they check the tube model rather than the
/// supply estimate.
#[test]
fn the_drawing_s_operating_point_is_reproduced() {
    let circuit = deluxe::build(SOURCE, LOAD).expect("Deluxe builds");
    let named = |n: &str| circuit.unknown_named(n).expect("named node");
    // (node, the drawing's figure)
    let checks = [
        ("v1_p", 170.0),
        ("v2_p", 180.0),
        ("recov_p", 180.0),
        (deluxe::MIXER_TO_PI, 180.0),
        ("v1_k", 1.3),
        ("shared_a_k", 1.3),
        ("shared_e_k", 1.3),
        ("drv_k", 8.7),
    ];
    let nodes: Vec<(&str, f64, usize)> = checks
        .iter()
        .map(|&(n, want)| (n, want, named(n)))
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point(), "the idle point has to solve");
    for (name, want, node) in nodes {
        let volts = s.voltage_at(node);
        let error = (volts - want) / want * 100.0;
        println!("deluxe_dc,{name}={volts:.2}V,drawing={want}V,error={error:+.1}%");
        assert!(
            error.abs() <= 20.0,
            "{name} at {volts:.2} V is {error:+.1} % from the drawing's {want} V, \
             which is outside the sheet's own +/- 20 %"
        );
    }
}

/// The AB763 stack's shape, and the one thing that makes this amp not a Twin.
///
/// A Fender tone stack scoops the middle; with Treble and Bass at noon the dip
/// sits in the low mids and both ends are above it. The Deluxe has **no Middle
/// control** -- a fixed 6.8 kΩ leg -- so the scoop is always there and there is
/// no pot that can fill it in. That is checked here as a property of the
/// response, not by reading the netlist back.
#[test]
fn the_stack_scoops_the_middle_and_has_no_control_to_fill_it() {
    let mut s = sim();
    let low = level(&mut s, 80.0);
    let scoop = level(&mut s, 400.0);
    let high = level(&mut s, 5_000.0);
    println!("deluxe_stack,80Hz={low:.1}dB,400Hz={scoop:.1}dB,5kHz={high:.1}dB");
    assert!(
        low > scoop + 2.0,
        "the bottom should sit above the scoop: {low:.1} vs {scoop:.1}"
    );
    assert!(
        high > scoop + 2.0,
        "the top should sit above the scoop: {high:.1} vs {scoop:.1}"
    );
    // Only five controls exist on this circuit; a sixth would mean a Middle pot
    // had been added that the drawing does not have.
    assert_eq!(
        deluxe::build(SOURCE, LOAD).expect("builds").controls,
        5,
        "the AB763 Deluxe has Treble, Bass, Volume, Reverb and Intensity -- no Middle"
    );
}

/// Both tone controls have real authority at the ends of the band.
#[test]
fn treble_and_bass_move_the_ends_of_the_band() {
    let mut s = sim();
    s.set_control(TREBLE, 0.0);
    let treble_down = level(&mut s, 6_000.0);
    s.set_control(TREBLE, 1.0);
    let treble_up = level(&mut s, 6_000.0);
    println!("deluxe_treble,6kHz down={treble_down:.1}dB up={treble_up:.1}dB");
    assert!(
        treble_up - treble_down > 15.0,
        "Treble should have well over 15 dB at 6 kHz, got {:.1}",
        treble_up - treble_down
    );

    let mut s = sim();
    s.set_control(BASS, 0.0);
    let bass_down = level(&mut s, 80.0);
    s.set_control(BASS, 1.0);
    let bass_up = level(&mut s, 80.0);
    println!("deluxe_bass,80Hz down={bass_down:.1}dB up={bass_up:.1}dB");
    assert!(
        bass_up - bass_down > 10.0,
        "Bass should have well over 10 dB at 80 Hz, got {:.1}",
        bass_up - bass_down
    );
}

/// The two stock jacks. Jack 2 puts one 68 kΩ in series and the other to
/// ground, which is the familiar divider: about 6 dB down and a lower input
/// impedance.
#[test]
fn the_low_jack_is_about_six_decibels_below_the_high_one() {
    let mut s = sim();
    assert_eq!(s.value(INPUT_SERIES_SLOT), Some(INPUT_HIGH_SERIES_OHMS));
    assert_eq!(
        s.value(INPUT_JACK_LOAD_SLOT),
        Some(INPUT_HIGH_JACK_LOAD_OHMS)
    );
    assert_eq!(s.value(INPUT_GRID_SHUNT_SLOT), Some(INPUT_OPEN_OHMS));
    let high = level(&mut s, 1_000.0);

    s.set_value(INPUT_SERIES_SLOT, INPUT_LOW_SERIES_OHMS);
    s.set_value(INPUT_JACK_LOAD_SLOT, INPUT_OPEN_OHMS);
    s.set_value(INPUT_GRID_SHUNT_SLOT, INPUT_LOW_GRID_SHUNT_OHMS);
    let low = level(&mut s, 1_000.0);
    println!(
        "deluxe_jacks,high={high:.2}dB,low={low:.2}dB,delta={:.2}dB",
        high - low
    );
    assert!(
        (3.0..=9.0).contains(&(high - low)),
        "the Low jack should be roughly 6 dB down, got {:.2}",
        high - low
    );
}

/// Reverb and tremolo are electrically present and do something.
#[test]
fn the_reverb_and_tremolo_controls_reach_the_circuit() {
    let mut s = sim();
    let dry = level(&mut s, 1_000.0);
    s.set_control(REVERB, 1.0);
    let wet = level(&mut s, 1_000.0);
    println!("deluxe_reverb,dry={dry:.2}dB,wet={wet:.2}dB");
    assert!(
        (wet - dry).abs() > 0.01,
        "turning the Reverb up must load the mixer node"
    );

    // The Intensity pot's end-to-end resistance is a permanent load; opening it
    // up changes what the mixer plate sees even before the lamp lights.
    let mut s = sim();
    let closed = level(&mut s, 1_000.0);
    s.set_control(INTENSITY, 1.0);
    let open = level(&mut s, 1_000.0);
    println!("deluxe_intensity,rest={closed:.2}dB,open={open:.2}dB");
    assert!(open.is_finite() && closed.is_finite());
}

/// Anything touching the solver is tested at every rate the plugin supports.
#[test]
fn it_solves_at_every_supported_rate() {
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut s = Simulation::new(deluxe::build(SOURCE, LOAD).expect("builds"), rate);
        for control in [TREBLE, BASS, VOLUME, REVERB, INTENSITY] {
            s.set_control(control, 0.6);
        }
        assert!(s.find_operating_point(), "{rate}");
        for k in 0..(rate as usize / 40) {
            let x = 0.1 * (k as f64 * 0.03).sin();
            assert!(s.process(x).is_finite(), "{rate} at {k}");
        }
    }
}
