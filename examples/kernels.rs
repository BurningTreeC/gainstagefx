//! Compiles the reduced solves the catalogue actually runs.
//!
//! ```text
//! cargo run --release --example kernels [-- --seconds 8] [-- --check]
//! ```
//!
//! Plays every preset over the fixture take with the partition census on,
//! then writes `src/dsp/partition/kernels.rs`: one straight-line elimination
//! per (pattern, pivot plan) that carries real traffic. See
//! `gainstagefx::dsp::partition::Kernel` for why and what a kernel must be.
//!
//! The table is a performance cache, not a behaviour: a kernel is only used
//! where its pattern covers everything the partition can hold and its plan is
//! the plan the partition learned, and then it is the masked replay to the
//! bit. A stale table costs speed, never correctness, so regenerate it when a
//! circuit changes and the kernels stop matching. `--check` reports what the
//! census would choose without writing anything.
use gainstagefx::dsp::partition::kernel_census;
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::Chain;
use std::collections::BTreeMap;
use std::fmt::Write as _;

const RATE: f64 = 48_000.0;
const TAKE: &str = "tests/fixtures/01-260912_1044.wav";
const OUTPUT: &str = "src/dsp/partition/kernels.rs";

/// A plan is compiled when it carries at least this share of its pattern's
/// replayed solves...
const MIN_SHARE: f64 = 0.01;
/// ...and at least this many of them in the census run.
const MIN_SOLVES: u64 = 20_000;
/// At most this many plans for one pattern. `KERNEL_CANDIDATES` in
/// `partition.rs` must not be smaller.
const MAX_PLANS: usize = 8;

