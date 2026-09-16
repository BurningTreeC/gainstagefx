//! The Cali Rectifier, against the node voltages Mesa marked on its own sheets.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::circuits::rectifier::{self, BASS, GAIN, MASTER, MIDDLE, TREBLE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, Level, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(rectifier::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    for control in [GAIN, TREBLE, BASS, MIDDLE, MASTER] {
        s.set_control(control, 0.5);
    }
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// The preamp sheet marks four plates and the power sheet two more. The rails
/// were set from them, and this is what the model then reads.
#[test]
fn the_operating_point_matches_the_sheets_own_voltages() {
    let pre = rectifier::build(10_000.0, 1_000_000.0).unwrap();
    let table = [
        ("v1_p", 200.0),
        ("v2_p", 280.0),
        ("v2b_p", 384.0),
        ("v3_p", 213.0),
        ("cf", 216.0),
    ];
    let nodes: Vec<usize> = table.iter().map(|(n, _)| pre.unknown_named(n).unwrap()).collect();
    let mut s = Simulation::new(pre, RATE);
    assert!(s.find_operating_point());
    for ((name, expected), node) in table.iter().zip(nodes) {
        let got = s.voltage_at(node);
        println!("{name}: {got:.1} V against the sheet's {expected} V");
        assert!((got - expected).abs() / expected < 0.06, "{name}: {got:.1} V");
    }

    let spec = &PowerSpec::RECTO_6L6;
    let amp = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "pi_k"];
    let nodes: Vec<usize> = names.iter().map(|n| amp.unknown_named(n).unwrap()).collect();
    let mut s = Simulation::new(amp, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    println!("inverter plates {:.0} / {:.0} V (sheet 280), cathodes {:.0} V (sheet 48)", v[0], v[1], v[2]);
    assert!((v[0] - 280.0).abs() < 20.0 && (v[1] - 280.0).abs() < 20.0);
    assert!((v[2] - 48.0).abs() < 8.0, "cathodes {:.1} V", v[2]);
}

/// The stage this amplifier is known for: V2B on a 39 k cathode resistor with
/// nothing across it. It is biased so cold that it clips one side of the wave
/// first, at levels where the stages around it are still linear.
#[test]
fn the_cold_stage_clips_one_side_first() {
    let to_cold = rectifier::tap(10_000.0, 1_000_000.0, "v2b_p").unwrap();
    let cold_bias = {
        let node = to_cold.unknown_named("v2b_k").unwrap();
        let mut s = Simulation::new(to_cold.clone(), RATE);
        s.find_operating_point();
        s.voltage_at(node)
    };
    println!("the cold stage sits at {cold_bias:.1} V of cathode bias");
    // Far colder than the 1.5 V a normal gain stage runs at.
    assert!(cold_bias > 3.5, "{cold_bias}");

    let mut s = Simulation::new(to_cold, RATE);
    for control in [GAIN, TREBLE, BASS, MIDDLE, MASTER] {
        s.set_control(control, 0.5);
    }
    // A guitar into the amplifier with the gain where a player would put it.
    s.set_control(GAIN, 0.8);
    s.find_operating_point();
    let tone = Tone::near(RATE, 16_384, 110.0, 0.122);
    let m = measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x));
    println!("{:.1} % distortion, 2nd {:.1} %, 3rd {:.1} %", m.thd_percent(), m.harmonic_percent(2), m.harmonic_percent(3));
    assert!(m.thd_percent() > 10.0, "{}", m.thd_percent());
    assert!(m.harmonic_percent(2) > m.harmonic_percent(3), "one side first");
}

#[test]
fn silence_in_is_silence_out() {
    let mut s = Simulation::new(rectifier::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    assert!(s.find_operating_point());
    let worst = (0..(RATE as usize / 4)).map(|_| s.process(0.0).abs()).fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

#[test]
fn the_stack_controls_go_the_way_they_are_labelled() {
    let small = 0.0005;
    let span = |control: usize, hz: f64| {
        at(&[(control, 1.0)], hz, small).gain_db() - at(&[(control, 0.0)], hz, small).gain_db()
    };
    let (treble, bass, middle) = (span(TREBLE, 4_000.0), span(BASS, 80.0), span(MIDDLE, 600.0));
    println!("treble {treble:.1} dB, bass {bass:.1} dB, middle {middle:.1} dB");
    assert!(treble > 6.0 && bass > 6.0 && middle > 3.0);
}

/// The red channel's master is a control of the preamplifier, where the sheet
/// has it, and the panel's Master knob turns it.
#[test]
fn the_master_is_the_red_channels_own() {
    assert_eq!(Gain::Recto.level_control(), Some(Level::Circuit(rectifier::MASTER)));
    let quiet = at(&[(MASTER, 0.1)], 1_000.0, 0.0005).gain_db();
    let loud = at(&[(MASTER, 1.0)], 1_000.0, 0.0005).gain_db();
    println!("master: {quiet:.1} dB to {loud:.1} dB");
    assert!(loud - quiet > 15.0, "{quiet} {loud}");
}

#[test]
fn matched_is_the_recto_stage_and_the_chain_stays_realtime_safe() {
    assert_eq!(PowerAmp::Matched.resolved(Gain::Recto), Some(PowerModel::Recto6L6));
    let settings = Settings {
        gain: Gain::Recto,
        drive: 0.7,
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

/// The switch the amplifier is named after. On SILICON the rail holds; on VALVE
/// it sits lower and gives way under a chord, because a rectifier valve's drop
/// follows its current. This is a class-AB stage, so the current really does
/// swing -- unlike the AC30's, which draws nearly the same either way.
#[test]
fn the_valve_rectifier_setting_sags_where_the_silicon_one_holds() {
    let rail = |spec: &PowerSpec| {
        let circuit = power::build(spec, 10_000.0).unwrap();
        let ht = circuit.unknown_named("ht").unwrap();
        let mut s = Simulation::new(circuit, 48_000.0);
        assert!(s.find_operating_point());
        let idle = s.voltage_at(ht);
        let mut lowest = idle;
        for k in 0..24_000 {
            s.process(60.0 * (k as f64 * std::f64::consts::TAU * 82.0 / 48_000.0).sin());
            lowest = lowest.min(s.voltage_at(ht));
        }
        (idle, idle - lowest)
    };
    let (silicon_idle, silicon_sag) = rail(&PowerSpec::RECTO_6L6);
    let (valve_idle, valve_sag) = rail(&PowerSpec::RECTO_6L6_TUBE);
    println!("silicon {silicon_idle:.0} V, {silicon_sag:.0} V of sag; valve {valve_idle:.0} V, {valve_sag:.0} V");
    assert!(PowerSpec::RECTO_6L6.rectifier.is_none());
    assert!(PowerSpec::RECTO_6L6_TUBE.rectifier.is_some());
    // The valve starts lower and gives way further.
    assert!(valve_idle < silicon_idle - 5.0, "{valve_idle} {silicon_idle}");
    assert!(valve_sag > silicon_sag * 1.2, "{valve_sag} {silicon_sag}");
}
