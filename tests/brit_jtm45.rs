//! The Brit 45 (Marshall JTM45, 1965) against Marshall's period drawing and
//! its valve voltage chart, and the KT66 fit against Marconi's sheet. See
//! `docs/models/brit_jtm45.md`; `examples/jtm45_op.rs` and
//! `tools/tube_fit/fit_kt66.py` print what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::{jtm45, power};
use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{PentodeSpec, GROUND};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::Circuit as Panel;
use gainstagefx::voice::{
    Chain, Gain, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};
use nice_plug::prelude::Enum;

fn near(got: f64, chart: f64, within: f64) -> bool {
    (got / chart - 1.0).abs() <= within
}

/// The KT66 fit meets Marconi's rows, each cathode-biased so that its grid
/// voltage is its resistor times its current, within 3 %; the mutual
/// conductance within 3 %; GEC's 22.5 kohm within 3 %.
#[test]
fn the_kt66_fit_meets_marconis_sheet() {
    let valve = Pentode::new(0, 1, GROUND, 2, 1.0, PentodeSpec::KT66);
    for (vp, vs, vg, ip, is) in [
        (250.0, 250.0, -15.52, 85.0, 6.3),
        (258.0, 258.0, -17.4, 81.0, 6.0),
        (400.0, 300.0, -26.0, 62.5, 0.0),
    ] {
        let (a, b) = valve.currents(vp, vg, vs);
        println!(
            "{vp}/{vs}/{vg}: {:.2} mA, screen {:.2} mA",
            a * 1e3,
            b * 1e3
        );
        // 3 % in the fit's own report; the 258 V row's is 3.002.
        assert!(near(a * 1e3, ip, 0.035), "{vp}/{vs}/{vg}: {a}");
        if is > 0.0 {
            assert!(near(b * 1e3, is, 0.035), "{vp}/{vs}/{vg}: {b}");
        }
    }
    let h = 1e-3;
    let gm = (valve.currents(250.0, -15.0 + h, 250.0).0
        - valve.currents(250.0, -15.0 - h, 250.0).0)
        / (2.0 * h);
    let ga = (valve.currents(250.0 + h, -15.0, 250.0).0
        - valve.currents(250.0 - h, -15.0, 250.0).0)
        / (2.0 * h);
    assert!(near(gm, 6.3e-3, 0.03), "{gm}");
    assert!(near(1.0 / ga, 22_500.0, 0.03), "{}", 1.0 / ga);
}

