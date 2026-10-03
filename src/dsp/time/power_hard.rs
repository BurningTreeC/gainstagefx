//! What any rig's power stage is doing in its hard solves.
//!
//! `cargo test --release --lib power_hard -- --ignored --nocapture`
//!
//! The rig starts from `GSFX_PRESET` (default *American SVT Grind*) and can be
//! changed by name: `GSFX_CIRCUIT`, `GSFX_POWER` (as the panel names them),
//! `GSFX_DRIVE` and `GSFX_MASTER` (0..1), `GSFX_OS` (the requested oversampling, 1/2/4/8) and
//! `GSFX_GAIN_DB` (the take's gain). `GSFX_TRACE=<host sample>` prints that
//! sample's first power solve pass by pass (`configure_full_power_trace`) and
//! stops.
//!
//! Prints how many solves are hard (>= 10 passes) and how many fall back, and
//! for every unknown of the power stage its range in hard solves against the
//! rest, sorted by how far the two differ -- which says where in the circuit
//! the hard solves happen.

use super::Simulation;
use crate::params::{Circuit, Oversampling, PowerAmp};
use crate::presets::{Preset, PRESETS};
use crate::voice::Chain;
use nice_plug::prelude::Enum;

const RATE: f64 = 48_000.0;

fn take() -> Vec<f64> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/01-260912_1044.wav"
    ))
    .expect("the take");
    let mut at = 12;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if &bytes[at..at + 4] == b"data" {
            data = Some(&bytes[at + 8..(at + 8 + len).min(bytes.len())]);
        }
        at += 8 + len + (len & 1);
    }
    data.unwrap()
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| ((b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64)
        .map(|v| v / 8_388_608.0)
        .collect()
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn by_name<T: Enum + Copy>(all: &[T], name: &str) -> T {
    *all.iter()
        .find(|x| T::variants()[x.to_index()] == name)
        .unwrap_or_else(|| panic!("no option named {name:?}"))
}

#[test]
#[ignore = "diagnostic: where a power stage's hard solves happen"]
fn power_hard() {
    let name = var("GSFX_PRESET").unwrap_or_else(|| "American SVT Grind".into());
    let base = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let mut rig = Preset { ..*base };
    if let Some(c) = var("GSFX_CIRCUIT") {
        rig.circuit = by_name(&Circuit::ALL, &c);
    }
    if let Some(p) = var("GSFX_POWER") {
        rig.power_amp = by_name(&PowerAmp::ALL, &p);
    }
    if let Some(d) = var("GSFX_DRIVE") {
        rig.drive = d.parse().expect("GSFX_DRIVE");
    }
    if let Some(m) = var("GSFX_MASTER") {
        rig.master = m.parse().expect("GSFX_MASTER");
    }
    if let Some(o) = var("GSFX_OS") {
        rig.oversampling = match o.as_str() {
            "1" => Oversampling::Off,
            "2" => Oversampling::Two,
            "4" => Oversampling::Four,
            _ => Oversampling::Eight,
        };
    }
    let gain_db: f64 = var("GSFX_GAIN_DB").map_or(0.0, |g| g.parse().expect("GSFX_GAIN_DB"));
    let scale = 10f64.powf((rig.input_trim as f64 + gain_db) / 20.0);
    let mut chain = Chain::new(RATE);
    chain.apply(&rig.settings());
    chain.settle();
    chain.find_operating_point();
    let trace: Option<usize> = var("GSFX_TRACE").map(|t| t.parse().expect("GSFX_TRACE"));
    let take = take();
    let n = take.len().min(8 * RATE as usize);
    let unknowns = chain
        .test_active_power_mut()
        .expect("a power stage")
        .unknowns();
    let mut passes = Vec::new();
    let mut fallbacks = Vec::new();
    let mut volts: Vec<Vec<f64>> = Vec::new();
    let (mut p0, mut f0) = {
        let s = chain.test_active_power_mut().unwrap();
        (s.newton_passes, s.fallbacks)
    };
    for (i, &x) in take[..n].iter().enumerate() {
        if trace == Some(i) {
            let s = chain.test_active_power_mut().unwrap();
            s.set_late_continuation(true);
            s.configure_full_power_trace(0, 1);
        }
        chain.process_stereo(x * scale);
        let s: &mut Simulation = chain.test_active_power_mut().unwrap();
        if trace == Some(i) {
            s.print_full_power_trace();
            return;
        }
        passes.push(s.newton_passes - p0);
        fallbacks.push(s.fallbacks - f0);
        (p0, f0) = (s.newton_passes, s.fallbacks);
        volts.push((0..unknowns).map(|i| s.voltage_at(i)).collect());
    }
    let factor = chain.effective_oversampling() as u64;
    let hard: Vec<bool> = passes.iter().map(|&p| p >= 10 * factor).collect();
    println!(
        "{name} as set: circuit {}, power {}, drive {}, {}x; {} host samples, {} hard, {} fallbacks, {:.3} passes a solve",
        rig.circuit.name(),
        rig.power_amp.name(),
        rig.drive,
        factor,
        n,
        hard.iter().filter(|&&h| h).count(),
        fallbacks.iter().sum::<u64>(),
        passes.iter().sum::<u64>() as f64 / (n as f64 * factor as f64),
    );
    let mut histogram = [0u64; 6];
    for &p in &passes {
        let per = p as f64 / factor as f64;
        histogram[if per < 1.5 {
            0
        } else if per < 2.5 {
            1
        } else if per < 3.5 {
            2
        } else if per < 4.5 {
            3
        } else if per < 9.5 {
            4
        } else {
            5
        }] += 1;
    }
    println!(
        "passes a solve, host samples: 1 {}  2 {}  3 {}  4 {}  5-9 {}  10+ {}",
        histogram[0], histogram[1], histogram[2], histogram[3], histogram[4], histogram[5]
    );
    let first: Vec<usize> = (0..n).filter(|&i| hard[i]).take(8).collect();
    println!("first hard host samples: {first:?}");
    let falling: Vec<usize> = (0..n).filter(|&i| fallbacks[i] > 0).take(8).collect();
    println!("first host samples with a fallback: {falling:?}");
    let s = chain.test_active_power_mut().unwrap();
    let mut rows: Vec<(f64, String)> = (0..unknowns)
        .map(|k| {
            let stats = |in_hard: bool| {
                let vs: Vec<f64> = (0..n).filter(|&i| hard[i] == in_hard).map(|i| volts[i][k]).collect();
                if vs.is_empty() {
                    return (0.0, 0.0, 0.0);
                }
                let lo = vs.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                (lo, hi, vs.iter().sum::<f64>() / vs.len() as f64)
            };
            let (rest, hardv) = (stats(false), stats(true));
            let spread = (hardv.1 - hardv.0) / (rest.1 - rest.0).abs().max(1e-9);
            (
                spread,
                format!(
                    "  {:>14}  rest {:>10.3} .. {:>10.3}   hard {:>10.3} .. {:>10.3}  mean {:>9.3} / {:>9.3}",
                    s.solver_unknown_name(k),
                    rest.0,
                    rest.1,
                    hardv.0,
                    hardv.1,
                    rest.2,
                    hardv.2
                ),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("unknowns, widest hard range relative to the rest first:");
    for (_, line) in rows.iter().take(30) {
        println!("{line}");
    }
}
