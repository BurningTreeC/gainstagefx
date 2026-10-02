//! The American VT-40 (Ampeg VT-40, DWG 06500 B) and the American 7027A
//! against Ampeg's drawing, its service section's A.C. voltage readings and
//! RCA's 7027-A sheet; and the valves behind them: the 6CG7 fit against
//! Tung-Sol's sheet, and the 6K11 as the 12AU7 and 12AX7 its sheets say its
//! units are. See `docs/models/american_vt40.md`; `examples/vt40_op.rs` and
//! `tools/tube_fit/fit_7027a.py` print what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::american_vt40 as vt40;
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::device::{Pentode, Triode};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{PentodeSpec, TriodeSpec, GROUND};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::editor::ToneKnobs;
use gainstagefx::params::{Circuit, PowerAmp, ToneStack};
use gainstagefx::voice::{Chain, Gain, PowerModel, Settings, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 48_000.0;

fn near(got: f64, want: f64, within: f64) -> bool {
    (got / want - 1.0).abs() <= within
}

/// Plate current and its two slopes at a point.
fn point(spec: TriodeSpec, vp: f64, vg: f64) -> (f64, f64, f64) {
    let valve = Triode::new(0, 1, GROUND, spec);
    let i = |vp: f64, vg: f64| valve.plate(vp, vg);
    let h = 1e-3;
    let gm = (i(vp, vg + h) - i(vp, vg - h)) / (2.0 * h);
    let ga = (i(vp + h, vg) - i(vp - h, vg)) / (2.0 * h);
    (i(vp, vg), gm, ga)
}

/// The 6CG7 fit meets Tung-Sol's 1959 sheet at its six targets within 2 %:
/// 10 mA and 3.0 mA/V at 90 V and 0 V; 9 mA, 2.6 mA/V and 7.7 kohm at
/// 250 V and -8 V; 1.3 mA at -12.5 V. See `tools/tube_fit/fit_6cg7.py`.
#[test]
fn the_6cg7_fit_meets_tung_sols_sheet() {
    let (i90, gm90, _) = point(TriodeSpec::T6CG7, 90.0, 0.0);
    let (i250, gm250, ga250) = point(TriodeSpec::T6CG7, 250.0, -8.0);
    let (i125, _, _) = point(TriodeSpec::T6CG7, 250.0, -12.5);
    println!(
        "90 V: {:.2} mA, {:.2} mA/V; 250 V: {:.2} mA, {:.2} mA/V, {:.0} ohm; -12.5 V: {:.2} mA",
        i90 * 1e3,
        gm90 * 1e3,
        i250 * 1e3,
        gm250 * 1e3,
        1.0 / ga250,
        i125 * 1e3
    );
    assert!(near(i90, 10e-3, 0.02) && near(gm90, 3.0e-3, 0.02));
    assert!(near(i250, 9e-3, 0.02) && near(gm250, 2.6e-3, 0.02));
    assert!(near(1.0 / ga250, 7_700.0, 0.02));
    assert!(near(i125, 1.3e-3, 0.02));
}

/// The 6K11's units, by RCA's and GE's sheets, are the 12AU7 (mu 17,
/// 7.7 kohm, 2.2 mA/V, 10.5 mA at 250 V / -8.5 V) and the 12AX7 (mu 100,
/// 62.5 kohm, 1.6 mA/V, 1.2 mA at 250 V / -2 V) to the figure, and are built
/// from the solver's ECC82 and ECC83. Where those stand at the sheets' points:
/// the ECC82 within 15 %; the ECC83, which is Koren's published 12AX7 and has
/// no fit of its own, 0.95 mA against 1.2 -- the model's known low current at
/// that point -- with its transconductance and amplification factor within
/// 11 %. The same model every 12AX7 in the catalogue uses.
#[test]
fn the_6k11s_units_are_the_12au7_and_the_12ax7() {
    let (i, gm, ga) = point(TriodeSpec::ECC82, 250.0, -8.5);
    println!(
        "medium-mu unit (ECC82): {:.2} mA, {:.2} mA/V, mu {:.1}",
        i * 1e3,
        gm * 1e3,
        gm / ga
    );
    assert!(near(i, 10.5e-3, 0.15) && near(gm, 2.2e-3, 0.15) && near(gm / ga, 17.0, 0.15));
    let (i, gm, ga) = point(TriodeSpec::ECC83, 250.0, -2.0);
    println!(
        "high-mu units (ECC83): {:.2} mA, {:.2} mA/V, mu {:.1}",
        i * 1e3,
        gm * 1e3,
        gm / ga
    );
    assert!(near(i, 1.2e-3, 0.25) && near(gm, 1.6e-3, 0.11) && near(gm / ga, 100.0, 0.11));
}

/// RCA's 7027-A (8-59), through `PentodeSpec::T7027A`: the class A row (250 V
/// on plate and screen, -14 V: 72 mA, 6000 umho, 22.5 kohm) within 3 % and
/// 2 %, the three fixed-bias rows' no-signal currents within 14 % (they do not
/// agree with each other); and at the VT-40's 586 V / 589 V / -65 V, under the
/// sheet's 35 W of plate dissipation.
#[test]
fn the_7027a_fit_meets_rcas_sheet() {
    let valve = Pentode::new(0, 1, GROUND, 2, 1.0, PentodeSpec::T7027A);
    let ip = |vp: f64, vs: f64, vg: f64| valve.currents(vp, vg, vs).0;
    assert!(near(ip(250.0, 250.0, -14.0), 72e-3, 0.03));
    let h = 1e-3;
    let gm = (ip(250.0, 250.0, -14.0 + h) - ip(250.0, 250.0, -14.0 - h)) / (2.0 * h);
    let ga = (ip(250.0 + h, 250.0, -14.0) - ip(250.0 - h, 250.0, -14.0)) / (2.0 * h);
    println!("gm {:.2} mA/V, ra {:.1} kohm", gm * 1e3, 1e-3 / ga);
    assert!(near(gm, 6.0e-3, 0.02) && near(1.0 / ga, 22_500.0, 0.02));
    for (vp, vs, vg, want) in [
        (400.0, 300.0, -25.0, 51e-3),
        (450.0, 350.0, -30.0, 47.5e-3),
        (540.0, 400.0, -38.0, 50e-3),
    ] {
        let got = ip(vp, vs, vg);
        println!(
            "{vp}/{vs}/{vg}: {:.1} mA (sheet {:.1})",
            got * 1e3,
            want * 1e3
        );
        assert!(near(got, want, 0.14), "{vp}/{vs}/{vg}: {got}");
    }
    let idle = ip(586.0, 589.0, -65.0);
    println!("VT-40: {:.1} mA, {:.1} W", idle * 1e3, idle * 586.0);
    assert!(idle * 586.0 < 35.0);
}

/// Channel one's volume set to put the table's 0.5 V on V1b's grid with its
/// 0.3 V at 400 Hz in, everything else at the middle, as the readings were
/// taken.
fn table_volume() -> f64 {
    let (mut lo, mut hi) = (0.0, 0.2);
    for _ in 0..18 {
        let mid = 0.5 * (lo + hi);
        if pre_rms("g1b", mid) < 0.5 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The rms at a preamplifier node with 0.3 V rms at 400 Hz at the input.
fn pre_rms(at: &str, volume: f64) -> f64 {
    let mut s = Simulation::new(vt40::tap(10_000.0, 1_000_000.0, at).unwrap(), RATE);
    s.set_control(vt40::VOLUME, volume);
    s.find_operating_point();
    let t = Tone::near(RATE, 4_800, 400.0, 0.3 * 2f64.sqrt());
    measure::run(t, 14_400, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 2f64.sqrt()
}

fn db(got: f64, table: f64) -> f64 {
    20.0 * (got / table).log10()
}

/// The preamplifier on the drawing's no-signal voltages: the rail, V1a's
/// cathode, the 6K11's two cathodes and V3a's within 6 %; V1a's and V1b's
/// plates, and unit 3's, within 13 % -- the solver's 12AX7 runs a little under
/// the published current, as everywhere in the catalogue. Unit 2's plate and
/// the follower's cathode are 16 % low: the drawing's own 1.37 V on unit 2's
/// cathode puts 0.42 mA through R207 and its plate at 160 V, which is where
/// the model has it, not at the drawing's 194 V.
#[test]
fn the_preamp_sits_on_the_drawing() {
    let c = vt40::build(10_000.0, 1_000_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let nodes = [
        ("bp", 354.0, 0.02),
        ("k1", 2.2, 0.06),
        ("k3", 24.0, 0.06),
        ("k2", 1.37, 0.06),
        ("k3a", 224.0, 0.02),
        ("p1", 205.0, 0.13),
        ("pm", 250.0, 0.06),
        ("p3", 209.0, 0.12),
        ("p2", 194.0, 0.17),
        ("kf", 196.0, 0.15),
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

/// Ampeg's A.C. voltage readings, pin by pin: with the volume putting the
/// table's 0.5 V on V1b's grid, every stage from there to V3a's cathode lands
/// within a decibel of the table -- V1b's plate 5.8 V, the 6K11's grid 0.47 V,
/// its cathode 0.5 V, unit 3's plate 0.1 V, unit 2's 5.2 V, the follower 5 V,
/// V3a's grid 0.32 V and cathode 0.3 V. And V1a's plate, with the volume
/// fully up, within a decibel of the table's 10 V: the SENSITIVITY switch's
/// middle position, which the netlist is built in.
#[test]
fn the_signal_follows_ampegs_ac_readings() {
    let v1a = pre_rms("p1", 1.0);
    println!("V1a plate {v1a:.2} V (10)");
    assert!(db(v1a, 10.0).abs() < 1.0);
    let volume = table_volume();
    println!("VOL 1 at {volume:.3}");
    for (node, table) in [
        ("pm", 5.8),
        ("g3", 0.47),
        ("k3", 0.5),
        ("p3", 0.1),
        ("p2", 5.2),
        ("kf", 5.0),
        ("g3a", 0.32),
        ("k3a", 0.3),
    ] {
        let v = pre_rms(node, volume);
        println!("{node:<4} {v:.3} V ({table})");
        assert!(db(v, table).abs() < 1.0, "{node}: {v} against {table}");
    }
}

/// SENSITIVITY: High, Middle, Low step V1a's gain down -- 15.6 V, 9.4 V and
/// 7.3 V at its plate from the table's 0.3 V -- and the table's 10 V is the
/// middle's.
#[test]
fn the_sensitivity_switch_steps_the_input_stage() {
    let plate = |position: usize| {
        let mut s = Simulation::new(vt40::tap(10_000.0, 1_000_000.0, "p1").unwrap(), RATE);
        s.set_control(vt40::VOLUME, 1.0);
        for (slot, value) in vt40::SENSITIVITY_SLOTS
            .into_iter()
            .zip(vt40::SENSITIVITY[position])
        {
            s.set_value(slot, value);
        }
        s.find_operating_point();
        let t = Tone::near(RATE, 4_800, 400.0, 0.3 * 2f64.sqrt());
        measure::run(t, 14_400, |x| s.process(x))
            .fundamental()
            .magnitude()
            / 2f64.sqrt()
    };
    let (high, middle, low) = (plate(0), plate(1), plate(2));
    println!("High {high:.2} V, Middle {middle:.2} V, Low {low:.2} V (table 10)");
    assert!(high > middle && middle > low);
    for other in [high, low] {
        assert!(db(middle, 10.0).abs() < db(other, 10.0).abs());
    }
}

/// MIDRANGE SELECT: the boost centres on 300 Hz, 800 Hz and 3 kHz, the VT-40's
/// published three, about 20 dB deep.
#[test]
fn the_midrange_select_moves_the_band_to_300_800_and_3000_hz() {
    let grid = [
        100.0, 150.0, 200.0, 300.0, 450.0, 800.0, 1_200.0, 2_000.0, 3_000.0, 4_500.0,
    ];
    let gain = |middle: f64, position: usize, hz: f64| {
        let mut s = Simulation::new(vt40::tap(10_000.0, 1_000_000.0, "out").unwrap(), RATE);
        s.set_control(vt40::VOLUME, 0.05);
        s.set_control(vt40::MIDDLE, middle);
        for (slot, value) in vt40::MID_SELECT_SLOTS
            .into_iter()
            .zip(vt40::MID_SELECT[position])
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

/// The power stage on the drawing's no-signal voltages: the 393 V, 594 V and
/// 589 V nodes (fitted), V3b's plate and cathode, the inverter's plates within
/// 7 % and its cathodes; each 7027A idles under its 35 W.
#[test]
fn the_power_stage_sits_on_the_drawing() {
    let spec = &PowerSpec::VT40_7027A;
    let c = power::build(spec, 10_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let nodes = [
        ("f_c", 393.0, 0.005),
        ("ht", 594.0, 0.005),
        ("scr", 589.0, 0.005),
        ("f_p1", 223.0, 0.03),
        ("f_k1", 1.96, 0.05),
        ("pi_p1", 218.0, 0.07),
        ("pi_p2", 218.0, 0.07),
        ("f_kp", 10.5, 0.05),
        ("og1", -65.0, 0.005),
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
    println!("a 7027A idles at {:.1} mA, {:.1} W", ia * 1e3, ia * vp);
    assert!(ia * vp < 35.0 && ia > 20e-3);
}

/// The rms at a power-stage node with `rms` at 400 Hz at its input.
fn power_rms(at: &str, rms: f64) -> f64 {
    let mut s = Simulation::new(
        power::tap(&PowerSpec::VT40_7027A, 10_000.0, at).unwrap(),
        RATE,
    );
    s.find_operating_point();
    let t = Tone::near(RATE, 4_800, 400.0, rms * 2f64.sqrt());
    measure::run(t, 19_200, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 2f64.sqrt()
}

/// The readings on through the power stage, from the table's 0.28 V on V3b's
/// grid: V3b's plate (3.5 V) and the inverter's plates (38 V) within 1.5 dB.
/// The output valves are hotter than the table: it gives 250 V on a plate for
/// 38 V on the grid, three-quarter power, 17.9 V across 8 ohm; the model makes
/// more than that for less drive, and its 8 ohm stands between the table and
/// 3 dB above it. Recorded in the log, not tuned.
#[test]
fn the_power_stage_follows_the_readings_with_hot_output_valves() {
    for (node, table, within) in [
        ("f_p1", 3.5, 1.5),
        ("pi_p2", 38.0, 1.5),
        ("pi_p1", 38.0, 1.5),
    ] {
        let v = power_rms(node, 0.28);
        println!("{node:<6} {v:.2} V ({table})");
        assert!(db(v, table).abs() < within, "{node}: {v} against {table}");
    }
    let out = power_rms("spk", 0.28);
    println!("8 ohm {out:.2} V (17.9)");
    assert!(db(out, 17.9) > 0.0 && db(out, 17.9) < 3.0, "{out}");
}

#[test]
fn it_is_registered_with_its_own_stage_and_controls() {
    let circuits = Circuit::ids().unwrap();
    assert_eq!(circuits[Circuit::AmericanVt40.to_index()], "amp_ampeg_vt40");
    assert_eq!(Circuit::AmericanVt40.name(), "American VT-40");
    let powers = PowerAmp::ids().unwrap();
    assert_eq!(
        powers[PowerAmp::American7027A.to_index()],
        "power_ampeg_7027a"
    );
    let gain = Circuit::AmericanVt40.voice();
    assert_eq!(gain, Gain::AmericanVt40);
    assert!(gain.is_modelled());
    assert_eq!(
        PowerAmp::Matched.voice().resolved(gain),
        Some(PowerModel::American7027A)
    );
    assert_eq!(
        PowerAmp::American7027A.voice().resolved(Gain::Clean),
        Some(PowerModel::American7027A)
    );
    assert_eq!(
        gain.own_tone(),
        Some((vt40::BASS, vt40::MIDDLE, vt40::TREBLE))
    );
    assert_eq!(gain.drive_name(), "VOLUME");
    assert!(gain.level_control().is_none());
    assert_eq!(gain.bright_switch().unwrap().slot, vt40::ULTRA_HI_SLOT);
    assert_eq!(
        gain.mid_switch().unwrap().labels,
        ["300 Hz", "800 Hz", "3 kHz"]
    );
    assert!(gain.low_switch().is_none());
}

/// The preamplifier's small-signal gain is the same at every rate the solver
/// runs at.
#[test]
fn the_preamp_is_the_same_at_every_rate() {
    let at = |rate: f64| {
        let mut s = Simulation::new(vt40::build(10_000.0, 1_000_000.0).unwrap(), rate);
        s.set_control(vt40::VOLUME, 0.3);
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

/// At every rate the plugin runs at, through its own power stage, hot and
/// finite, and without allocating.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::AmericanVt40,
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
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 110.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(work.gain.solves > 0 && work.power.solves > 0, "{rate}");
    }
}

/// The reverb, through the chain: a 50 ms burst at 500 Hz leaves a tail that
/// is still there 100-250 ms after it ends with the panel's Reverb up, and is
/// a hundred times quieter with it down; and the tank is driven within reason
/// (the coil's copper well under a volt). The Reverb knob is live for the
/// VT-40 and Speed and Intensity are not.
#[test]
fn the_reverb_knob_reaches_a_tank_that_rings() {
    let tail = |reverb: f64| {
        let mut chain = Chain::new(RATE);
        chain.apply(&Settings {
            gain: Gain::AmericanVt40,
            drive: 0.4,
            reverb,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let burst = (0.05 * RATE) as usize;
        for k in 0..burst {
            chain.process(0.05 * (std::f64::consts::TAU * 500.0 * k as f64 / RATE).sin());
        }
        let (start, end) = ((0.1 * RATE) as usize, (0.25 * RATE) as usize);
        (0..end)
            .map(|_| chain.process(0.0))
            .enumerate()
            .filter(|(k, _)| *k >= start)
            .map(|(_, y)| y * y)
            .sum::<f64>()
    };
    let (off, on) = (tail(0.0), tail(0.6));
    println!("tail energy: Reverb 0 {off:.3e}, Reverb 0.6 {on:.3e}");
    assert!(on > 100.0 * off, "{off} {on}");
    let knobs = ToneKnobs::for_state(Circuit::AmericanVt40, ToneStack::Off);
    assert_eq!(knobs.extras, [true, false, false]);
    let twin = ToneKnobs::for_state(Circuit::Twin, ToneStack::Off);
    assert_eq!(twin.extras, [true, true, true]);
}

/// The tank's drive: V202's second plate at 400 Hz with the A.C. table's
/// 0.48 V on the treble wiper is within 3 dB of the table's 2.3 V -- the
/// 1475 ohm coil resonating with C209 -- where a 200 ohm coil would leave it
/// near 4.3 V.
#[test]
fn the_reverb_driver_is_loaded_as_the_table_has_it() {
    let volume = table_volume();
    let grid = pre_rms("x1", volume);
    let plate = pre_rms("d2p", volume);
    println!("V202 grid {grid:.3} V (.48), second plate {plate:.2} V (2.3)");
    assert!(db(grid, 0.48).abs() < 1.0);
    assert!(db(plate, 2.3).abs() < 3.0, "{plate}");
}
