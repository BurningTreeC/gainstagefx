//! Reproducible CPU/deadline and allocator audit of the public DSP path.
//!
//! cargo run --release --example performance_audit -- steady
//! cargo run --release --example performance_audit -- changes
//! Optional positional arguments: mode, sample rate, block size, seconds.
//! This measures Chain::apply + DC sharing + per-channel dry/wet processing,
//! not the NIH-plug wrapper, parameter smoothers, GUI, or the audio driver.
//! Stimuli and result storage are allocated before measurement. Allocation
//! counts include alloc/alloc_zeroed/realloc calls; frees are counted separately.

use gainstagefx::dsp::time::enable_ftz_daz;
use gainstagefx::voice::{Amplifier, Cabinet, Chain, Gain, Iron, Settings, Tone};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

struct CountingAllocator;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

// The benchmark measures one thread at a time. Chain's initialization workers
// are joined before counting starts. No timing workers run concurrently.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.load(Relaxed) {
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn settings(gain: Gain) -> Settings {
    Settings {
        gain,
        amplifier: Amplifier::Valve,
        drive: 0.8,
        iron: Iron::Steel,
        tone: Tone::Scooping,
        cabinet: Cabinet::Stack,
        oversampling: 4,
        ..Settings::default()
    }
}

fn apply(chains: &mut [Chain], settings: &Settings) {
    let (first, rest) = chains.split_first_mut().unwrap();
    first.apply(settings);
    for c in rest.iter_mut() {
        c.apply(settings);
    }
    // Matches the current plugin's conditional sharing, including its limits.
    if first.needs_operating_point() {
        black_box(first.find_operating_point());
        for c in rest {
            c.share_operating_point_from(first.operating_point());
            if let Some(op) = first.iron_operating_point() {
                c.share_iron_operating_point_from(op);
            }
            if let Some(op) = first.power_operating_point() {
                c.share_power_operating_point_from(op);
            }
        }
    }
}

fn stimulus(k: usize, channel: usize, rate: f64) -> f64 {
    let t = k as f64 / rate;
    let phase = channel as f64 * 0.37;
    // Same hot plucked spectrum as deadline.rs, with distinct stereo phases.
    // Peak can exceed 1: this deliberately includes hot input, not just nominal.
    0.9 * (-(t % 0.5) * 6.0).exp()
        * ((std::f64::consts::TAU * 110.0 * t + phase).sin()
            + 0.6 * (std::f64::consts::TAU * 330.0 * t + phase).sin()
            + 0.3 * (std::f64::consts::TAU * 1757.0 * t + phase).sin())
}

fn frame(chains: &mut [Chain], input: [f64; 2]) {
    for (channel, chain) in chains.iter_mut().enumerate() {
        let x = if input[channel].abs() < 1e-4 {
            0.0
        } else {
            input[channel]
        };
        let dry = chain.delayed_dry(x);
        let wet = chain.process(x);
        black_box(0.25 * dry + 0.75 * wet);
    }
}

fn change(settings: &mut Settings, kind: &str, block: usize) {
    let position = 0.5 + 0.05 * (block as f64 * 0.13).sin();
    match kind {
        "drive" => settings.drive = position,
        "tone" => {
            settings.bass = position;
            settings.mid = position;
            settings.treble = position;
        }
        "master" => settings.master = position,
        "graphic" => settings.graphic = [position; 5],
        "reverb" => settings.reverb = position,
        "switch" => {
            settings.gain = if block % 2 == 0 {
                Gain::Twin
            } else {
                Gain::Boogie
            }
        }
        "quality" => settings.oversampling = if block % 2 == 0 { 1 } else { 4 },
        _ => {}
    }
}

