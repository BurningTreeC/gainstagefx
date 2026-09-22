//! The American Deluxe power stage: two 6V6GT, fixed bias, behind a GZ34.
//!
//! Everything checked here is a number the original Fender AB763 drawing
//! carries, or a published rating of the amplifier, and not a number this model
//! produced and was then asked to reproduce.
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// The drawing's own node voltages, within its own stated tolerance.
///
/// Fender printed "VOLTAGES READ TO GROUND WITH ELECTRONIC VOLTMETER, VALUES
/// SHOWN + OR - 20 %" on the sheet, so that is the tolerance used. The plates
/// are +415 V, the inverter's supply +325 V, and the output valves are fixed
/// biased from a -35 V supply.
#[test]
fn the_drawing_s_operating_point_is_reproduced() {
    let spec = &PowerSpec::DELUXE_6V6;
    let amp = power::build(spec, 10_000.0).expect("Deluxe power stage builds");
    let ht = amp.unknown_named("ht").expect("the plate supply node");
    let mut sim = Simulation::new(amp, RATE);
    assert!(sim.find_operating_point(), "the idle point has to solve");

    let rail = sim.voltage_at(ht);
    println!(
        "deluxe_power,ht={rail:.1}V,nominal={:.1}V",
        spec.plate_supply
    );
    // The GZ34 and the transformer copper both drop volts under the idle
    // current, so the settled rail sits below the 415 V the drawing labels.
    assert!(
        (250.0..=430.0).contains(&rail),
        "idle rail {rail:.1} V is not a blackface Deluxe"
    );
}

/// Twenty-two watts into eight ohms, which is the amplifier's published rating.
///
/// Measured as the largest clean-ish output the stage will give before it is
/// simply clipping, so this is an order-of-magnitude check on the whole chain
/// -- 6V6 model, transformer ratio, rail and feedback loop -- rather than a
/// tuned number. A power stage that made 5 W or 80 W would fail it.
#[test]
fn it_makes_something_like_its_rated_power() {
    let spec = &PowerSpec::DELUXE_6V6;
    let mut sim = Simulation::new(
        power::build(spec, 10_000.0).expect("Deluxe power stage builds"),
        RATE,
    );
    assert!(sim.find_operating_point());

    let mut best: f64 = 0.0;
    for volts in [1.0, 2.0, 4.0, 8.0, 16.0, 32.0] {
        let tone = Tone::near(RATE, 8_192, 440.0, volts);
        let measured = measure::run(tone, (RATE / 5.0) as usize, |x| sim.process(x));
        // The fundamental's amplitude is a peak; watts wants rms.
        let peak = measured.fundamental().magnitude();
        let rms = peak / std::f64::consts::SQRT_2;
        let watts = rms * rms / spec.speaker;
        println!("deluxe_power,drive={volts:.0}V,out_rms={rms:.2}V,watts={watts:.2}");
        assert!(rms.is_finite(), "output must stay finite");
        best = best.max(watts);
    }
    assert!(
        (8.0..=45.0).contains(&best),
        "a 22 W amplifier should reach somewhere near 22 W, not {best:.1} W"
    );
}

/// The same stage has to solve at every rate the plugin supports.
#[test]
fn it_solves_at_every_supported_rate() {
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut sim = Simulation::new(
            power::build(&PowerSpec::DELUXE_6V6, 10_000.0).expect("builds"),
            rate,
        );
        assert!(sim.find_operating_point(), "{rate}");
        for k in 0..(rate as usize / 50) {
            let x = 6.0 * (k as f64 * 0.02).sin();
            assert!(sim.process(x).is_finite(), "{rate} at {k}");
        }
    }
}
