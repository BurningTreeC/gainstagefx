//! The American 6550 (Ampeg SVT, D 591720 H) against its own drawing.
//!
//! The drawing gives the no-signal DC voltages, the calibration procedure's
//! idle (0.072 V across each bank's 1 ohm: 24 mA a valve), and boxed AC test
//! voltages at full output: 0.257 V at V1's grid, 5.14 V and 4.52 V at the
//! inverter's plate and cathode, 37.5 V at a follower's grid, 33.2 V at the
//! output grids, 372 V on each half primary, 34.6 V across the 4 ohm load.
//!
//! This prints the model's idle beside the drawing's, fits the four constants
//! that put it there -- `FollowerFront::bias_rest` and the open-circuit
//! voltages of A, E and D (`power::SVT_A_OPEN`, `SVT_E_OPEN`, `SVT_D_OPEN`) --
//! and then drives the stage with the drawing's 0.257 V and prints every boxed
//! node. Paste the fitted constants into `circuits/power.rs` when they move.
//!
//! `cargo run --release --example svt_op`

use gainstagefx::circuits::power::{self, FollowerFront, PowerSpec};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 1_000.0;

struct Idle {
    a: f64,
    e: f64,
    h: f64,
    d: f64,
    c: f64,
    bank: [f64; 2],
    nodes: Vec<(&'static str, f64)>,
}

const NODES: [&str; 14] = [
    "f_p1", "f_k1", "f_p2", "f_k2", "f_kb", "f_dp1", "f_dk1", "f_dp2", "f_dk2", "f_fg1", "f_fk1",
    "f_fg2", "f_fk2", "os1",
];

fn spec_with(a: f64, e: f64, d: f64, rest: f64) -> PowerSpec {
    let front: &'static FollowerFront = Box::leak(Box::new(FollowerFront {
        bias_rest: rest,
        bias_rail: d,
        ..FollowerFront::SVT
    }));
    PowerSpec {
        plate_supply: a,
        screen_supply: e,
        follower_front: Some(front),
        ..PowerSpec::SVT_6550
    }
}

fn idle(spec: &PowerSpec) -> Idle {
    let c = power::build(spec, SOURCE).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap_or_else(|| panic!("{n}"));
    let (ht, scr, h, d, cc) = (at("ht"), at("scr"), at("f_h"), at("f_neg"), at("f_c"));
    let (pa, pb, va, vb) = (at("pl_a"), at("pl_b"), at("ov1"), at("ov2"));
    let idx: Vec<usize> = NODES.iter().map(|n| at(n)).collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let r = spec.plate_resistor;
    Idle {
        a: s.voltage_at(ht),
        e: s.voltage_at(scr),
        h: s.voltage_at(h),
        d: s.voltage_at(d),
        c: s.voltage_at(cc),
        // Each bank's plate current through its plate resistors, a valve.
        bank: [
            (s.voltage_at(pa) - s.voltage_at(va)) / r / 3.0,
            (s.voltage_at(pb) - s.voltage_at(vb)) / r / 3.0,
        ],
        nodes: NODES
            .iter()
            .copied()
            .zip(idx.iter().map(|&i| s.voltage_at(i)))
            .collect(),
    }
}

