//! The Oregon T (Sunn Model T, drawing D-1029 A, 1973) against the voltage
//! chart printed on its drawing, the 6550 fit against General Electric's
//! sheet, and the ultra-linear stage's solve. See `docs/models/oregon_t.md`;
//! `examples/oregon_t_op.rs` prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::oregon_t::{self, Input};
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{PentodeSpec, Taper, GROUND};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::Circuit as Panel;
use gainstagefx::voice::{
    Chain, Gain, Level, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};
use nice_plug::prelude::Enum;

const SOURCE: f64 = 10_000.0;

fn near(got: f64, chart: f64, within: f64) -> bool {
    (got / chart - 1.0).abs() <= within
}

/// Every control at 10, as the chart was taken.
fn all_up(pre: &mut Simulation, pow: &mut Simulation) {
    for control in [
        oregon_t::VOLUME,
        oregon_t::NORMAL_VOLUME,
        oregon_t::TREBLE,
        oregon_t::BASS,
        oregon_t::MIDDLE,
        oregon_t::MASTER,
    ] {
        pre.set_control(control, 1.0);
    }
    pow.set_control(power::PRESENCE, 1.0);
}

/// The chart's no-signal preamplifier column: C 351 V, V1's plates 227 and
/// 232 V on a shared 1.71 V cathode, V2's plate 203 V and cathode 1.3 V, the
/// follower 205 V. Within 6 %.
#[test]
fn the_preamplifier_sits_on_the_chart() {
    let c = oregon_t::tap_with(Input::Both, SOURCE, 500_000.0, "out").unwrap();
    let rows = [
        ("c", 351.0),
        ("v1a_p", 227.0),
        ("v1b_p", 232.0),
        ("v1_k", 1.71),
        ("v2_p", 203.0),
        ("v2_k", 1.3),
        ("cf", 205.0),
    ];
    let nodes: Vec<usize> = rows
        .iter()
        .map(|(n, _)| c.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(c, 48_000.0);
    assert!(s.find_operating_point());
    for ((name, chart), node) in rows.iter().zip(nodes) {
        let got = s.voltage_at(node);
        println!("{name}: {got:.2} V (chart {chart})");
        assert!(near(got, *chart, 0.06), "{name}: {got:.2} against {chart}");
    }
}

struct Column {
    a: f64,
    plate: f64,
    screen: f64,
    grid: f64,
    pi_p1: f64,
    pi_p2: f64,
    pi_k: f64,
    rms_4_ohm: f64,
}

/// The power stage alone, a 1 kHz sine of `volts` peak at its input,
/// averaged over the last 0.3 s of 0.8.
fn power_column(volts: f64, rate: f64) -> Column {
    let spec = PowerSpec::OREGON_6550;
    let c = power::build(&spec, SOURCE).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let nodes = [
        at("ht"),
        at("ov1"),
        at("os1"),
        at("og1"),
        at("pi_p1"),
        at("pi_p2"),
        at("pi_k"),
    ];
    let mut s = Simulation::new(c, rate);
    s.set_control(power::PRESENCE, 1.0);
    s.find_operating_point();
    let (total, window) = ((rate * 0.8) as usize, (rate * 0.3) as usize);
    let mut sums = [0.0f64; 8];
    for n in 0..total {
        let x = volts * (std::f64::consts::TAU * 1_000.0 * n as f64 / rate).sin();
        let y = s.process(x);
        if n >= total - window {
            sums[0] += y * y;
            for (sum, &node) in sums[1..].iter_mut().zip(&nodes) {
                *sum += s.voltage_at(node);
            }
        }
    }
    let w = window as f64;
    Column {
        // Built at the 16 ohm tap; referred to the chart's 4 ohm.
        rms_4_ohm: (sums[0] / w).sqrt() / 2.0,
        a: sums[1] / w,
        plate: sums[2] / w,
        screen: sums[3] / w,
        grid: sums[4] / w,
        pi_p1: sums[5] / w,
        pi_p2: sums[6] / w,
        pi_k: sums[7] / w,
    }
}

/// The chart's no-signal power-stage column: A 519 V, plates 507, screens
/// 504, grids -53, the inverter's plates 271 and 251 and its cathodes 38.
#[test]
fn the_power_stage_idles_on_the_chart() {
    let idle = power_column(0.0, 48_000.0);
    println!(
        "A {:.1}, plate {:.1}, screen {:.1}, grid {:.1}, PI {:.1} / {:.1}, k {:.1}",
        idle.a, idle.plate, idle.screen, idle.grid, idle.pi_p1, idle.pi_p2, idle.pi_k
    );
    assert!(near(idle.a, 519.0, 0.01));
    assert!(near(idle.plate, 507.0, 0.02));
    assert!(near(idle.screen, 504.0, 0.015));
    assert!((idle.grid + 53.0).abs() < 0.5);
    assert!(near(idle.pi_p1, 271.0, 0.03));
    assert!(near(idle.pi_p2, 251.0, 0.03));
    // 22 k from the 16 ohm tap is in parallel with R26 at DC; built as 11 k
    // from 4 ohm this read 32.8 V.
    assert!(near(idle.pi_k, 38.0, 0.05), "{}", idle.pi_k);
}

/// The chart's 150 W column, with the stage delivering 24.5 V rms into 4 ohm:
/// A 481 V, plates 472, screens 459. The supply's two constants were fitted
/// to A's two readings (`power::OREGON_A_OPEN`, `OREGON_A_SOURCE`); the plates
/// and screens are what the stage then does.
#[test]
fn the_power_stage_sits_on_the_chart_at_150_watts() {
    let (mut lo, mut hi) = (10.0f64, 24.0f64);
    let mut full = power_column(hi, 48_000.0);
    for _ in 0..9 {
        let mid = 0.5 * (lo + hi);
        let c = power_column(mid, 48_000.0);
        if c.rms_4_ohm < 24.5 {
            lo = mid;
        } else {
            hi = mid;
            full = c;
        }
    }
    println!(
        "{:.2} V rms: A {:.1}, plate {:.1}, screen {:.1}",
        full.rms_4_ohm, full.a, full.plate, full.screen
    );
    assert!((full.rms_4_ohm - 24.5).abs() < 0.3);
    assert!(near(full.a, 481.0, 0.01));
    assert!(near(full.plate, 472.0, 0.015));
    assert!(near(full.screen, 459.0, 0.015));
}

/// And the whole amplifier, from BOTH IN with every control at 10, makes its
/// rating before its inverter blocks: at least 140 W into 4 ohm somewhere
/// near the 31 mV the example finds it at.
#[test]
fn the_whole_amplifier_makes_its_rating() {
    let rate = 48_000.0;
    let most = [0.025, 0.031, 0.037]
        .iter()
        .map(|&volts| {
            let mut pre = Simulation::new(
                oregon_t::tap_with(Input::Both, SOURCE, 500_000.0, "out").unwrap(),
                rate,
            );
            let mut pow =
                Simulation::new(power::build(&PowerSpec::OREGON_6550, SOURCE).unwrap(), rate);
            all_up(&mut pre, &mut pow);
            pre.find_operating_point();
            pow.find_operating_point();
            let (total, window) = ((rate * 1.0) as usize, (rate * 0.3) as usize);
            let mut sum = 0.0;
            for n in 0..total {
                let x = volts * (std::f64::consts::TAU * 1_000.0 * n as f64 / rate).sin();
                let y = pow.process(pre.process(x));
                if n >= total - window {
                    sum += y * y;
                }
            }
            let rms_4 = (sum / window as f64).sqrt() / 2.0;
            rms_4 * rms_4 / 4.0
        })
        .fold(0.0, f64::max);
    println!("most {most:.0} W");
    assert!(most > 140.0, "{most}");
}

/// Driven far past its rating from BOTH IN, at every presence setting, the
/// ultra-linear stage settles every sample. Its screens hang on the primary,
/// so a Newton step that put a plate on its edge used to take the screen's
/// current with it and the solve two-cycled -- unsettled on every sample from
/// 300 mV at 96 kHz, frozen at a DC level. See `device::Pentode::on_winding`.
#[test]
fn the_ultra_linear_stage_settles_driven_far_past_its_rating() {
    for rate in [48_000.0, 96_000.0] {
        for presence in [0.0, 0.5, 1.0] {
            for volts in [0.3, 1.0] {
                let mut pre = Simulation::new(
                    oregon_t::tap_with(Input::Both, SOURCE, 500_000.0, "out").unwrap(),
                    rate,
                );
                let mut pow =
                    Simulation::new(power::build(&PowerSpec::OREGON_6550, SOURCE).unwrap(), rate);
                all_up(&mut pre, &mut pow);
                pow.set_control(power::PRESENCE, presence);
                pre.find_operating_point();
                pow.find_operating_point();
                for n in 0..(rate * 0.2) as usize {
                    let x = volts * (std::f64::consts::TAU * 1_000.0 * n as f64 / rate).sin();
                    assert!(pow.process(pre.process(x)).is_finite());
                }
                let unsettled = pow.statistics().2;
                assert_eq!(unsettled, 0, "{rate} Hz, presence {presence}, {volts} V");
            }
        }
    }
}

/// R25 is printed "REV LOG" and wired as a rheostat under C9: as a C track it
/// has a tenth of its 25 k left in circuit at half rotation, and turned up it
/// takes the top out of the loop, so the stage brightens.
#[test]
fn the_presence_is_a_reverse_log_rheostat_and_brightens() {
    let spec = PowerSpec::OREGON_6550;
    assert_eq!(spec.presence_taper, Taper::AntiAudio);
    let left = 1.0 - Taper::AntiAudio.fraction(0.5);
    assert!((left - 0.1).abs() < 1e-12, "{left}");
    let tilt = |presence: f64| {
        let at = |hz: f64| {
            let mut s = Simulation::new(power::build(&spec, SOURCE).unwrap(), 96_000.0);
            s.set_control(power::PRESENCE, presence);
            s.find_operating_point();
            let t = Tone::near(96_000.0, 16_384, hz, 1.0);
            measure::run(t, 24_000, |x| s.process(x)).gain_db()
        };
        at(5_000.0) - at(500.0)
    };
    let (down, up) = (tilt(0.0), tilt(1.0));
    println!("5 kHz against 500 Hz: presence 0 {down:+.2} dB, 1 {up:+.2} dB");
    assert!(up > down + 2.0);
}

/// The 6550 fit meets General Electric's 6550-A sheet of April 1972 at the
/// four points it was fitted to and holds the ultra-linear row's screen
/// current, which it was not. See `tools/tube_fit/fit_6550.py`.
#[test]
fn the_6550_fit_meets_the_ge_sheet() {
    let valve = Pentode::new(0, 1, GROUND, 2, 1.0, PentodeSpec::T6550);
    let rows = [
        // (plate, screen, grid, plate mA, screen mA or 0 where not checked)
        (250.0, 250.0, -14.0, 140.0, 12.0),
        (400.0, 225.0, -16.5, 87.0, 0.0),
        (450.0, 450.0, -48.0, 75.0, 6.0),
    ];
    for (vp, vs, vg, ip, is) in rows {
        let (a, b) = valve.currents(vp, vg, vs);
        println!(
            "{vp}/{vs}/{vg}: {:.2} mA, screen {:.2} mA",
            a * 1e3,
            b * 1e3
        );
        assert!(near(a * 1e3, ip, 0.01), "{vp}/{vs}/{vg}: {a}");
        if is > 0.0 {
            assert!(near(b * 1e3, is, 0.05), "{vp}/{vs}/{vg}: {b}");
        }
    }
    let h = 1e-3;
    let gm = (valve.currents(250.0, -14.0 + h, 250.0).0
        - valve.currents(250.0, -14.0 - h, 250.0).0)
        / (2.0 * h);
    assert!(near(gm, 11e-3, 0.01), "{gm}");
}

#[test]
fn it_is_registered_with_its_own_stage_and_its_master() {
    let ids = Panel::ids().unwrap();
    assert_eq!(ids[Panel::OregonT.to_index()], "amp_sunn_model_t");
    assert_eq!(Panel::OregonT.voice(), Gain::OregonT);
    assert_eq!(Panel::OregonT.name(), "Oregon T");
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::OregonT),
        Some(PowerModel::Oregon6550)
    );
    assert_eq!(
        Gain::OregonT.level_control(),
        Some(Level::Circuit(oregon_t::MASTER))
    );
    assert_eq!(PowerSpec::OREGON_6550.presence_name(), Some("PRESENCE"));
}

/// At every rate, the whole chain hot and finite, and without allocating.
#[test]
fn the_chain_is_realtime_safe_at_every_rate() {
    let settings = Settings {
        gain: Gain::OregonT,
        drive: 1.0,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    };
    let amplitude = 4.0 * 10f64.powf(NOMINAL_DBFS / 20.0);
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
                let x = amplitude * (k as f64 * std::f64::consts::TAU * 55.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(work.gain.solves > 0 && work.power.solves > 0, "{rate}");
    }
}
