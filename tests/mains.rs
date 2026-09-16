//! The mains: what an amplifier does when it is not plugged into the wall.
use gainstagefx::circuits::{plexi, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::Mains;
use gainstagefx::voice::{Chain, Gain, Settings, Tone as Stack, NOMINAL_DBFS};

const RATE: f64 = 96_000.0;

/// Every supply comes down together, and nothing else changes. That is all a
/// variac does, and it is what makes the amplifier behave differently: the
/// valves have less room, so the same playing pushes them further.
#[test]
fn lowering_the_mains_lowers_every_rail_in_proportion() {
    let nodes = ["v1n", "v2n", "pin"];
    let rails = |scale: f64| {
        let circuit = plexi::build(10_000.0, 1_000_000.0).unwrap();
        let idx: Vec<usize> = nodes.iter().map(|n| circuit.unknown_named(n).unwrap()).collect();
        let mut s = Simulation::new(circuit, RATE);
        s.set_supply_scale(scale);
        assert!(s.find_operating_point());
        idx.iter().map(|&i| s.voltage_at(i)).collect::<Vec<f64>>()
    };
    let full = rails(1.0);
    let low = rails(0.7);
    for (a, b) in full.iter().zip(&low) {
        let ratio = b / a;
        println!("{a:.0} V -> {b:.0} V ({ratio:.2})");
        // Not exactly 0.7: the valves' own currents move too, which is the
        // point -- a rail with a dropper in front of it falls a little less.
        assert!((0.6..0.85).contains(&ratio), "{ratio}");
    }
}

/// And the power stage, where it matters: the same drive is further into
/// distortion on a variac than on the wall.
#[test]
fn a_variac_makes_the_power_stage_give_way_sooner() {
    let driven = |scale: f64| {
        let mut s = Simulation::new(
            power::build(&power::PowerSpec::PLEXI_EL34, 10_000.0).unwrap(),
            RATE,
        );
        s.set_supply_scale(scale);
        s.find_operating_point();
        let m = measure::run(Tone::near(RATE, 16_384, 220.0, 8.0), (RATE / 5.0) as usize, |x| {
            s.process(x)
        });
        (m.gain_db(), m.thd_percent())
    };
    let (wall_db, wall_thd) = driven(1.0);
    let (variac_db, variac_thd) = driven(0.7);
    println!("wall {wall_db:.1} dB {wall_thd:.1} %, variac {variac_db:.1} dB {variac_thd:.1} %");
    assert!(variac_thd > wall_thd * 1.4, "{variac_thd} against {wall_thd}");
    assert!(variac_db < wall_db, "and it is quieter");
}

/// The whole chain answers to it, and the setting reaches it from `Settings`.
#[test]
fn the_chain_carries_the_setting_and_stays_stable() {
    assert_eq!(Mains::Nominal.fraction(), 1.0);
    assert_eq!(Mains::Seventy.fraction(), 0.7);
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let rms = |mains: f64| {
        let mut chain = Chain::new(48_000.0);
        chain.apply(&Settings {
            gain: Gain::Plexi,
            drive: 0.9,
            tone: Stack::Off,
            oversampling: 1,
            mains,
            ..Settings::default()
        });
        chain.settle();
        chain.find_operating_point();
        let mut sum = 0.0;
        for k in 0..48_000 {
            let y = chain.process(amplitude * (k as f64 * std::f64::consts::TAU * 110.0 / 48_000.0).sin());
            assert!(y.is_finite());
            if k >= 24_000 {
                sum += y * y;
            }
        }
        (sum / 24_000.0).sqrt()
    };
    let wall = rms(1.0);
    let variac = rms(0.7);
    println!("wall {:.4}, variac {:.4}", wall, variac);
    assert!(variac < wall, "a variac is quieter: {variac} against {wall}");
    assert!(variac > wall * 0.2, "but not by everything: {variac}");
}
