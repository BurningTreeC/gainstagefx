//! The Rodent, against its schematic and its op-amp's data sheet.
#[path = "support/micpre.rs"]
mod micpre;
use gainstagefx::circuits::rodent::{self, DISTORTION, FILTER};
use gainstagefx::dsp::netlist::Netlist;
use micpre::measure;

const RATE: f64 = 96_000.0;

/// The LM308 model on its own as a unity-gain follower: it slews at the data
/// sheet's 0.3 V/us, and in a gain of 101 its bandwidth is its 1.1 MHz divided by
/// that.
#[test]
fn the_op_amp_model_has_the_lm308s_speed() {
    let mut net = Netlist::new("follower");
    net.supply("vref", 1.0, 4.5)
        .input("src", 1.0)
        .capacitor("src", "in", 1e-3)
        .resistor("in", "vref", 1e6);
    // The input rides on the 4.5 V bias point, as it does in the pedal.
    rodent::lm308(&mut net, "u", "in", "out", "out");
    net.resistor("out", "vref", 10_000.0);
    let follower = net.build("out").unwrap();
    // A 3 V step: the output rises no faster than the slew rate. The rise takes
    // 10 us, so it is watched at 2 MHz to see it over several samples.
    let rate = 2_000_000.0;
    let mut sim = gainstagefx::dsp::time::Simulation::new(follower, rate);
    sim.find_operating_point();
    let start = sim.process(0.0);
    let mut fastest: f64 = 0.0;
    let mut previous = start;
    for _ in 0..20_000 {
        let y = sim.process(3.0);
        fastest = fastest.max((y - previous) * rate);
        previous = y;
    }
    assert!(
        (previous - start - 3.0).abs() < 0.05,
        "settles to the step: {}",
        previous - start
    );
    assert!(
        fastest < rodent::SLEW * 1.1 && fastest > rodent::SLEW * 0.8,
        "slewed at {fastest} V/s"
    );

    let mut net = Netlist::new("gain of 101");
    net.supply("vref", 1.0, 4.5)
        .input("src", 1.0)
        .capacitor("src", "in", 1e-3)
        .resistor("in", "vref", 1e6);
    rodent::lm308(&mut net, "u", "in", "m", "out");
    net.resistor("out", "m", 100_000.0)
        .resistor("m", "vref", 1_000.0)
        .resistor("out", "vref", 10_000.0);
    let amp = net.build("out").unwrap();
    let low = measure(&amp, RATE, &[], 100.0, 1e-4).gain_db();
    let corner = measure(&amp, RATE, &[], rodent::GBW_HZ / 101.0, 1e-4).gain_db();
    assert!((low - 40.1).abs() < 0.3, "{low}");
    assert!((corner - (low - 3.0)).abs() < 1.0, "{corner} vs {low}");
}

/// Gain 1 + Distortion / (47 || 560): 0 dB to 67 dB in the drawing, and at full
/// Distortion the LM308 cannot deliver that above a few hundred hertz.
#[test]
fn distortion_sweeps_the_gain_and_the_op_amp_runs_out_of_bandwidth() {
    let amp = rodent::tap(10_000.0, 470_000.0, "amp").unwrap();
    let at = |d: f64, hz: f64| measure(&amp, RATE, &[(DISTORTION, d)], hz, 1e-5).gain_db();
    assert!(at(0.0, 1_000.0).abs() < 1.0);
    let full = at(1.0, 1_000.0);
    assert!((58.0..67.5).contains(&full), "{full}");
    // ElectroSmash, from the data sheet: "At 10KHz ... no more than 30dB"; the
    // gain-bandwidth line puts it near 41 dB.
    let top = at(1.0, 10_000.0);
    assert!(top < 42.0 && top > 30.0, "{top}");
}

/// The filter sweeps a low-pass from 32 kHz down to 475 Hz; the panel's Tone knob
/// turns it the bright way.
#[test]
fn the_filter_darkens_as_it_turns_up() {
    let pedal = rodent::build(10_000.0, 470_000.0).unwrap();
    let tilt = |filter: f64| {
        measure(
            &pedal,
            RATE,
            &[(DISTORTION, 0.0), (FILTER, filter)],
            5_000.0,
            1e-3,
        )
        .gain_db()
            - measure(
                &pedal,
                RATE,
                &[(DISTORTION, 0.0), (FILTER, filter)],
                300.0,
                1e-3,
            )
            .gain_db()
    };
    assert!(
        tilt(1.0) < tilt(0.0) - 12.0,
        "{} vs {}",
        tilt(1.0),
        tilt(0.0)
    );
}

/// Distortion pulled from full to nothing while a note rings: the op-amp goes
/// from a gain of two thousand to a follower with a megahertz loop in one
/// block. Its transconductance used to be linearised on its tangent, which is
/// flat once saturated; every solve then failed, the reactances froze on the
/// last good answer, and the pedal stuck -- silent and crackling -- until the
/// knob came back above about 0.02.
#[test]
fn distortion_can_be_turned_down_while_playing() {
    let rate = 48_000.0;
    let pedal = rodent::build(10_000.0, 470_000.0).unwrap();
    let mut sim = gainstagefx::dsp::time::Simulation::new(pedal, rate);
    sim.set_control(DISTORTION, 1.0);
    sim.find_operating_point();
    let block = 128;
    let mut out = Vec::new();
    for b in 0..300 {
        // Down to almost nothing after a quarter of a second, then creeping up.
        let d = if b < 94 {
            1.0
        } else {
            0.0005 + 0.0002 * (b - 94) as f64
        };
        sim.set_control(DISTORTION, d);
        for i in 0..block {
            let t = (b * block + i) as f64 / rate;
            let x = 0.39 * (-(t % 0.5) * 6.0).exp() * (std::f64::consts::TAU * 82.41 * t).sin();
            out.push(sim.process(x));
        }
    }
    let (solves, passes, unsettled, _) = sim.statistics();
    assert!(
        unsettled <= 4,
        "{unsettled} of {solves} solves did not settle"
    );
    assert!(
        (passes as f64 / solves as f64) < 4.0,
        "{} passes a solve",
        passes as f64 / solves as f64
    );
    // And it is still playing: the last tenth of a second is not a frozen value.
    let tail = &out[out.len() - 4_800..];
    let spread =
        tail.iter().fold(f64::MIN, |m, v| m.max(*v)) - tail.iter().fold(f64::MAX, |m, v| m.min(*v));
    assert!(spread > 1e-3, "output frozen: spread {spread}");
}
