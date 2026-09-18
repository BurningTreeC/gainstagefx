//! The Brit DR103, against Hiwatt's drawings and the published analysis of its
//! phase inverter.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::dr103::{self, BASS, MASTER, MIDDLE, TREBLE, VOLUME};
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{
    Chain, Gain, Level, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};

const RATE: f64 = 96_000.0;

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(dr103::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    for control in [VOLUME, TREBLE, BASS, MIDDLE, MASTER] {
        s.set_control(control, 0.5);
    }
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// The preamplifier sheet gives one voltage: V2a's plate at 140 V. The rail is
/// set so the model lands there, and the driver then sits where the published
/// analysis of this amplifier reads it.
#[test]
fn the_operating_point_matches_the_sheets_one_voltage_and_the_driver() {
    let pre = dr103::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["v2_p", "cf2", "v1_p", "v3_p"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| pre.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(pre, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    println!(
        "V2a plate {:.1} V, driver {:.1} V, V1b plate {:.1} V, V3a plate {:.1} V",
        v[0], v[1], v[2], v[3]
    );
    // The drawing's own figure, within the tolerance a generic ECC83 fit earns.
    assert!(
        (v[0] - 140.0).abs() < 14.0,
        "V2a plate {:.1} V against the sheet's 140 V",
        v[0]
    );
    // And the constant the power stage is handed.
    assert!(
        (v[1] - dr103::DRIVER_VOLTS).abs() < 3.0,
        "driver {:.1} V",
        v[1]
    );
}

/// The inverter is direct-coupled: no capacitor between the driver and the
/// grids, so nothing there can charge up and shift the bias when the amplifier
/// is driven hard. Ampbooks reads the real one at about 73 V on the grids, 75 V
/// on the cathodes, 285 V plate-to-cathode and 1.56 mA a triode.
#[test]
fn the_inverter_is_direct_coupled_and_sits_where_the_analysis_says() {
    let spec = &PowerSpec::DR103_EL34;
    assert_eq!(spec.pi_couple, 0.0, "no coupling capacitor");
    assert!(
        spec.driver_volts > 50.0,
        "and the driver's volts are stated"
    );
    let amp = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_a", "pi_k", "pi_p1", "pi_p2", "ht"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| amp.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(amp, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    let bias = v[0] - v[1];
    let current = (spec.pi_supply - v[2]) / spec.pi_plate_driven;
    println!(
        "grid {:.1} V, cathode {:.1} V, bias {bias:.2} V, plate-to-cathode {:.0} V, {:.2} mA",
        v[0],
        v[1],
        v[2] - v[1],
        current * 1e3
    );
    assert!((-3.0..-0.5).contains(&bias), "bias {bias}");
    assert!(
        ((v[2] - v[1]) - 285.0).abs() < 60.0,
        "plate to cathode {:.0} V",
        v[2] - v[1]
    );
    assert!((1.0e-3..2.2e-3).contains(&current), "{current}");
}

/// The driver's cathode carries its direct voltage, and what leaves the block
/// does not: the volts are handed to the power stage as a number instead, so
/// the signal between the two is a signal like every other one here.
#[test]
fn the_drivers_volts_stay_in_the_preamplifier() {
    let circuit = dr103::build(10_000.0, 1_000_000.0).unwrap();
    let cathode = circuit.unknown_named("cf2").unwrap();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    assert!((s.voltage_at(cathode) - dr103::DRIVER_VOLTS).abs() < 3.0);
    let worst = (0..(RATE as usize / 4))
        .map(|_| s.process(0.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

/// Each control moves its own end of the band. The Middle is a 100 k track on
/// this drawing where a Fender-style stack has 10 k or 25 k, so it dominates the
/// others: with it up, the Bass control has little left to do, and that is the
/// stack rather than the model. Each control is therefore measured with the
/// Middle out of its way.
#[test]
fn the_stack_controls_go_the_way_they_are_labelled() {
    let small = 0.001;
    let span = |control: usize, hz: f64, others: &[(usize, f64)]| {
        let up: Vec<(usize, f64)> = others.iter().copied().chain([(control, 1.0)]).collect();
        let down: Vec<(usize, f64)> = others.iter().copied().chain([(control, 0.0)]).collect();
        at(&up, hz, small).gain_db() - at(&down, hz, small).gain_db()
    };
    let treble = span(TREBLE, 4_000.0, &[(MIDDLE, 0.0)]);
    // The bass control is a gentle one on this stack -- a 470 k track against a
    // 100 k middle -- and what it moves is the bottom octave, not 100 Hz.
    let bass = span(BASS, 40.0, &[(MIDDLE, 0.0)]);
    let middle = span(MIDDLE, 600.0, &[]);
    println!("treble {treble:.1} dB at 4 kHz, bass {bass:.1} dB at 40 Hz, middle {middle:.1} dB");
    assert!(treble > 5.0, "{treble}");
    assert!(bass > 4.0, "{bass}");
    assert!(middle > 3.0, "{middle}");
}

/// The master volume is a control of this amplifier's own, in the preamplifier
/// where the drawing has it, and the panel's Master knob turns it.
#[test]
fn the_master_volume_is_in_the_preamplifier() {
    assert_eq!(
        Gain::DR103.level_control(),
        Some(Level::Circuit(dr103::MASTER))
    );
    let quiet = at(&[(MASTER, 0.1)], 1_000.0, 0.001).gain_db();
    let loud = at(&[(MASTER, 1.0)], 1_000.0, 0.001).gain_db();
    println!("master: {quiet:.1} dB to {loud:.1} dB");
    assert!(loud - quiet > 15.0, "{quiet} {loud}");
}

/// The brilliant channel's 1000 pF coupling is what makes it the brilliant one.
#[test]
fn the_brilliant_channel_throws_the_bass_away_before_it_is_amplified() {
    let tilt = at(&[], 4_000.0, 0.001).gain_db() - at(&[], 100.0, 0.001).gain_db();
    println!("4 kHz over 100 Hz: {tilt:.1} dB");
    assert!(tilt > 6.0, "{tilt}");
}

#[test]
fn matched_is_the_dr103_stage_and_the_chain_stays_realtime_safe() {
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::DR103),
        Some(PowerModel::DR103EL34)
    );
    let settings = Settings {
        gain: Gain::DR103,
        drive: 0.6,
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
        assert!(
            work.power.unsettled == 0 && work.gain.unsettled == 0,
            "{rate}: {work:?}"
        );
    }
}