/// The preamplifier on the chart: V1's plates 220 V and shared cathode
/// 1.6 V, the gain stage 190 V and 1.1 V, the follower's cathode 190 V, the
/// supply nodes 380 V and 310 V.
#[test]
fn the_preamp_sits_on_marshalls_chart() {
    let c = jtm45::build(10_000.0, 1_000_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let nodes = [
        ("pin", 380.0, 0.03),
        ("v1n", 310.0, 0.05),
        ("v1_p", 220.0, 0.05),
        ("v1_k", 1.6, 0.10),
        ("v2_p", 190.0, 0.05),
        ("v2_k", 1.1, 0.10),
        ("cf", 190.0, 0.05),
    ];
    let idx: Vec<usize> = nodes.iter().map(|(n, _, _)| at(n)).collect();
    let mut s = Simulation::new(c, 48_000.0);
    s.find_operating_point();
    for ((name, chart, within), i) in nodes.iter().zip(idx) {
        let v = s.voltage_at(i);
        println!("{name:<5} {v:7.2} V (chart {chart})");
        assert!(near(v, *chart, *within), "{name}: {v} against {chart}");
    }
}

/// The power stage on the chart: the inverter's plates 250 V and cathodes
/// 40 V, the KT66s' plates 430 V; the KT66s idle under GEC's 25 W design
/// maximum at the derived -48 V of bias; and the inverter draws from its node
/// what the preamplifier's chain charges it with (`jtm45::INVERTER_IDLE`).
#[test]
fn the_power_stage_sits_on_the_chart() {
    let spec = &power::PowerSpec::JTM45_KT66;
    let c = power::build(spec, 10_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let (p1, p2, k, ht, scr) = (at("pi_p1"), at("pi_p2"), at("pi_k"), at("ht"), at("scr"));
    let mut s = Simulation::new(c, 48_000.0);
    s.find_operating_point();
    let (p1, p2, k, ht, scr) = (
        s.voltage_at(p1),
        s.voltage_at(p2),
        s.voltage_at(k),
        s.voltage_at(ht),
        s.voltage_at(scr),
    );
    println!("inverter plates {p1:.1} / {p2:.1} V, cathodes {k:.1} V; plates {ht:.1} V, screen node {scr:.1} V");
    assert!(near(p1, 250.0, 0.05) && near(p2, 250.0, 0.05), "{p1} {p2}");
    assert!(near(k, 40.0, 0.07), "{k}");
    assert!(near(ht, 430.0, 0.03), "{ht}");
    let valve = Pentode::new(0, 1, GROUND, 2, 1.0, PentodeSpec::KT66);
    let (ia, _) = valve.currents(ht, spec.bias, scr);
    println!("a KT66 idles at {:.1} mA, {:.1} W", ia * 1e3, ia * ht);
    assert!(ia * ht < 25.0, "{}", ia * ht);
    let draw =
        (spec.pi_supply - p1) / spec.pi_plate_driven + (spec.pi_supply - p2) / spec.pi_plate_other;
    assert!(near(spec.pi_supply / draw, jtm45::INVERTER_IDLE, 0.05));
}

/// The bright channel's 100 pF across its volume: with the volume low it
/// passes the treble round the track, so 5 kHz against 200 Hz stands well
/// above where it is with the volume full, where the capacitor is shorted.
#[test]
fn the_bright_cap_lifts_the_treble_at_low_volume() {
    let tilt = |volume: f64| {
        let c = jtm45::tap(10_000.0, 1_000_000.0, "v2_g").unwrap();
        let g = |hz: f64| {
            let mut s = Simulation::new(c.clone(), 96_000.0);
            s.set_control(jtm45::VOLUME, volume);
            s.find_operating_point();
            let t = Tone::near(96_000.0, 16_384, hz, 1e-4);
            measure::run(t, 24_000, |x| s.process(x)).gain_db()
        };
        g(5_000.0) - g(200.0)
    };
    let (low, full) = (tilt(0.25), tilt(1.0));
    println!("5 kHz against 200 Hz at V2a's grid: volume 0.25 {low:+.1} dB, full {full:+.1} dB");
    assert!(low > full + 3.0, "{low} {full}");
}

#[test]
fn it_is_registered_with_its_own_matched_stage() {
    let ids = Panel::ids().unwrap();
    assert_eq!(ids[Panel::Brit45.to_index()], "amp_marshall_jtm45");
    assert_eq!(Panel::Brit45.voice(), Gain::Brit45);
    assert_eq!(Panel::Brit45.name(), "Brit 45");
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::Brit45),
        Some(PowerModel::Brit45KT66)
    );
    assert_eq!(
        PowerAmp::Brit45KT66.resolved(Gain::Clean),
        Some(PowerModel::Brit45KT66)
    );
    assert!(Gain::Brit45.level_control().is_none());
    assert_eq!(Gain::Brit45.drive_name(), "VOLUME");
}

/// The preamplifier's small-signal gain to the treble wiper is the same at
/// every rate the solver runs at.
#[test]
fn the_preamp_is_the_same_at_every_rate() {
    let at = |rate: f64| {
        let mut s = Simulation::new(jtm45::build(10_000.0, 1_000_000.0).unwrap(), rate);
        s.set_control(jtm45::VOLUME, 0.5);
        s.find_operating_point();
        let t = Tone::near(rate, 8_192, 1_000.0, 1e-4);
        measure::run(t, (rate * 0.2) as usize, |x| s.process(x)).gain_db()
    };
    let at48 = at(48_000.0);
    for rate in [44_100.0, 88_200.0, 96_000.0, 192_000.0] {
        let g = at(rate);
        assert!((g - at48).abs() < 0.3, "{rate}: {g} against {at48}");
    }
}

/// At every rate, the whole chain hot and finite, and without allocating.
#[test]
fn the_chain_is_realtime_safe_at_every_rate() {
    let settings = Settings {
        gain: Gain::Brit45,
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
                let x = amplitude * (k as f64 * std::f64::consts::TAU * 110.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(work.gain.solves > 0 && work.power.solves > 0, "{rate}");
    }
}
