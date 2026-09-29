//! Ten independent GainStageFx instances, each permanently owned by one thread.
//!
//! This is deliberately NOT a worker pool. Each thread constructs one Chain,
//! keeps it for the whole run, and processes exactly one 64-sample block per
//! simulated host period. A barrier releases all ten threads together and a
//! second barrier marks completion; the coordinator measures the wall time
//! from release until all ten instances have finished.
//!
//! Run:
//!   cargo run --release --example ten_independent_instances
//!
//! Useful Linux comparisons:
//!   taskset -c 0-15 cargo run --release --example ten_independent_instances
//!   taskset -c 0-9  cargo run --release --example ten_independent_instances
//!
//! The first command lets Linux use all 16 logical CPUs. The second restricts
//! the process to CPUs 0..9. Neither pins individual Rust threads: this test is
//! intended to answer the host-like question "what happens when ten persistent
//! independent instances become runnable together?" without adding a custom
//! DSP scheduler to GainStageFx.

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};
use std::hint::black_box;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const CALLBACK_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const INSTANCES: usize = 10;
const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4096;
const PRESET_NAME: &str = "Puppet Master '86";

#[derive(Debug)]
struct WorkerResult {
    id: usize,
    times_us: Vec<f64>,
    checksum: f64,
    pedal_passes: f64,
    pedal_unsettled: u64,
    gain_passes: f64,
    gain_unsettled: u64,
    power_passes: f64,
    power_unsettled: u64,
}

#[derive(Clone, Copy)]
struct Stats {
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    p999: f64,
    max: f64,
    over_1ms: usize,
    over_12ms: usize,
    misses: usize,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn stats(values: &[f64]) -> Stats {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    Stats {
        mean: v.iter().sum::<f64>() / v.len() as f64,
        p50: percentile(&v, 0.50),
        p95: percentile(&v, 0.95),
        p99: percentile(&v, 0.99),
        p999: percentile(&v, 0.999),
        max: *v.last().unwrap(),
        over_1ms: v.iter().filter(|&&x| x > 1000.0).count(),
        over_12ms: v.iter().filter(|&&x| x > 1200.0).count(),
        misses: v.iter().filter(|&&x| x > CALLBACK_US).count(),
    }
}

fn longest_miss_run(values: &[f64]) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for &x in values {
        if x > CALLBACK_US {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest
}

#[inline]
fn input(instance: usize, sample: usize, amplitude: f64) -> f64 {
    let t = sample as f64 / RATE;
    // Slight phase/frequency variation prevents ten instances from being
    // bit-identical while keeping the workload and level comparable.
    let detune = 1.0 + instance as f64 * 0.0007;
    amplitude
        * (0.50 * (std::f64::consts::TAU * 82.4 * detune * t).sin()
            + 0.30 * (std::f64::consts::TAU * 123.5 * detune * t).sin()
            + 0.20 * (std::f64::consts::TAU * 246.9 * detune * t).sin())
}

fn main() {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == PRESET_NAME)
        .unwrap_or_else(|| panic!("shipped preset {PRESET_NAME:?} not found"));
    let settings = preset.settings();
    let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);

    println!("GainStageFx ten-INDEPENDENT-instance realtime benchmark");
    println!("Preset: {PRESET_NAME:?}");
    println!("Each of 10 persistent threads permanently owns exactly one Chain.");
    println!("No worker pool, no job queue, no Chain migration, no per-block thread creation.");
    println!(
        "48 kHz / 64 samples = {CALLBACK_US:.3} us ({:.6} ms) per host period.",
        CALLBACK_US / 1000.0
    );
    println!("Warmup: {WARMUP_BLOCKS}; measured: {MEASURE_BLOCKS} host periods.");
    println!("Shipped oversampling: {}.", preset.oversampling.name());

    // Workers + coordinator participate in both barriers. The coordinator's
    // interval therefore measures release -> slowest worker completed.
    let start_barrier = Arc::new(Barrier::new(INSTANCES + 1));
    let done_barrier = Arc::new(Barrier::new(INSTANCES + 1));

    let mut handles = Vec::with_capacity(INSTANCES);
    for id in 0..INSTANCES {
        let start_barrier = Arc::clone(&start_barrier);
        let done_barrier = Arc::clone(&done_barrier);

        handles.push(thread::spawn(move || {
            let mut chain = Chain::new(RATE);
            chain.apply(&settings);
            chain.settle();
            chain.find_operating_point();

            let mut sample = id * 997;
            let mut checksum = 0.0;
            let mut times_us = Vec::with_capacity(MEASURE_BLOCKS);

            for _ in 0..WARMUP_BLOCKS {
                start_barrier.wait();
                for _ in 0..BLOCK {
                    checksum += chain.process(input(id, sample, amplitude));
                    sample += 1;
                }
                done_barrier.wait();
            }

            let before = chain.solver_breakdown();
            for _ in 0..MEASURE_BLOCKS {
                start_barrier.wait();
                let begin = Instant::now();
                for _ in 0..BLOCK {
                    checksum += chain.process(input(id, sample, amplitude));
                    sample += 1;
                }
                times_us.push(begin.elapsed().as_secs_f64() * 1_000_000.0);
                done_barrier.wait();
            }
            let health = chain.solver_breakdown().saturating_delta(before);
            black_box(checksum);

            WorkerResult {
                id,
                times_us,
                checksum,
                pedal_passes: health.pedal.passes_per_solve(),
                pedal_unsettled: health.pedal.unsettled,
                gain_passes: health.gain.passes_per_solve(),
                gain_unsettled: health.gain.unsettled,
                power_passes: health.power.passes_per_solve(),
                power_unsettled: health.power.unsettled,
            }
        }));
    }

