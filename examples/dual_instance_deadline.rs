//! Dual-instance realtime benchmark for the exact REAPER workload:
//!
//!   Track 1 playback: Jazz Chorus
//!   Track 2 live monitor/record: Puppet Master '86
//!
//! It measures each shipped preset alone and then both instances serially
//! inside the same 48 kHz / 64-sample callback. The dual row gives both chains
//! the SAME absolute callback deadline, matching the fact that a DAW callback
//! has one wall-clock deadline rather than one deadline per plugin instance.
//!
//! Run:
//!   cargo run --release --example dual_instance_deadline
//!
//! Optional second run:
//!   taskset -c 2 cargo run --release --example dual_instance_deadline
//!
//! This benchmark intentionally does not create a new OS thread per callback.
//! Thread creation/synchronisation would dominate a 1.333 ms period and would
//! not model REAPER's persistent worker pool. The serial dual row is the hard
//! single-core dependency-path case; the two single-instance rows show the
//! independent costs a DAW scheduler may place on separate workers.

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, SolverBreakdown, NOMINAL_DBFS};
use std::hint::black_box;
use std::time::{Duration, Instant};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const CALLBACK_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const DEADLINE_FRACTION: f64 = 0.90;
const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4096;

const JAZZ: &str = "Jazz Chorus";
const PUPPET: &str = "Puppet Master '86";

#[derive(Clone, Copy, Debug)]
struct Stats {
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
    over_1ms: usize,
    over_12ms: usize,
    misses: usize,
    longest_miss_run: usize,
}

