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
/// set so the model lands there.
#[test]
fn the_operating_point_matches_the_sheets_one_voltage() {
    let pre = dr103::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["v2_p", "v1_p"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| pre.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(pre, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    println!("V2a plate {:.1} V, V1b plate {:.1} V", v[0], v[1]);
    // The drawing's own figure, within the tolerance a generic ECC83 fit earns.
    assert!(
        (v[0] - 140.0).abs() < 14.0,
        "V2a plate {:.1} V against the sheet's 140 V",
        v[0]
    );
}

/// Hiwatt's Issue 4 sheet, read at 300 dpi: V3a is the gain stage and couples
/// into the inverter through 47 nF; V3b carries no signal and holds both grids
/// at a direct voltage from its own divider (1 M and 220 k from H.T. SUPPLY 3).
/// So the grids sit where V3b's cathode does, a fifth of the rail up, and no
/// current flows in the 100 k between them.
///
/// Ampbooks reads a real one at 73 V on the grids and -2.1 V of bias; this
/// model, on its ESTIMATED 465 V rail, sits at 88 V and -1.2 V. The divider
/// puts V3b at a fixed fraction of the rail, so the difference is the rail
/// estimate (or a different revision on Ampbooks' bench), not the network. See
/// `docs/models/brit_dr103.md`.
#[test]
fn v3b_holds_the_inverter_as_issue_4_draws_it() {
    let spec = &PowerSpec::DR103_EL34;
    let driver = spec.driver.expect("the DR103 stage carries its driver");
    assert_eq!(driver.couple, 47e-9, "V3a couples in through a capacitor");
    let amp = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_a", "pi_k", "pi_p1", "v3a_p", "v3a_k", "v3b_g", "v3b_k"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| amp.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(amp, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    let (grid, cathode, plate, v3a_p, v3a_k, v3b_g, v3b_k) =
        (v[0], v[1], v[2], v[3], v[4], v[5], v[6]);
    println!(
        "V3a plate {v3a_p:.1} V ({:.2} mA); V3b grid {v3b_g:.1} V, cathode {v3b_k:.1} V; \
         inverter grid {grid:.1} V, bias {:.2} V, plate-to-cathode {:.0} V",
        v3a_k / driver.cathode * 1e3,
        grid - cathode,
        plate - cathode
    );
    // V3b's grid is the divider's, a fifth of the rail.
    let divided =
        spec.pi_supply * driver.reference_lower / (driver.reference_upper + driver.reference_lower);
    assert!(
        (v3b_g - divided).abs() < 3.0,
        "V3b grid {v3b_g:.1} against {divided:.1}"
    );
    // The inverter's grid is V3b's cathode, through a resistor carrying nothing.
    assert!((grid - v3b_k).abs() < 0.5, "grid {grid:.1}, V3b {v3b_k:.1}");
    assert!(
        (-3.0..-0.5).contains(&(grid - cathode)),
        "bias {}",
        grid - cathode
    );
    // V3a runs at a plain gain stage's current.
    assert!((1.0e-3..2.5e-3).contains(&(v3a_k / driver.cathode)));
}

/// The block hands the power stage the master's wiper: a signal about zero,
/// and silence stays silent.
#[test]
fn the_preamplifier_hands_over_at_the_masters_wiper() {
    let circuit = dr103::build(10_000.0, 1_000_000.0).unwrap();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    let worst = (0..(RATE as usize / 4))
        .map(|_| s.process(0.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

/// The presence control, built at last: a loop from the feedback node back to
/// V3a's plate. Turned up it shunts the top of the band out of the feedback, so
/// the power stage lifts 4 kHz against 200 Hz; turned down it hangs 1000 pF
/// and 470 R across V3a's plate and takes the top away.
#[test]
fn the_presence_control_reaches_back_to_v3a() {
    assert_eq!(PowerModel::DR103EL34.presence_name(), Some("PRESENCE"));
    let spec = &PowerSpec::DR103_EL34;
    let gain = |presence: f64, hz: f64| {
        let mut s = Simulation::new(power::build(spec, 10_000.0).unwrap(), RATE);
        s.set_control(power::PRESENCE, presence);
        s.find_operating_point();
        let tone = Tone::near(RATE, 16_384, hz, 1e-3);
        measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x)).gain_db()
    };
    let tilt = |presence: f64| gain(presence, 4_000.0) - gain(presence, 200.0);
    let (down, middle, up) = (tilt(0.0), tilt(0.5), tilt(1.0));
    println!("4 kHz over 200 Hz: {down:+.1} dB down, {middle:+.1} at half, {up:+.1} up");
    assert!(
        up - middle > 5.0,
        "presence up lifts the top by only {:.1}",
        up - middle
    );
    assert!(up - down > 6.0, "the control spans only {:.1}", up - down);
    let top = |presence: f64| gain(presence, 8_000.0) - gain(presence, 200.0);
    assert!(
        top(0.0) < top(1.0) - 8.0,
        "turned down, 8 kHz is not taken away"
    );
}

/// V3a is the Hiwatt's own last preamp valve, so only the Hiwatt gets it.
/// "DR103 EL34" chosen behind any other preamplifier is the stage from the
/// inverter on -- the same grid network, feedback and presence, with the input
/// where V3a's plate would be -- and behind the DR103 it is the whole thing.
#[test]
fn only_the_hiwatt_gets_v3a() {
    assert_eq!(
        PowerAmp::DR103EL34.resolved(Gain::DR103),
        Some(PowerModel::DR103EL34)
    );
    for gain in [Gain::Brit800, Gain::Clean, Gain::Boogie] {
        assert_eq!(
            PowerAmp::DR103EL34.resolved(gain),
            Some(PowerModel::DR103EL34Return),
            "{}",
            gain.name()
        );
    }
    let full = power::build(&PowerSpec::DR103_EL34, 10_000.0).unwrap();
    let ret = power::build(&PowerSpec::DR103_EL34_RETURN, 10_000.0).unwrap();
    assert!(full.unknown_named("v3a_k").is_some());
    assert!(
        ret.unknown_named("v3a_k").is_none(),
        "the return has no V3a"
    );
    assert!(
        ret.unknown_named("v3b_k").is_some(),
        "but V3b still holds the grids"
    );
    assert_eq!(
        PowerModel::DR103EL34Return.presence_name(),
        Some("PRESENCE")
    );
    // And without V3a it is a power amplifier's gain, not a preamp valve's.
    let gain = |circuit| {
        let mut s = Simulation::new(circuit, RATE);
        s.find_operating_point();
        let tone = Tone::near(RATE, 16_384, 1_000.0, 1e-3);
        measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x)).gain_db()
    };
    let (with, without) = (
        gain(power::build(&PowerSpec::DR103_EL34, 10_000.0).unwrap()),
        gain(power::build(&PowerSpec::DR103_EL34_RETURN, 10_000.0).unwrap()),
    );
    println!("1 kHz small signal: {with:+.1} dB with V3a, {without:+.1} dB without");
    assert!(
        with - without > 15.0,
        "V3a adds only {:.1} dB",
        with - without
    );
}

/// The full stage begins at V3a now, so the power stages' 40 V abuse test
/// drives its return variant instead (`tests/speaker_load.rs`). This holds the
/// full stage to the worst its own preamplifier can give it -- Volume, Master
/// and Treble all the way up, 12 dB over nominal -- which is 11.8 V at 82 Hz
/// and under a volt at 7 kHz (`examples/dr103_limits.rs`): at the lowest and
/// highest rates, the power stage settles all but one sample in a thousand.
#[test]
fn the_full_stage_settles_at_everything_its_preamplifier_can_give_it() {
    for rate in [48_000.0, 192_000.0] {
        for hz in [82.0, 1_000.0, 7_000.0] {
            let mut chain = Chain::new(rate);
            chain.apply(&Settings {
                gain: Gain::DR103,
                drive: 1.0,
                master: 1.0,
                treble: 1.0,
                tone: Stack::Off,
                oversampling: 1,
                ..Settings::default()
            });
            chain.settle();
            let a = 10f64.powf((NOMINAL_DBFS + 12.0) / 20.0);
            let before = chain.solver_breakdown();
            let n = (rate * 0.2) as usize;
            for k in 0..n {
                let x = a * (std::f64::consts::TAU * hz * k as f64 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate} {hz}");
            }
            let work = chain.solver_breakdown().saturating_delta(before);
            assert!(
                work.power.unsettled as usize <= n / 1_000,
                "{rate} Hz, {hz} Hz: {} unsettled",
                work.power.unsettled
            );
        }
    }
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
