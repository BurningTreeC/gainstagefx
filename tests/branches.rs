//! The parts that impose a voltage rather than a conductance.
//!
//! Each carries its own current as an unknown, which puts a zero on the
//! diagonal of the row it adds -- so the factorisation has to pivot, and a
//! solver that assumed a symmetric positive definite matrix would fall over
//! here rather than quietly give a wrong answer.

use gainstagefx::dsp::device::OpAmp;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::Netlist;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::dsp::{ac, netlist::Circuit};

const RATE: f64 = 96_000.0;

/// An inverting amplifier's gain is two resistors and nothing else, provided
/// the op-amp is doing its job. This is the textbook case, and it is exact.
#[test]
fn feedback_sets_the_gain_and_nothing_else_does() {
    // The inverting input is a virtual ground, so what the input resistor
    // actually works against is itself plus whatever is driving it. The gain
    // is Rf / (Rin + Rsource), and leaving the source out of that reads as a
    // one per cent error in the op-amp.
    const SOURCE: f64 = 100.0;
    for (rin, rf) in [
        (10_000.0, 10_000.0),
        (10_000.0, 100_000.0),
        (100_000.0, 10_000.0),
    ] {
        let expected = -rf / (rin + SOURCE);
        let mut net = Netlist::new("inverting");
        net.input("in", SOURCE)
            .resistor("in", "minus", rin)
            .resistor("minus", "out", rf)
            .opamp("out", "gnd", "minus", OpAmp::STUDIO)
            // The output needs a load or it is a node with one connection.
            .resistor("out", "gnd", 10_000_000.0);
        let circuit = net.build("out").expect("builds");

        let mut sim = Simulation::new(circuit, RATE);
        let volts = 0.01;
        let mut settled = 0.0;
        for _ in 0..2_000 {
            settled = sim.process(volts);
        }
        let gain = settled / volts;
        assert!(
            (gain - expected).abs() < 0.001 * expected.abs().max(1.0),
            "{rin} into {rf} gave {gain:.5}, wanted {expected:.5}"
        );
    }
}

/// And it stops at the rail rather than sailing past it, which is the whole
/// reason the thing has two states.
#[test]
fn the_output_stops_at_the_rail() {
    let mut net = Netlist::new("inverting");
    net.input("in", 100.0)
        .resistor("in", "minus", 1_000.0)
        .resistor("minus", "out", 100_000.0)
        .opamp("out", "gnd", "minus", OpAmp::NINE_VOLT)
        .resistor("out", "gnd", 10_000_000.0);
    let circuit = net.build("out").expect("builds");
    let mut sim = Simulation::new(circuit, RATE);

    // A hundred times gain on a volt is a hundred volts, and there is three
    // and a half on offer.
    let mut settled = 0.0;
    for _ in 0..2_000 {
        settled = sim.process(1.0);
    }
    assert!(
        (settled.abs() - OpAmp::NINE_VOLT).abs() < 0.05,
        "the output went to {settled:.3} V against a {} V rail",
        OpAmp::NINE_VOLT
    );
}

/// A transformer changes volts one way and amps the other, and both fall out
/// of the same single unknown.
#[test]
fn a_transformer_steps_voltage_by_its_ratio() {
    for ratio in [0.25, 1.0, 4.0] {
        let mut net = Netlist::new("iron");
        net.input("in", 1.0)
            .resistor("in", "p", 0.001)
            .transformer("p", "gnd", "s", "gnd", ratio)
            .resistor("s", "gnd", 10_000.0);
        let circuit = net.build("s").expect("builds");
        // Primary is `ratio` times the secondary, so the secondary is the
        // input divided by it.
        let gain = ac::solve(&circuit, &[], 1_000.0).magnitude();
        let expected = 1.0 / ratio;
        assert!(
            (gain - expected).abs() < 0.01,
            "a {ratio}:1 transformer gave {gain:.4}, wanted {expected:.4}"
        );
    }
}

/// The load a transformer presents is the load on its secondary multiplied by
/// the square of the ratio. That is the thing a step-up is bought for, and it
/// is a consequence of the single stamp rather than something written in.
#[test]
fn a_transformer_reflects_its_load_by_the_square_of_the_ratio() {
    let seen_by_source = |ratio: f64| {
        let mut net = Netlist::new("iron");
        // A stiff-ish source, so the divider against the reflected load shows.
        net.input("in", 10_000.0)
            .transformer("in", "gnd", "s", "gnd", ratio)
            .resistor("s", "gnd", 10_000.0);
        let circuit = net.build("in").expect("builds");
        // What survives at the primary tells us what it is loaded by.
        ac::solve(&circuit, &[], 1_000.0).magnitude()
    };
    // 1:1 reflects 10k against the 10k source: half survives.
    assert!((seen_by_source(1.0) - 0.5).abs() < 0.01);
    // 2:1 reflects four times that, so more survives.
    assert!(seen_by_source(2.0) > 0.75, "{}", seen_by_source(2.0));
    // 1:2 reflects a quarter, so less does.
    assert!(seen_by_source(0.5) < 0.25, "{}", seen_by_source(0.5));
}

/// And the two solvers still agree once there are branch rows in the matrix.
#[test]
fn the_solvers_agree_with_a_transformer_in_the_way() {
    let mut net = Netlist::new("iron and a filter");
    net.input("in", 600.0)
        .resistor("in", "p", 50.0)
        .transformer("p", "gnd", "s", "gnd", 0.5)
        .resistor("s", "out", 30.0)
        .capacitor("out", "gnd", 22e-9)
        .resistor("out", "gnd", 10_000.0);
    let circuit: Circuit = net.build("out").expect("builds");

    for hz in [100.0, 1_000.0, 4_000.0] {
        let tone = Tone::near(RATE, 16_384, hz, 0.1);
        let mut sim = Simulation::new(circuit.clone(), RATE);
        let m = measure::run(tone, (RATE * 0.2) as usize, |x| sim.process(x));
        let direct = ac::solve(&circuit, &[], tone.hz()).db();
        assert!(
            (m.gain_db() - direct).abs() < 0.1,
            "at {:.0} Hz: stepped {:.3} dB, direct {direct:.3} dB",
            tone.hz(),
            m.gain_db()
        );
    }
}
