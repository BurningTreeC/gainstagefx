//! Explicit-affinity 10-instance multicore deadline benchmark.
//!
//! Linux-only benchmark. Persistent worker threads are pinned explicitly.
//! CPU order is physical cores first, then SMT siblings for the user's 8C/16T topology:
//!   0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
//!
//! Run:
//!   cargo run --release --example ten_instance_affinity
//!
//! Optional:
//!   taskset -c 0-15 cargo run --release --example ten_instance_affinity
//!
//! Do NOT use `taskset -c 0-7` for the 10/16-worker rows: that defeats the point
//! of explicitly testing the SMT siblings.

#![cfg(target_os = "linux")]

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::Chain;
use std::ffi::c_int;
use std::sync::{mpsc, Arc, Barrier};
use std::thread;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const WARMUP: usize = 256;
const BLOCKS: usize = 4096;
const INSTANCES: usize = 10;
const BUDGET_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const WORKERS: &[usize] = &[1, 2, 4, 8, 10, 16];

// Ryzen-style topology observed on the target machine:
// CPU 0..7 = physical cores 0..7, CPU 8..15 = SMT siblings of 0..7.
const CPU_ORDER: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

#[repr(C)]
struct CpuSet {
    bits: [u64; 16], // 1024 CPUs, matching Linux cpu_set_t on common glibc targets.
}

unsafe extern "C" {
    fn sched_setaffinity(pid: c_int, cpusetsize: usize, mask: *const CpuSet) -> c_int;
}

fn pin_current_thread(cpu: usize) {
    assert!(
        cpu < 1024,
        "CPU {cpu} is outside this benchmark's cpu_set_t"
    );
    let mut set = CpuSet { bits: [0; 16] };
    set.bits[cpu / 64] |= 1u64 << (cpu % 64);

    let rc = unsafe { sched_setaffinity(0, std::mem::size_of::<CpuSet>(), &set as *const CpuSet) };
    if rc != 0 {
        panic!(
            "sched_setaffinity(cpu={cpu}) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

fn preset(name: &str) -> &'static gainstagefx::presets::Preset {
    PRESETS
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("preset '{name}' not found"))
}

#[derive(Clone, Copy)]
enum Workload {
    Puppet,
    Jazz,
    Mixed,
}

impl Workload {
    fn title(self) -> &'static str {
        match self {
            Self::Puppet => "10 x Puppet Master '86",
            Self::Jazz => "10 x Jazz Chorus",
            Self::Mixed => "5 x Jazz Chorus + 5 x Puppet Master '86",
        }
    }

    fn preset_for(self, instance: usize) -> &'static gainstagefx::presets::Preset {
        match self {
            Self::Puppet => preset("Puppet Master '86"),
            Self::Jazz => preset("Jazz Chorus"),
            Self::Mixed => {
                if instance < 5 {
                    preset("Jazz Chorus")
                } else {
                    preset("Puppet Master '86")
                }
            }
        }
    }
}

enum Command {
    Process { block: usize },
    Stop,
}

struct Done {
    checksum: f64,
}

struct Stats {
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
    misses: usize,
    over_1ms: usize,
    over_12ms: usize,
    longest: usize,
    checksum: f64,
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i]
}

fn signal(instance: usize, block: usize, sample: usize) -> f64 {
    // Deterministic, nontrivial guitar-ish excitation. Each instance differs
    // slightly so ten chains do not all follow exactly the same solver path.
    let n = block * BLOCK + sample;
    let t = n as f64 / RATE;
    let f0 = 82.406_889 * (1.0 + instance as f64 * 0.0075);
    let env = 0.55 + 0.45 * (std::f64::consts::TAU * 1.7 * t).sin().abs();
    0.11 * env
        * ((std::f64::consts::TAU * f0 * t).sin()
            + 0.42 * (std::f64::consts::TAU * f0 * 2.01 * t).sin()
            + 0.20 * (std::f64::consts::TAU * f0 * 3.02 * t).sin())
}

