//! The REAPER scenario, on the real take: per-callback cost and solver work.
//!
//! Plays a DI recording through shipped presets exactly as the plugin does on a
//! stereo track carrying a mono guitar -- the dual-mono path, one `Chain`,
//! `process_stereo` -- in 64-sample callbacks at 48 kHz, and reports each
//! preset's callback-time tail beside the machine-independent counts that
//! explain it: Newton passes, line-search backtracks, fallbacks, unsettled
//! samples and deadline aborts, per stage.
//!
//! It also prints a hash of every output sample's bits. An optimisation that
//! claims to be exact must leave that hash unchanged; one that does not claim
//! it can write the output with `--write` and be null-tested against it.
//!
//! Two deadlines matter and they are not the same number. The host period is
//! 1333 us, but on a USB interface that delivers audio in 0.5 ms packets, a
//! 64-frame PipeWire quantum wakes at 1.5, 1.5, 1.0 ms: one cycle in three is
//! a millisecond long. Measured in REAPER on 2026-09-29, a callback running
//! into that short cycle past ~930 us was an xrun every time (see
//! `tools/rt_trace.py`). So the report counts callbacks over 930 us as well as
//! over the full period.
//!
//! ```text
//! cargo run --release --example rt_scenario
//! cargo run --release --example rt_scenario -- --preset "Puppet Master '86" --seconds 60
//! cargo run --release --example rt_scenario -- --gain-db 6 --deadline --paced
//! cargo run --release --example rt_scenario -- --write /tmp/out.f64
//! cargo run --release --example rt_scenario -- --all --seconds 4   # every preset's hash
//! ```
//!
//! `--deadline` arms the solver's wall-clock cutoff the way the plugin does
//! (`REALTIME_CUTOFF_PERIODS` from the callback's start); without it the run is
//! deterministic and the hash is comparable across machines and builds.
//! `--cut-from N --cut-every M` instead forces the cutoff, already passed, on
//! samples N.. of every Mth block, sample by sample: what an abandoned solve
//! does to the sound, measured against a run without `--cut-from`.
//! `--block` runs each callback through `Chain::process_block` instead of
//! sample by sample, and `--pipeline` gives it a `StageWorker`; the two must
//! print the same output hash. So must the default, sample by sample, unless
//! a block speculated on its power stage's second half (`SpecBlock` in
//! `voice.rs`), which the per-sample path never does; `--no-speculation`
//! turns that off. `--helper` runs the blocks serially with a `StageWorker`
//! that only takes the power stage's shadow, which is what the plugin does
//! while it is not pipelining. `--block-size` sets the callback length.
//! `--events` prints every block in which a solve ended unsettled or fell back.
//! `--callbacks FILE` writes one CSV row per callback: its time, where the
//! pipeline ran the second half, each stage's solver work, the input's peak
//! and, with `--stages`, each stage's time -- which is what says what the
//! slowest callbacks have in common.
//! `--oversampling N` overrides the preset's factor (the voice's cap still
//! applies; the header line prints the factor that ran).
//! `--paced` waits out each callback's period like a host instead of running
//! blocks back to back; it takes as long as the audio it plays. `--stages`
//! arms the per-stage clocks the realtime trace uses and prints where each
//! callback's time went (the clocks themselves cost ~10 reads a sample).

use gainstagefx::presets::PRESETS;
use gainstagefx::stage_worker::StageWorker;
use gainstagefx::voice::PipelineUse;
use gainstagefx::voice::{Chain, SolverBreakdown, SolverHealth};
use std::hint::black_box;
use std::io::Write;
use std::time::{Duration, Instant};

const RATE: f64 = 48_000.0;
const DEFAULT_TAKE: &str = "tests/fixtures/01-260912_1044.wav";
/// The short USB cycle's budget for the plugin, measured in REAPER.
const SHORT_CYCLE_US: f64 = 930.0;

struct Options {
    presets: Vec<String>,
    take: String,
    gain_db: f64,
    block_size: usize,
    seconds: f64,
    deadline: bool,
    paced: bool,
    write: Option<String>,
    stages: bool,
    block: bool,
    pipeline: bool,
    cut_from: Option<usize>,
    cut_every: usize,
    events: bool,
    oversampling: Option<usize>,
    callbacks: Option<String>,
    speculation: bool,
    helper: bool,
}

impl Options {
    fn block_len(&self) -> usize {
        self.block_size
    }
}

