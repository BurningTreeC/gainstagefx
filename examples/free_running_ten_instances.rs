//! Free-running, explicitly pinned 10-instance realtime benchmark.
//!
//! Each thread permanently owns one `Chain`, has its own absolute 1.333333 ms
//! schedule, and never waits for another plugin instance after startup.
//!
//! This deliberately has NO per-period barrier, worker pool, job queue, Chain
//! migration, or per-period thread creation. It is intended to answer whether
//! ten independent GainStageFx instances can each meet their own host deadline.
//!
//! Linux only.
//!
//! Run:
//!   cargo run --release --example free_running_ten_instances
//!   taskset -c 0-15 cargo run --release --example free_running_ten_instances

#![cfg(target_os = "linux")]

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::Chain;
use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const WARMUP: usize = 256;
const BLOCKS: usize = 4096;
const INSTANCES: usize = 10;
const PERIOD: Duration = Duration::from_nanos(1_333_333);
const BUDGET_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const PRESET: &str = "Puppet Master '86";

// Target topology reported by lscpu:
// CPUs 0..7 are distinct physical cores; 8 and 9 are SMT siblings of 0 and 1.
const CPUS: [usize; INSTANCES] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

#[repr(C)]
struct CpuSet {
    bits: [u64; 16],
}

unsafe extern "C" {
    fn sched_setaffinity(pid: c_int, cpusetsize: usize, mask: *const CpuSet) -> c_int;
}

