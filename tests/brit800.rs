//! The Brit 800 preamplifier, against its manufacturer's drawing.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::brit800::{self, BASS, MIDDLE, TREBLE, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

fn sim(at: &str) -> Simulation {
    let mut sim = Simulation::new(brit800::tap(10_000.0, 1_000_000.0, at).expect("builds"), RATE);
    for control in [VOLUME, TREBLE, BASS, MIDDLE] {
        sim.set_control(control, 0.5);
    }
    sim
}

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = sim("out");
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// The 1981 drawing's own voltage table, 100 W master-volume column, which it
/// says are "average only". Node numbers are the drawing's.
#[test]
fn the_operating_point_agrees_with_the_drawings_voltage_table() {
    let circuit = brit800::build(10_000.0, 1_000_000.0).unwrap();
    let table = [
        ("n5", 280.0),   // 5
        ("n6", 290.0),   // 6
        ("v1_p", 210.0), // 4
        ("v1_k", 1.75),  // 3
        ("v2_p", 250.0), // 2
        ("v2_k", 2.6),   // 1
        ("v3_p", 165.0), // 8
        ("v3_k", 1.0),   // 7
        ("v4_k", 167.0), // 9
    ];
    let nodes: Vec<usize> = table
        .iter()
        .map(|(name, _)| circuit.unknown_named(name).expect("node exists"))
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    for ((name, expected), node) in table.iter().zip(nodes) {
        let got = s.voltage_at(node);
        let error = (got - expected).abs() / expected;
        println!("{name}: {got:.2} V against {expected} V");
        // A generic ECC83 fit against one measured amplifier's averages: ten per
        // cent for the plates and rails, thirty for the small cathode voltages.
        let limit = if *expected < 10.0 { 0.3 } else { 0.1 };
        assert!(error < limit, "{name}: {got:.2} V against the table's {expected} V");
    }
}

#[test]
fn silence_in_is_silence_out() {
    let mut s = sim("out");
    assert!(s.find_operating_point());
    let worst = (0..(RATE as usize / 4)).map(|_| s.process(0.0).abs()).fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

/// Each tone control moves the part of the band it is named for, in the
/// direction on its label.
#[test]
fn the_stack_controls_go_the_way_they_are_labelled() {
    let small = 0.002;
    let span = |control: usize, hz: f64| {
        at(&[(control, 1.0)], hz, small).gain_db() - at(&[(control, 0.0)], hz, small).gain_db()
    };
    let (treble, bass, middle) = (span(TREBLE, 4_000.0), span(BASS, 80.0), span(MIDDLE, 600.0));
    println!("treble {treble:.1} dB at 4 k, bass {bass:.1} dB at 80 Hz, middle {middle:.1} dB at 600 Hz");
    assert!(treble > 6.0, "{treble}");
    assert!(bass > 6.0, "{bass}");
    assert!(middle > 3.0, "{middle}");
}

/// The Preamp Volume runs from nearly nothing to the preamp distorting on a
/// guitar-level signal, and the bright cap makes a low setting brighter.
#[test]
fn the_preamp_volume_sets_the_gain_and_the_bright_cap_works() {
    let low = at(&[(VOLUME, 0.05)], 1_000.0, 0.01).gain_db();
    let high = at(&[(VOLUME, 1.0)], 1_000.0, 0.001).gain_db();
    assert!(high - low > 30.0, "{low:.1} to {high:.1} dB");
    let tilt = |volume: f64| {
        at(&[(VOLUME, volume)], 5_000.0, 0.001).gain_db() - at(&[(VOLUME, volume)], 200.0, 0.001).gain_db()
    };
    assert!(tilt(0.2) > tilt(1.0) + 3.0, "{} vs {}", tilt(0.2), tilt(1.0));
    let driven = at(&[(VOLUME, 1.0)], 220.0, 0.122);
    println!("{:.1} % distortion at a guitar's level", driven.thd_percent());
    assert!(driven.thd_percent() > 15.0, "{}", driven.thd_percent());
}

#[test]
fn matched_is_the_brit_el34_and_the_chain_stays_realtime_safe() {
    assert_eq!(PowerAmp::Matched.resolved(Gain::Brit800), Some(PowerModel::BritEL34));
    let settings = Settings {
        gain: Gain::Brit800,
        drive: 0.8,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    };
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&settings);
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let before = chain.solver_breakdown();
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = amplitude * (k as f64 * std::f64::consts::TAU * 110.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(work.gain.solves > 0 && work.power.solves > 0, "{rate}");
    }
}
