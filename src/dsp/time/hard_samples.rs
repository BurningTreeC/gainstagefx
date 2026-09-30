//! What the power stage's hard samples are made of.
//!
//! `cargo test --release --lib hard_samples -- --ignored --nocapture`
//! (`GSFX_PRESET="<name>"` probes another preset; Jazz Chorus by default.)
//!
//! Plays a preset over the fixture take and, for every power-stage solve,
//! records its Newton passes, how many of its stamps had a junction limiter
//! holding a device back, its line-search trials and fallbacks, and the power
//! stage's own input. Then compares the hard solves with the rest.

use super::Simulation;
use crate::presets::PRESETS;
use crate::voice::Chain;

const RATE: f64 = 48_000.0;
const HARD: u64 = 10;

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

#[derive(Clone, Copy, Default)]
struct Solve {
    input: f64,
    passes: u64,
    stamps: u64,
    held: u64,
    trials: u64,
    held_trials: u64,
    backtracks: u64,
    fallbacks: u64,
    replays: u64,
    kernel_replays: u64,
    learns: u64,
    invalidations: u64,
}

fn snapshot(sim: &Simulation) -> Solve {
    let pivots = sim
        .nonlinear_partition
        .as_ref()
        .map(|p| p.test_pivot_profile())
        .unwrap_or_default();
    Solve {
        replays: pivots.replays,
        kernel_replays: pivots.kernel_replays,
        learns: pivots.learns,
        invalidations: pivots.invalidations,
        input: sim.previous_input,
        passes: sim.newton_passes,
        stamps: sim.test_stamps[0],
        held: sim.test_stamps[1],
        trials: sim.test_stamps[2],
        held_trials: sim.test_stamps[3],
        backtracks: sim.backtrack_count,
        fallbacks: sim.fallbacks,
    }
}

fn delta(now: Solve, was: Solve) -> Solve {
    Solve {
        input: now.input,
        passes: now.passes - was.passes,
        stamps: now.stamps - was.stamps,
        held: now.held - was.held,
        trials: now.trials - was.trials,
        held_trials: now.held_trials - was.held_trials,
        backtracks: now.backtracks - was.backtracks,
        fallbacks: now.fallbacks - was.fallbacks,
        replays: now.replays - was.replays,
        kernel_replays: now.kernel_replays - was.kernel_replays,
        learns: now.learns - was.learns,
        invalidations: now.invalidations - was.invalidations,
    }
}

fn percentile(values: &mut [f64], p: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * p) as usize]
}