fn main() {
    // --- the fit ----------------------------------------------------------
    let (mut a, mut e, mut d, mut rest) = (
        power::SVT_A_OPEN,
        power::SVT_E_OPEN,
        power::SVT_D_OPEN,
        FollowerFront::SVT.bias_rest,
    );
    for _ in 0..6 {
        // The bias rest for 24 mA a valve, by bisection: more rest puts more
        // of the track below the wiper, a more negative grid, less current.
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            let i = idle(&spec_with(a, e, d, mid)).bank[0];
            if i < 24e-3 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        rest = 0.5 * (lo + hi);
        let m = idle(&spec_with(a, e, d, rest));
        a += 660.0 - m.a;
        e += 350.0 - m.e;
        d += -150.0 - m.d;
    }
    let m = idle(&spec_with(a, e, d, rest));
    println!(
        "fitted: SVT_A_OPEN {a:.1}  SVT_E_OPEN {e:.1}  SVT_D_OPEN {d:.1}  bias_rest {rest:.4}"
    );
    println!(
        "idle:  A {:.1} (660)  E {:.1} (350)  H {:.1} (342)  D {:.1} (-150)  C {:.1} (362)",
        m.a, m.e, m.h, m.d, m.c
    );
    println!(
        "       per valve {:.1} / {:.1} mA (24, the procedure's 0.072 V across 1 ohm a bank of three)",
        m.bank[0] * 1e3,
        m.bank[1] * 1e3
    );
    let drawing = [
        ("f_p1", "V1a plate 207"),
        ("f_k1", "V1a cathode 1.7"),
        ("f_p2", "V1b plate 280"),
        ("f_k2", "V1b cathode 95"),
        ("f_kb", "R8/R9 junction"),
        ("f_dp1", "V2a plate 159"),
        ("f_dk1", "V2a cathode 7"),
        ("f_dp2", "V3a plate 159"),
        ("f_dk2", "V3a cathode 7"),
        ("f_fg1", "V2b grid -77"),
        ("f_fk1", "V2b cathode -47"),
        ("f_fg2", "V3b grid -77"),
        ("f_fk2", "V3b cathode -47"),
        ("os1", "screens (E less 22/3 ohm)"),
    ];
    for ((n, v), (_, label)) in m.nodes.iter().zip(drawing) {
        println!("       {n:6} {v:8.2}   {label}");
    }

    // --- the boxed AC voltages ---------------------------------------------
    let spec = spec_with(a, e, d, rest);
    let c = power::tap(&spec, SOURCE, "spk").unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let probes = [
        ("f_g1", 0.257, "V1 grid"),
        ("f_p2", 5.14, "inverter plate"),
        ("f_k2", 4.52, "inverter cathode"),
        ("f_fg1", 37.5, "follower grid"),
        ("f_fk1", 33.2, "output grids"),
        ("pl_a", 372.0, "half primary (plate to A)"),
        ("spk", 34.6, "4 ohm load"),
    ];
    let idx: Vec<usize> = probes.iter().map(|p| at(p.0)).collect();
    let ht = at("ht");
    for (label, volts) in [("drawing's 0.257 V", 0.257), ("half that", 0.1285)] {
        let mut s = Simulation::new(power::tap(&spec, SOURCE, "spk").unwrap(), RATE);
        s.find_operating_point();
        let n = (RATE * 0.6) as usize;
        let from = (RATE * 0.3) as usize;
        let mut sum = vec![0.0; probes.len()];
        let mut mean = vec![0.0; probes.len()];
        let mut sum_a = 0.0;
        for k in 0..n {
            let x = volts * 2f64.sqrt() * (std::f64::consts::TAU * 1_000.0 * k as f64 / RATE).sin();
            s.process(x);
            if k >= from {
                for (j, &i) in idx.iter().enumerate() {
                    let v = s.voltage_at(i);
                    let v = if probes[j].0 == "pl_a" {
                        v - s.voltage_at(ht)
                    } else {
                        v
                    };
                    sum[j] += v * v;
                    mean[j] += v;
                }
                sum_a += s.voltage_at(ht);
            }
        }
        let count = (n - from) as f64;
        println!("\n1 kHz, {label} rms at the input:");
        for (j, (node, want, what)) in probes.iter().enumerate() {
            let m = mean[j] / count;
            let rms = (sum[j] / count - m * m).max(0.0).sqrt();
            println!("  {node:6} {rms:8.3} V rms  ({want} on the drawing: {what})");
        }
        let out = {
            let m = mean[6] / count;
            (sum[6] / count - m * m).sqrt()
        };
        println!(
            "  power {:.0} W into 4 ohm; A averages {:.1} V",
            out * out / 4.0,
            sum_a / count
        );
    }
}
