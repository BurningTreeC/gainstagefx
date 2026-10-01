//! The Brit Plexi Bass (Marshall 1992 Super Bass, Unicord 70-13-11) against
//! its drawing, and against the 1959 of the same drawing set wherever the two
//! differ. See `docs/models/brit_plexi_bass.md`; `examples/plexi_bass_op.rs`
//! prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::{plexi, plexi_bass, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::Circuit as Panel;
use gainstagefx::voice::{
    Chain, Gain, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

fn gain(circuit: Circuit, volume: usize, hz: f64) -> f64 {
    let mut s = Simulation::new(circuit, RATE);
    s.set_control(volume, 1.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-4);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
}

fn bass(at: &str) -> Circuit {
    plexi_bass::tap(10_000.0, 1_000_000.0, at).unwrap()
}

fn lead(at: &str) -> Circuit {
    plexi::tap(10_000.0, 1_000_000.0, at).unwrap()
}

/// The inverter node the power stage is fed from is the one this
/// preamplifier's chain arrives at, and both of V1's halves, on their shared
/// 820 ohm, sit where each other's current puts them.
#[test]
fn the_supply_chain_and_the_shared_cathode_are_self_consistent() {
    let c = plexi_bass::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["pin", "v1_p", "v1b_p", "v1_k"];
    let nodes: Vec<usize> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    println!("{v:?}");
    assert!((v[0] - plexi_bass::INVERTER_NODE).abs() < 1.0, "{}", v[0]);
    // Two halves at rest draw the same: the played plate and the idle one.
    assert!((v[1] - v[2]).abs() < 0.5, "{} {}", v[1], v[2]);
    // Twice an ECC83's current through 820 ohm: more than one half's would
    // give, which is the point of building both.
    let one = v[3] / 820.0 / 2.0;
    assert!((0.5e-3..1.2e-3).contains(&one), "{one}");
}

/// The bass head's whole difference, from the jack to the treble wiper with
/// the tone controls at noon: the 1959 throws the bottom away at its first
/// coupling capacitor and the 1992 does not, so relative to 1 kHz the 1992
/// carries at least 10 dB more of 40 Hz.
#[test]
fn it_keeps_the_bass_the_lead_amplifier_throws_away() {
    let tilt = |c: fn(&str) -> Circuit, volume| {
        gain(c("out"), volume, 40.0) - gain(c("out"), volume, 1_000.0)
    };
    let (bass_tilt, lead_tilt) = (tilt(bass, plexi_bass::VOLUME), tilt(lead, plexi::VOLUME));
    println!("40 Hz against 1 kHz: 1992 {bass_tilt:+.1} dB, 1959 {lead_tilt:+.1} dB");
    assert!(bass_tilt > lead_tilt + 10.0);
}

/// V2a unbypassed: 820 ohm and nothing across it. An ECC83 with a 100 k
/// plate and that cathode gives `mu RL / (rp + RL + (mu + 1) Rk)`, about
/// 30 dB into the follower's grid, where the 1959's bypassed stage gives
/// several decibels more.
#[test]
fn the_second_stage_is_unbypassed() {
    let stage = |c: fn(&str) -> Circuit, volume| {
        gain(c("v2_p"), volume, 1_000.0) - gain(c("v2_g"), volume, 1_000.0)
    };
    let (b, l) = (stage(bass, plexi_bass::VOLUME), stage(lead, plexi::VOLUME));
    println!("V2a: 1992 {b:.1} dB, 1959 {l:.1} dB");
    assert!((28.0..33.0).contains(&b), "{b}");
    assert!(l > b + 2.0, "{l} {b}");
}

/// The power stage is the 1959's but for the .1 uF couplings into the output
/// valves' 220 k leaks, which move that corner from 33 Hz to 7 Hz: at 10 Hz
/// the 1992's grids get several decibels more of what the inverter sends.
#[test]
fn its_power_stage_couples_the_bottom_through() {
    let a = power::PowerSpec::PLEXI_BASS_EL34;
    let b = power::PowerSpec::PLEXI_EL34;
    assert_eq!(a.couple, 0.1e-6);
    assert_eq!(b.couple, 0.022e-6);
    assert_eq!(a.feedback, b.feedback);
    assert_eq!(a.presence_pot, b.presence_pot);
    let grid = |spec: &power::PowerSpec, hz: f64| {
        let mut s = Simulation::new(power::tap(spec, 10_000.0, "on1").unwrap(), RATE);
        s.find_operating_point();
        let t = Tone::near(RATE, 32_768, hz, 0.1);
        measure::run(t, (RATE / 2.0) as usize, |x| s.process(x)).gain_db()
    };
    let (bass_grid, lead_grid) = (
        grid(&a, 10.0) - grid(&a, 1_000.0),
        grid(&b, 10.0) - grid(&b, 1_000.0),
    );
    println!("10 Hz at the grids against 1 kHz: 1992 {bass_grid:+.1} dB, 1959 {lead_grid:+.1} dB");
    assert!(bass_grid > lead_grid + 3.0);
}

#[test]
fn it_is_registered_with_its_own_matched_stage() {
    let ids = Panel::ids().unwrap();
    assert_eq!(ids[Panel::PlexiBass.to_index()], "amp_marshall_1992");
    assert_eq!(Panel::PlexiBass.voice(), Gain::PlexiBass);
    assert_eq!(Panel::PlexiBass.name(), "Brit Plexi Bass");
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::PlexiBass),
        Some(PowerModel::BritPlexiBassEL34)
    );
    assert_eq!(
        PowerAmp::BritPlexiBassEL34.resolved(Gain::Clean),
        Some(PowerModel::BritPlexiBassEL34)
    );
    assert!(Gain::PlexiBass.level_control().is_none());
    assert_eq!(Gain::PlexiBass.drive_name(), "VOLUME");
}

/// At every rate, the whole chain hot and finite, and without allocating.
#[test]
fn the_chain_is_realtime_safe_at_every_rate() {
    let settings = Settings {
        gain: Gain::PlexiBass,
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
