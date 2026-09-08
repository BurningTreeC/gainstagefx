//! How much of each circuit is linear, which decides whether a partitioned
//! solve is worth building.
//!
//! `CLAUDE.md` §13 and §20.2 prescribe the shape: derive the linear network
//! once, isolate the nonlinear devices, solve locally. In matrix terms that is
//! a Schur complement. Order the unknowns so the nodes no nonlinear device
//! touches come first and the rest come last:
//!
//!     M = [ A  B ]
//!         [ C  D ]
//!
//! `A`, `B` and `C` are the linear network. For a fixed sample rate and fixed
//! control positions they do not change from sample to sample, let alone from
//! Newton pass to Newton pass -- only the device conductances stamped into `D`
//! do. So `A` is factorised once and `C A⁻¹ B` precomputed, and each pass
//! costs a factorisation of the q by q Schur complement rather than of the
//! whole n by n matrix.
//!
//! A factorisation costs the cube of its size, so the win is `(n/q)³` on the
//! part that dominates -- but only if q is genuinely smaller than n. This
//! reports both, per voice, so that is a measurement rather than a hope.
//!
//! `cargo run --release --example partition`

use gainstagefx::circuits::power;
use gainstagefx::dsp::netlist::{Circuit, Part};
use gainstagefx::voice::{self, Amplifier, Diode, Gain};

/// The nodes a part imposes a nonlinear relationship between.
fn nonlinear_nodes(part: &Part) -> Vec<usize> {
    match part {
        Part::Diode { a, k, .. } => vec![*a, *k],
        Part::Triode { p, g, k, .. } => vec![*p, *g, *k],
        Part::Pentode { p, g, k, s, .. } => vec![*p, *g, *k, *s],
        Part::Jfet { d, g, s, .. } => vec![*d, *g, *s],
        Part::Bipolar { c, b, e, .. } => vec![*c, *b, *e],
        Part::Core { a, b, .. } => vec![*a, *b],
        Part::OpAmp { .. } => Vec::new(),
        _ => Vec::new(),
    }
}

fn main() {
    println!("Unknowns a partitioned solve would still have to factorise.\n");
    println!(
        "  {:<12}{:>7}{:>12}{:>9}{:>10}{:>12}",
        "voice", "n", "nonlinear", "q", "q/n", "(n/q)^3"
    );

    let mut rows: Vec<(String, Circuit)> = Vec::new();
    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Neve,
        Gain::Screamer,
        Gain::Muff,
        Gain::Crunch,
        Gain::Distortion,
    ] {
        if let Ok(c) = voice::build_voice(gain, Diode::Silicon, Amplifier::Jfet) {
            rows.push((gain.name().to_string(), c));
        }
    }
    // The power stages are separate simulations and, at two Newton passes for
    // roughly half the cost of an amplifier voice, the more expensive half.
    for (name, spec) in [
        ("5150 power", &power::PowerSpec::EVH5150),
        ("IIC+ power", &power::PowerSpec::MARKIIC),
    ] {
        if let Ok(c) = power::build(spec, 1.0) {
            rows.push((name.to_string(), c));
        }
    }

    for (name, circuit) in &rows {
        let n = circuit.unknowns();

        let mut touched = vec![false; n];
        let mut devices = 0usize;
        for part in &circuit.parts {
            let nodes = nonlinear_nodes(part);
            if nodes.is_empty() {
                continue;
            }
            devices += 1;
            for node in nodes {
                // Ground is not an unknown; it is the reference.
                if node > 0 && node <= n {
                    touched[node - 1] = true;
                }
            }
        }
        let q = touched.iter().filter(|t| **t).count();
        let win = if q > 0 {
            (n as f64 / q as f64).powi(3)
        } else {
            f64::INFINITY
        };
        println!(
            "  {name:<12}{n:>7}{devices:>12}{q:>9}{:>9.0}%{win:>11.1}x",
            100.0 * q as f64 / n as f64
        );
    }

    println!("\n`q` counts the unknowns at least one nonlinear device touches, so");
    println!("it is the size of the block that has to be refactorised each Newton");
    println!("pass.\n");
    println!("`(n/q)^3` is a ceiling and an optimistic one, for two reasons worth");
    println!("stating rather than discovering later. It compares a dense n by n");
    println!("factorisation with a dense q by q one, but the factorisation here is");
    println!("already bounded by each row's nonzeros -- so the numerator is smaller");
    println!("than n^3 to begin with. And the Schur complement D - C A-inverse B is");
    println!("generally dense even when D is not, so the denominator really is q^3");
    println!("with no sparsity left to exploit. Against that, the triangular solves");
    println!("through A survive, and the right hand side's linear part is constant");
    println!("across a sample's Newton passes, so it is solved once and not per");
    println!("pass. What the change is actually worth has to be measured by");
    println!("building it; this only says where it is worth looking.");
}