fn run(gain: Gain, channels: usize, kind: &str, rate: f64, block: usize, seconds: f64) {
    let mut s = settings(gain);
    if kind == "effects" || kind == "reverb" {
        s.reverb = 0.5;
        s.intensity = 0.7;
    }
    let mut chains: Vec<_> = (0..channels).map(|_| Chain::new(rate)).collect();
    apply(&mut chains, &s);
    let warm = 4096;
    for k in 0..warm {
        frame(&mut chains, [stimulus(k, 0, rate), stimulus(k, 1, rate)]);
    }
    let blocks = ((seconds * rate / block as f64).ceil() as usize).max(1);
    let input: Vec<_> = (warm..warm + blocks * block)
        .map(|k| {
            if kind == "silence" {
                [0.0; 2]
            } else {
                [stimulus(k, 0, rate), stimulus(k, 1, rate)]
            }
        })
        .collect();
    let mut times = vec![0.0; blocks];
    let before: Vec<_> = chains.iter().map(Chain::solver_health).collect();
    ALLOCS.store(0, Relaxed);
    FREES.store(0, Relaxed);
    for (b, samples) in input.chunks_exact(block).enumerate() {
        change(&mut s, kind, b);
        COUNTING.store(true, Relaxed);
        let start = Instant::now();
        if kind == "reset" {
            for c in &mut chains {
                c.reset();
            }
        }
        apply(&mut chains, &s);
        for &x in samples {
            frame(&mut chains, x);
        }
        let elapsed = start.elapsed().as_secs_f64() * 1e6;
        COUNTING.store(false, Relaxed);
        times[b] = elapsed;
    }
    let allocations = ALLOCS.load(Relaxed);
    let frees = FREES.load(Relaxed);
    let (mut solves, mut passes, mut unsettled, mut backtracks) = (0, 0, 0, 0);
    // Active-voice counters cannot be compared across a circuit switch.
    for (c, before) in chains.iter().zip(before).filter(|_| kind != "switch") {
        let after = c.solver_health();
        solves += after.solves - before.solves;
        passes += after.passes - before.passes;
        unsettled += after.unsettled - before.unsettled;
        backtracks += after.backtracks - before.backtracks;
    }
    let deadline = block as f64 / rate * 1e6;
    let mean = times.iter().sum::<f64>() / blocks as f64;
    let missed = times.iter().filter(|&&x| x > deadline).count();
    times.sort_by(f64::total_cmp);
    let p99 = times[((blocks - 1) as f64 * 0.99).round() as usize];
    println!("{},{channels},{kind},{rate:.0},{block},{blocks},{},{mean:.2},{p99:.2},{:.2},{:.2},{:.2},{allocations},{frees},{:.3},{unsettled},{backtracks}",
        gain.name(), chains[0].effective_oversampling(), times[blocks-1],
        mean / deadline * 100.0, missed as f64 / blocks as f64 * 100.0,
        if kind == "switch" { f64::NAN } else { passes as f64 / solves.max(1) as f64 });
}

fn main() {
    enable_ftz_daz();
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("steady");
    let rate = args
        .get(2)
        .map(|s| s.parse::<f64>().expect("sample rate"))
        .unwrap_or(48_000.0);
    let block = args
        .get(3)
        .map(|s| s.parse::<usize>().expect("block size"))
        .unwrap_or(64);
    let seconds = args
        .get(4)
        .map(|s| s.parse::<f64>().expect("seconds"))
        .unwrap_or(1.0);
    assert!(rate.is_finite() && rate > 0.0 && block > 0 && seconds.is_finite() && seconds > 0.0);
    println!("voice,channels,case,rate,block,blocks,effective_os,mean_us,p99_us,max_us,cpu_percent,miss_percent,allocations,frees,passes_per_solve,unsettled,backtracks");
    match mode {
        "steady" => {
            for gain in Gain::ALL {
                for channels in [1, 2] {
                    run(gain, channels, "fixed", rate, block, seconds);
                }
            }
            run(Gain::Twin, 2, "effects", rate, block, seconds);
        }
        "changes" => {
            for (gain, kind) in [
                (Gain::Peavey, "drive"),
                (Gain::Boogie, "drive"),
                (Gain::Twin, "tone"),
                (Gain::Crunch, "tone"),
                (Gain::Boogie, "graphic"),
                (Gain::Boogie, "master"),
                (Gain::Twin, "reverb"),
                (Gain::Boogie, "switch"),
                (Gain::Crunch, "quality"),
                (Gain::Twin, "reset"),
            ] {
                run(gain, 2, kind, rate, block, seconds);
            }
        }
        "dense" => run(Gain::Boogie, 2, "graphic", rate, block, seconds),
        "silence" => {
            for gain in [Gain::Peavey, Gain::Boogie, Gain::Twin] {
                run(gain, 2, "silence", rate, block, seconds);
            }
        }
        _ => panic!("mode must be steady, changes, dense, or silence"),
    }
}