fn run(workload: Workload, workers: usize) -> Stats {
    let active_workers = workers.min(INSTANCES);
    let start_barrier = Arc::new(Barrier::new(active_workers + 1));
    let (done_tx, done_rx) = mpsc::channel::<Done>();

    let mut command_txs = Vec::with_capacity(active_workers);
    let mut handles = Vec::with_capacity(active_workers);

    // Static instance assignment. This deliberately avoids work stealing,
    // migration and per-callback allocation.
    for (worker, &cpu) in CPU_ORDER[..active_workers].iter().enumerate() {
        let assigned: Vec<usize> = (0..INSTANCES)
            .filter(|i| i % active_workers == worker)
            .collect();

        let mut chains: Vec<(usize, Chain)> = assigned
            .into_iter()
            .map(|i| {
                let mut chain = Chain::new(RATE);
                chain.apply(&workload.preset_for(i).settings());
                (i, chain)
            })
            .collect();

        let (tx, rx) = mpsc::channel::<Command>();
        command_txs.push(tx);

        let done = done_tx.clone();
        let barrier = Arc::clone(&start_barrier);
        handles.push(thread::spawn(move || {
            pin_current_thread(cpu);

            while let Ok(command) = rx.recv() {
                match command {
                    Command::Process { block } => {
                        barrier.wait();
                        let mut checksum = 0.0;
                        for (instance, chain) in &mut chains {
                            for s in 0..BLOCK {
                                checksum += chain.process(signal(*instance, block, s));
                            }
                        }
                        done.send(Done { checksum }).unwrap();
                    }
                    Command::Stop => break,
                }
            }
        }));
    }
    drop(done_tx);

    // Warm the exact worker placement and exact chains that will be measured.
    for block in 0..WARMUP {
        for tx in &command_txs {
            tx.send(Command::Process { block }).unwrap();
        }
        start_barrier.wait();
        for _ in 0..active_workers {
            let _ = done_rx.recv().unwrap();
        }
    }

    let mut times = Vec::with_capacity(BLOCKS);
    let mut checksum = 0.0;

    for block in WARMUP..WARMUP + BLOCKS {
        for tx in &command_txs {
            tx.send(Command::Process { block }).unwrap();
        }

        // Start timing immediately before releasing all workers.
        let t0 = Instant::now();
        start_barrier.wait();

        for _ in 0..active_workers {
            checksum += done_rx.recv().unwrap().checksum;
        }
        times.push(t0.elapsed().as_secs_f64() * 1_000_000.0);
    }

    for tx in &command_txs {
        let _ = tx.send(Command::Stop);
    }
    for h in handles {
        h.join().unwrap();
    }

    let mut sorted = times.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));

    let misses = times.iter().filter(|&&x| x > BUDGET_US).count();
    let over_1ms = times.iter().filter(|&&x| x > 1000.0).count();
    let over_12ms = times.iter().filter(|&&x| x > 1200.0).count();

    let mut longest = 0;
    let mut run = 0;
    for &x in &times {
        if x > BUDGET_US {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }

    Stats {
        mean: times.iter().sum::<f64>() / times.len() as f64,
        p50: percentile(&sorted, 0.50),
        p95: percentile(&sorted, 0.95),
        p99: percentile(&sorted, 0.99),
        max: *sorted.last().unwrap(),
        misses,
        over_1ms,
        over_12ms,
        longest,
        checksum,
    }
}

fn main() {
    println!("GainStageFx 10-instance EXPLICIT-AFFINITY deadline benchmark");
    println!(
        "48 kHz / 64 samples = {:.3} us ({:.6} ms) per callback.",
        BUDGET_US,
        BUDGET_US / 1000.0
    );
    println!("Warmup: {WARMUP} blocks; measured: {BLOCKS} blocks per row.");
    println!("Persistent workers; static instance assignment; Linux sched_setaffinity.");
    println!("CPU order: {CPU_ORDER:?}");
    println!("Worker counts requested: {WORKERS:?}");
    println!(
        "NOTE: with only {INSTANCES} instances, 16 requested workers means only {INSTANCES} active DSP workers;"
    );
    println!("      the extra six cannot accelerate ten indivisible plugin instances.");
    println!();

    for workload in [Workload::Puppet, Workload::Jazz, Workload::Mixed] {
        println!("============================================================");
        println!("{}", workload.title());
        println!("============================================================");

        let mut rows = Vec::new();
        for &workers in WORKERS {
            let active = workers.min(INSTANCES);
            let cpus = &CPU_ORDER[..active];
            let s = run(workload, workers);

            println!(
                "{workers:>4} requested / {active:>2} active | CPUs {:<31} | mean {:8.1} us | p50 {:8.1} | p95 {:8.1} | p99 {:8.1} | max {:8.1}",
                format!("{cpus:?}"),
                s.mean, s.p50, s.p95, s.p99, s.max
            );
            println!(
                "                         >1ms {:4} | >1.2ms {:4} | misses {:4}/{BLOCKS} | longest {:4} | p99 headroom {:+8.1} us | checksum {:.6e}",
                s.over_1ms,
                s.over_12ms,
                s.misses,
                s.longest,
                BUDGET_US - s.p99,
                s.checksum
            );
            rows.push((workers, active, s));
        }

        println!("\n  Summary:");
        println!(
            "  {:>9} {:>8} {:>10} {:>10} {:>10} {:>10} {:>9}",
            "requested", "active", "mean us", "p95 us", "p99 us", "max us", "misses"
        );
        for (requested, active, s) in &rows {
            println!(
                "  {requested:>9} {active:>8} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>9}",
                s.mean, s.p95, s.p99, s.max, s.misses
            );
        }

        let best = rows
            .iter()
            .min_by(|a, b| a.2.p99.total_cmp(&b.2.p99))
            .unwrap();
        println!(
            "\n  Best p99: {} requested / {} active, {:.1} us ({:+.1} us headroom).\n",
            best.0,
            best.1,
            best.2.p99,
            BUDGET_US - best.2.p99
        );
    }

    println!("Interpretation:");
    println!("  * Explicit affinity removes worker migration as a major variable.");
    println!("  * 1/2/4/8 use distinct physical cores on this 8C/16T topology.");
    println!("  * 10 uses CPUs 0..7 plus SMT siblings 8 and 9.");
    println!("  * 16 is included deliberately, but ten plugin instances provide only ten");
    println!("    independent jobs here, so six workers would have no DSP work.");
    println!("  * If 10-affinity improves materially over the old 10-worker row, migration/");
    println!("    scheduling jitter was costing us. If not, DSP throughput remains dominant.");
}
