//! The parts that bend, and the stage built from one.
//!
//! Checked against what a data sheet says and against Kirchhoff, not against
//! my own output. An operating point that merely *converged* is not an
//! operating point: a wrong stamp has a fixed point too, and it looks
//! perfectly settled from the inside.

use gainstagefx::circuits::valve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{DiodeSpec, Netlist, TriodeSpec};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn node(circuit: &gainstagefx::dsp::netlist::Circuit, name: &str) -> usize {
    circuit.names.iter().position(|n| n == name).expect("named node")
}

/// A 12AX7 with a hundred kilohm load on a three hundred volt rail sits with
/// its plate around two hundred volts, drawing about a milliamp, and gives a
/// stage gain in the high fifties. None of that is dialled in -- it falls out
/// of Koren's equations and the resistors around them.
#[test]
fn the_valve_stage_sits_where_the_data_sheet_says() {
    let circuit = valve::build(&valve::CLASSIC, 10_000.0, 1_000_000.0).expect("builds");
    let mut sim = Simulation::new(circuit.clone(), RATE);
    sim.set_control(valve::BYPASS, 1.0);
    assert!(sim.find_operating_point(), "the operating point did not settle");

    let plate = sim.voltage_at(node(&circuit, "plate"));
    let cathode = sim.voltage_at(node(&circuit, "cathode"));
    assert!(
        (150.0..250.0).contains(&plate),
        "the plate is at {plate:.1} V"
    );
    assert!(
        (1.0..2.0).contains(&cathode),
        "the cathode is at {cathode:.2} V"
    );
}

/// The check that a merely-converged answer cannot pass.
///
/// Every milliamp the plate draws has to come back out of the cathode, so the
/// cathode voltage is not free: it is the plate current times the cathode
/// resistance. A stamp with a sign or a term wrong still settles, and still
/// reports a plausible looking plate voltage -- it just quietly loses current
/// somewhere, and this is where that shows.
#[test]
fn the_operating_point_obeys_kirchhoff() {
    let values = valve::CLASSIC;
    let circuit = valve::build(&values, 10_000.0, 1_000_000.0).expect("builds");
    let mut sim = Simulation::new(circuit.clone(), RATE);
    sim.set_control(valve::BYPASS, 1.0);
    sim.find_operating_point();

    let plate = sim.voltage_at(node(&circuit, "plate"));
    let cathode = sim.voltage_at(node(&circuit, "cathode"));
    let current = (values.supply - plate) / values.plate_load;
    // At rest the bypass leg carries nothing -- its capacitor is an open
    // circuit -- so the cathode resistor is the whole of what the current has
    // to flow through.
    let implied = current * values.cathode;
    assert!(
        (cathode - implied).abs() < 0.02,
        "the cathode is at {cathode:.3} V but {:.3} mA through {:.0} ohm implies \
         {implied:.3} V -- current is going missing",
        current * 1e3,
        values.cathode
    );
}

/// A single ended triode is asymmetric, so it makes second harmonic before it
/// makes third, and both grow with how hard it is driven. That ordering is the
/// difference between a valve and a diode.
#[test]
fn the_valve_is_second_harmonic_first() {
    let circuit = valve::build(&valve::CLASSIC, 10_000.0, 1_000_000.0).expect("builds");
    let run = |volts: f64| {
        let mut sim = Simulation::new(circuit.clone(), RATE);
        sim.set_control(valve::BYPASS, 1.0);
        sim.find_operating_point();
        let tone = Tone::near(RATE, 16_384, 220.0, volts);
        measure::run(tone, (RATE * 0.3) as usize, |x| sim.process(x))
    };

    let quiet = run(0.05);
    let loud = run(1.0);

    assert!(
        quiet.harmonic_percent(2) > quiet.harmonic_percent(3) * 5.0,
        "second {:.3} %, third {:.3} % -- a single ended stage should be second first",
        quiet.harmonic_percent(2),
        quiet.harmonic_percent(3)
    );
    assert!(
        loud.thd_percent() > quiet.thd_percent() * 4.0,
        "driving it harder barely changed it: {:.2} % against {:.2} %",
        loud.thd_percent(),
        quiet.thd_percent()
    );
    // And it amplifies: a 12AX7 stage of this shape is in the high fifties.
    let gain = 10f64.powf(quiet.gain_db() / 20.0);
    assert!((45.0..70.0).contains(&gain), "the stage gives {gain:.1} times");
}

