//! GainStageFx 10-instance multicore deadline benchmark.
//!
//! Workloads:
//!   A. 10 x Puppet Master '86
//!   B. 10 x Jazz Chorus
//!   C. 5 x Jazz Chorus + 5 x Puppet Master '86
//!
//! Worker counts:
//!   1, 2, 4, 8, 10
//!
//! Important:
//! - Worker threads are persistent. No threads are created in the timed callback.
//! - Every worker owns its Chain instances; no Chain crosses threads.
//! - A callback generation is dispatched to all workers, then the coordinator
//!   waits until all workers finish. The measured wall time is therefore the
//!   multicore makespan for all ten instances.
//! - Deadline protection is disabled inside the Chains. This benchmark measures
//!   whether normal DSP can fit in 1.333333 ms without deadline-abort recovery
//!   contaminating the result.
//! - The worker pool itself uses atomics plus spin waiting. This is a benchmark,
//!   not code intended to be copied into the plugin audio path.
//!
//! Run:
//!   cargo run --release --example ten_instance_multicore
//!
//! For a cleaner Linux run, optionally pin the benchmark to a CPU set large
//! enough for the maximum worker count, for example:
//!   taskset -c 0-11 cargo run --release --example ten_instance_multicore

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};
use std::hint::{black_box, spin_loop};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const CALLBACK_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;

const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4096;
const WORKERS: &[usize] = &[1, 2, 4, 8, 10];

const PUPPET: &str = "Puppet Master '86";
const JAZZ: &str = "Jazz Chorus";
const INSTANCES: usize = 10;

#[derive(Clone, Copy)]
enum Kind {
    Jazz,
    Puppet,
}

impl Kind {
    fn preset(self) -> &'static str {
        match self {
            Self::Jazz => JAZZ,
            Self::Puppet => PUPPET,
        }
    }

    fn phase(self, index: usize) -> f64 {
        let base = match self {
            Self::Jazz => 0.31,
            Self::Puppet => 1.17,
        };
        base + index as f64 * 0.173
    }
}

#[derive(Clone, Copy)]
struct InstanceSpec {
    kind: Kind,
    index: usize,
}

#[derive(Clone, Copy)]
struct Workload {
    label: &'static str,
    specs: [InstanceSpec; INSTANCES],
}

const fn spec(kind: Kind, index: usize) -> InstanceSpec {
    InstanceSpec { kind, index }
}

const TEN_PUPPET: Workload = Workload {
    label: "10 x Puppet Master '86",
    specs: [
        spec(Kind::Puppet, 0),
        spec(Kind::Puppet, 1),
        spec(Kind::Puppet, 2),
        spec(Kind::Puppet, 3),
        spec(Kind::Puppet, 4),
        spec(Kind::Puppet, 5),
        spec(Kind::Puppet, 6),
        spec(Kind::Puppet, 7),
        spec(Kind::Puppet, 8),
        spec(Kind::Puppet, 9),
    ],
};

const TEN_JAZZ: Workload = Workload {
    label: "10 x Jazz Chorus",
    specs: [
        spec(Kind::Jazz, 0),
        spec(Kind::Jazz, 1),
        spec(Kind::Jazz, 2),
        spec(Kind::Jazz, 3),
        spec(Kind::Jazz, 4),
        spec(Kind::Jazz, 5),
        spec(Kind::Jazz, 6),
        spec(Kind::Jazz, 7),
        spec(Kind::Jazz, 8),
        spec(Kind::Jazz, 9),
    ],
};

const MIXED: Workload = Workload {
    label: "5 x Jazz Chorus + 5 x Puppet Master '86",
    specs: [
        spec(Kind::Jazz, 0),
        spec(Kind::Puppet, 0),
        spec(Kind::Jazz, 1),
        spec(Kind::Puppet, 1),
        spec(Kind::Jazz, 2),
        spec(Kind::Puppet, 2),
        spec(Kind::Jazz, 3),
        spec(Kind::Puppet, 3),
        spec(Kind::Jazz, 4),
        spec(Kind::Puppet, 4),
    ],
};

