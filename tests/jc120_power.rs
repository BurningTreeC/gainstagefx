//! The Roland JC-120's power amplifier.
//!
//! Every figure checked here is printed on the MAIN AMP BOARD sheet of the
//! Sep. 2000 service notes, or falls out of two values on it. None of it was
//! read out of this model and then asserted back at it.
//!
//! This circuit went in wrong twice before it went in right, both times because
//! a junction was judged by eye, so the checks are deliberately the ones that
//! would have caught it: the sheet prints an input level and an output level at
//! that input, and an amplifier wired any other way does not hit both.
use gainstagefx::circuits::jc120_power::{self, LOWER_DRIVE, OUTPUT, RAIL_VOLTS, UPPER_DRIVE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
/// The main amplifier board's own stages drive this, through an emitter
/// follower, so the source is low.
const SOURCE: f64 = 1_000.0;
/// One 8 ohm speaker a side.
const LOAD: f64 = 8.0;

fn level(s: &mut Simulation, volts: f64) -> (f64, f64) {
    let tone = Tone::near(RATE, 16_384, 1_000.0, volts);
    let r = measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x));
    (r.gain_db(), r.thd_percent())
}

fn amp() -> Simulation {
    Simulation::new(jc120_power::build(SOURCE, LOAD).expect("builds"), RATE)
}

/// The closed-loop gain, which is R88 over R93 and nothing else.
///
/// `1 + 68 k / 3.3 k` is 21.6, or 26.7 dB. The sheet's two test points --
/// `+3 dBm, 3.10 Vpp` at the input and `+29 dBm, 61.8 Vpp` at the output --
/// say 26 dB. Those are independent of each other: one is a pair of resistors
/// read off the drawing, the other is a measurement somebody took on the
/// bench, and a model that is wired right lands between them.
#[test]
fn the_closed_loop_gain_is_the_sheet_s_twenty_six_decibels() {
    let mut s = amp();
    let (db, _) = level(&mut s, 0.5);
    assert!(
        (db - 26.0).abs() < 1.5,
        "the amplifier gives {db:.2} dB; the feedback divider says 26.7 and the \
         sheet's test points say 26.0"
    );
}

/// And at the sheet's own input level, the sheet's own output level.
///
/// This is the check the two wrong versions would have failed. 3.10 Vpp in is
/// 1.55 V peak; the sheet says that comes out as 61.8 Vpp, which is 21.85 V rms
/// into 8 ohms -- 59.7 W, and Roland's published 60 W. It also has to still be
/// *clean* there, because 61.8 Vpp is the rated output and not the clipping
/// point.
#[test]
fn the_sheet_s_input_level_gives_the_sheet_s_output_level() {
    let mut s = amp();
    let drive = 1.55; // 3.10 Vpp
    let (db, thd) = level(&mut s, drive);
    let vpp = 2.0 * drive * 10f64.powf(db / 20.0);
    assert!(
        (vpp - 61.8).abs() < 4.0,
        "3.10 Vpp in gives {vpp:.1} Vpp out; the sheet prints 61.8"
    );
    assert!(
        thd < 0.5,
        "and it has to be clean there -- 61.8 Vpp is the rated output, not the \
         clipping point -- but the distortion is {thd:.2} %"
    );
    let watts = (vpp / (2.0 * std::f64::consts::SQRT_2)).powi(2) / LOAD;
    assert!(
        (watts - 60.0).abs() < 8.0,
        "which is {watts:.1} W into 8 ohms against the published 60"
    );
}

/// It clips above that, and not far above.
///
/// The rails are the one DERIVED number on this board -- no secondary voltage
/// is printed -- so this is what holds them honest. If +-40 V were wrong the
/// rated output would not sit just below the clipping point, which is where a
/// manufacturer puts it.
#[test]
fn it_clips_just_above_the_rated_output() {
    let mut s = amp();
    let (_, clean) = level(&mut s, 1.55);
    let mut hard = amp();
    let (_, clipped) = level(&mut hard, 3.0);
    assert!(clean < 0.5, "rated output is clean: {clean:.2} %");
    assert!(
        clipped > 8.0,
        "twice the rated drive has to be visibly clipped against +-{RAIL_VOLTS} V \
         rails, but the distortion is only {clipped:.2} %"
    );
}

/// Distortion *falls* as the level rises, which is what a feedback amplifier
/// with a class-AB output stage does and what a mis-wired one does not.
///
/// The crossover region is a fixed number of millivolts wide, so it is a larger
/// fraction of a small signal than of a large one. An output stage that was not
/// biased -- or one whose spreader had been wired as something else, which is
/// exactly the mistake that was made here twice -- gets this backwards.
#[test]
fn the_distortion_falls_as_the_level_rises() {
    let mut small = amp();
    let (_, quiet) = level(&mut small, 0.05);
    let mut large = amp();
    let (_, loud) = level(&mut large, 1.0);
    assert!(
        quiet > loud,
        "small signal {quiet:.2} % against large signal {loud:.2} %: a class-AB \
         stage distorts its quietest signals most"
    );
}

