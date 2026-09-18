//! The Yellow Dist, against the published analysis of its circuit.
use gainstagefx::circuits::distortion_plus::{self, DISTORTION, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    tapped("out", controls, hz, volts)
}

/// The analysis quotes the *amplifier stage's* gain, so that is where it is
/// measured from: the pedal's own output is six decibels down on it, because
/// R5 10 k works into a 10 k Output track.
fn tapped(node: &str, controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(
        distortion_plus::tap(10_000.0, 470_000.0, node).unwrap(),
        RATE,
    );
    s.set_control(VOLUME, 1.0);
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    s.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// The analysis works the range out from the values: "Gv(min) = 1 + (1M/(4.7K +
/// 1M)) = 1.5 (3.5dB)" and "Gv(max) = 1 + (1M / (4.7K + 0)) = 213 (46.5 dB)".
/// The top of that is what a perfect amplifier would give; a 741 is not one, and
/// what it is short by at a kilohertz is its own gain-bandwidth.
#[test]
fn the_distortion_control_spans_what_the_analysis_says() {
    let down = tapped("amp", &[(DISTORTION, 0.0)], 1_000.0, 1e-4).gain_db();
    let up = tapped("amp", &[(DISTORTION, 1.0)], 1_000.0, 1e-5).gain_db();
    println!("{down:.1} dB down, {up:.1} dB up (the analysis: 3.5 and 46.5)");
    assert!((down - 3.5).abs() < 2.5, "{down}");
    assert!((40.0..47.0).contains(&up), "{up}");
}

/// Germanium to ground, so the ceiling is low: the analysis puts it at 350 mV.
#[test]
fn the_germanium_diodes_clip_where_the_analysis_says() {
    let mut s = Simulation::new(
        distortion_plus::tap(10_000.0, 470_000.0, "clip").unwrap(),
        RATE,
    );
    s.set_control(DISTORTION, 0.0);
    s.set_control(VOLUME, 1.0);
    s.find_operating_point();
    // Drive it well past the diodes' knee and see where the node stops.
    let mut peak: f64 = 0.0;
    for k in 0..(RATE as usize / 10) {
        let y = s.process(0.5 * (k as f64 * std::f64::consts::TAU * 220.0 / RATE).sin());
        peak = peak.max(y.abs());
    }
    println!("the clipping node stops at {peak:.3} V (the analysis: 0.35)");
    assert!((0.25..0.5).contains(&peak), "{peak}");
}

/// And at a guitar's level with the control up it is a distortion pedal.
#[test]
fn a_guitar_into_it_comes_out_distorted() {
    let m = at(&[(DISTORTION, 0.8)], 220.0, 0.122);
    println!(
        "{:.1} % distortion, 2nd {:.1} %, 3rd {:.1} %",
        m.thd_percent(),
        m.harmonic_percent(2),
        m.harmonic_percent(3)
    );
    assert!(m.thd_percent() > 30.0, "{}", m.thd_percent());
}

/// The pedal has no tone control, and the panel is told so.
#[test]
fn it_has_two_knobs() {
    assert!(!gainstagefx::voice::Pedal::YellowDist.has_tone());
}

/// And the control is usable across its travel rather than doing everything at
/// the top: the gain rises by roughly equal steps in decibels.
#[test]
fn the_distortion_control_sweeps_evenly() {
    let db: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0]
        .iter()
        .map(|&p| tapped("amp", &[(DISTORTION, p)], 1_000.0, 1e-5).gain_db())
        .collect();
    println!("{db:?}");
    for pair in db.windows(2) {
        let step = pair[1] - pair[0];
        assert!((5.0..18.0).contains(&step), "{db:?}");
    }
}