fn options() -> Options {
    let mut o = Options {
        presets: Vec::new(),
        take: DEFAULT_TAKE.into(),
        gain_db: 0.0,
        block_size: 64,
        seconds: 0.0,
        deadline: false,
        paced: false,
        write: None,
        stages: false,
        block: false,
        pipeline: false,
        cut_from: None,
        cut_every: 1,
        events: false,
        oversampling: None,
        callbacks: None,
        speculation: true,
        helper: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--preset" => o.presets.push(value()),
            "--wav" => o.take = value(),
            "--gain-db" => o.gain_db = value().parse().expect("--gain-db takes dB"),
            "--block-size" => o.block_size = value().parse().expect("--block-size takes samples"),
            "--seconds" => o.seconds = value().parse().expect("--seconds takes seconds"),
            "--deadline" => o.deadline = true,
            "--paced" => o.paced = true,
            "--write" => o.write = Some(value()),
            "--stages" => o.stages = true,
            "--block" => o.block = true,
            "--pipeline" => o.pipeline = true,
            "--cut-from" => {
                o.cut_from = Some(value().parse().expect("--cut-from takes a sample index"))
            }
            "--cut-every" => {
                o.cut_every = value().parse().expect("--cut-every takes a block count")
            }
            "--events" => o.events = true,
            "--no-speculation" => o.speculation = false,
            "--helper" => {
                o.block = true;
                o.helper = true;
            }
            "--callbacks" => o.callbacks = Some(value()),
            "--oversampling" => {
                o.oversampling = Some(value().parse().expect("--oversampling takes 1, 2, 4 or 8"))
            }
            "--all" => o.presets = PRESETS.iter().map(|p| p.name.to_string()).collect(),
            other => panic!("unknown argument {other}"),
        }
    }
    if o.presets.is_empty() {
        o.presets = vec!["Puppet Master '86".into(), "Jazz Chorus".into()];
    }
    o
}

