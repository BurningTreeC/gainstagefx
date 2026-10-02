//! The American V-4B (Ampeg V-4B, DWG 06700 A) and the American V-4B 7027A
//! against Ampeg's drawing. See `docs/models/american_vt40.md`, which covers
//! both; `examples/v4b_op.rs` prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::american_v4b as v4b;
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::GROUND;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PowerAmp};
use gainstagefx::voice::{Chain, Gain, PowerModel, Settings, Throw, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 48_000.0;

fn near(got: f64, want: f64, within: f64) -> bool {
    (got / want - 1.0).abs() <= within
}

/// The preamplifier on the drawing's no-signal voltages: the rail, V1b's
/// cathode, the 6K11's cathodes and V3a's within 7 %; the 12AX7 plates within
/// 13 %. The 6K11's unit 2 plate and the follower as on the VT-40: the
/// drawing's own 1.26 V on unit 2's cathode puts its plate near 146 V, and the
/// model has it at 151 V, not the printed 178.
#[test]
fn the_preamp_sits_on_the_drawing() {
    let c = v4b::build(10_000.0, 1_000_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let nodes = [
        ("bp", 325.0, 0.03),
        ("k1", 1.48, 0.07),
        ("k1b", 2.1, 0.05),
        ("k3", 22.0, 0.08),
        ("k2", 1.26, 0.05),
        ("k3a", 200.0, 0.04),
        ("p1", 155.0, 0.08),
        ("pm", 230.0, 0.07),
        ("p3", 192.0, 0.13),
        ("p2", 178.0, 0.17),
        ("kf", 140.0, 0.13),
    ];
    let idx: Vec<usize> = nodes.iter().map(|(n, _, _)| at(n)).collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    for ((name, drawing, within), i) in nodes.iter().zip(idx) {
        let v = s.voltage_at(i);
        println!("{name:<4} {v:7.2} V (drawing {drawing})");
        assert!(near(v, *drawing, *within), "{name}: {v} against {drawing}");
    }
}

/// The power stage on the drawing: the 360 V, 545 V and 540 V nodes (fitted),
/// V3b's plate and cathode, the inverter's plates within 7 % and its cathodes;
/// the four 7027As idle under their 35 W.
#[test]
fn the_power_stage_sits_on_the_drawing() {
    let spec = &PowerSpec::V4B_7027A;
    let c = power::build(spec, 10_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let nodes = [
        ("f_c", 360.0, 0.005),
        ("ht", 545.0, 0.005),
        ("scr", 540.0, 0.005),
        ("f_p1", 205.0, 0.03),
        ("f_k1", 1.8, 0.05),
        ("pi_p1", 200.0, 0.07),
        ("pi_p2", 200.0, 0.07),
        ("f_kp", 9.7, 0.05),
        ("og1", -64.0, 0.005),
    ];
    let idx: Vec<usize> = nodes.iter().map(|(n, _, _)| at(n)).collect();
    let (plate, screen) = (at("pl_a"), at("os1"));
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    for ((name, drawing, within), i) in nodes.iter().zip(idx) {
        let v = s.voltage_at(i);
        println!("{name:<5} {v:8.2} V (drawing {drawing})");
        assert!(near(v, *drawing, *within), "{name}: {v} against {drawing}");
    }
    let valve = Pentode::new(0, 1, GROUND, 2, 1.0, spec.tube);
    let vp = s.voltage_at(plate);
    let (ia, _) = valve.currents(vp, spec.bias, s.voltage_at(screen));
    println!("each 7027A idles at {:.1} mA, {:.1} W", ia * 1e3, ia * vp);
    assert!(ia * vp < 35.0 && ia > 10e-3);
}

fn gain(position: usize, hz: f64) -> f64 {
    let mut s = Simulation::new(v4b::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    s.set_control(v4b::VOLUME, 0.3);
    for (slot, value) in v4b::ULTRA_LO_SLOTS.into_iter().zip(v4b::ULTRA_LO[position]) {
        s.set_value(slot, value);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 9_600, hz, 1e-3);
    measure::run(t, 24_000, |x| s.process(x)).gain_db()
}

/// ULTRA LO, on against off: the low end kept (within 3 dB at 80 and 150 Hz),
/// the middle and top taken away (more than 10 dB down at 600 Hz and 20 dB at
/// 2.5 kHz). The drawing's network, measured; nothing published to hold it to.
#[test]
fn ultra_lo_keeps_the_bottom_and_takes_the_rest() {
    let lift = |hz: f64| gain(0, hz) - gain(1, hz);
    let (a, b, c, d) = (lift(80.0), lift(150.0), lift(600.0), lift(2_500.0));
    println!("80 Hz {a:+.1}, 150 Hz {b:+.1}, 600 Hz {c:+.1}, 2.5 kHz {d:+.1} dB");
    assert!(a > -3.0 && b > -3.0);
    assert!(c < -10.0 && d < -20.0);
}

/// MIDRANGE SELECTOR: the VT-40's network, so the same three bands.
#[test]
fn the_midrange_select_moves_the_band_to_300_800_and_3000_hz() {
    let grid = [
        100.0, 150.0, 200.0, 300.0, 450.0, 800.0, 1_200.0, 2_000.0, 3_000.0, 4_500.0,
    ];
    let gain = |middle: f64, position: usize, hz: f64| {
        let mut s = Simulation::new(v4b::tap(10_000.0, 1_000_000.0, "out").unwrap(), RATE);
        s.set_control(v4b::VOLUME, 0.05);
        s.set_control(v4b::MIDDLE, middle);
        for (slot, value) in v4b::MID_SELECT_SLOTS
            .into_iter()
            .zip(v4b::MID_SELECT[position])
        {
            s.set_value(slot, value);
        }
        s.find_operating_point();
        let t = Tone::near(RATE, 9_600, hz, 1e-3);
        measure::run(t, 24_000, |x| s.process(x)).gain_db()
    };
    for (position, want) in [(0, 300.0), (1, 800.0), (2, 3_000.0)] {
        let lifts: Vec<f64> = grid
            .iter()
            .map(|&f| gain(1.0, position, f) - gain(0.5, position, f))
            .collect();
        let (peak, at) =
            lifts.iter().zip(grid).fold(
                (f64::MIN, 0.0),
                |(m, a), (&l, f)| {
                    if l > m {
                        (l, f)
                    } else {
                        (m, a)
                    }
                },
            );
        println!("position {position}: {peak:+.1} dB at {at} Hz");
        assert_eq!(at, want);
        assert!(peak > 15.0, "{peak}");
    }
}

#[test]
fn it_is_registered_with_its_own_stage_and_controls() {
    let circuits = Circuit::ids().unwrap();
    assert_eq!(circuits[Circuit::AmericanV4b.to_index()], "amp_ampeg_v4b");
    assert_eq!(Circuit::AmericanV4b.name(), "American V-4B");
    let powers = PowerAmp::ids().unwrap();
    assert_eq!(
        powers[PowerAmp::AmericanV4b7027A.to_index()],
        "power_ampeg_v4b_7027a"
    );
    let gain = Circuit::AmericanV4b.voice();
    assert_eq!(gain, Gain::AmericanV4b);
    assert!(gain.is_modelled());
    assert_eq!(
        PowerAmp::Matched.voice().resolved(gain),
        Some(PowerModel::AmericanV4b7027A)
    );
    assert_eq!(gain.own_tone(), Some((v4b::BASS, v4b::MIDDLE, v4b::TREBLE)));
    assert_eq!(gain.drive_name(), "VOLUME");
    assert_eq!(gain.bright_switch().unwrap().slot, v4b::ULTRA_HI_SLOT);
    assert_eq!(gain.low_switch().unwrap().labels, ["Ultra Lo", "Off"]);
    assert_eq!(
        gain.mid_switch().unwrap().labels,
        ["300 Hz", "800 Hz", "3 kHz"]
    );
    assert!(!gain.has_reverb() && !gain.has_tremolo());
}

/// ULTRA LO thrown from the panel reaches the circuit on the audio thread,
/// without allocating: a 1 kHz tone falls by more than 10 dB.
#[test]
fn ultra_lo_is_thrown_from_the_panel_without_allocating() {
    let rms = |chain: &mut Chain| {
        let mut sum = 0.0;
        for k in 0..8_192 {
            let x = 0.05 * (k as f64 * std::f64::consts::TAU * 1_000.0 / RATE).sin();
            let y = chain.process(x);
            if k > 4_096 {
                sum += y * y;
            }
        }
        sum.sqrt()
    };
    let mut chain = Chain::new(RATE);
    let base = Settings {
        gain: Gain::AmericanV4b,
        drive: 0.3,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    };
    chain.apply(&base);
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let off = rms(&mut chain);
    let thrown = Settings {
        low_switch: Throw::Left,
        ..base
    };
    let mut on = 0.0;
    assert_no_heap(|| {
        chain.apply(&thrown);
        on = rms(&mut chain);
        chain.apply(&base);
    });
    println!("1 kHz: off {off:.4}, Ultra Lo {on:.4}");
    assert!(on < off * 0.32, "{off} {on}");
}

/// The preamplifier's small-signal gain is the same at every rate.
#[test]
fn the_preamp_is_the_same_at_every_rate() {
    let at = |rate: f64| {
        let mut s = Simulation::new(v4b::build(10_000.0, 1_000_000.0).unwrap(), rate);
        s.set_control(v4b::VOLUME, 0.3);
        s.find_operating_point();
        let t = Tone::near(rate, 8_192, 1_000.0, 1e-4);
        measure::run(t, (rate * 0.3) as usize, |x| s.process(x)).gain_db()
    };
    let at48 = at(48_000.0);
    for rate in [44_100.0, 88_200.0, 96_000.0, 192_000.0] {
        let g = at(rate);
        assert!((g - at48).abs() < 0.3, "{rate}: {g} against {at48}");
    }
}

/// At every rate, through its own power stage, hot and finite, and without
/// allocating.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::AmericanV4b,
            drive: 0.8,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let before = chain.solver_breakdown();
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 55.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(work.gain.solves > 0 && work.power.solves > 0, "{rate}");
    }
}
