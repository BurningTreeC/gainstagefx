//! The Brit AC30 preamplifier and its cathode-biased power stage.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::ac30::{self, BASS, TREBLE, VOLUME};
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(ac30::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    for control in [VOLUME, TREBLE, BASS] {
        s.set_control(control, 0.5);
    }
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// The factory sheet's one operating figure is the output valves' bias: "12.5V
/// at 50 watts, quiescent 10V" across the shared cathode resistor. The supply,
/// which no drawing gives, is set so the model lands there -- and the
/// preamplifier's droppers hang off that same node.
#[test]
fn the_output_valves_idle_where_the_factory_sheet_says() {
    let spec = &PowerSpec::AC30_EL84;
    let amp = power::build(spec, 10_000.0).unwrap();
    let names = ["ok", "ht", "pi_p1", "pi_p2", "pi_k", "pi_b"];
    let nodes: Vec<usize> = names.iter().map(|n| amp.unknown_named(n).unwrap()).collect();
    let mut s = Simulation::new(amp, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    let (cathode, ht) = (v[0], v[1]);
    let per_valve = cathode / spec.cathode_bias / 4.0;
    println!(
        "{cathode:.1} V of bias, {:.1} mA and {:.1} W a valve, HT {ht:.0} V",
        per_valve * 1e3,
        per_valve * (ht - cathode)
    );
    assert!((9.5..11.5).contains(&cathode), "{cathode}");
    // An AC30 runs its EL84s past the data sheet's 12 W, which is why they do
    // not last. That is the amplifier, not a fault in the model.
    assert!((14.0..18.0).contains(&(per_valve * (ht - cathode))));
    // The preamplifier hangs off the same node.
    assert!((ht - ac30::HT).abs() < 5.0, "HT {ht:.1} against {}", ac30::HT);
    // And the inverter's supply is that node less its own drop through R11 22 k.
    let inverter = (spec.pi_supply - v[2]) / spec.pi_plate_driven + (spec.pi_supply - v[3]) / spec.pi_plate_other;
    let node = ac30::HT - inverter * 22_000.0;
    assert!((node - ac30::INVERTER_NODE).abs() < 6.0, "{node:.1}");
    // The pair biases itself off its 47 k tail: a volt or two, not a Marshall's.
    let bias = v[4] - v[5];
    assert!((0.8..2.5).contains(&bias), "inverter bias {bias:.2} V");
}

/// No loop anywhere: the feedback resistor and the presence control are simply
/// not built, and what is built instead is the cut control across the pair.
#[test]
fn there_is_no_feedback_and_the_cut_control_is_across_the_inverter() {
    let spec = &PowerSpec::AC30_EL84;
    assert_eq!(spec.feedback, 0.0);
    let amp = power::build(spec, 10_000.0).unwrap();
    assert!(amp.unknown_named("pres").is_none(), "no presence network");
    assert!(amp.unknown_named("bias").is_none(), "no bias supply");
    assert!(amp.unknown_named("ok").is_some(), "a shared cathode instead");
    assert!(amp.unknown_named("cut").is_some(), "the cut control");

    // Turning the cut down takes the top off, and only the top.
    let response = |control: f64, hz: f64| {
        let mut s = Simulation::new(power::build(spec, 10_000.0).unwrap(), RATE);
        s.set_control(power::PRESENCE, control);
        s.find_operating_point();
        let tone = Tone::near(RATE, 16_384, hz, 0.5);
        measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x)).gain_db()
    };
    // The control is the amplifier's own: up is more cut.
    let (bright_top, dark_top) = (response(0.0, 4_000.0), response(1.0, 4_000.0));
    let (bright_low, dark_low) = (response(0.0, 200.0), response(1.0, 200.0));
    println!("cut: 4 kHz {bright_top:.1} -> {dark_top:.1} dB, 200 Hz {bright_low:.1} -> {dark_low:.1} dB");
    assert!(bright_top - dark_top > 4.0, "{bright_top} {dark_top}");
    assert!((bright_low - dark_low).abs() < 1.0, "{bright_low} {dark_low}");
}

#[test]
fn silence_in_is_silence_out() {
    let mut s = Simulation::new(ac30::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    assert!(s.find_operating_point());
    let worst = (0..(RATE as usize / 4)).map(|_| s.process(0.0).abs()).fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

/// Treble and Bass move their own ends of the band. There is no Middle: the
/// stack has a 10 k resistor where that pot would be.
#[test]
fn the_stack_has_two_controls_and_they_go_the_way_they_are_labelled() {
    let small = 0.001;
    let span = |control: usize, hz: f64| {
        at(&[(control, 1.0)], hz, small).gain_db() - at(&[(control, 0.0)], hz, small).gain_db()
    };
    let (treble, bass) = (span(TREBLE, 4_000.0), span(BASS, 80.0));
    println!("treble {treble:.1} dB at 4 kHz, bass {bass:.1} dB at 80 Hz");
    assert!(treble > 6.0, "{treble}");
    assert!(bass > 6.0, "{bass}");
    assert_eq!(Gain::AC30.own_tone_knobs(), [true, false, true]);
}

/// The volume's 100 pF keeps the top end when the control comes down.
#[test]
fn the_bright_capacitor_keeps_the_top_when_the_volume_comes_down() {
    let tilt = |volume: f64| {
        at(&[(VOLUME, volume)], 5_000.0, 0.001).gain_db() - at(&[(VOLUME, volume)], 200.0, 0.001).gain_db()
    };
    let (low, high) = (tilt(0.2), tilt(1.0));
    println!("5 kHz over 200 Hz: {low:.1} dB at Volume 0.2, {high:.1} dB at 1.0");
    assert!(low > high + 3.0, "{low} vs {high}");
}

#[test]
fn matched_is_the_ac30_stage_and_the_chain_stays_realtime_safe() {
    assert_eq!(PowerAmp::Matched.resolved(Gain::AC30), Some(PowerModel::AC30EL84));
    let settings = Settings {
        gain: Gain::AC30,
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
        assert!(work.power.unsettled == 0 && work.gain.unsettled == 0, "{rate}: {work:?}");
    }
}