#[test]
#[ignore = "diagnostic: prints the anatomy of the power stage's hard samples"]
fn hard_samples() {
    let name = std::env::var("GSFX_PRESET").unwrap_or_else(|_| "Jazz Chorus".into());
    let preset = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let scale = 10f64.powf(preset.input_trim as f64 / 20.0);
    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();
    let take = take();

    use crate::dsp::device::junction_probe::TALLY;
    TALLY.with(|t| t.borrow_mut().enabled = true);
    let mut tallies = Vec::with_capacity(take.len());
    let mut solves = Vec::with_capacity(take.len());
    if let Some(partition) = chain
        .test_active_power_mut()
        .expect("a power stage")
        .nonlinear_partition
        .as_mut()
    {
        partition.set_test_pivot_profile(true);
    }
    let mut was = snapshot(chain.test_active_power_mut().expect("a power stage"));
    for &x in &take {
        chain.process_stereo(x * scale);
        let now = snapshot(chain.test_active_power_mut().unwrap());
        solves.push(delta(now, was));
        tallies.push(TALLY.with(|t| std::mem::take(&mut *t.borrow_mut())));
        TALLY.with(|t| t.borrow_mut().enabled = true);
        was = now;
    }

    let total: u64 = solves.iter().map(|s| s.passes).sum();
    println!(
        "{name}: {} power solves, {total} passes ({:.3} a solve)",
        solves.len(),
        total as f64 / solves.len() as f64
    );
    let mut histogram = [0u64; 8];
    for s in &solves {
        let bucket = match s.passes {
            0..=2 => 0,
            3 => 1,
            4 => 2,
            5..=6 => 3,
            7..=9 => 4,
            10..=19 => 5,
            20..=39 => 6,
            _ => 7,
        };
        histogram[bucket] += 1;
    }
    println!(
        "  passes a solve: <=2 {}  3 {}  4 {}  5-6 {}  7-9 {}  10-19 {}  20-39 {}  40+ {}",
        histogram[0],
        histogram[1],
        histogram[2],
        histogram[3],
        histogram[4],
        histogram[5],
        histogram[6],
        histogram[7]
    );

    for (label, hard_only) in [("normal (<10)", false), ("hard (>=10)", true)] {
        let g: Vec<&Solve> = solves
            .iter()
            .filter(|s| (s.passes >= HARD) == hard_only)
            .collect();
        let n = g.len().max(1) as f64;
        let sum = |f: &dyn Fn(&Solve) -> u64| g.iter().map(|s| f(s)).sum::<u64>() as f64;
        println!(
            "  {label:13}  reduced solves per solve: kernel replays {:.2}, masked replays {:.2}, plan learns {:.2}, invalidations {:.2}",
            sum(&|s| s.kernel_replays) / n,
            sum(&|s| s.replays - s.kernel_replays) / n,
            sum(&|s| s.learns) / n,
            sum(&|s| s.invalidations) / n,
        );
        println!(
            "  {label:13}: {:7} solves, {:5.1} % of all passes; per solve: passes {:.2}, stamps {:.2} ({:.0} % limiter-held), trials {:.2} ({:.0} % held), backtracks {:.2}, fallbacks {:.3}",
            g.len(),
            100.0 * sum(&|s| s.passes) / total as f64,
            sum(&|s| s.passes) / n,
            sum(&|s| s.stamps) / n,
            100.0 * sum(&|s| s.held) / sum(&|s| s.stamps).max(1.0),
            sum(&|s| s.trials) / n,
            100.0 * sum(&|s| s.held_trials) / sum(&|s| s.trials).max(1.0),
            sum(&|s| s.backtracks) / n,
            sum(&|s| s.fallbacks) / n,
        );
    }

    // Do hard solves come in runs, and what is the input doing there?
    let hard: Vec<usize> = (0..solves.len())
        .filter(|&i| solves[i].passes >= HARD)
        .collect();
    let after_hard = hard
        .iter()
        .filter(|&&i| i > 0 && solves[i - 1].passes >= HARD)
        .count();
    println!(
        "  a hard solve follows another hard solve {:.0} % of the time (base rate {:.2} %)",
        100.0 * after_hard as f64 / hard.len().max(1) as f64,
        100.0 * hard.len() as f64 / solves.len() as f64
    );
    let feature = |i: usize| -> (f64, f64, f64) {
        let x = solves[i].input;
        let x1 = if i > 0 { solves[i - 1].input } else { x };
        let x2 = if i > 1 { solves[i - 2].input } else { x1 };
        (x.abs(), (x - x1).abs(), ((x - x1) - (x1 - x2)).abs())
    };
    for (label, set) in [
        ("all", (2..solves.len()).collect::<Vec<_>>()),
        ("hard", hard.iter().copied().filter(|&i| i > 1).collect()),
    ] {
        let mut level: Vec<f64> = set.iter().map(|&i| feature(i).0).collect();
        let mut slope: Vec<f64> = set.iter().map(|&i| feature(i).1).collect();
        let mut bend: Vec<f64> = set.iter().map(|&i| feature(i).2).collect();
        println!(
            "  input at {label:4} solves: |x| p50 {:.3} p90 {:.3}   |dx| p50 {:.4} p90 {:.4}   |d2x| p50 {:.5} p90 {:.5}",
            percentile(&mut level, 0.5),
            percentile(&mut level, 0.9),
            percentile(&mut slope, 0.5),
            percentile(&mut slope, 0.9),
            percentile(&mut bend, 0.5),
            percentile(&mut bend, 0.9),
        );
    }
    // Which limiter branch fires, in hard solves and in the rest. The tally
    // covers every transistor in the chain, preamp included.
    for (label, hard_only) in [("normal", false), ("hard", true)] {
        let mut sum = crate::dsp::device::junction_probe::Tally::default();
        for (s, t) in solves.iter().zip(&tallies) {
            if (s.passes >= HARD) != hard_only {
                continue;
            }
            sum.calls += t.calls;
            sum.free += t.free;
            sum.walk += t.walk;
            sum.walk_from_below_critical += t.walk_from_below_critical;
            sum.from_reverse += t.from_reverse;
            sum.to_critical += t.to_critical;
            for k in 0..5 {
                sum.walk_steps[k] += t.walk_steps[k];
            }
        }
        println!(
            "  junction limits in {label:6} solves: {} calls, free {:.1} %, log walk {:.2} % (from below critical {:.0} % of those; steps <0.1 {} <0.3 {} <1 {} <3 {} >=3 {}), from reverse {:.2} %, to critical {:.3} %",
            sum.calls,
            100.0 * sum.free as f64 / sum.calls.max(1) as f64,
            100.0 * sum.walk as f64 / sum.calls.max(1) as f64,
            100.0 * sum.walk_from_below_critical as f64 / sum.walk.max(1) as f64,
            sum.walk_steps[0], sum.walk_steps[1], sum.walk_steps[2], sum.walk_steps[3], sum.walk_steps[4],
            100.0 * sum.from_reverse as f64 / sum.calls.max(1) as f64,
            100.0 * sum.to_critical as f64 / sum.calls.max(1) as f64,
        );
    }
    // The input around the first few clusters of hard solves.
    let mut shown = 0;
    let mut last_shown = 0usize;
    for &i in &hard {
        if shown >= 3 || (last_shown > 0 && i < last_shown + 200) {
            continue;
        }
        shown += 1;
        last_shown = i;
        println!("  around solve {i} ({:.3} s):", i as f64 / RATE);
        for (k, s) in solves.iter().enumerate().skip(i.saturating_sub(8)).take(16) {
            println!(
                "    {k:8}  input {:+9.4}  passes {:3}  held {:3}  trials {:3}  fallbacks {}",
                s.input, s.passes, s.held, s.trials, s.fallbacks
            );
        }
    }
    // Where in the take.
    let mut seconds = std::collections::BTreeMap::<u64, u64>::new();
    for &i in &hard {
        *seconds.entry(i as u64 / 48_000).or_default() += 1;
    }
    println!("  hard solves per second of the take: {seconds:?}");
}

