//! The British 47 (EMI REDD.47) against EMI's drawing REDD.47/C1 and its
//! description REDD.M47. See `docs/models/british_47.md`;
//! `examples/british_47_op.rs` prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::british_47::{self as b47, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::Circuit;
use gainstagefx::voice::{Chain, Gain, Settings, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

/// Gain from the input terminals to a 200 ohm load, as REDD.M47 states it.
fn terminal_gain(position: f64, hz: f64, volts: f64) -> (f64, f64) {
    let c = tap(200.0, 200.0, "out").unwrap();
    let input = c.unknown_named("in").unwrap();
    let mut s = Simulation::new(c, RATE);
    s.set_control(b47::GAIN, position);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts);
    let mut terminal = Vec::new();
    let m = measure::run(t, (RATE / 2.0) as usize, |x| {
        let y = s.process(x);
        terminal.push(s.voltage_at(input));
        y
    });
    let tail = &terminal[terminal.len() / 2..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let rms =
        (tail.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / tail.len() as f64).sqrt();
    (
        m.gain_db() + 20.0 * ((volts / 2f64.sqrt()) / rms).log10(),
        m.thd_percent(),
    )
}

/// "With a 200 ohm load connected between terminals 2/5 and 2/6, the voltage
/// gain measured between input and output terminals is equivalent to 34, 40,
/// or 46 dB, selected by the switch." The fine gain is set for the middle, as
/// EMI sets it; the other two land on their own -- which is the S1 network,
/// read off a 150 ppi scan, agreeing with EMI's numbers.
#[test]
fn the_switch_gives_34_40_and_46_db() {
    for (position, want) in [(0.15, 34.0), (0.5, 40.0), (0.85, 46.0)] {
        let (g, _) = terminal_gain(position, 1_000.0, 1e-3);
        println!("{want} dB position: {g:.2}");
        assert!((g - want).abs() < 0.6, "{want}: {g}");
    }
}

/// The fine gain that does it sits inside EMI's stated limits for R5
/// shunted, 77 to 120 ohm.
#[test]
fn the_fine_gain_is_inside_emis_limits() {
    let r5 = 120.0 * b47::FINE_GAIN_SHUNT / (120.0 + b47::FINE_GAIN_SHUNT);
    assert!((77.0..120.0).contains(&r5), "{r5}");
}

/// The drawing's operating points: V2's pair at 18 mA on 200 ohm (the fit
/// gives about 17), V1 within its 1.35 mA's spread.
#[test]
fn it_runs_where_the_drawing_does() {
    let c = tap(200.0, 200.0, "out").unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let (k1, k2, p2) = (at("k1"), at("k2"), at("p2"));
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let pair = s.voltage_at(k2) / 200.0;
    let v1 = s.voltage_at(k1) / 1_120.0;
    println!(
        "V2 {:.1} mA, anode {:.1} V; V1 {:.2} mA",
        pair * 1e3,
        s.voltage_at(p2),
        v1 * 1e3
    );
    assert!((pair - 18e-3).abs() < 2e-3);
    assert!((s.voltage_at(p2) - 138.0).abs() < 12.0);
    assert!((v1 - 1.35e-3).abs() < 0.3e-3);
}

/// Low distortion at its working level, and overload where EMI puts it: at
/// 0 dBm into 200 ohm (0.447 V) well under 1 %, and past EMI's +14 dBm peak
/// rating (2.24 V into 200 ohm) running into its limit.
#[test]
fn it_is_clean_at_its_level_and_overloads_past_its_rating() {
    // At the 40 dB position: -45 dBu in is about -5 dBu, 0 dBm(200), out.
    let quiet = 0.775 * 10f64.powf(-45.0 / 20.0) * 2f64.sqrt() * 2.0;
    let loud = 0.775 * 10f64.powf(-22.0 / 20.0) * 2f64.sqrt() * 2.0;
    let (_, thd_quiet) = terminal_gain(0.5, 1_000.0, quiet);
    let (_, thd_loud) = terminal_gain(0.5, 1_000.0, loud);
    println!("THD at working level {thd_quiet:.2} %, driven {thd_loud:.1} %");
    assert!(thd_quiet < 0.5);
    assert!(thd_loud > 5.0);
}

#[test]
fn it_is_registered_and_realtime_safe_at_every_rate() {
    let ids = Circuit::ids().unwrap();
    assert_eq!(ids[Circuit::British47.to_index()], "pre_emi_redd47");
    assert_eq!(Circuit::British47.voice(), Gain::British47);
    assert!(Gain::British47.is_modelled());
    assert!(Gain::British47.power_stage().is_none());
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::British47,
            drive: 0.9,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 440.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
    }
}