struct ResultRow {
    label: &'static str,
    stats: Stats,
    checksum: f64,
    jazz_health: Option<SolverBreakdown>,
    puppet_health: Option<SolverBreakdown>,
    jazz_aborts: u64,
    puppet_aborts: u64,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn stats(mut times: Vec<f64>) -> Stats {
    let mut longest = 0usize;
    let mut run = 0usize;
    for &t in &times {
        if t > CALLBACK_US {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }

    let over_1ms = times.iter().filter(|&&x| x > 1000.0).count();
    let over_12ms = times.iter().filter(|&&x| x > 1200.0).count();
    let misses = times.iter().filter(|&&x| x > CALLBACK_US).count();
    let mean = times.iter().sum::<f64>() / times.len() as f64;

    times.sort_by(|a, b| a.total_cmp(b));
    Stats {
        mean,
        p50: percentile(&times, 0.50),
        p95: percentile(&times, 0.95),
        p99: percentile(&times, 0.99),
        max: *times.last().unwrap(),
        over_1ms,
        over_12ms,
        misses,
        longest_miss_run: longest,
    }
}

#[inline]
fn input(sample: usize, amplitude: f64, phase: f64) -> f64 {
    let t = sample as f64 / RATE;
    amplitude
        * (0.50 * (std::f64::consts::TAU * 82.4 * t + phase).sin()
            + 0.30 * (std::f64::consts::TAU * 123.5 * t + phase * 0.7).sin()
            + 0.20 * (std::f64::consts::TAU * 246.9 * t + phase * 1.3).sin())
}

fn make_chain(name: &str) -> (Chain, f64) {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("shipped preset {name:?} not found"));

    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();

    let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
    (chain, amplitude)
}

fn health_line(name: &str, h: SolverBreakdown, aborts: u64) {
    println!(
        "  {name:<7} pedal {:4.2}/solve {:4} unsettled | gain {:4.2}/solve {:4} unsettled | power {:4.2}/solve {:4} unsettled | deadline aborts {aborts}",
        h.pedal.passes_per_solve(),
        h.pedal.unsettled,
        h.gain.passes_per_solve(),
        h.gain.unsettled,
        h.power.passes_per_solve(),
        h.power.unsettled,
    );
}

fn print_row(row: &ResultRow) {
    let s = row.stats;
    println!("\n{}", row.label);
    println!(
        "  mean             : {:9.3} us  ({:5.1}% budget)",
        s.mean,
        s.mean / CALLBACK_US * 100.0
    );
    println!("  p50              : {:9.3} us", s.p50);
    println!("  p95              : {:9.3} us", s.p95);
    println!(
        "  p99              : {:9.3} us  ({:5.1}% budget)",
        s.p99,
        s.p99 / CALLBACK_US * 100.0
    );
    println!(
        "  max              : {:9.3} us  ({:5.1}% budget)",
        s.max,
        s.max / CALLBACK_US * 100.0
    );
    println!("  > 1.000 ms       : {:9} / {}", s.over_1ms, MEASURE_BLOCKS);
    println!(
        "  > 1.200 ms       : {:9} / {}",
        s.over_12ms, MEASURE_BLOCKS
    );
    println!("  deadline misses  : {:9} / {}", s.misses, MEASURE_BLOCKS);
    println!("  longest miss run : {:9} callbacks", s.longest_miss_run);
    println!("  p99 headroom     : {:+9.3} us", CALLBACK_US - s.p99);
    println!("  max headroom     : {:+9.3} us", CALLBACK_US - s.max);
    if let Some(h) = row.jazz_health {
        health_line("Jazz", h, row.jazz_aborts);
    }
    if let Some(h) = row.puppet_health {
        health_line("Puppet", h, row.puppet_aborts);
    }
    println!("  checksum         : {:.6e}", row.checksum);
}

fn run_single(name: &'static str, label: &'static str, phase: f64) -> ResultRow {
    let (mut chain, amplitude) = make_chain(name);
    let mut sample = 0usize;
    let mut checksum = 0.0;

    // Warmup without realtime pressure.
    chain.set_realtime_deadline(None);
    for _ in 0..WARMUP_BLOCKS {
        for _ in 0..BLOCK {
            checksum += chain.process(input(sample, amplitude, phase));
            sample += 1;
        }
    }

    let health_before = chain.solver_breakdown();
    let aborts_before = chain.deadline_aborts();
    let mut times = Vec::with_capacity(MEASURE_BLOCKS);

    for _ in 0..MEASURE_BLOCKS {
        let started = Instant::now();
        let deadline = started + Duration::from_secs_f64(BLOCK as f64 / RATE * DEADLINE_FRACTION);
        chain.set_realtime_deadline(Some(deadline));

        for _ in 0..BLOCK {
            checksum += chain.process(input(sample, amplitude, phase));
            sample += 1;
        }
        times.push(started.elapsed().as_secs_f64() * 1_000_000.0);
    }
    chain.set_realtime_deadline(None);

    let health = chain.solver_breakdown().saturating_delta(health_before);
    let aborts = chain.deadline_aborts().saturating_sub(aborts_before);
    black_box(checksum);

    ResultRow {
        label,
        stats: stats(times),
        checksum,
        jazz_health: (name == JAZZ).then_some(health),
        puppet_health: (name == PUPPET).then_some(health),
        jazz_aborts: if name == JAZZ { aborts } else { 0 },
        puppet_aborts: if name == PUPPET { aborts } else { 0 },
    }
}

fn run_dual() -> ResultRow {
    let (mut jazz, jazz_amp) = make_chain(JAZZ);
    let (mut puppet, puppet_amp) = make_chain(PUPPET);

    let mut sample = 0usize;
    let mut checksum = 0.0;

    jazz.set_realtime_deadline(None);
    puppet.set_realtime_deadline(None);
    for _ in 0..WARMUP_BLOCKS {
        for i in 0..BLOCK {
            let n = sample + i;
            checksum += jazz.process(input(n, jazz_amp, 0.31));
        }
        for i in 0..BLOCK {
            let n = sample + i;
            checksum += puppet.process(input(n, puppet_amp, 1.17));
        }
        sample += BLOCK;
    }

    let jazz_health_before = jazz.solver_breakdown();
    let puppet_health_before = puppet.solver_breakdown();
    let jazz_aborts_before = jazz.deadline_aborts();
    let puppet_aborts_before = puppet.deadline_aborts();

    let mut times = Vec::with_capacity(MEASURE_BLOCKS);

    for _ in 0..MEASURE_BLOCKS {
        let started = Instant::now();

        // Both instances share the same callback cutoff. The second instance
        // does NOT get a fresh 1.2 ms budget after the first one finishes.
        let deadline = started + Duration::from_secs_f64(BLOCK as f64 / RATE * DEADLINE_FRACTION);
        jazz.set_realtime_deadline(Some(deadline));
        puppet.set_realtime_deadline(Some(deadline));

        // Playback track first, then the live monitored/recorded track.
        for i in 0..BLOCK {
            let n = sample + i;
            checksum += jazz.process(input(n, jazz_amp, 0.31));
        }
        for i in 0..BLOCK {
            let n = sample + i;
            checksum += puppet.process(input(n, puppet_amp, 1.17));
        }

        sample += BLOCK;
        times.push(started.elapsed().as_secs_f64() * 1_000_000.0);
    }

    jazz.set_realtime_deadline(None);
    puppet.set_realtime_deadline(None);

    let jazz_health = jazz.solver_breakdown().saturating_delta(jazz_health_before);
    let puppet_health = puppet
        .solver_breakdown()
        .saturating_delta(puppet_health_before);
    let jazz_aborts = jazz.deadline_aborts().saturating_sub(jazz_aborts_before);
    let puppet_aborts = puppet
        .deadline_aborts()
        .saturating_sub(puppet_aborts_before);

    black_box(checksum);

    ResultRow {
        label: "3. Jazz Chorus playback + Puppet Master '86 live (serial)",
        stats: stats(times),
        checksum,
        jazz_health: Some(jazz_health),
        puppet_health: Some(puppet_health),
        jazz_aborts,
        puppet_aborts,
    }
}

fn main() {
    println!("GainStageFx dual-instance realtime deadline benchmark");
    println!("Exact workload: Jazz Chorus playback + Puppet Master '86 live.");
    println!(
        "48 kHz / 64 samples = {CALLBACK_US:.3} us ({:.6} ms) per host callback.",
        CALLBACK_US / 1000.0
    );
    println!(
        "Solver emergency cutoff = {:.0}% of period = {:.3} us.",
        DEADLINE_FRACTION * 100.0,
        CALLBACK_US * DEADLINE_FRACTION
    );
    println!("Warmup: {WARMUP_BLOCKS} blocks; measured: {MEASURE_BLOCKS} blocks per row.");
    println!("All preset settings, including shipped oversampling, are retained.");

    let jazz = run_single(JAZZ, "1. Jazz Chorus alone", 0.31);
    print_row(&jazz);

    let puppet = run_single(PUPPET, "2. Puppet Master '86 alone", 1.17);
    print_row(&puppet);

    let dual = run_dual();
    print_row(&dual);

    println!("\n================ SUMMARY ================");
    println!(
        "{:<58} {:>9} {:>9} {:>9} {:>8} {:>8}",
        "path", "mean us", "p99 us", "max us", "misses", "run"
    );
    for row in [&jazz, &puppet, &dual] {
        println!(
            "{:<58} {:>9.1} {:>9.1} {:>9.1} {:>8} {:>8}",
            row.label,
            row.stats.mean,
            row.stats.p99,
            row.stats.max,
            row.stats.misses,
            row.stats.longest_miss_run
        );
    }

    let predicted_sum = jazz.stats.mean + puppet.stats.mean;
    println!("\nCost accounting:");
    println!("  Jazz-alone mean            : {:8.1} us", jazz.stats.mean);
    println!(
        "  Puppet-alone mean          : {:8.1} us",
        puppet.stats.mean
    );
    println!("  sum of independent means   : {:8.1} us", predicted_sum);
    println!("  measured serial dual mean  : {:8.1} us", dual.stats.mean);
    println!("  host hard deadline         : {:8.1} us", CALLBACK_US);

    println!("\nInterpretation:");
    println!("  * If each single row is safe but the serial row misses heavily,");
    println!("    aggregate per-period work is enough to explain the REAPER symptom");
    println!("    when both instances land on one realtime dependency path/core.");
    println!("  * Deadline aborts show whether the new protection actually fired.");
    println!("  * A long consecutive miss run is more consistent with audible");
    println!("    stuttering/dropout than isolated one-off late callbacks.");
    println!("  * REAPER may schedule independent tracks in parallel; this serial row");
    println!("    is intentionally the hard single-core/dependency-path case.");
}
