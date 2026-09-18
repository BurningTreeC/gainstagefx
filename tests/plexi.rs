//! The Brit Plexi preamplifier and power stage, against the drawings.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::plexi::{self, BASS, MIDDLE, TREBLE, VOLUME};
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{
    Chain, Gain, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};

const RATE: f64 = 96_000.0;

fn at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(plexi::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    for control in [VOLUME, TREBLE, BASS, MIDDLE] {
        s.set_control(control, 0.5);
    }
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x))
}

/// The supply chain agrees with itself: the inverter node the preamplifier's
/// droppers settle at is the one the power stage's inverter is fed from, and
/// the inverter's idle current is the load that was assumed for it.
#[test]
fn the_supply_chain_is_self_consistent() {
    let pre = plexi::build(10_000.0, 1_000_000.0).unwrap();
    let names = ["pin", "v2n", "v1n", "v1_k", "v2_k"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| pre.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(pre, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    println!(
        "inverter node {:.1} V, V2 {:.1} V, V1 {:.1} V",
        v[0], v[1], v[2]
    );
    assert!(
        (v[0] - plexi::INVERTER_NODE).abs() < 3.0,
        "inverter node {:.1}",
        v[0]
    );
    // Below the c. 1967 drawing's 300 / 270 V, which had an 8.2 k dropper where
    // Unicord's has 20 k, and above the 2203's 290 / 280 V is not required.
    assert!(
        (240.0..310.0).contains(&v[1]) && (225.0..290.0).contains(&v[2]),
        "{v:?}"
    );
    assert!(
        (1.0..2.5).contains(&v[3]) && (0.6..1.6).contains(&v[4]),
        "{v:?}"
    );

    let spec = &PowerSpec::PLEXI_EL34;
    let amp = power::build(spec, 10_000.0).unwrap();
    let names = ["pi_p1", "pi_p2", "ht"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| amp.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(amp, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    let inverter = (spec.pi_supply - v[0]) / spec.pi_plate_driven
        + (spec.pi_supply - v[1]) / spec.pi_plate_other;
    let assumed = spec.pi_supply / plexi::INVERTER_IDLE;
    assert!(
        (inverter / assumed - 1.0).abs() < 0.05,
        "{inverter} A against {assumed} A"
    );
    // And the valves idle at a sane fraction of an EL34's 25 W.
    let per_valve = (spec.plate_supply - v[2]) / spec.supply_resistance / 4.0 * v[2];
    println!("{per_valve:.1} W a valve at idle");
    assert!((12.0..20.0).contains(&per_valve), "{per_valve}");
}

#[test]
fn silence_in_is_silence_out() {
    let mut s = Simulation::new(plexi::build(10_000.0, 1_000_000.0).unwrap(), RATE);
    assert!(s.find_operating_point());
    let worst = (0..(RATE as usize / 4))
        .map(|_| s.process(0.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

/// The bright channel is bright, and brightest with the Volume low: .0022 uF
/// into the Volume, the 5000 pF round it and the 500 pF round the mixer.
#[test]
fn the_bright_channel_is_bright_and_brightest_turned_down() {
    let tilt = |volume: f64| {
        at(&[(VOLUME, volume)], 5_000.0, 0.001).gain_db()
            - at(&[(VOLUME, volume)], 100.0, 0.001).gain_db()
    };
    let (low, high) = (tilt(0.2), tilt(1.0));
    println!("5 kHz over 100 Hz: {low:.1} dB at Volume 0.2, {high:.1} dB at 1.0");
    assert!(high > 6.0, "{high}");
    assert!(low > high + 6.0, "{low} vs {high}");
}

#[test]
fn the_stack_controls_go_the_way_they_are_labelled() {
    let small = 0.001;
    let span = |control: usize, hz: f64| {
        at(&[(control, 1.0)], hz, small).gain_db() - at(&[(control, 0.0)], hz, small).gain_db()
    };
    let (treble, bass, middle) = (span(TREBLE, 4_000.0), span(BASS, 80.0), span(MIDDLE, 600.0));
    println!("treble {treble:.1} dB, bass {bass:.1} dB, middle {middle:.1} dB");
    assert!(treble > 6.0 && bass > 6.0 && middle > 3.0);
}

/// With no master, the Volume decides how hard the power stage is driven: at a
/// guitar's level a quarter of the way up the preamplifier already hands the
/// inverter more than the few volts where the power stage starts to clip, and
/// turned up it hands it several times that.
#[test]
fn the_volume_drives_the_power_stage() {
    let peak =
        |volume: f64| 0.122 * 10f64.powf(at(&[(VOLUME, volume)], 220.0, 0.122).gain_db() / 20.0);
    let (quarter, full) = (peak(0.25), peak(1.0));
    println!("{quarter:.1} V at a quarter, {full:.1} V full");
    assert!(quarter > 3.0 && full > 20.0, "{quarter} {full}");

    let mut amp = Simulation::new(
        power::build(&PowerSpec::PLEXI_EL34, 10_000.0).unwrap(),
        RATE,
    );
    amp.find_operating_point();
    let tone = Tone::near(RATE, 16_384, 220.0, 2.0);
    let clean = measure::run(tone, (RATE / 5.0) as usize, |x| amp.process(x));
    assert!(clean.thd_percent() < 2.0, "{}", clean.thd_percent());
    let tone = Tone::near(RATE, 16_384, 220.0, 20.0);
    let cranked = measure::run(tone, (RATE / 5.0) as usize, |x| amp.process(x));
    assert!(cranked.thd_percent() > 20.0, "{}", cranked.thd_percent());
}

#[test]
fn matched_is_the_plexi_el34_and_the_chain_stays_realtime_safe() {
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::Plexi),
        Some(PowerModel::BritPlexiEL34)
    );
    let settings = Settings {
        gain: Gain::Plexi,
        drive: 0.8,
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