const WORKLOADS: &[Workload] = &[TEN_PUPPET, TEN_JAZZ, MIXED];

struct OwnedInstance {
    chain: Chain,
    amplitude: f64,
    phase: f64,
}

fn make_instance(spec: InstanceSpec) -> OwnedInstance {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == spec.kind.preset())
        .unwrap_or_else(|| panic!("shipped preset {:?} not found", spec.kind.preset()));

    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();

    // This benchmark measures normal processing cost, not the emergency
    // deadline-recovery mechanism.
    chain.set_realtime_deadline(None);

    let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);

    OwnedInstance {
        chain,
        amplitude,
        phase: spec.kind.phase(spec.index),
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

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn stats(mut times: Vec<f64>) -> Stats {
    let mut longest = 0;
    let mut run = 0;

    for &t in &times {
        if t > CALLBACK_US {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }

    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let over_1ms = times.iter().filter(|&&t| t > 1000.0).count();
    let over_12ms = times.iter().filter(|&&t| t > 1200.0).count();
    let misses = times.iter().filter(|&&t| t > CALLBACK_US).count();

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

struct RunResult {
    workers: usize,
    stats: Stats,
    checksum: f64,
}

struct Shared {
    generation: AtomicUsize,
    done: AtomicUsize,
    sample_base: AtomicUsize,
    stop: AtomicBool,
    checksum_bits: Vec<AtomicU64>,
}

fn partition(specs: &[InstanceSpec; INSTANCES], workers: usize) -> Vec<Vec<InstanceSpec>> {
    // Greedy round-robin is deliberately simple and deterministic. For the
    // mixed workload, interleaving Jazz/Puppet in MIXED keeps the expensive
    // Puppet instances reasonably spread across workers.
    let mut buckets = vec![Vec::new(); workers];
    for (i, &s) in specs.iter().enumerate() {
        buckets[i % workers].push(s);
    }
    buckets
}

fn run(workload: Workload, workers: usize) -> RunResult {
    let buckets = partition(&workload.specs, workers);

    let shared = Arc::new(Shared {
        generation: AtomicUsize::new(0),
        done: AtomicUsize::new(0),
        sample_base: AtomicUsize::new(0),
        stop: AtomicBool::new(false),
        checksum_bits: (0..workers).map(|_| AtomicU64::new(0)).collect(),
    });

    let mut handles = Vec::with_capacity(workers);

    for (worker_id, bucket) in buckets.into_iter().enumerate() {
        let shared = Arc::clone(&shared);

        handles.push(thread::spawn(move || {
            let mut instances: Vec<OwnedInstance> = bucket.into_iter().map(make_instance).collect();
            let mut seen_generation = 0usize;
            let mut checksum = 0.0f64;

            loop {
                let generation = loop {
                    let g = shared.generation.load(Ordering::Acquire);
                    if g != seen_generation {
                        break g;
                    }
                    if shared.stop.load(Ordering::Acquire) {
                        shared.checksum_bits[worker_id]
                            .store(checksum.to_bits(), Ordering::Release);
                        return;
                    }
                    spin_loop();
                };

                let sample_base = shared.sample_base.load(Ordering::Acquire);

                for instance in &mut instances {
                    for i in 0..BLOCK {
                        checksum += instance.chain.process(input(
                            sample_base + i,
                            instance.amplitude,
                            instance.phase,
                        ));
                    }
                }

                seen_generation = generation;
                shared.done.fetch_add(1, Ordering::Release);
            }
        }));
    }

    let mut generation = 0usize;
    let mut sample_base = 0usize;

    // Warm all chains and the worker scheduling path before timing.
    for _ in 0..WARMUP_BLOCKS {
        shared.done.store(0, Ordering::Relaxed);
        shared.sample_base.store(sample_base, Ordering::Release);
        generation += 1;
        shared.generation.store(generation, Ordering::Release);

        while shared.done.load(Ordering::Acquire) != workers {
            spin_loop();
        }

        sample_base += BLOCK;
    }

    let mut times = Vec::with_capacity(MEASURE_BLOCKS);

    for _ in 0..MEASURE_BLOCKS {
        shared.done.store(0, Ordering::Relaxed);
        shared.sample_base.store(sample_base, Ordering::Release);

        let started = Instant::now();
        generation += 1;
        shared.generation.store(generation, Ordering::Release);

        while shared.done.load(Ordering::Acquire) != workers {
            spin_loop();
        }

        times.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        sample_base += BLOCK;
    }

    shared.stop.store(true, Ordering::Release);
    // Wake workers that are waiting for a new generation.
    generation += 1;
    shared.generation.store(generation, Ordering::Release);

    for handle in handles {
        handle.join().expect("worker thread panicked");
    }

    let checksum = shared
        .checksum_bits
        .iter()
        .map(|v| f64::from_bits(v.load(Ordering::Acquire)))
        .sum::<f64>();

    black_box(checksum);

    RunResult {
        workers,
        stats: stats(times),
        checksum,
    }
}

fn print_result(r: &RunResult) {
    let s = r.stats;
    println!(
        "  {:>2} workers | mean {:8.1} us | p50 {:8.1} | p95 {:8.1} | p99 {:8.1} | max {:8.1} | misses {:4}/{:4} | longest {:3}",
        r.workers,
        s.mean,
        s.p50,
        s.p95,
        s.p99,
        s.max,
        s.misses,
        MEASURE_BLOCKS,
        s.longest_miss_run
    );
    println!(
        "             >1ms {:4} | >1.2ms {:4} | p99 headroom {:+8.1} us | checksum {:.6e}",
        s.over_1ms,
        s.over_12ms,
        CALLBACK_US - s.p99,
        r.checksum
    );
}

fn main() {
    println!("GainStageFx 10-instance multicore deadline benchmark");
    println!(
        "48 kHz / 64 samples = {:.3} us ({:.6} ms) per callback.",
        CALLBACK_US,
        CALLBACK_US / 1000.0
    );
    println!(
        "Warmup: {WARMUP_BLOCKS} blocks; measured: {MEASURE_BLOCKS} blocks per worker-count row."
    );
    println!("Persistent workers; no per-callback thread creation.");
    println!("Chain deadline-abort protection: DISABLED for this throughput test.");
    println!("Worker counts: {WORKERS:?}");
    println!();

    for &workload in WORKLOADS {
        println!("============================================================");
        println!("{}", workload.label);
        println!("============================================================");

        let mut results = Vec::new();
        for &workers in WORKERS {
            let result = run(workload, workers);
            print_result(&result);
            results.push(result);
        }

        println!("\n  Summary:");
        println!(
            "  {:>7} {:>10} {:>10} {:>10} {:>10} {:>9} {:>9}",
            "workers", "mean us", "p95 us", "p99 us", "max us", "misses", "run"
        );
        for r in &results {
            println!(
                "  {:>7} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>9} {:>9}",
                r.workers,
                r.stats.mean,
                r.stats.p95,
                r.stats.p99,
                r.stats.max,
                r.stats.misses,
                r.stats.longest_miss_run
            );
        }

        if let Some(best) = results
            .iter()
            .min_by(|a, b| a.stats.p99.total_cmp(&b.stats.p99))
        {
            println!(
                "\n  Best p99 in this run: {} workers, {:.1} us ({:+.1} us headroom).",
                best.workers,
                best.stats.p99,
                CALLBACK_US - best.stats.p99
            );
        }
        println!();
    }

    println!("Interpretation:");
    println!("  * A row is hard-deadline safe only if misses are 0.");
    println!("  * p99 comfortably below 1333.3 us is more useful than merely a low mean.");
    println!("  * Compare 1 -> 2 -> 4 -> 8 workers to see actual multicore scaling.");
    println!("  * 10 workers can be worse than 8 due to scheduling/cache/SMT overhead.");
    println!("  * This tests whether the existing DSP has enough aggregate multicore");
    println!("    throughput; it does not prove REAPER will schedule instances identically.");
}
