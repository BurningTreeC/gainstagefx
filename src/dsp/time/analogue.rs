//! Would starting a hard power-stage solve from an earlier, similar moment help?
//!
//! `cargo test --release --lib analogue -- --ignored --nocapture`
//! (rig as `power_hard`: `GSFX_PRESET`, `GSFX_CIRCUIT`, `GSFX_POWER`,
//! `GSFX_DRIVE`, `GSFX_MASTER`, `GSFX_GAIN_DB`; always 1x.)
//!
//! A guitar note is nearly periodic, so the solve that is hard now -- a
//! clipping edge, where extrapolating the last two samples lands on the wrong
//! side of a kink -- was usually solved once already, a period ago. The
//! *method of analogues*: find the past moment whose recent input looked most
//! like now, and start Newton from the answer the solver reached then.
//!
//! An upper bound first. Run once as the plugin runs and note which solves
//! were hard (10 passes or more). Run again from the same state, and on exactly
//! those samples start from the best analogue in the last 50 ms, matched on the
//! last `WINDOW` samples of the chain's input. Compare their passes. A starting
//! point only: the solve converges to the same test on the same equations, and
//! the two runs' outputs are compared to say so.

use crate::params::{Circuit, Oversampling, PowerAmp};
use crate::presets::{Preset, PRESETS};
use crate::voice::Chain;
use nice_plug::prelude::Enum;

use super::super::device::Linearisation;

const RATE: f64 = 48_000.0;
const WINDOW: usize = 24;
const HISTORY: usize = 2_400;

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

fn rig() -> Preset {
    let name = var("GSFX_PRESET").unwrap_or_else(|| "American 800RB Clank".into());
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
    rig.oversampling = Oversampling::Off;
    rig
}

struct Run {
    passes: Vec<u64>,
    output: Vec<f64>,
}

/// One pass over the take; on the samples `warm` marks, start the power stage
/// from the best analogue.
fn run(rig: &Preset, input: &[f64], warm: Option<&[bool]>) -> Run {
    let mut chain = Chain::new(RATE);
    chain.apply(&rig.settings());
    chain.settle();
    chain.find_operating_point();
    let sim = chain.test_active_power_mut().expect("a power stage");
    let (n, devices) = (sim.unknowns(), sim.device_count());
    let mut volts = vec![0.0; input.len() * n];
    let mut lin = vec![Linearisation::default(); input.len() * devices];
    let mut passes = Vec::with_capacity(input.len());
    let mut output = Vec::with_capacity(input.len());
    let mut p0 = sim.newton_passes();
    for i in 0..input.len() {
        if warm.is_some_and(|w| w[i]) && i > HISTORY {
            // The past moment whose last WINDOW inputs, this one included,
            // look most like now's.
            let mut best = (f64::INFINITY, 0usize);
            for m in (i - HISTORY)..(i - WINDOW) {
                let mut err = 0.0;
                for k in 0..WINDOW {
                    let d = input[i - k] - input[m - k];
                    err += d * d;
                    if err >= best.0 {
                        break;
                    }
                }
                if err < best.0 {
                    best = (err, m);
                }
            }
            let m = best.1;
            let sim = chain.test_active_power_mut().unwrap();
            sim.set_start_point(
                &volts[m * n..(m + 1) * n],
                &lin[m * devices..(m + 1) * devices],
            );
        }
        output.push(chain.process_stereo(input[i]).0);
        let sim = chain.test_active_power_mut().unwrap();
        passes.push(sim.newton_passes() - p0);
        p0 = sim.newton_passes();
        volts[i * n..(i + 1) * n].copy_from_slice(sim.voltages());
        sim.linearisations_into(&mut lin[i * devices..(i + 1) * devices]);
    }
    Run { passes, output }
}

#[test]
#[ignore = "diagnostic: an upper bound on what the method of analogues saves"]
fn analogue() {
    let rig = rig();
    let gain_db: f64 = var("GSFX_GAIN_DB").map_or(6.0, |g| g.parse().expect("GSFX_GAIN_DB"));
    let scale = 10f64.powf((rig.input_trim as f64 + gain_db) / 20.0);
    let input: Vec<f64> = take()[..8 * RATE as usize]
        .iter()
        .map(|x| x * scale)
        .collect();
    let plain = run(&rig, &input, None);
    let hard: Vec<bool> = plain.passes.iter().map(|&p| p >= 10).collect();
    let warm = run(&rig, &input, Some(&hard));
    let (mut before, mut after, mut count) = (0u64, 0u64, 0u64);
    let mut buckets = [0u64; 4];
    for ((&was, &now), _) in plain
        .passes
        .iter()
        .zip(&warm.passes)
        .zip(&hard)
        .filter(|(_, &h)| h)
    {
        count += 1;
        before += was;
        after += now;
        let ratio = now as f64 / was as f64;
        buckets[if ratio <= 0.34 {
            0
        } else if ratio <= 0.67 {
            1
        } else if ratio <= 1.0 {
            2
        } else {
            3
        }] += 1;
    }
    let total_plain: u64 = plain.passes.iter().sum();
    let total_warm: u64 = warm.passes.iter().sum();
    let (mut err, mut energy) = (0.0, 0.0);
    for (a, b) in plain.output.iter().zip(&warm.output) {
        err += (a - b) * (a - b);
        energy += a * a;
    }
    println!(
        "{} with {} at {:+} dB: {count} hard solves of {}",
        rig.circuit.name(),
        rig.power_amp.name(),
        gain_db,
        input.len()
    );
    println!(
        "  their passes: {before} as the plugin runs, {after} from the analogue ({:.1} -> {:.1} a solve)",
        before as f64 / count.max(1) as f64,
        after as f64 / count.max(1) as f64
    );
    println!(
        "  analogue / plain on each: <=1/3 {}, <=2/3 {}, <=1 {}, worse {}",
        buckets[0], buckets[1], buckets[2], buckets[3]
    );
    println!(
        "  whole take: {total_plain} -> {total_warm} passes; outputs differ by {:.1} dB rms",
        10.0 * (err / energy.max(1e-30)).log10()
    );
}
