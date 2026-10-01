//! The Orange Phase (MXR Phase 90, script logo, R28 switchable) against
//! ElectroSmash's analysis of the traced circuit. See
//! `docs/models/orange_phase.md`; `examples/orange_phase_op.rs` prints what
//! these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::orange_phase::{self as p90, tap, J2N5952};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PedalModel};
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone as Stack, PEDAL_TONES};
use nice_plug::prelude::Enum;

const RATE: f64 = 48_000.0;

fn sim(controls: &[(usize, f64)]) -> Simulation {
    let mut s = Simulation::new(tap(10_000.0, 470_000.0, p90::OUTPUT).unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    s.find_operating_point();
    s
}

/// Gain at `hz` with the trimmer fully down, which holds the JFETs off and the
/// oscillator out of it: the four stages are then 24 k and 47 nF each.
fn frozen(block: f64, hz: f64) -> f64 {
    let mut s = sim(&[(p90::BIAS, 0.0), (p90::BLOCK, block)]);
    let t = Tone::near(RATE, 16_384, hz, 0.01);
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.4 }) as usize;
    measure::run(t, settle, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 0.01
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// ElectroSmash's arithmetic: with the JFETs off, four first-order stages of
/// 24 k and 47 nF put the two notches where each stage turns 45 and 135
/// degrees -- 58.5 Hz and 340.8 Hz -- and the dry and the shifted, summed at
/// unity each, are some 6 dB up between and beyond them (their SPICE plot of
/// the same circuit).
#[test]
fn the_notches_sit_where_24k_and_47nf_put_them() {
    let (low, high) = (db(frozen(0.0, 58.5)), db(frozen(0.0, 340.8)));
    let (between, above) = (db(frozen(0.0, 130.0)), db(frozen(0.0, 5_000.0)));
    println!("script, JFETs off: {low:+.1} dB at 58.5 Hz, {high:+.1} at 340.8, {between:+.1} at 130, {above:+.1} at 5 kHz");
    assert!(low < -35.0 && high < -35.0, "{low} {high}");
    assert!(between > 3.0 && between < 7.0, "{between}");
    assert!(above > 3.5 && above < 7.0, "{above}");
}

/// The block logo's R28 feeds the last stage back into the second: the
/// notches turn shallow and move, and a resonance rises between them --
/// "makes the effect stronger and more pronounced, causing boost". In
/// ElectroSmash's plot the peak is some 4 dB over the script's.
#[test]
fn r28_turns_the_notches_into_a_resonance() {
    let peak = |block: f64| {
        [110.0, 130.0, 150.0, 170.0]
            .into_iter()
            .map(|hz| db(frozen(block, hz)))
            .fold(f64::MIN, f64::max)
    };
    let (script, block) = (peak(0.0), peak(1.0));
    let notch = db(frozen(1.0, 58.5)).max(db(frozen(1.0, 340.8)));
    println!("between the notches: script {script:+.1} dB, block {block:+.1}; block's notches no deeper than {notch:+.1}");
    assert!(block - script > 2.0, "{script} {block}");
    assert!(notch > -30.0, "{notch}");
}

/// The oscillator's period and the gate line's span, at one Speed.
fn oscillator(speed: f64, seconds: f64) -> (f64, f64, f64) {
    let c = tap(10_000.0, 470_000.0, p90::OUTPUT).unwrap();
    let (gate, lfo) = (
        c.unknown_named("gate").unwrap(),
        c.unknown_named("lfo").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    s.set_control(p90::SPEED, speed);
    s.find_operating_point();
    let n = (RATE * seconds) as usize;
    let (mut g, mut l) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for _ in 0..n {
        s.process(0.0);
        g.push(s.voltage_at(gate));
        l.push(s.voltage_at(lfo));
    }
    // Rising crossings of the square wave, past the first second.
    let rises: Vec<usize> = l
        .windows(2)
        .enumerate()
        .skip(RATE as usize)
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(i, _)| i)
        .collect();
    assert!(
        rises.len() >= 2,
        "speed {speed}: the oscillator did not run"
    );
    let period = (rises[rises.len() - 1] - rises[0]) as f64 / (rises.len() - 1) as f64 / RATE;
    let tail = &g[RATE as usize..];
    let lo = tail.iter().cloned().fold(f64::MAX, f64::min);
    let hi = tail.iter().cloned().fold(f64::MIN, f64::max);
    (period, lo, hi)
}

/// "From tenths of Hz to some Hertz", and running at every setting from the
/// moment the pedal is reset -- the slow end too, which sat on the operating
/// point's balance until C10 was given its power-on charge.
#[test]
fn the_speed_knob_runs_from_tenths_of_a_hertz_to_hertz() {
    let (slow, _, _) = oscillator(0.0, 18.0);
    let (middle, _, _) = oscillator(0.5, 5.0);
    let (fast, _, _) = oscillator(1.0, 3.0);
    println!(
        "speed 0: {:.3} Hz, 0.5: {:.3} Hz, 1: {:.2} Hz",
        1.0 / slow,
        1.0 / middle,
        1.0 / fast
    );
    assert!(1.0 / slow < 0.3 && 1.0 / slow > 0.05);
    assert!(1.0 / fast > 4.0 && 1.0 / fast < 20.0);
    assert!(slow > middle && middle > fast);
}

/// The bias procedure: the trimmer set so the bottom of the oscillator's
/// swing takes the gates to the JFETs' cut-off, "for a maximum span"; the top
/// a third of a volt above it, and never above the source.
#[test]
fn the_trimmer_rests_where_the_procedure_sets_it() {
    let (_, lo, hi) = oscillator(0.5, 4.0);
    println!(
        "gates {lo:+.3} .. {hi:+.3} V against the cut-off {}",
        J2N5952.pinch_off
    );
    assert!((lo - J2N5952.pinch_off).abs() < 0.05, "{lo}");
    assert!(hi - lo > 0.2 && hi < 0.0, "{lo} {hi}");
}

/// And the notches sweep: a 1 kHz tone through the running pedal is cut deep
/// once a cycle, when the upper notch passes it, and is whole at other times.
#[test]
fn the_notches_sweep_through_the_band() {
    let mut s = sim(&[(p90::SPEED, 0.75)]);
    let window = (RATE / 1_000.0 * 10.0) as usize;
    let mut envelope = Vec::new();
    let (mut sum, mut count) = (0.0, 0);
    for k in 0..(RATE as usize * 2) {
        let y = s.process(0.05 * (std::f64::consts::TAU * 1_000.0 * k as f64 / RATE).sin());
        if k < RATE as usize / 2 {
            continue;
        }
        sum += y * y;
        count += 1;
        if count == window {
            envelope.push((sum / count as f64).sqrt());
            sum = 0.0;
            count = 0;
        }
    }
    let lo = envelope.iter().cloned().fold(f64::MAX, f64::min);
    let hi = envelope.iter().cloned().fold(f64::MIN, f64::max);
    println!(
        "1 kHz through the sweep: {:.1} dB from loudest to quietest",
        db(hi / lo)
    );
    assert!(db(hi / lo) > 12.0, "{}", db(hi / lo));
}

/// Registered under its ids, a pedal and a circuit, its one knob on the drive
/// and its switch on the tone knob, no level.
#[test]
fn it_is_registered_as_a_pedal_and_a_circuit() {
    let pedals = PedalModel::ids().unwrap();
    assert_eq!(
        pedals[PedalModel::OrangePhase.to_index()],
        "pedal_mxr_phase90"
    );
    let circuits = Circuit::ids().unwrap();
    assert_eq!(
        circuits[Circuit::OrangePhase.to_index()],
        "pedal_mxr_phase90_circuit"
    );
    assert_eq!(PedalModel::OrangePhase.voice(), Pedal::OrangePhase);
    assert_eq!(Pedal::OrangePhase.as_circuit(), Some(Gain::OrangePhase));
    assert_eq!(Circuit::OrangePhase.voice(), Gain::OrangePhase);
    assert!(Pedal::OrangePhase.has_drive() && !Pedal::OrangePhase.has_level());
    assert_eq!(Pedal::OrangePhase.tone_labels()[0], Some("block"));
    assert_eq!(Pedal::OrangePhase.drive_and_level_labels().0, "speed");
    assert_eq!(Gain::OrangePhase.drive_name(), "SPEED");
    assert_eq!(
        Gain::OrangePhase.own_single_tone(),
        Some((p90::BLOCK, "BLOCK", false))
    );
    assert!(Gain::OrangePhase.drive_is_channel_volume());
}

/// In the slot in front of an amplifier and alone as the circuit, at every
/// rate, finite, moving, and without allocating -- with the switch thrown
/// while it plays.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    for as_pedal in [true, false] {
        let mut chain = Chain::new(44_100.0);
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            chain.set_rate(rate);
            let base = if as_pedal {
                Settings {
                    gain: Gain::Crunch,
                    tone: Stack::Off,
                    pedal: PedalSettings {
                        pedal: Pedal::OrangePhase,
                        drive: 0.9,
                        tone: [1.0; PEDAL_TONES],
                        level: 0.5,
                    },
                    ..Settings::default()
                }
            } else {
                Settings {
                    gain: Gain::OrangePhase,
                    drive: 0.9,
                    tone: Stack::Off,
                    circuit_tone: 1.0,
                    ..Settings::default()
                }
            };
            chain.apply(&base);
            chain.settle();
            chain.reset();
            chain.find_operating_point();
            let thrown = if as_pedal {
                Settings {
                    pedal: PedalSettings {
                        tone: [0.0; PEDAL_TONES],
                        ..base.pedal
                    },
                    ..base
                }
            } else {
                Settings {
                    circuit_tone: 0.0,
                    ..base
                }
            };
            let mut peak: f64 = 0.0;
            assert_no_heap(|| {
                for k in 0..4_096 {
                    if k == 2_048 {
                        chain.apply(&thrown);
                    }
                    let x = 0.3 * (k as f64 * std::f64::consts::TAU * 220.0 / rate).sin();
                    let y = chain.process(x);
                    assert!(y.is_finite(), "{rate}");
                    peak = peak.max(y.abs());
                }
            });
            assert!(peak > 1e-3, "{rate} {as_pedal}: silent");
        }
    }
}
