//! Sub-block asynchronous pipeline benchmark for GainStageFx.
//!
//! Tests 2 and 10 independent "Puppet Master '86" instances with persistent
//! per-instance DSP workers and pipeline quanta of 1/2/4/8/16/32/64 samples.
//!
//! The host still delivers 64-sample callbacks. Each worker permanently owns
//! one Chain. The coordinator dispatches quantum N, then collects quantum N-1,
//! so the intentional pipeline latency is exactly one quantum.
//!
//! Run:
//!   cargo run --release --example async_quantum_deadline
//! Optional:
//!   taskset -c 0-15 cargo run --release --example async_quantum_deadline

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};
use std::cell::UnsafeCell;
use std::hint::{black_box, spin_loop};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const RATE: f64 = 48_000.0;
const HOST_BLOCK: usize = 64;
const HOST_US: f64 = HOST_BLOCK as f64 / RATE * 1_000_000.0;
const PRESET: &str = "Puppet Master '86";
const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4096;
const QUANTA: &[usize] = &[1, 2, 4, 8, 16, 32, 64];
const COUNTS: &[usize] = &[2, 10];

struct Exchange {
    input: UnsafeCell<[f64; HOST_BLOCK]>,
    output: UnsafeCell<[f64; HOST_BLOCK]>,
    input_seq: AtomicU64,
    input_consumed: AtomicU64,
    output_seq: AtomicU64,
    output_consumed: AtomicU64,
    stop: AtomicBool,
    deadline_aborts: AtomicU64,
}
unsafe impl Sync for Exchange {}

impl Exchange {
    fn new() -> Self {
        Self {
            input: UnsafeCell::new([0.0; HOST_BLOCK]),
            output: UnsafeCell::new([0.0; HOST_BLOCK]),
            input_seq: AtomicU64::new(0),
            input_consumed: AtomicU64::new(0),
            output_seq: AtomicU64::new(0),
            output_consumed: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            deadline_aborts: AtomicU64::new(0),
        }
    }
}