fn pin_current_thread(cpu: usize) {
    let mut set = CpuSet { bits: [0; 16] };
    set.bits[cpu / 64] |= 1u64 << (cpu % 64);
    let rc = unsafe { sched_setaffinity(0, std::mem::size_of::<CpuSet>(), &set) };
    if rc != 0 {
        panic!(
            "sched_setaffinity(cpu={cpu}) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

fn preset() -> &'static gainstagefx::presets::Preset {
    PRESETS
        .iter()
        .find(|p| p.name == PRESET)
        .unwrap_or_else(|| panic!("preset {PRESET:?} not found"))
}

fn signal(instance: usize, block: usize, sample: usize) -> f64 {
    let n = block * BLOCK + sample;
    let t = n as f64 / RATE;
    let f0 = 82.406_889 * (1.0 + instance as f64 * 0.0075);
    let env = 0.55 + 0.45 * (std::f64::consts::TAU * 1.7 * t).sin().abs();
    0.11 * env
        * ((std::f64::consts::TAU * f0 * t).sin()
            + 0.42 * (std::f64::consts::TAU * f0 * 2.01 * t).sin()
            + 0.20 * (std::f64::consts::TAU * f0 * 3.02 * t).sin())
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

fn wait_until(deadline: Instant) {
    // Do not busy-spin for the whole idle interval: instances 8/9 share physical
    // cores with 0/1. Sleep most of the interval, then use a very short spin to
    // reduce wakeup error without artificially consuming an SMT sibling's CPU.
    const SPIN: Duration = Duration::from_micros(20);
    loop {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        let left = deadline.duration_since(now);
        if left > SPIN + Duration::from_micros(30) {
            thread::sleep(left - SPIN);
        } else {
            std::hint::spin_loop();
        }
    }
}

#[derive(Debug)]
struct ResultRow {
    id: usize,
    cpu: usize,
    process_us: Vec<f64>,
    finish_late_us: Vec<f64>,
    start_late_us: Vec<f64>,
    misses: usize,
    longest_miss_run: usize,
    checksum: f64,
}

fn main() {
    println!("GainStageFx free-running ten-instance realtime benchmark");
    println!("Preset: {PRESET:?}");
    println!("48 kHz / 64 samples = {BUDGET_US:.3} us per independent instance deadline.");
    println!("Each persistent thread owns one Chain and one absolute periodic schedule.");
    println!("NO per-period barriers, queues, worker pools, or cross-instance waiting.");
    println!("CPU mapping: {CPUS:?} (8/9 are SMT siblings of physical cores 0/1).");
    println!("Warmup: {WARMUP}; measured: {BLOCKS} periods per instance.\n");

    let go = Arc::new(AtomicBool::new(false));
    let epoch = Instant::now() + Duration::from_millis(250);
    let (tx, rx) = mpsc::channel::<ResultRow>();
    let mut handles = Vec::new();

    for (id, &cpu) in CPUS.iter().enumerate() {
        let tx = tx.clone();
        let go = Arc::clone(&go);
        handles.push(thread::spawn(move || {
            pin_current_thread(cpu);
            let mut chain = Chain::new(RATE);
            chain.apply(&preset().settings());
            let mut checksum = 0.0;

            while !go.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }

            // Warm up before the measured common epoch. Warmup is deliberately
            // not paced: it heats code/data and solver state without extending
            // the benchmark by hundreds of milliseconds.
            for b in 0..WARMUP {
                for s in 0..BLOCK {
                    checksum += chain.process(signal(id, b, s));
                }
            }

            wait_until(epoch);
            let mut process_us = Vec::with_capacity(BLOCKS);
            let mut start_late_us = Vec::with_capacity(BLOCKS);
            let mut finish_late_us = Vec::with_capacity(BLOCKS);
            let mut misses = 0usize;
            let mut run = 0usize;
            let mut longest = 0usize;

            for b in 0..BLOCKS {
                let scheduled_start = epoch + PERIOD * (b as u32);
                let deadline = scheduled_start + PERIOD;
                wait_until(scheduled_start);
                let actual_start = Instant::now();
                let start_late = actual_start
                    .saturating_duration_since(scheduled_start)
                    .as_secs_f64()
                    * 1e6;

                let t0 = actual_start;
                for s in 0..BLOCK {
                    checksum += chain.process(signal(id, WARMUP + b, s));
                }
                let finished = Instant::now();
                let elapsed = finished.duration_since(t0).as_secs_f64() * 1e6;
                let finish_late = if finished > deadline {
                    finished.duration_since(deadline).as_secs_f64() * 1e6
                } else {
                    -deadline.duration_since(finished).as_secs_f64() * 1e6
                };

                process_us.push(elapsed);
                start_late_us.push(start_late);
                finish_late_us.push(finish_late);
                if finished > deadline {
                    misses += 1;
                    run += 1;
                    longest = longest.max(run);
                } else {
                    run = 0;
                }
            }

            tx.send(ResultRow {
                id,
                cpu,
                process_us,
                finish_late_us,
                start_late_us,
                misses,
                longest_miss_run: longest,
                checksum,
            })
            .unwrap();
        }));
    }
    drop(tx);
    go.store(true, Ordering::Release);

    let mut rows: Vec<_> = rx.into_iter().collect();
    rows.sort_by_key(|r| r.id);
    for h in handles {
        h.join().unwrap();
    }

    println!("================ PER INSTANCE ================");
    println!(
        " id cpu    mean     p95     p99   p99.9     max | start99  misses longest | worst finish"
    );

    let mut total_misses = 0usize;
    let mut worst_p99 = 0.0f64;
    let mut worst_p999 = 0.0f64;
    let mut worst_max = 0.0f64;
    let mut all_safe = true;

    for r in &mut rows {
        r.process_us.sort_by(|a, b| a.total_cmp(b));
        r.start_late_us.sort_by(|a, b| a.total_cmp(b));
        let mean = r.process_us.iter().sum::<f64>() / r.process_us.len() as f64;
        let p95 = percentile(&r.process_us, 0.95);
        let p99 = percentile(&r.process_us, 0.99);
        let p999 = percentile(&r.process_us, 0.999);
        let max = *r.process_us.last().unwrap();
        let start99 = percentile(&r.start_late_us, 0.99);
        let worst_finish = r
            .finish_late_us
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        println!(
            "{:3} {:3} {:7.1} {:7.1} {:7.1} {:7.1} {:7.1} | {:7.1} {:7} {:7} | {:+11.1} us",
            r.id,
            r.cpu,
            mean,
            p95,
            p99,
            p999,
            max,
            start99,
            r.misses,
            r.longest_miss_run,
            worst_finish
        );
        println!("        checksum {:.6e}", r.checksum);
        total_misses += r.misses;
        worst_p99 = worst_p99.max(p99);
        worst_p999 = worst_p999.max(p999);
        worst_max = worst_max.max(max);
        all_safe &= r.misses == 0;
    }

    println!("\n================ SUMMARY ================");
    println!("hard deadline              : {BUDGET_US:.3} us");
    println!(
        "worst per-instance p99     : {worst_p99:.3} us ({:+.3} us headroom)",
        BUDGET_US - worst_p99
    );
    println!(
        "worst per-instance p99.9   : {worst_p999:.3} us ({:+.3} us headroom)",
        BUDGET_US - worst_p999
    );
    println!(
        "worst processing maximum   : {worst_max:.3} us ({:+.3} us headroom)",
        BUDGET_US - worst_max
    );
    println!(
        "aggregate deadline misses  : {total_misses} / {} instance-periods",
        INSTANCES * BLOCKS
    );
    println!(
        "result                     : {}",
        if all_safe {
            "PASS: every instance met every deadline"
        } else {
            "FAIL: one or more independent instances missed deadlines"
        }
    );
    println!("\nInterpretation:");
    println!("  * There is intentionally no 'whole host period' barrier result here.");
    println!("    Each instance succeeds or fails only against its own periodic deadline.");
    println!("  * start99 exposes Linux wake/scheduling jitter separately from DSP time.");
    println!("  * CPUs 8/9 share physical cores with 0/1; compare those four rows closely.");
    println!("  * If CPUs 0..7 are safe but 8/9 fail, eight physical cores are the likely limit");
    println!("    for ten copies of this exact heavy preset on this CPU at 64 samples.");
    println!("  * Deadline protection remains whatever the current Chain implementation enables;");
    println!("    this benchmark does not disable or override it.");
}
