//! What the builder refuses to build.
//!
//! Each of these is a mistake made for real in the first version, where it
//! produced no error at all -- just a circuit that returned `NaN`, or indexed
//! past its own matrix, or quietly did nothing. The point of naming nodes is
//! that the builder can catch them and say which node it means.

use gainstagefx::dsp::netlist::{Fault, Netlist, Taper};

#[test]
fn a_name_always_means_the_same_node() {
    let mut net = Netlist::new("same name");
    let first = net.node("plate");
    let second = net.node("plate");
    assert_eq!(first, second);
    assert_ne!(first, net.node("cathode"));
}

/// The one that cost the most: a node the stack owned but the bare circuit
/// still declared. Its row of the matrix was empty, the factorisation divided
/// by zero, and every sample came out `NaN` with nothing to say why.
#[test]
fn a_node_with_nothing_on_it_is_refused() {
    let mut net = Netlist::new("dangling");
    net.input("in", 1_000.0)
        .resistor("in", "out", 1_000.0)
        .resistor("out", "gnd", 1_000.0)
        // Touched once and never again.
        .capacitor("out", "orphan", 1e-9);
    match net.build("out") {
        Err(Fault::Dangling { node, connections }) => {
            assert_eq!(node, "orphan");
            assert_eq!(connections, 1);
        }
        other => panic!("expected a dangling node, got {other:?}", other = other.map(|_| "a circuit")),
    }
}

/// A stage whose output cannot be reached from its input does nothing, and
/// says nothing about it.
#[test]
fn an_unreachable_output_is_refused() {
    let mut net = Netlist::new("island");
    net.input("in", 1_000.0)
        .resistor("in", "mid", 1_000.0)
        .resistor("mid", "gnd", 1_000.0)
        // A little island, connected to ground and to nothing else.
        .resistor("out", "gnd", 1_000.0)
        .capacitor("out", "gnd", 1e-9);
    match net.build("out") {
        Err(Fault::Unreachable { from, to }) => {
            assert_eq!(from, "in");
            assert_eq!(to, "out");
        }
        other => panic!("expected an unreachable output, got {other:?}", other = other.map(|_| "a circuit")),
    }
}

#[test]
fn a_circuit_needs_an_input_and_a_real_output() {
    let mut net = Netlist::new("no input");
    net.resistor("a", "b", 1_000.0).resistor("b", "gnd", 1_000.0);
    assert!(matches!(net.build("b"), Err(Fault::Malformed(_))));

    let mut net = Netlist::new("no such output");
    net.input("in", 1_000.0).resistor("in", "gnd", 1_000.0);
    assert!(matches!(net.build("nowhere"), Err(Fault::Malformed(_))));
}

#[test]
fn a_part_worth_nothing_is_refused() {
    let mut net = Netlist::new("zero");
    net.input("in", 1_000.0)
        .resistor("in", "out", 0.0)
        .resistor("out", "gnd", 1_000.0);
    assert!(matches!(net.build("out"), Err(Fault::Malformed(_))));
}

/// A rheostat leaves `1 - fraction` of its track in circuit, so a plain linear
/// track runs backwards in that position. Having both directions named is what
/// stops that being rediscovered each time.
#[test]
fn the_reverse_tapers_are_the_mirror_of_the_others() {
    for p in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert!((Taper::Linear.fraction(p) - (1.0 - Taper::ReverseLinear.fraction(p))).abs() < 1e-12);
        assert!((Taper::Audio.fraction(p) - (1.0 - Taper::ReverseAudio.fraction(p))).abs() < 1e-12);
    }
    assert!(Taper::Linear.fraction(0.0) < Taper::Linear.fraction(1.0));
    assert!(Taper::ReverseLinear.fraction(0.0) > Taper::ReverseLinear.fraction(1.0));
}

#[test]
fn a_good_circuit_builds_and_knows_its_names() {
    let mut net = Netlist::new("fine");
    net.input("in", 1_000.0)
        .resistor("in", "out", 1_000.0)
        .capacitor("out", "gnd", 1e-9);
    let circuit = net.build("out").expect("builds");
    assert_eq!(circuit.nodes, 2);
    assert_eq!(circuit.node_name(circuit.output), "out");
}