fn read_take() -> Vec<f64> {
    let bytes = std::fs::read(TAKE).unwrap_or_else(|e| panic!("{TAKE}: {e}"));
    assert!(&bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE");
    let mut at = 12;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        match &bytes[at..at + 4] {
            b"fmt " => assert_eq!(
                (
                    u16::from_le_bytes([body[2], body[3]]),
                    u16::from_le_bytes([body[14], body[15]])
                ),
                (1, 24),
                "the take is mono PCM24"
            ),
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    data.expect("a data chunk")
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| ((b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64)
        .map(|v| v / 8_388_608.0)
        .collect()
}

/// The masked replay's elimination for one pattern and plan, unrolled.
/// Returns the function body and its multiply-subtract count.
///
/// Walks `solve_masked_planned` symbolically: the same swaps, the same rows in
/// the same order, the same columns ascending, with each row's pattern growing
/// by the pivot row's as the masked path's masks grow by their fill.
fn emit_kernel(name: &str, plan: &[u8], pattern: &[u64]) -> (String, usize) {
    let n = plan.len();
    let mut mask = pattern.to_vec();
    let mut out = String::new();
    let mut ops = 0;
    let at = |row: usize, column: usize| row * n + column;
    writeln!(
        out,
        "fn {name}(m: &mut [f64], r: &mut [f64], sound: &mut bool) -> Result<(), Bail> {{"
    )
    .unwrap();
    writeln!(
        out,
        "    let Ok(m) = <&mut [f64; {}]>::try_from(m) else {{ return Err(Bail::Tail(0)); }};",
        n * n
    )
    .unwrap();
    writeln!(out, "    let Some(Ok(r)) = r.get_mut(..{n}).map(<&mut [f64; {n}]>::try_from) else {{ return Err(Bail::Tail(0)); }};").unwrap();
    for column in 0..n {
        let pivot = plan[column] as usize;
        if pivot != column {
            let both = mask[column] | mask[pivot];
            for j in (column..n).filter(|&j| both >> j & 1 == 1) {
                writeln!(out, "    m.swap({}, {});", at(column, j), at(pivot, j)).unwrap();
            }
            writeln!(out, "    r.swap({column}, {pivot});").unwrap();
            mask.swap(column, pivot);
        }
        let upper: Vec<usize> = (column + 1..n)
            .filter(|&j| mask[column] >> j & 1 == 1)
            .collect();
        let upper_bits = upper.iter().fold(0u64, |bits, &j| bits | 1 << j);
        let rows: Vec<usize> = (column + 1..n)
            .filter(|&row| mask[row] >> column & 1 == 1)
            .collect();
        writeln!(out, "    let d = m[{}];", at(column, column)).unwrap();
        writeln!(
            out,
            "    if d.abs() < 1e-30 || !d.is_finite() {{ return Err(Bail::Tail({column})); }}"
        )
        .unwrap();
        writeln!(out, "    let inv = 1.0 / d;").unwrap();
        writeln!(out, "    m[{}] = inv;", at(column, column)).unwrap();
        if rows.is_empty() {
            continue;
        }
        writeln!(out, "    let ceiling = d.abs() * 16.0;").unwrap();
        writeln!(out, "    let pr = r[{column}];").unwrap();
        for row in rows {
            writeln!(out, "    let e = m[{}];", at(row, column)).unwrap();
            writeln!(out, "    if e.abs() > ceiling {{ *sound = false; }}").unwrap();
            writeln!(out, "    let f = e * inv;").unwrap();
            for &j in &upper {
                writeln!(out, "    m[{}] -= f * m[{}];", at(row, j), at(column, j)).unwrap();
                ops += 1;
            }
            writeln!(out, "    r[{row}] -= f * pr;").unwrap();
            ops += 1;
            mask[row] |= upper_bits;
        }
    }
    for row in (0..n).rev() {
        let columns: Vec<usize> = (row + 1..n).filter(|&j| mask[row] >> j & 1 == 1).collect();
        if columns.is_empty() {
            writeln!(out, "    let v = r[{row}];").unwrap();
        } else {
            writeln!(out, "    let mut v = r[{row}];").unwrap();
        }
        for j in columns {
            writeln!(out, "    v -= m[{}] * r[{j}];", at(row, j)).unwrap();
            ops += 1;
        }
        writeln!(out, "    let d = m[{}];", at(row, row)).unwrap();
        writeln!(
            out,
            "    if !d.is_finite() || d == 0.0 {{ return Err(Bail::Failed); }}"
        )
        .unwrap();
        writeln!(out, "    let x = v * d;").unwrap();
        writeln!(out, "    r[{row}] = x;").unwrap();
        writeln!(out, "    if !x.is_finite() {{ return Err(Bail::Failed); }}").unwrap();
    }
    writeln!(out, "    Ok(())\n}}").unwrap();
    (out, ops)
}

fn main() {
    let mut seconds = 8.0;
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => {
                seconds = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--seconds takes seconds")
            }
            "--check" => check = true,
            other => panic!("unknown argument {other}"),
        }
    }

    kernel_census::enable();
    let take = read_take();
    let length = ((seconds * RATE) as usize).min(take.len());
    for preset in PRESETS {
        let scale = 10f64.powf(preset.input_trim as f64 / 20.0);
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        chain.settle();
        chain.find_operating_point();
        for &x in &take[..length] {
            chain.process_stereo(x * scale);
        }
        eprint!(".");
    }
    eprintln!();

    // Group by pattern; the pattern's length is the boundary size.
    let mut by_pattern: BTreeMap<Vec<u64>, Vec<(Vec<u8>, u64)>> = BTreeMap::new();
    for (pattern, plan, count) in kernel_census::take() {
        by_pattern.entry(pattern).or_default().push((plan, count));
    }
    let mut chosen = Vec::new();
    let (mut covered, mut total) = (0u64, 0u64);
    for (pattern, mut plans) in by_pattern {
        let solves: u64 = plans.iter().map(|&(_, count)| count).sum();
        total += solves;
        plans.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (plan, count) in plans.into_iter().take(MAX_PLANS) {
            if count < MIN_SOLVES || (count as f64) < MIN_SHARE * solves as f64 {
                break;
            }
            covered += count;
            chosen.push((pattern.clone(), plan, count));
        }
    }
    // Most-used first, so a partition's candidates are its busiest plans.
    chosen.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });

    let mut functions = String::new();
    let mut table = String::new();
    let mut all_ops = 0;
    for (index, (pattern, plan, count)) in chosen.iter().enumerate() {
        let name = format!("kernel_{index}");
        let (code, ops) = emit_kernel(&name, plan, pattern);
        all_ops += ops;
        println!(
            "{name}: {} unknowns, {ops} multiply-subtracts, {count} solves",
            plan.len()
        );
        functions.push_str(&code);
        functions.push('\n');
        writeln!(
            table,
            "    Kernel {{ plan: &{plan:?}, pattern: &{pattern:?}, run: {name} }},"
        )
        .unwrap();
    }
    println!(
        "{} kernels, {all_ops} multiply-subtracts, covering {:.1} % of {total} replayed solves",
        chosen.len(),
        100.0 * covered as f64 / total.max(1) as f64
    );
    if check {
        return;
    }
    let file = format!(
        "//! Generated by `cargo run --release --example kernels`. Do not edit.\n\
         //!\n\
         //! Each kernel is `solve_masked_planned` unrolled for one pattern and one\n\
         //! pivot plan; see `Kernel` in `partition.rs`.\n\
         #![allow(clippy::all)]\n\
         use super::{{Bail, Kernel}};\n\n\
         {functions}\
         pub(super) static KERNELS: &[Kernel] = &[\n{table}];\n"
    );
    std::fs::write(OUTPUT, file).expect("write the kernel table");
    println!("wrote {OUTPUT}");
}