/// Backing the bypass off puts the cathode resistor back in the signal path,
/// which is local feedback: less gain and a stage that stays clean longer.
/// This is what makes a preamplifier able to be subtle.
#[test]
fn the_bypass_control_trades_gain_for_headroom() {
    let circuit = valve::build(&valve::CLASSIC, 10_000.0, 1_000_000.0).expect("builds");
    let run = |bypass: f64| {
        let mut sim = Simulation::new(circuit.clone(), RATE);
        sim.set_control(valve::BYPASS, bypass);
        sim.find_operating_point();
        let tone = Tone::near(RATE, 16_384, 220.0, 0.5);
        measure::run(tone, (RATE * 0.3) as usize, |x| sim.process(x))
    };
    let open = run(1.0);
    let shut = run(0.0);
    // Five decibels and change, and it is bounded by the circuit rather than
    // by the control: the most degeneration on offer is the whole cathode
    // resistor, and 1.5k against a valve of this transconductance is what it
    // is. A wider range would mean a different cathode resistor, not a
    // different pot.
    assert!(
        open.gain_db() > shut.gain_db() + 5.0,
        "the bypass barely moved the gain: {:.1} dB against {:.1} dB",
        open.gain_db(),
        shut.gain_db()
    );
    assert!(
        shut.thd_percent() < open.thd_percent(),
        "degeneration should clean it up: {:.2} % against {:.2} %",
        shut.thd_percent(),
        open.thd_percent()
    );
}

/// The three diodes differ by their forward voltage, and that is the whole of
/// why anyone swaps them. Germanium conducts at about a third of silicon's,
/// an LED at roughly three times it.
#[test]
fn the_diodes_clip_at_their_own_thresholds() {
    let clip = |spec: DiodeSpec, volts: f64| {
        let mut net = Netlist::new("clipper");
        net.input("in", 1_000.0)
            .resistor("in", "out", 10_000.0)
            .diode("out", "gnd", spec)
            .diode("gnd", "out", spec)
            .resistor("out", "gnd", 1_000_000.0);
        let circuit = net.build("out").expect("builds");
        let mut sim = Simulation::new(circuit, RATE);
        let tone = Tone::near(RATE, 8_192, 220.0, volts);
        measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x))
    };

    // Driven the same, each holds the output at its own ceiling.
    let si = clip(DiodeSpec::SILICON, 3.0).fundamental().magnitude();
    let ge = clip(DiodeSpec::GERMANIUM, 3.0).fundamental().magnitude();
    let led = clip(DiodeSpec::LED, 3.0).fundamental().magnitude();
    assert!(ge < si, "germanium should hold lower: {ge:.3} V against {si:.3} V");
    assert!(led > si, "an LED should hold higher: {led:.3} V against {si:.3} V");

    // A symmetric pair cannot make even harmonics. Not approximately: the
    // waveform has half wave symmetry, so the second harmonic is absent.
    let hard = clip(DiodeSpec::SILICON, 3.0);
    assert!(
        hard.harmonic_percent(2) < 0.5,
        "a symmetric clipper made {:.2} % second harmonic",
        hard.harmonic_percent(2)
    );
    assert!(
        hard.harmonic_percent(3) > 5.0,
        "it should be making third: {:.2} %",
        hard.harmonic_percent(3)
    );
}

/// A lower amplification factor is a lower gain, and it is the commonest valve
/// swap there is.
#[test]
fn a_lower_mu_triode_gives_less_gain() {
    let gain_of = |spec: TriodeSpec| {
        let mut values = valve::CLASSIC;
        values.triode = spec;
        let circuit = valve::build(&values, 10_000.0, 1_000_000.0).expect("builds");
        let mut sim = Simulation::new(circuit, RATE);
        sim.set_control(valve::BYPASS, 1.0);
        sim.find_operating_point();
        let tone = Tone::near(RATE, 16_384, 220.0, 0.05);
        measure::run(tone, (RATE * 0.3) as usize, |x| sim.process(x)).gain_db()
    };
    let high = gain_of(TriodeSpec::ECC83);
    let low = gain_of(TriodeSpec::ECC82);
    assert!(
        high > low + 6.0,
        "the two bottles gave {high:.1} dB and {low:.1} dB"
    );
}