/// PCM 16/24/32 or float 32, summed to mono, at its recorded level: the take's
/// own level is the realistic one, so unlike `stutter` it is not normalised.
fn read_wav(path: &str) -> Vec<f64> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(
        &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "{path} is not RIFF/WAVE"
    );
    let u16at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (mut format, mut channels, mut rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    let mut data = None;
    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let size = u32at(at + 4) as usize;
        let body = at + 8;
        match &bytes[at..at + 4] {
            b"fmt " => {
                format = u16at(body);
                channels = u16at(body + 2);
                rate = u32at(body + 4);
                bits = u16at(body + 14);
                if format == 0xFFFE {
                    format = u16at(body + 24);
                }
            }
            b"data" => data = Some((body, size.min(bytes.len() - body))),
            _ => {}
        }
        at = body + size + (size & 1);
    }
    assert_eq!(rate as f64, RATE, "{path} must be {RATE} Hz");
    let (start, size) = data.expect("no data chunk");
    let step = (bits / 8) as usize;
    let frame = step * channels as usize;
    (0..size / frame)
        .map(|f| {
            let mut sum = 0.0;
            for c in 0..channels as usize {
                let i = start + f * frame + c * step;
                let b = &bytes[i..i + step];
                sum += match (format, bits) {
                    (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f64 / 32_768.0,
                    (1, 24) => {
                        (((b[0] as i32) | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8)
                            as f64
                            / 8_388_608.0
                    }
                    (1, 32) => {
                        i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64 / 2_147_483_648.0
                    }
                    (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                    _ => panic!("{path}: format {format}/{bits} bits not handled"),
                };
            }
            sum / channels as f64
        })
        .collect()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn row(name: &str, h: SolverHealth, solves: u64) {
    if h.solves == 0 {
        return;
    }
    println!(
        "  {name:<6} passes/solve {:6.3}  backtracks {:>9}  fallbacks {:>7}  unsettled {:>6}  ({} solves)",
        h.passes as f64 / h.solves as f64,
        h.backtracks,
        h.fallbacks,
        h.unsettled,
        solves
    );
    if h.half_step_attempts > 0 {
        println!(
            "  {name:<6} half-step rescue: {} failed solves, {} bridged, {} settled at the full step",
            h.half_step_attempts, h.half_step_bridged, h.half_step_rescued
        );
    }
}

fn main() {
    let o = options();
    let take = read_wav(&o.take);
    let length = if o.seconds > 0.0 {
        (o.seconds * RATE) as usize
    } else {
        take.len()
    };
    let blocks = length / o.block_len();
    let period = o.block_len() as f64 / RATE;
    println!(
        "{} ({:.1} s), {:+.1} dB, {} samples at {RATE} Hz = {:.1} us per callback, {} callbacks{}{}",
        o.take,
        take.len() as f64 / RATE,
        o.gain_db,
        o.block_len(),
        period * 1e6,
        blocks,
        if o.deadline { ", deadline armed" } else { "" },
        if o.paced { ", paced" } else { "" },
    );
    let mut callbacks = o.callbacks.as_ref().map(|p| {
        let mut file = std::fs::File::create(p).expect("callbacks file");
        writeln!(
            file,
            "preset,block,us,pipeline,input_peak,pedal_passes,gain_passes,power_passes,\
             power_backtracks,power_fallbacks,power_unsettled,power_rescues,\
             pedal_us,gain_us,power_us,iron_us,tone_us,cabinet_us,speculated"
        )
        .unwrap();
        file
    });
    let mut out = o
        .write
        .as_ref()
        .map(|p| std::fs::File::create(p).expect("output file"));

    for name in &o.presets {
        let preset = PRESETS
            .iter()
            .find(|p| p.name == name.as_str())
            .unwrap_or_else(|| panic!("no preset named {name:?}"));
        let scale = 10f64.powf((preset.input_trim as f64 + o.gain_db) / 20.0);
        let mut chain = Chain::new(RATE);
        chain.apply(&preset.settings());
        if let Some(factor) = o.oversampling {
            chain.set_oversampling(factor);
        }
        chain.settle();
        chain.find_operating_point();
        chain.set_realtime_stage_timing(o.stages);
        chain.set_speculation(o.speculation);
        let stages_before = chain.realtime_stage_timings();

        let before: SolverBreakdown = chain.solver_breakdown();
        let mut last = before;
        let aborts_before = chain.deadline_aborts();
        let mut times = Vec::with_capacity(blocks);
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut samples = Vec::with_capacity(o.block_len());
        let worker = (o.pipeline || o.helper).then(StageWorker::new);
        let (mut inputs, mut right) = (vec![0.0; o.block_len()], vec![0.0; o.block_len()]);
        let mut uses = [0usize; 4];
        let (mut callback_health, mut callback_stages) = (before, stages_before);
        let mut callback_speculated = chain.speculated_blocks();
        let mut next = Instant::now();
        for b in 0..blocks {
            if o.paced {
                while Instant::now() < next {
                    std::hint::spin_loop();
                }
                next += Duration::from_secs_f64(period);
            }
            let started = Instant::now();
            chain.set_realtime_deadline(o.deadline.then(|| {
                started
                    + Duration::from_secs_f64(period * gainstagefx::plugin::REALTIME_CUTOFF_PERIODS)
            }));
            samples.clear();
            let mut last_use = 0;
            if o.block || o.pipeline {
                for (k, x) in inputs.iter_mut().enumerate() {
                    *x = take[(b * o.block_len() + k) % take.len()] * scale;
                }
                samples.resize(o.block_len(), 0.0);
                let used = chain.process_block(
                    &inputs,
                    &mut samples,
                    &mut right,
                    true,
                    worker.as_ref(),
                    o.pipeline,
                );
                last_use = match used {
                    PipelineUse::Serial => 0,
                    PipelineUse::Worker => 1,
                    PipelineUse::Reclaimed => 2,
                    PipelineUse::Shared => 3,
                };
                uses[last_use] += 1;
            } else {
                for k in 0..o.block_len() {
                    if let Some(from) = o.cut_from {
                        // An already-passed cutoff: deterministic, unlike a real one.
                        let cut = k >= from && b % o.cut_every == 0;
                        chain.set_realtime_deadline(cut.then(|| started - Duration::from_secs(1)));
                    }
                    let x = take[(b * o.block_len() + k) % take.len()] * scale;
                    samples.push(chain.process_stereo(x).0);
                }
            }
            times.push(started.elapsed().as_secs_f64() * 1e6);
            if let Some(file) = callbacks.as_mut() {
                let health = chain.solver_breakdown();
                let stages = chain.realtime_stage_timings();
                let (h, t) = (
                    health.saturating_delta(callback_health),
                    stages.delta(callback_stages),
                );
                let peak = (0..o.block_len())
                    .map(|k| (take[(b * o.block_len() + k) % take.len()] * scale).abs())
                    .fold(0.0, f64::max);
                let us = |ns: u64| ns as f64 / 1000.0;
                let speculated = chain.speculated_blocks() - callback_speculated;
                callback_speculated = chain.speculated_blocks();
                writeln!(
                    file,
                    "{name:?},{b},{:.1},{last_use},{peak:.4},{},{},{},{},{},{},{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{speculated}",
                    times[b],
                    h.pedal.passes,
                    h.gain.passes,
                    h.power.passes,
                    h.power.backtracks,
                    h.power.fallbacks,
                    h.power.unsettled,
                    h.power.half_step_attempts,
                    us(t.pedal_ns),
                    us(t.gain_ns),
                    us(t.power_ns),
                    us(t.iron_ns),
                    us(t.tone_ns),
                    us(t.cabinet_ns),
                )
                .unwrap();
                (callback_health, callback_stages) = (health, stages);
            }
            if o.events {
                let now = chain.solver_breakdown();
                for (stage, h, was) in [
                    ("pedal", now.pedal, last.pedal),
                    ("gain", now.gain, last.gain),
                    ("power", now.power, last.power),
                ] {
                    let unsettled = h.unsettled.saturating_sub(was.unsettled);
                    let fallbacks = h.fallbacks.saturating_sub(was.fallbacks);
                    if unsettled > 0 || fallbacks > 0 {
                        println!(
                            "  event block {b} (samples {}..{}): {stage} unsettled {unsettled} fallbacks {fallbacks}",
                            b * o.block_len(),
                            (b + 1) * o.block_len()
                        );
                    }
                }
                last = now;
            }
            for &y in &samples {
                hash = (hash ^ black_box(y).to_bits()).wrapping_mul(0x0100_0000_01b3);
            }
            if let Some(file) = out.as_mut() {
                for &y in &samples {
                    file.write_all(&y.to_le_bytes()).unwrap();
                }
            }
        }
        let h = chain.solver_breakdown().saturating_delta(before);
        let aborts = chain.deadline_aborts() - aborts_before;
        let mut sorted = times.clone();
        sorted.sort_by(f64::total_cmp);
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        println!("\n{name}");
        println!(
            "  callback us: mean {mean:6.1}  p50 {:6.1}  p95 {:6.1}  p99 {:6.1}  p99.9 {:6.1}  max {:6.1}",
            percentile(&sorted, 0.50),
            percentile(&sorted, 0.95),
            percentile(&sorted, 0.99),
            percentile(&sorted, 0.999),
            sorted[sorted.len() - 1],
        );
        println!(
            "  over {SHORT_CYCLE_US:.0} us: {}   over {:.0} us: {}   deadline aborts: {aborts}",
            times.iter().filter(|&&t| t > SHORT_CYCLE_US).count(),
            period * 1e6,
            times.iter().filter(|&&t| t > period * 1e6).count(),
        );
        row("pedal", h.pedal, h.pedal.solves);
        row("gain", h.gain, h.gain.solves);
        row("power", h.power, h.power.solves);
        row("iron", h.iron, h.iron.solves);
        println!("  output hash {hash:016x}");
        if o.pipeline {
            println!(
                "  second half on the worker in {} blocks, shared with the caller in {}, reclaimed by the caller in {}",
                uses[1], uses[3], uses[2]
            );
        }
        if o.block || o.pipeline {
            let (shadow_passes, starts) = chain.speculation_work();
            println!(
                "  power stage's second half speculated in {} blocks; {} solves waited for the shadow; shadow passes {shadow_passes}, starts used {starts}; front/power work {:.2}",
                chain.speculated_blocks(),
                chain.speculation_waits(),
                chain.speculation_gate()
            );
        }
        if o.stages {
            let t = chain.realtime_stage_timings().delta(stages_before);
            let per = |ns: u64| ns as f64 / 1000.0 / blocks as f64;
            println!(
                "  stage us per callback: pedal {:.1}  gain {:.1}  iron {:.1}  power {:.1}  tone {:.1}  cabinet {:.1}  (sum {:.1} of {mean:.1})",
                per(t.pedal_ns),
                per(t.gain_ns),
                per(t.iron_ns),
                per(t.power_ns),
                per(t.tone_ns),
                per(t.cabinet_ns),
                per(t.pedal_ns + t.gain_ns + t.iron_ns + t.power_ns + t.tone_ns + t.cabinet_ns),
            );
        }
    }
}
