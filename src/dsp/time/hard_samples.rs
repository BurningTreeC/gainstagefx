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
use crate::dsp::device::Device;
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

/// A copy of `source`: the same circuit, controls and running state.
fn duplicate(source: &Simulation) -> Simulation {
    let mut sim = Simulation::new(source.circuit.clone(), source.rate);
    sim.controls.copy_from_slice(&source.controls);
    sim.values.copy_from_slice(&source.values);
    sim.supply_scale = source.supply_scale;
    sim.ceiling = source.ceiling;
    sim.backtracks = source.backtracks;
    sim.late_continuation = source.late_continuation;
    sim.dirty = true;
    sim.at_rest = false;
    sim.rebuild();
    sim.dirty = false;
    sim.copy_runtime_state_from(source);
    sim
}

/// What a speculative second half would buy on the power stage.
///
/// Per 64-sample block: the true solve runs the first half; a shadow copy,
/// started from the block's first state, solves the second half at the same
/// time; then the true solve runs the second half starting from the shadow's
/// answer where the shadow found a solve hard. Each second-half solve waits
/// for the shadow's answer to it, so the critical path, counted in Newton
/// passes against today's whole-block passes, is the first half and then the
/// redo in lockstep with the shadow: started at the block's start ("critical
/// path with speculation"), or once the first half has had a hard solve
/// ("shadow started at the first hard solve", which is what the chain does
/// outside `EAGER_WITHIN`). `GSFX_HARDFIRST` and `GSFX_HARD_AT` are
/// `SPECULATE_AFTER` and `SHADOW_HARD` in `voice.rs`; `GSFX_PREV` tries
/// deciding from the real stage's previous solve instead.
///
/// ```text
/// GSFX_PRESET="Brown '84" GSFX_HARDFIRST=10 GSFX_HARD_AT=4 RUST_MIN_STACK=268435456 \
///     cargo test --release --lib speculative_second_half -- --ignored --nocapture
/// ```
#[test]
#[ignore = "diagnostic: estimates a parallel-in-time power stage"]
fn speculative_second_half() {
    let name = std::env::var("GSFX_PRESET").unwrap_or_else(|_| "Jazz Chorus".into());
    let gain_db: f64 = std::env::var("GSFX_GAIN_DB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let block: usize = 64;
    let split: usize = std::env::var("GSFX_SPLIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(32);
    // Use the shadow's start only where the shadow itself needed this many
    // passes; elsewhere the predictor's start is the better one.
    let hard_at: u64 = std::env::var("GSFX_HARD_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    // Speculate only on blocks whose second-half input moves this much in one
    // sample somewhere; the input is known before the shadow would run.
    let gate: f64 = std::env::var("GSFX_GATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let preset = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let scale = 10f64.powf((preset.input_trim as f64 + gain_db) / 20.0);
    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();
    let take = take();

    // The power stage's inputs do not depend on it: record them, and its
    // starting state, from one ordinary run. The state after the first
    // sample, not before: the chain installs the preset's oversampling at its
    // first call, and a copy taken earlier solves at the old rate -- 192 kHz
    // for inputs recorded at 48 -- which is what this estimate first measured.
    chain.process_stereo(0.0);
    let (start, solves) = {
        let sim = chain.test_active_power_mut().expect("a power stage");
        (duplicate(sim), sim.statistics().0)
    };
    let mut inputs = Vec::with_capacity(take.len());
    for &x in &take {
        chain.process_stereo(x * scale);
        inputs.push(chain.test_active_power_mut().unwrap().previous_input);
    }
    assert_eq!(
        chain.test_active_power_mut().unwrap().statistics().0 - solves,
        inputs.len() as u64,
        "one power solve a host sample (1x)"
    );

    let passes = |sim: &Simulation| sim.newton_passes;
    let mut today = duplicate(&start);
    let mut real = duplicate(&start);
    let mut shadow = duplicate(&start);
    let (mut base_blocks, mut path_blocks) = (Vec::new(), Vec::new());
    // The same, with the shadow started only once the first half has had a
    // hard solve rather than at the block's start.
    let mut gated_blocks = Vec::new();
    // Second-half solves that waited for the shadow.
    let mut waits = 0u64;
    let mut speculated_at = Vec::new();
    // Wall time, passes and line-search backtracks: the shadow's second-half
    // solves, and the real stage's second half in the same blocks.
    let (mut shadow_time, mut real_time) = ((0u64, 0u64, 0u64), (0u64, 0u64, 0u64));
    let mut redo_per_sample = (0u64, 0u64);
    let (mut work_today, mut work_spec, mut speculated) = (0u64, 0u64, 0usize);
    // A negative gate means "learned": half an average of the input step at
    // the real solve's own hard samples, and no speculation until one is seen.
    let adaptive = gate < 0.0;
    let (mut hard_jump, mut previous_input) = (f64::INFINITY, 0.0f64);
    let step = |real: &mut Simulation, x: f64, hard_jump: &mut f64, previous: &mut f64| {
        let was = real.newton_passes;
        real.process(x);
        let learn_at: u64 = std::env::var("GSFX_LEARN_AT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        if real.newton_passes - was >= learn_at {
            let jump = (x - *previous).abs();
            *hard_jump = if hard_jump.is_finite() {
                0.9 * *hard_jump + 0.1 * jump
            } else {
                jump
            };
        }
        *previous = x;
    };
    for chunk in inputs.chunks_exact(block) {
        let before = passes(&today);
        for &x in chunk {
            today.process(x);
        }
        base_blocks.push(passes(&today) - before);
        work_today += passes(&today) - before;

        // "Hard first half" rule: use the shadow's starts for the second half
        // only if the real solve's first half already had a sample this hard.
        let hardfirst: u64 = std::env::var("GSFX_HARDFIRST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if hardfirst > 0 {
            shadow.copy_runtime_state_from(&real);
            let before = passes(&real);
            let mut hardest = 0;
            // Passes into the first half at the end of its first solve this
            // hard: when a shadow gated on it could start.
            let mut first_hard = None;
            let mut hardest_last = 0;
            for &x in &chunk[..split] {
                let was = passes(&real);
                real.process(x);
                hardest_last = passes(&real) - was;
                hardest = hardest.max(passes(&real) - was);
                if hardest >= hardfirst && first_hard.is_none() {
                    first_hard = Some(passes(&real) - before);
                }
            }
            let a = passes(&real) - before;
            if hardest < hardfirst {
                let before = passes(&real);
                for &x in &chunk[split..] {
                    real.process(x);
                }
                let rest = passes(&real) - before;
                path_blocks.push(a + rest);
                gated_blocks.push(a + rest);
                work_spec += a + rest;
                continue;
            }
            speculated += 1;
            speculated_at.push(base_blocks.len() - 1);
            let before = passes(&shadow);
            let mut starts = Vec::with_capacity(block - split);
            let mut shadow_costs = Vec::with_capacity(block - split);
            let mut full_starts = Vec::with_capacity(block - split);
            for &x in &chunk[split..] {
                let was = passes(&shadow);
                let (backtracks, started) = (shadow.backtrack_count, std::time::Instant::now());
                shadow.process(x);
                shadow_time.0 += started.elapsed().as_nanos() as u64;
                shadow_time.1 += passes(&shadow) - was;
                shadow_time.2 += shadow.backtrack_count - backtracks;
                shadow_costs.push(passes(&shadow) - was);
                full_starts.push((
                    shadow.voltage.clone(),
                    shadow
                        .devices
                        .iter()
                        .map(|d| d.linearisation())
                        .collect::<Vec<_>>(),
                ));
                let hard = passes(&shadow) - was >= hard_at;
                starts.push(hard.then(|| {
                    (
                        shadow.voltage.clone(),
                        shadow
                            .devices
                            .iter()
                            .map(|d| d.linearisation())
                            .collect::<Vec<_>>(),
                    )
                }));
            }
            let b = passes(&shadow) - before;
            let before = passes(&real);
            let (mut real_done, mut gated_done) = (a, a);
            let (mut shadow_done, mut gated_shadow) = (0, first_hard.unwrap_or(0));
            // "prev": take the shadow's answer where the real stage's previous
            // solve was this hard, whatever the shadow found; the real stage
            // waits for the shadow only there.
            let previous_rule: Option<u64> =
                std::env::var("GSFX_PREV").ok().and_then(|v| v.parse().ok());
            let mut previous = hardest_last;
            // "budget": an answer is usable only if the shadow's passes up to
            // it fit in the real stage's first-half passes after the shadow
            // could start -- all of them ("eager") or those after the first
            // hard solve ("gated"). Past that the real stage stops waiting,
            // once the shadow has crossed it.
            let budget_mode = std::env::var("GSFX_BUDGET").ok();
            let first = first_hard.unwrap_or(0);
            let budget = match budget_mode.as_deref() {
                Some("eager") => Some(a),
                Some("gated") => Some(a - first),
                _ => None,
            };
            let (mut cumulative, mut crossed) = (0u64, None::<u64>);
            for (((&x, start), &cost), full) in chunk[split..]
                .iter()
                .zip(starts)
                .zip(&shadow_costs)
                .zip(&full_starts)
            {
                shadow_done += cost;
                gated_shadow += cost;
                let wait = match previous_rule {
                    Some(at) => {
                        let used = previous >= at;
                        if used {
                            real.set_start_point(&full.0, &full.1);
                        }
                        used
                    }
                    None => {
                        cumulative += cost;
                        let usable = budget.is_none_or(|b| cumulative <= b);
                        if !usable && crossed.is_none() {
                            crossed = Some(cumulative);
                        }
                        if usable {
                            if let Some((voltage, linearisation)) = &start {
                                real.set_start_point(voltage, linearisation);
                            }
                        } else {
                            // Waits only until the shadow crossed the budget.
                            let at = crossed.unwrap();
                            real_done = real_done.max(at);
                            gated_done = gated_done.max(first + at);
                        }
                        usable
                    }
                };
                let was = passes(&real);
                let (backtracks, started) = (real.backtrack_count, std::time::Instant::now());
                real.process(x);
                real_time.0 += started.elapsed().as_nanos() as u64;
                real_time.1 += passes(&real) - was;
                real_time.2 += real.backtrack_count - backtracks;
                let redo = passes(&real) - was;
                previous = redo;
                if wait {
                    real_done = real_done.max(shadow_done);
                    gated_done = gated_done.max(gated_shadow);
                    waits += 1;
                }
                real_done += redo;
                gated_done += redo;
            }
            let redo = passes(&real) - before;
            redo_per_sample.0 += redo;
            redo_per_sample.1 += (block - split) as u64;
            path_blocks.push(real_done);
            gated_blocks.push(gated_done);
            let _ = b;
            work_spec += a + b + redo;
            continue;
        }

        // First-half gate: what the worker already knows when it reaches the
        // split, so the decision cannot depend on thread timing.
        let first_half = std::env::var_os("GSFX_GATE_FIRST").is_some();
        let window = if first_half {
            &chunk[..split]
        } else {
            &chunk[split - 1..]
        };
        let jump = window
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        let threshold = if adaptive { 0.5 * hard_jump } else { gate };
        if jump < threshold {
            let before = passes(&real);
            for &x in chunk {
                step(&mut real, x, &mut hard_jump, &mut previous_input);
            }
            let p = passes(&real) - before;
            work_spec += p;
            path_blocks.push(p);
            continue;
        }
        speculated += 1;
        shadow.copy_runtime_state_from(&real);
        let before = passes(&shadow);
        let mut starts = Vec::with_capacity(block - split);
        for &x in &chunk[split..] {
            let was = passes(&shadow);
            shadow.process(x);
            let hard = passes(&shadow) - was >= hard_at;
            starts.push(hard.then(|| {
                (
                    shadow.voltage.clone(),
                    shadow
                        .devices
                        .iter()
                        .map(|d| d.linearisation())
                        .collect::<Vec<_>>(),
                )
            }));
        }
        let b = passes(&shadow) - before;
        let before = passes(&real);
        for &x in &chunk[..split] {
            step(&mut real, x, &mut hard_jump, &mut previous_input);
        }
        let a = passes(&real) - before;
        let before = passes(&real);
        for (&x, start) in chunk[split..].iter().zip(starts) {
            if let Some((voltage, linearisation)) = &start {
                real.set_start_point(voltage, linearisation);
            }
            step(&mut real, x, &mut hard_jump, &mut previous_input);
        }
        let redo = passes(&real) - before;
        redo_per_sample.0 += redo;
        redo_per_sample.1 += (block - split) as u64;
        path_blocks.push(a.max(b) + redo);
        work_spec += a + b + redo;
    }
    let n = base_blocks.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| base_blocks[i]);
    let mean =
        |v: &[u64], idx: &[usize]| idx.iter().map(|&i| v[i] as f64).sum::<f64>() / idx.len() as f64;
    let all: Vec<usize> = (0..n).collect();
    let top1 = &order[n - n / 100..];
    let top01 = &order[n - n / 1000..];
    println!(
        "{name} {gain_db:+} dB, split {split}/{}: {n} blocks",
        block - split
    );
    for (label, idx) in [
        ("all blocks", &all[..]),
        ("heaviest 1 %", top1),
        ("heaviest 0.1 %", top01),
    ] {
        println!(
            "  {label:15}: passes today {:7.1}, critical path with speculation {:7.1} ({:+.0} %)",
            mean(&base_blocks, idx),
            mean(&path_blocks, idx),
            100.0 * (mean(&path_blocks, idx) / mean(&base_blocks, idx) - 1.0)
        );
        if gated_blocks.len() == n {
            println!(
                "  {:15}  shadow started at the first hard solve {:7.1} ({:+.0} %)",
                "",
                mean(&gated_blocks, idx),
                100.0 * (mean(&gated_blocks, idx) / mean(&base_blocks, idx) - 1.0)
            );
        }
    }
    println!(
        "  redo passes a sample: {:.2}",
        redo_per_sample.0 as f64 / redo_per_sample.1.max(1) as f64
    );
    println!("  second-half solves that waited for the shadow: {waits}");
    for (who, (ns, p, b)) in [("shadow", shadow_time), ("real, second half", real_time)] {
        println!(
            "  {who}: {:.0} ns a pass, {:.2} backtracks a pass ({p} passes)",
            ns as f64 / p.max(1) as f64,
            b as f64 / p.max(1) as f64
        );
    }
    let mut changed: Vec<(f64, u64)> = (0..n)
        .filter(|&i| gated_blocks.len() == n && speculated_at.contains(&i))
        .map(|i| {
            (
                gated_blocks[i] as f64 - base_blocks[i] as f64,
                base_blocks[i],
            )
        })
        .collect();
    if !changed.is_empty() {
        changed.sort_by(|a, b| a.0.total_cmp(&b.0));
        let m = changed.len();
        println!(
            "  speculated blocks (gated model): median {:+.1} passes, mean {:+.1}; slower in {} of {m}",
            changed[m / 2].0,
            changed.iter().map(|c| c.0).sum::<f64>() / m as f64,
            changed.iter().filter(|c| c.0 > 0.0).count()
        );
    }
    println!(
        "  speculated on {speculated} of {n} blocks; total solver work {:+.1} %",
        100.0 * (work_spec as f64 / work_today as f64 - 1.0)
    );
    println!(
        "  unsettled today {}, with speculation {}",
        today.unsettled, real.unsettled
    );
}

/// What `Simulation::follow` costs on a power stage, and what it copies.
///
/// `cargo test --release --lib follow_cost -- --ignored --nocapture`
#[test]
#[ignore = "diagnostic: times the shadow's per-block follow"]
fn follow_cost() {
    for name in ["Jazz Chorus", "Brown '84", "Puppet Master '86"] {
        let preset = PRESETS.iter().find(|p| p.name == name).expect("preset");
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        chain.settle();
        chain.find_operating_point();
        let take = take();
        for &x in &take[..4800] {
            chain.process_stereo(x);
        }
        let real = chain.test_active_power_mut().expect("a power stage");
        let mut shadow = real.speculative_copy();
        shadow.follow(real);
        let reps = 20_000;
        let started = std::time::Instant::now();
        for _ in 0..reps {
            shadow.follow(std::hint::black_box(&*real));
        }
        let ns = started.elapsed().as_nanos() as f64 / reps as f64;
        let n = real.n;
        println!(
            "{name}: follow {ns:.0} ns; {n} unknowns, {} devices, {} capacitors, matrix {} f64 x3, partitions {}/{}",
            real.devices.len(),
            real.capacitors.len(),
            n * n,
            real.linear_partition.is_some(),
            real.nonlinear_partition.is_some(),
        );
    }
}

/// The estimator's copies of the power stage solve exactly as the chain's own
/// does, solve for solve, until the chain's first half-step rescue (which a
/// speculative copy does not have): `duplicate`, and `speculative_copy` then
/// `follow`. Both taken after the chain's first sample. Taken before it, they
/// solved at 192 kHz for a chain running its power stage at 48, which is how
/// the first estimates of the speculative second half came out twice too
/// good.
///
/// `cargo test --release --lib the_estimators_copies_follow_the_chain -- --ignored`
#[test]
#[ignore = "diagnostic: checks the estimator's setup"]
fn the_estimators_copies_follow_the_chain() {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == "Jazz Chorus")
        .expect("preset");
    let scale = 10f64.powf(preset.input_trim as f64 / 20.0);
    for speculative in [false, true] {
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        chain.settle();
        chain.find_operating_point();
        chain.process_stereo(0.0);
        let sim = chain.test_active_power_mut().unwrap();
        let mut copy = if speculative {
            let mut copy = sim.speculative_copy();
            copy.follow(sim);
            copy
        } else {
            duplicate(sim)
        };
        let take = take();
        for (k, &x) in take.iter().enumerate() {
            let before = chain.test_active_power_mut().unwrap().newton_passes;
            chain.process_stereo(x * scale);
            let sim = chain.test_active_power_mut().unwrap();
            if sim.half_step_health().0 > 0 {
                println!(
                    "speculative {speculative}: identical for {k} solves, to the first rescue"
                );
                break;
            }
            let passes = copy.newton_passes;
            let y = copy.process(sim.previous_input);
            assert_eq!(
                (y.to_bits(), copy.newton_passes - passes),
                (
                    sim.voltage[sim.circuit.output].to_bits(),
                    sim.newton_passes - before
                ),
                "speculative {speculative}: diverged at solve {k}"
            );
        }
    }
}

/// Cost per Newton pass against circuit size, every preset: what a
/// deterministic estimate of a stage's work would have to predict.
///
/// `cargo test --release --lib pass_cost_by_size -- --ignored --nocapture`
#[test]
#[ignore = "diagnostic: fits a work model"]
fn pass_cost_by_size() {
    let take = take();
    println!(
        "preset,stage,unknowns,boundary,internal,devices,passes_per_solve,ns_per_pass,ns_per_solve"
    );
    for preset in PRESETS.iter() {
        let scale = 10f64.powf(preset.input_trim as f64 / 20.0);
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        chain.settle();
        chain.find_operating_point();
        chain.process_stereo(0.0);
        let before = chain.solver_breakdown();
        chain.set_realtime_stage_timing(true);
        let t0 = chain.realtime_stage_timings();
        for &x in &take[..96_000] {
            chain.process_stereo(x * scale);
        }
        let t = chain.realtime_stage_timings().delta(t0);
        let h = chain.solver_breakdown().saturating_delta(before);
        let (pedal, gain) = chain.test_front();
        let row = |stage: &str, sim: &Simulation, ns: u64, health: crate::voice::SolverHealth| {
            let (b, m) = sim.nonlinear_reduction().unwrap_or((0, 0));
            println!(
                "{:?},{stage},{},{b},{m},{},{:.3},{:.0},{:.0}",
                preset.name,
                sim.n,
                sim.devices.len(),
                health.passes as f64 / health.solves.max(1) as f64,
                ns as f64 / health.passes.max(1) as f64,
                ns as f64 / health.solves.max(1) as f64,
            );
        };
        if let Some(sim) = pedal {
            row("pedal", sim, t.pedal_ns, h.pedal);
        }
        row("gain", gain, t.gain_ns, h.gain);
        if let Some(sim) = chain.test_power() {
            row("power", sim, t.power_ns, h.power);
        }
    }
}