    // Warm up all ten chains in parallel.
    for _ in 0..WARMUP_BLOCKS {
        start_barrier.wait();
        done_barrier.wait();
    }

    // Measure the simulated host period: all ten released -> all ten done.
    let mut host_times_us = Vec::with_capacity(MEASURE_BLOCKS);
    for _ in 0..MEASURE_BLOCKS {
        let begin = Instant::now();
        start_barrier.wait();
        done_barrier.wait();
        host_times_us.push(begin.elapsed().as_secs_f64() * 1_000_000.0);
    }

    let mut workers: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("DSP worker panicked"))
        .collect();
    workers.sort_by_key(|w| w.id);

    println!("\n================ PER INSTANCE ================");
    println!(
        "{:>4} {:>9} {:>9} {:>9} {:>9} {:>9} {:>7} {:>7} {:>7}",
        "id", "mean", "p95", "p99", "p99.9", "max", ">1ms", ">1.2", "miss"
    );
    for w in &workers {
        let s = stats(&w.times_us);
        println!(
            "{:>4} {:>9.1} {:>9.1} {:>9.1} {:>9.1} {:>9.1} {:>7} {:>7} {:>7}",
            w.id, s.mean, s.p95, s.p99, s.p999, s.max, s.over_1ms, s.over_12ms, s.misses
        );
        println!(
            "     solver pedal {:.2}/solve {:>5} unsettled | gain {:.2}/solve {:>5} | power {:.2}/solve {:>5} | checksum {:.4e}",
            w.pedal_passes,
            w.pedal_unsettled,
            w.gain_passes,
            w.gain_unsettled,
            w.power_passes,
            w.power_unsettled,
            w.checksum
        );
    }

    let host = stats(&host_times_us);
    println!("\n================ WHOLE HOST PERIOD ================");
    println!(
        "  mean             : {:9.3} us ({:5.1}% budget)",
        host.mean,
        host.mean / CALLBACK_US * 100.0
    );
    println!("  p50              : {:9.3} us", host.p50);
    println!("  p95              : {:9.3} us", host.p95);
    println!(
        "  p99              : {:9.3} us ({:5.1}% budget)",
        host.p99,
        host.p99 / CALLBACK_US * 100.0
    );
    println!("  p99.9            : {:9.3} us", host.p999);
    println!(
        "  max              : {:9.3} us ({:5.1}% budget)",
        host.max,
        host.max / CALLBACK_US * 100.0
    );
    println!(
        "  > 1.000 ms       : {:9} / {MEASURE_BLOCKS}",
        host.over_1ms
    );
    println!(
        "  > 1.200 ms       : {:9} / {MEASURE_BLOCKS}",
        host.over_12ms
    );
    println!("  deadline misses  : {:9} / {MEASURE_BLOCKS}", host.misses);
    println!(
        "  longest miss run : {:9} periods",
        longest_miss_run(&host_times_us)
    );
    println!("  p99 headroom     : {:+9.3} us", CALLBACK_US - host.p99);
    println!("  max headroom     : {:+9.3} us", CALLBACK_US - host.max);

    // For every host period, derive the fastest and slowest *DSP computation*
    // observed among the ten workers. This excludes barrier/coordinator cost.
    let mut fastest = Vec::with_capacity(MEASURE_BLOCKS);
    let mut slowest = Vec::with_capacity(MEASURE_BLOCKS);
    for block in 0..MEASURE_BLOCKS {
        let mut lo = f64::INFINITY;
        let mut hi: f64 = 0.0;
        for w in &workers {
            lo = lo.min(w.times_us[block]);
            hi = hi.max(w.times_us[block]);
        }
        fastest.push(lo);
        slowest.push(hi);
    }
    let fast = stats(&fastest);
    let slow = stats(&slowest);
    println!("\nAcross the ten DSP computations in each period (barriers excluded):");
    println!("  fastest-instance mean : {:9.3} us", fast.mean);
    println!("  slowest-instance mean : {:9.3} us", slow.mean);
    println!("  slowest-instance p99  : {:9.3} us", slow.p99);
    println!("  slowest-instance max  : {:9.3} us", slow.max);

    println!("\nInterpretation:");
    println!("  * The WHOLE HOST PERIOD row is the important pass/fail result.");
    println!("  * Zero host misses means all ten independently owned instances completed");
    println!("    before 1.333 ms for every measured period on this run.");
    println!("  * Per-instance rows reveal whether one CPU/thread is repeatedly the tail.");
    println!("  * Barrier overhead is intentionally included in host-period wall time,");
    println!("    making this stricter than looking only at the slowest Chain::process time.");
}