/// The direct-current solve, against what the topology requires.
#[test]
fn the_operating_point_is_what_the_topology_requires() {
    let circuit = jc120_power::build(SOURCE, LOAD).expect("builds");
    let mut s = Simulation::new(circuit.clone(), RATE);
    assert!(s.find_operating_point(), "the DC solve converges");
    let at = |name: &str| {
        let n = circuit
            .unknown_named(name)
            .unwrap_or_else(|| panic!("{name} is a node"));
        s.voltage_at(n)
    };

    // A global loop that is closed at direct current puts the output at the
    // inverting input's voltage, which is ground through R93.
    let out = at(OUTPUT);
    assert!(
        out.abs() < 1.0,
        "the output idles at {out:.3} V; a loop closed at DC holds it at zero"
    );

    // The bias spreader has to hold the two driver bases apart by enough to
    // turn on four base-emitter junctions' worth of output stage, and not so
    // much that it runs away.
    //
    // This range used to be 0.8..3.0, and it was far too loose to mean
    // anything: at 1.48 V the whole PNP half of the output stage was reverse
    // biased and the test passed. Two driver junctions and two output
    // junctions is about 2.4 V, so that is what is asked for.
    let spread = at(UPPER_DRIVE) - at(LOWER_DRIVE);
    assert!(
        (2.2..3.0).contains(&spread),
        "the spreader holds the driver bases {spread:.2} V apart, which is not \
         four base-emitter junctions' worth"
    );

    // And the bootstrap sits up near the supply, holding the collector load
    // above the output rather than tied to the rail.
    let boot = at("boot");
    assert!(
        boot > 0.5 * RAIL_VOLTS,
        "the bootstrapped node is at {boot:.1} V, which is not above the output"
    );
}

/// **Both halves of the output stage conduct at idle**, which is what makes it
/// a complementary stage rather than two single-ended ones taking turns.
///
/// This is the check the spread-voltage assertion above could not make, and
/// the defect it would have caught is the reason it exists. R79 -- 3.9 k, in
/// parallel with R76 -- was missing from the netlist, so the spreader held
/// 1.48 V instead of 2.4 V; the NPN half carried the entire 9.6 mA idle and
/// the PNP half sat at **-0.34 V of base-emitter bias, reverse biased and
/// fully off**. The amplifier was single-ended with a dead zone across every
/// zero crossing: crossover distortion on an amplifier whose whole reputation
/// is being clean, and an ill-conditioned solve every time the signal passed
/// through zero. A recorded DI take put 150,000 fallbacks a minute through it.
///
/// See `docs/models/jazz_120.md`.
#[test]
fn both_halves_of_the_output_stage_are_turned_on() {
    let circuit = jc120_power::build(SOURCE, LOAD).expect("builds");
    let mut s = Simulation::new(circuit.clone(), RATE);
    assert!(s.find_operating_point(), "the DC solve converges");
    let at = |name: &str| {
        let n = circuit
            .unknown_named(name)
            .unwrap_or_else(|| panic!("{name} is a node"));
        s.voltage_at(n)
    };

    // The NPN output device wants its base above its emitter, the PNP wants
    // its emitter above its base, and both want about a silicon junction.
    let npn = at("out_n_b") - at("e_n");
    let pnp = at("e_p") - at("out_p_b");
    for (half, volts) in [("NPN", npn), ("PNP", pnp)] {
        assert!(
            volts > 0.4,
            "the {half} half of the output stage idles at {volts:.3} V of \
             base-emitter bias, which is not turned on"
        );
    }

    // And neither is turned on so hard that the stage is running class A into
    // its own heatsink: a volt would be amps through a 0.33 ohm emitter
    // resistor.
    for (half, volts) in [("NPN", npn), ("PNP", pnp)] {
        assert!(
            volts < 0.75,
            "the {half} half idles at {volts:.3} V, which is not a bias, it is \
             a short"
        );
    }
}

/// Solid-state, so nothing in it depends on the sample rate.
#[test]
fn it_is_the_same_at_every_rate() {
    let mut first = None;
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut s = Simulation::new(jc120_power::build(SOURCE, LOAD).expect("builds"), rate);
        let tone = Tone::near(rate, 16_384, 1_000.0, 0.5);
        let db = measure::run(tone, (rate / 5.0) as usize, |x| s.process(x)).gain_db();
        match first {
            None => first = Some(db),
            Some(f) => assert!(
                (db - f).abs() < 0.3,
                "{rate} Hz gives {db:.2} dB against {f:.2} dB at 44.1 kHz"
            ),
        }
    }
}