/// Which pivot plans the power stage replays that no compiled kernel covers,
/// and how concentrated they are.
///
/// `cargo test --release --lib uncovered_plans -- --ignored --nocapture`
#[test]
#[ignore = "diagnostic: prints the plans replayed without a kernel"]
fn uncovered_plans() {
    use crate::dsp::partition::kernel_census;
    let name = std::env::var("GSFX_PRESET").unwrap_or_else(|_| "Jazz Chorus".into());
    let preset = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let take = take();
    kernel_census::enable();
    for gain_db in [0.0, 6.0] {
        let scale = 10f64.powf((preset.input_trim as f64 + gain_db) / 20.0);
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        chain.settle();
        chain.find_operating_point();
        for &x in &take {
            chain.process_stereo(x * scale);
        }
    }
    let census = kernel_census::take();
    let covered =
        |pattern: &[u64], plan: &[u8]| crate::dsp::partition::test_kernel_covers(pattern, plan);
    // The largest pattern by traffic is the power stage's.
    let mut by_pattern = std::collections::HashMap::<Vec<u64>, Vec<(Vec<u8>, u64)>>::new();
    for (pattern, plan, count) in census {
        by_pattern.entry(pattern).or_default().push((plan, count));
    }
    let mut patterns: Vec<_> = by_pattern.into_iter().collect();
    patterns.sort_by_key(|(_, plans)| std::cmp::Reverse(plans.iter().map(|p| p.1).sum::<u64>()));
    for (pattern, mut plans) in patterns.into_iter().take(3) {
        let total: u64 = plans.iter().map(|p| p.1).sum();
        plans.retain(|(plan, _)| !covered(&pattern, plan));
        plans.sort_by_key(|p| std::cmp::Reverse(p.1));
        let uncovered: u64 = plans.iter().map(|p| p.1).sum();
        println!(
            "{} unknowns: {total} replays, {uncovered} ({:.2} %) without a kernel, in {} plans",
            pattern.len(),
            100.0 * uncovered as f64 / total as f64,
            plans.len()
        );
        let mut running = 0;
        for (k, (_, count)) in plans.iter().enumerate().take(40) {
            running += count;
            if k < 10 || k % 5 == 4 {
                println!(
                    "   top {:2}: {count:7} replays, cumulative {:.1} % of the uncovered",
                    k + 1,
                    100.0 * running as f64 / uncovered.max(1) as f64
                );
            }
        }
    }
}
