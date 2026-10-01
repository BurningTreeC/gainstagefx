//! The Brum 100 (Laney Supergroup 100 Mk I, the 1969 trace) against its
//! drawing. See `docs/models/brum_100.md`; `examples/brum100_op.rs` prints
//! what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::{brum100, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{Netlist, TriodeSpec};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::Circuit as Panel;
use gainstagefx::voice::{
    Chain, Gain, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

/// The constants the preamplifier and the power stage hand each other --
/// HT3, the inverter's current as a resistance, and the unbuilt BASS half's --
/// are the ones each side actually arrives at.
#[test]
fn the_supply_chain_is_self_consistent() {
    let c = brum100::build(10_000.0, 1_000_000.0).unwrap();
    let (ht3, ht5) = (
        c.unknown_named("ht3").unwrap(),
        c.unknown_named("ht5").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let (ht3, ht5) = (s.voltage_at(ht3), s.voltage_at(ht5));
    println!("HT3 {ht3:.1} V, HT5 {ht5:.1} V");
    assert!((ht3 - brum100::INVERTER_NODE).abs() < 1.0, "{ht3}");

    let spec = power::PowerSpec::BRUM_EL34;
    let c = power::build(&spec, 10_000.0).unwrap();
    let (p1, p2) = (
        c.unknown_named("pi_p1").unwrap(),
        c.unknown_named("pi_p2").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let inverter = (spec.pi_supply - s.voltage_at(p1)) / spec.pi_plate_driven
        + (spec.pi_supply - s.voltage_at(p2)) / spec.pi_plate_other;
    let as_ohms = spec.pi_supply / inverter;
    println!("inverter {:.3} mA, {as_ohms:.0} ohm", inverter * 1e3);
    assert!(
        (as_ohms / brum100::INVERTER_IDLE - 1.0).abs() < 0.02,
        "{as_ohms}"
    );

    let mut net = Netlist::new("U1A");
    net.input("in", 1.0)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "g", 34_000.0)
        .supply("p", 100_000.0, ht5)
        .resistor("k", "gnd", 1_500.0)
        .triode("p", "g", "k", TriodeSpec::ECC83);
    let c = net.build("k").unwrap();
    let k = c.unknown_named("k").unwrap();
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let half = ht5 / (s.voltage_at(k) / 1_500.0);
    assert!((half / brum100::BASS_V1_IDLE - 1.0).abs() < 0.03, "{half}");
}

/// On the trace's own two numbers, 600 V at HT1 and -54 V of bias, four
/// EL34s idle warm and inside their rating.
#[test]
fn the_power_stage_idles_on_the_traces_rails() {
    let spec = power::PowerSpec::BRUM_EL34;
    assert_eq!(spec.plate_supply, 600.0);
    assert_eq!(spec.bias, -54.0);
    let c = power::build(&spec, 10_000.0).unwrap();
    let (ht, grid) = (
        c.unknown_named("ht").unwrap(),
        c.unknown_named("og1").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let each = (spec.plate_supply - s.voltage_at(ht)) / spec.supply_resistance / 4.0;
    let watts = each * s.voltage_at(ht);
    println!("{:.1} mA and {watts:.1} W a valve", each * 1e3);
    assert!((s.voltage_at(grid) + 54.0).abs() < 0.5);
    assert!((0.020..0.045).contains(&each), "{each}");
    assert!(watts < 25.0, "{watts}");
}

/// R37, 3.3 k whose track is the tail's return: turned up it takes the top
/// out of the loop, so the stage gets brighter.
#[test]
fn the_presence_brightens_the_stage() {
    let tilt = |presence: f64| {
        let at = |hz: f64| {
            let mut s = Simulation::new(
                power::build(&power::PowerSpec::BRUM_EL34, 10_000.0).unwrap(),
                RATE,
            );
            s.set_control(power::PRESENCE, presence);
            s.find_operating_point();
            let t = Tone::near(RATE, 16_384, hz, 0.5);
            measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
        };
        at(5_000.0) - at(500.0)
    };
    let (down, up) = (tilt(0.0), tilt(1.0));
    println!("5 kHz against 500 Hz: presence 0 {down:+.2} dB, 1 {up:+.2} dB");
    assert!(up > down + 2.0);
}

#[test]
fn it_is_registered_with_its_own_matched_stage() {
    let ids = Panel::ids().unwrap();
    assert_eq!(ids[Panel::Brum100.to_index()], "amp_laney_supergroup");
    assert_eq!(Panel::Brum100.voice(), Gain::Brum100);
    assert_eq!(Panel::Brum100.name(), "Brum 100");
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::Brum100),
        Some(PowerModel::BrumEL34)
    );
    assert!(Gain::Brum100.level_control().is_none());
    assert_eq!(Gain::Brum100.drive_name(), "GAIN");
}

/// At every rate, the whole chain hot and finite, and without allocating.
#[test]
fn the_chain_is_realtime_safe_at_every_rate() {
    let settings = Settings {
        gain: Gain::Brum100,
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