struct Worker {
    x: Arc<Exchange>,
    join: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn spawn(quantum: usize, phase: f64) -> Self {
        let x = Arc::new(Exchange::new());
        let wx = Arc::clone(&x);
        let join = thread::spawn(move || {
            let preset = PRESETS
                .iter()
                .find(|p| p.name == PRESET)
                .expect("preset missing");
            let mut chain = Chain::new(RATE);
            chain.apply(&preset.settings());
            chain.settle();
            chain.find_operating_point();
            // In the asynchronous design each quantum has to keep pace with its
            // own realtime cadence. Arm the existing protection for 90% of one
            // quantum before processing that quantum.
            let quantum_budget = Duration::from_secs_f64(quantum as f64 / RATE * 0.90);
            let mut wanted = 1u64;
            let mut checksum = phase * 0.0;
            while !wx.stop.load(Ordering::Acquire) {
                if wx.input_seq.load(Ordering::Acquire) != wanted {
                    spin_loop();
                    continue;
                }
                let mut local = [0.0; HOST_BLOCK];
                unsafe {
                    local[..quantum].copy_from_slice(&(&*wx.input.get())[..quantum]);
                }
                wx.input_consumed.store(wanted, Ordering::Release);

                chain.set_realtime_deadline(Some(Instant::now() + quantum_budget));
                for s in &mut local[..quantum] {
                    *s = chain.process(*s);
                    checksum += *s;
                }
                wx.deadline_aborts
                    .store(chain.deadline_aborts(), Ordering::Relaxed);

                while wx.output_consumed.load(Ordering::Acquire) + 1 != wanted {
                    if wx.stop.load(Ordering::Acquire) {
                        black_box(checksum);
                        return;
                    }
                    spin_loop();
                }
                unsafe {
                    (&mut *wx.output.get())[..quantum].copy_from_slice(&local[..quantum]);
                }
                wx.output_seq.store(wanted, Ordering::Release);
                wanted += 1;
            }
            black_box(checksum);
        });
        Self {
            x,
            join: Some(join),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.x.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[inline]
fn stimulus(sample: usize, amp: f64, phase: f64) -> f64 {
    let t = sample as f64 / RATE;
    amp * (0.50 * (std::f64::consts::TAU * 82.4 * t + phase).sin()
        + 0.30 * (std::f64::consts::TAU * 123.5 * t + phase * 0.7).sin()
        + 0.20 * (std::f64::consts::TAU * 246.9 * t + phase * 1.3).sin())
}

fn pct(v: &[f64], p: f64) -> f64 {
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn wait_eq(a: &AtomicU64, wanted: u64) {
    while a.load(Ordering::Acquire) != wanted {
        spin_loop();
    }
}

fn run(count: usize, quantum: usize) {
    assert_eq!(HOST_BLOCK % quantum, 0);
    let preset = PRESETS
        .iter()
        .find(|p| p.name == PRESET)
        .expect("preset missing");
    let amp = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);
    let workers: Vec<_> = (0..count)
        .map(|i| Worker::spawn(quantum, 0.17 * i as f64))
        .collect();
    let chunks = HOST_BLOCK / quantum;
    let total_blocks = WARMUP_BLOCKS + MEASURE_BLOCKS;
    let mut seq = 1u64;
    let mut sample = 0usize;
    let mut previous: Option<u64> = None;
    let mut times = Vec::with_capacity(MEASURE_BLOCKS);
    let mut checksum = 0.0;

    // Prime the one-quantum pipeline with silence as output latency.
    for block in 0..total_blocks {
        let timed = block >= WARMUP_BLOCKS;
        let start = Instant::now();
        for _ in 0..chunks {
            // Ensure each one-slot input mailbox is free, then dispatch this quantum.
            for (i, w) in workers.iter().enumerate() {
                if seq > 1 {
                    wait_eq(&w.x.input_consumed, seq - 1);
                }
                unsafe {
                    for j in 0..quantum {
                        (*w.x.input.get())[j] = stimulus(sample + j, amp, i as f64 * 0.17);
                    }
                }
                w.x.input_seq.store(seq, Ordering::Release);
            }

            // Collect the previous quantum while workers compute the current one.
            if let Some(prev) = previous {
                for w in &workers {
                    wait_eq(&w.x.output_seq, prev);
                    unsafe {
                        for &y in &(&*w.x.output.get())[..quantum] {
                            checksum += y;
                        }
                    }
                    w.x.output_consumed.store(prev, Ordering::Release);
                }
            }
            previous = Some(seq);
            seq += 1;
            sample += quantum;
        }
        if timed {
            times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        }
    }

    // Drain the final outstanding quantum outside measured host time.
    if let Some(prev) = previous {
        for w in &workers {
            wait_eq(&w.x.output_seq, prev);
            unsafe {
                for &y in &(&*w.x.output.get())[..quantum] {
                    checksum += y;
                }
            }
            w.x.output_consumed.store(prev, Ordering::Release);
        }
    }
    black_box(checksum);
    times.sort_by(|a, b| a.total_cmp(b));
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let p95 = pct(&times, 0.95);
    let p99 = pct(&times, 0.99);
    let p999 = pct(&times, 0.999);
    let max = *times.last().unwrap();
    let misses = times.iter().filter(|&&x| x > HOST_US).count();
    let q_us = quantum as f64 / RATE * 1_000_000.0;
    let aborts: u64 = workers
        .iter()
        .map(|w| w.x.deadline_aborts.load(Ordering::Relaxed))
        .sum();
    println!("{:>2} inst | q {:>2} ({:7.3} us latency) | mean {:8.1} | p95 {:8.1} | p99 {:8.1} | p99.9 {:8.1} | max {:8.1} | misses {:4}/{} | aborts {:7} | p99 headroom {:+8.1}",
        count, quantum, q_us, mean, p95, p99, p999, max, misses, MEASURE_BLOCKS, aborts, HOST_US-p99);
}

fn main() {
    println!("GainStageFx asynchronous sub-block quantum benchmark");
    println!("Preset: {PRESET:?}; 48 kHz / 64 = {HOST_US:.3} us host deadline.");
    println!("Persistent worker per plugin instance; one-quantum pipeline.");
    println!("Measured latency is quantum/RATE only; host callback remains 64 samples.");
    println!("Warmup {WARMUP_BLOCKS}, measured {MEASURE_BLOCKS} callbacks per row.\n");
    for &count in COUNTS {
        println!("================ {count} INDEPENDENT INSTANCES ================");
        for &q in QUANTA {
            run(count, q);
        }
        println!();
        // Let scheduler settle between instance-count groups.
        thread::sleep(Duration::from_millis(50));
    }
    println!("Interpretation:");
    println!("  * Primary pass criterion: 0 host misses at 1333.333 us.");
    println!("  * Prefer the smallest quantum that stays comfortably safe at p99/p99.9.");
    println!(
        "  * Added pipeline latency: 1 sample=20.833 us, 4=83.333 us, 8=166.667 us, 16=333.333 us."
    );
    println!("  * Very small quanta may lose to atomic/cache/scheduler handoff overhead.");
}
