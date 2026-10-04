//! What the reservoir buys, measured: shipped presets on the real take through
//! the whole plugin -- `GainStageFx::process`, the parameters, the reservoir
//! and its worker -- paced like a host at 64 samples / 48 kHz.
//!
//! Each run plays every instance for `--seconds` on a fixed schedule, one
//! period apart, and reports per callback: its time, whether it finished
//! inside the period and inside the 930 us the short cycle of a USB interface
//! leaves (`docs/realtime-multi-instance.md`); per reservoir: underruns,
//! overflows, the fill it read with and how far below the target it went,
//! the slack the worker left, and the worker's own time per segment; per
//! instance: the solver's work, and whether the output is the synchronous
//! plugin's (`--depths` including 0) delayed by the reservoir, to the bit.
//!
//! ```text
//! cargo run --release --example reservoir
//! cargo run --release --example reservoir -- --depths 0,64 --instances 1,2,4,8,16
//! cargo run --release --example reservoir -- --host serial --instances 4
//! cargo run --release --example reservoir -- --fifo 70 --preset "Jazz Chorus"
//! ```
//!
//! - `--depths` reservoir sizes in samples; 0 is the synchronous plugin as it
//!   was. Default 0,32,64,96,128.
//! - `--instances` how many plugins play at once. Default 1.
//! - `--host parallel` (default): one host thread per instance, each on its own
//!   schedule -- REAPER with live FX multiprocessing. `--host serial`: one
//!   thread calls every instance in turn each period, and the whole cycle has
//!   the deadline.
//! - `--fifo PRIORITY` runs the host threads `SCHED_FIFO`, as PipeWire runs
//!   REAPER's; the reservoir's worker adopts it. Needs an rtprio limit.
//! - `--preset NAME`, repeatable. Default: Jazz Chorus, Puppet Master '86 and
//!   Twin, Modern Purple -- the catalogue's hardest.
//! - `--seconds`, `--block`.
//!
//! Timing depends on the machine and on what else it is doing: run it on a
//! quiet one, and compare depths within a run rather than across runs.

use gainstagefx::params::GainStageParams;
use gainstagefx::plugin::GainStageFx;
use gainstagefx::presets;
use gainstagefx::voice::SolverHealth;
use nice_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

const RATE: f64 = 48_000.0;
const TAKE: &str = "tests/fixtures/01-260912_1044.wav";
const SHORT_CYCLE_US: f64 = 930.0;
const WARMUP_SECONDS: f64 = 0.5;

struct Options {
    presets: Vec<String>,
    depths: Vec<usize>,
    instances: Vec<usize>,
    serial: bool,
    fifo: Option<i32>,
    seconds: f64,
    block: usize,
}

fn options() -> Options {
    let mut o = Options {
        presets: Vec::new(),
        depths: vec![0, 32, 64, 96, 128],
        instances: vec![1],
        serial: false,
        fifo: None,
        seconds: 10.0,
        block: 64,
    };
    let list = |s: String| -> Vec<usize> {
        s.split(',')
            .map(|x| x.trim().parse().expect("a number"))
            .collect()
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--preset" => o.presets.push(value()),
            "--depths" => o.depths = list(value()),
            "--instances" => o.instances = list(value()),
            "--host" => o.serial = value() == "serial",
            "--fifo" => o.fifo = Some(value().parse().expect("a priority")),
            "--seconds" => o.seconds = value().parse().expect("seconds"),
            "--block" => o.block = value().parse().expect("a block size"),
            other => panic!("unknown option {other}"),
        }
    }
    if o.presets.is_empty() {
        o.presets = ["Jazz Chorus", "Puppet Master '86", "Twin, Modern Purple"]
            .map(String::from)
            .to_vec();
    }
    o
}

// ---------------------------------------------------------------------------
// A host
// ---------------------------------------------------------------------------

struct Host;

impl ActivateContext<GainStageFx> for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute(&self, _: ()) {}
    fn set_latency_samples(&self, _: u32) {}
    fn set_current_voice_capacity(&self, _: u32) {}
}

impl ProcessContext<GainStageFx> for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute_background(&self, _: ()) {}
    fn execute_gui(&self, _: ()) {}
    fn transport(&self) -> &Transport {
        unreachable!("GainStageFx does not read the transport")
    }
    fn next_event(&mut self) -> Option<PluginNoteEvent<GainStageFx>> {
        None
    }
    fn try_send_event(
        &mut self,
        event: PluginNoteEvent<GainStageFx>,
    ) -> Result<
        (),
        (
            PluginNoteEvent<GainStageFx>,
            nice_plug::context::process::SendEventError,
        ),
    > {
        Err((
            event,
            nice_plug::context::process::SendEventError::HostBufferFull,
        ))
    }
    fn set_latency_samples(&self, _: u32) {}
    fn set_current_voice_capacity(&self, _: u32) {}
    fn request_restart(&self) {}
}

/// A plugin with `preset` loaded the way a host restores a session, then
/// activated on a stereo bus carrying a mono guitar (REAPER's usual case).
fn instance(preset: &str, depth: usize, block: usize) -> GainStageFx {
    let stored = presets::load_all(&GainStageParams::default())
        .into_iter()
        .find(|p| p.name == preset)
        .unwrap_or_else(|| panic!("no preset named {preset}"));
    let mut plugin = GainStageFx::default();
    plugin.set_reservoir_delay(depth);
    let params = plugin.params();
    for (id, ptr, _) in params.param_map() {
        if let Some(&value) = stored.values.get(&id) {
            // SAFETY: the plugin is not processing; this is what nice-plug's
            // wrappers do when a host restores state.
            unsafe {
                ptr._internal_set_normalized_value(value);
                ptr._internal_update_smoother(RATE as f32, true);
            }
        }
    }
    assert!(plugin.activate(
        &GainStageFx::AUDIO_IO_LAYOUTS[0],
        &BufferConfig {
            sample_rate: RATE as f32,
            min_buffer_size: Some(1),
            max_buffer_size: block as u32,
            process_mode: ProcessMode::Realtime,
        },
        &mut Host,
    ));
    plugin.reset();
    plugin
}

// ---------------------------------------------------------------------------
// Playing
// ---------------------------------------------------------------------------

/// One instance's part in a run.
struct Player {
    plugin: GainStageFx,
    buffer: Buffer<'static>,
    left: Vec<f32>,
    right: Vec<f32>,
    input: Arc<Vec<f32>>,
    offset: usize,
    /// Callback times, ns, after the warm-up.
    times: Vec<u64>,
    /// How long after its due time each callback finished, ns.
    finished: Vec<u64>,
    output: Vec<f32>,
}

impl Player {
    fn new(
        plugin: GainStageFx,
        input: Arc<Vec<f32>>,
        offset: usize,
        block: usize,
        blocks: usize,
    ) -> Self {
        Self {
            plugin,
            buffer: Buffer::default(),
            left: vec![0.0; block],
            right: vec![0.0; block],
            input,
            offset,
            times: Vec::with_capacity(blocks),
            finished: Vec::with_capacity(blocks),
            output: Vec::with_capacity(blocks * block),
        }
    }

    /// Call `process` for block `k`; returns its duration.
    fn call(&mut self, k: usize, block: usize) -> Duration {
        let len = self.input.len();
        for i in 0..block {
            let x = self.input[(self.offset + k * block + i) % len];
            self.left[i] = x;
            self.right[i] = x;
        }
        // SAFETY: the two vectors are the player's own, disjoint, never
        // resized after construction, and outlive every use of the buffer,
        // which is only ever inside this call; the buffer's reference vector
        // is reused, so this allocates only the first time.
        unsafe {
            let left: &'static mut [f32] =
                std::slice::from_raw_parts_mut(self.left.as_mut_ptr(), block);
            let right: &'static mut [f32] =
                std::slice::from_raw_parts_mut(self.right.as_mut_ptr(), block);
            self.buffer.set_slices(block, |slices| {
                slices.clear();
                slices.push(left);
                slices.push(right);
            });
        }
        let mut aux = AuxiliaryBuffers {
            inputs: &mut [],
            outputs: &mut [],
        };
        let started = Instant::now();
        self.plugin.process(&mut self.buffer, &mut aux, &mut Host);
        started.elapsed()
    }
}

fn wait_until(due: Instant) {
    let now = Instant::now();
    if due > now + Duration::from_micros(80) {
        thread::sleep(due - now - Duration::from_micros(60));
    }
    while Instant::now() < due {
        std::hint::spin_loop();
    }
}

fn set_fifo(priority: i32) {
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct Param {
            priority: i32,
        }
        unsafe extern "C" {
            fn pthread_self() -> usize;
            fn pthread_setschedparam(thread: usize, policy: i32, param: *const Param) -> i32;
        }
        // SAFETY: a valid parameter block for the calling thread. SCHED_FIFO = 1.
        let result = unsafe { pthread_setschedparam(pthread_self(), 1, &Param { priority }) };
        assert_eq!(
            result, 0,
            "SCHED_FIFO {priority} refused: is there an rtprio limit?"
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = priority;
}

/// Play `players` for `blocks` (after the warm-up), one thread each or all on
/// one; returns them with their times filled in.
fn play(
    mut players: Vec<Player>,
    o: &Options,
    warmup: usize,
    blocks: usize,
) -> (Vec<Player>, Vec<u64>) {
    let block = o.block;
    let period = Duration::from_secs_f64(block as f64 / RATE);
    let fifo = o.fifo;
    let measure = move |player: &mut Player, k: usize, due: Instant| {
        let spent = player.call(k, block);
        if k == warmup {
            if let Some(stats) = player.plugin.reservoir_stats() {
                stats.clear();
            }
        }
        if k >= warmup {
            player.times.push(spent.as_nanos() as u64);
            player
                .finished
                .push(Instant::now().saturating_duration_since(due).as_nanos() as u64);
            player.output.extend_from_slice(&player.left[..block]);
        }
    };
    if o.serial {
        let handle = thread::spawn(move || {
            if let Some(priority) = fifo {
                set_fifo(priority);
            }
            let mut cycles = Vec::with_capacity(blocks);
            let start = Instant::now() + Duration::from_millis(5);
            for k in 0..warmup + blocks {
                let due = start + period * k as u32;
                wait_until(due);
                for player in players.iter_mut() {
                    measure(player, k, due);
                }
                if k >= warmup {
                    cycles.push(Instant::now().saturating_duration_since(due).as_nanos() as u64);
                }
            }
            (players, cycles)
        });
        handle.join().expect("the host thread")
    } else {
        let barrier = Arc::new(Barrier::new(players.len()));
        let start = Instant::now() + Duration::from_millis(20);
        let handles: Vec<_> = players
            .into_iter()
            .map(|mut player| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    if let Some(priority) = fifo {
                        set_fifo(priority);
                    }
                    barrier.wait();
                    for k in 0..warmup + blocks {
                        let due = start + period * k as u32;
                        wait_until(due);
                        measure(&mut player, k, due);
                    }
                    player
                })
            })
            .collect();
        (
            handles
                .into_iter()
                .map(|h| h.join().expect("a host thread"))
                .collect(),
            Vec::new(),
        )
    }
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn quantile(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[index] as f64 / 1_000.0
}

/// What the process has used, from /proc: CPU seconds, threads, context
/// switches. Linux only; zeros elsewhere.
fn process_usage() -> (f64, usize, u64) {
    let cpu = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| {
            let after = stat.rsplit_once(')')?.1;
            let fields: Vec<&str> = after.split_whitespace().collect();
            let utime: f64 = fields.get(11)?.parse().ok()?;
            let stime: f64 = fields.get(12)?.parse().ok()?;
            Some((utime + stime) / 100.0)
        })
        .unwrap_or(0.0);
    let threads = std::fs::read_dir("/proc/self/task")
        .map(|d| d.count())
        .unwrap_or(0);
    let switches = std::fs::read_to_string("/proc/self/status")
        .ok()
        .map(|status| {
            status
                .lines()
                .filter(|l| l.contains("ctxt_switches"))
                .filter_map(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
                .sum()
        })
        .unwrap_or(0);
    (cpu, threads, switches)
}

fn total(breakdown: &gainstagefx::voice::SolverBreakdown) -> SolverHealth {
    let mut sum = SolverHealth::default();
    for stage in [
        &breakdown.pedal,
        &breakdown.line,
        &breakdown.gain,
        &breakdown.power,
        &breakdown.iron,
        &breakdown.reverb_return,
    ] {
        sum.solves += stage.solves;
        sum.passes += stage.passes;
        sum.backtracks += stage.backtracks;
        sum.fallbacks += stage.fallbacks;
        sum.unsettled += stage.unsettled;
        sum.continuation_attempts += stage.continuation_attempts;
        sum.half_step_attempts += stage.half_step_attempts;
        sum.deadline_aborts += stage.deadline_aborts;
    }
    sum
}

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
                    (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                    _ => panic!("{path}: format {format}/{bits} bits not handled"),
                };
            }
            sum / channels as f64
        })
        .collect()
}

fn main() {
    let o = options();
    let take: Arc<Vec<f32>> = Arc::new(read_wav(TAKE).into_iter().map(|x| x as f32).collect());
    let period_us = o.block as f64 / RATE * 1e6;
    let warmup = (WARMUP_SECONDS * RATE / o.block as f64).ceil() as usize;
    let blocks = (o.seconds * RATE / o.block as f64).ceil() as usize;
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    println!(
        "# {} s at {}/{} Hz ({:.0} us period), host {}, {}; load average {}",
        o.seconds,
        o.block,
        RATE,
        period_us,
        if o.serial { "serial" } else { "parallel" },
        o.fifo.map_or("normal scheduling".to_string(), |p| format!(
            "SCHED_FIFO {p}"
        )),
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" "),
    );
    println!(
        "| preset | D | ms | inst | cb p50 | p99 | p99.9 | max | >930 | >period | cycle>period | underruns (frames) | ovf in/out | fill min/mean | <50% / <25% | slack p0.1 | worker p50/p99/max | wake p50/p99 | waits early/struct | aborts | backtracks | fallbacks | unsettled | vs D=0 | CPU cores | threads |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|");

    for preset in &o.presets {
        for &count in &o.instances {
            // The synchronous output, per instance, for the comparison.
            let mut reference: Vec<Vec<f32>> = Vec::new();
            for &depth in &o.depths {
                let players: Vec<Player> = (0..count)
                    .map(|i| {
                        let offset = (i * 54_321) % take.len();
                        Player::new(
                            instance(preset, depth, o.block),
                            Arc::clone(&take),
                            offset,
                            o.block,
                            blocks,
                        )
                    })
                    .collect();
                let (cpu0, _, switches0) = process_usage();
                let wall = Instant::now();
                let (mut players, cycles) = play(players, &o, warmup, blocks);
                let wall = wall.elapsed().as_secs_f64();
                let (cpu1, threads, switches1) = process_usage();
                let _ = switches1.saturating_sub(switches0);

                let mut times: Vec<u64> = players
                    .iter()
                    .flat_map(|p| p.times.iter().copied())
                    .collect();
                times.sort_unstable();
                let period_ns = (period_us * 1_000.0) as u64;
                let short = (SHORT_CYCLE_US * 1_000.0) as u64;
                let over_short = players
                    .iter()
                    .flat_map(|p| &p.finished)
                    .filter(|&&f| f > short)
                    .count();
                let over_period = players
                    .iter()
                    .flat_map(|p| &p.finished)
                    .filter(|&&f| f > period_ns)
                    .count();
                let cycle_over = cycles.iter().filter(|&&c| c > period_ns).count();

                let (mut underruns, mut underrun_frames, mut ovf_in, mut ovf_out) = (0, 0, 0, 0);
                let (mut fill_min, mut fill_mean, mut below_half, mut below_quarter) =
                    (u64::MAX, 0.0, 0, 0);
                let (mut slack, mut worker) = (f64::MAX, (0.0f64, 0.0f64, 0.0f64));
                let mut wake = (0.0f64, 0.0f64);
                let (mut early, mut structural) = (0, 0);
                for player in &players {
                    if let Some(stats) = player.plugin.reservoir_stats() {
                        let c = &stats.callback;
                        underruns += stats.underruns();
                        underrun_frames += c.underrun_frames.load(Ordering::Relaxed);
                        ovf_in += stats.input_overflows();
                        ovf_out += stats.output_overflows();
                        fill_min = fill_min.min(stats.fill_min().unwrap_or(u64::MAX));
                        fill_mean += stats.fill_mean() / count as f64;
                        below_half += c.low_water[1].load(Ordering::Relaxed);
                        below_quarter += c.low_water[2].load(Ordering::Relaxed);
                        slack = slack.min(c.slack.quantile(0.001) as f64 / 1_000.0);
                        let w = &stats.worker.segment;
                        worker.0 = worker.0.max(w.quantile(0.5) as f64 / 1_000.0);
                        worker.1 = worker.1.max(w.quantile(0.99) as f64 / 1_000.0);
                        worker.2 = worker.2.max(w.max() as f64 / 1_000.0);
                        let q = &stats.worker.queue;
                        wake.0 = wake.0.max(q.quantile(0.5) as f64 / 1_000.0);
                        wake.1 = wake.1.max(q.quantile(0.99) as f64 / 1_000.0);
                        early += c.early_waits.load(Ordering::Relaxed);
                        structural += c.structural_waits.load(Ordering::Relaxed);
                    }
                }

                // Solver work, from the engines brought back to this thread.
                let mut solver = SolverHealth::default();
                for player in players.iter_mut() {
                    player.plugin.deactivate();
                    if let Some(b) = player.plugin.engine().and_then(|e| e.solver_breakdown()) {
                        let t = total(&b);
                        solver.backtracks += t.backtracks;
                        solver.fallbacks += t.fallbacks;
                        solver.unsettled += t.unsettled;
                        solver.deadline_aborts += t.deadline_aborts;
                    }
                }

                // Against the synchronous run: this output, `depth` frames on,
                // must be that one.
                let comparison = if reference.is_empty() && depth == 0 {
                    reference = players.iter().map(|p| p.output.clone()).collect();
                    "reference".to_string()
                } else if reference.len() == players.len() {
                    let mut differing = 0usize;
                    let mut worst = 0.0f32;
                    for (player, reference) in players.iter().zip(&reference) {
                        // The warm-up shifts both runs' windows alike, so
                        // frame t here is frame t - depth there.
                        for t in depth..player.output.len() {
                            let d = (player.output[t] - reference[t - depth]).abs();
                            if d != 0.0 {
                                differing += 1;
                                worst = worst.max(d);
                            }
                        }
                    }
                    if differing == 0 {
                        "exact".to_string()
                    } else {
                        format!(
                            "{differing} differ, worst {:.1} dB",
                            20.0 * worst.max(1e-12).log10()
                        )
                    }
                } else {
                    "-".to_string()
                };

                let reservoir = depth > 0;
                let dash = |s: String| if reservoir { s } else { "-".to_string() };
                println!(
                    "| {preset} | {depth} | {:.3} | {count} | {:.0} | {:.0} | {:.0} | {:.0} | {over_short} | {over_period} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {comparison} | {:.2} | {threads} |",
                    depth as f64 / RATE * 1e3,
                    quantile(&times, 0.5),
                    quantile(&times, 0.99),
                    quantile(&times, 0.999),
                    quantile(&times, 1.0),
                    if o.serial { cycle_over.to_string() } else { "-".into() },
                    dash(format!("{underruns} ({underrun_frames})")),
                    dash(format!("{ovf_in}/{ovf_out}")),
                    dash(if fill_min == u64::MAX { "-".into() } else { format!("{fill_min}/{fill_mean:.1}") }),
                    dash(format!("{below_half}/{below_quarter}")),
                    dash(if slack == f64::MAX { "-".into() } else { format!("{slack:.0}") }),
                    dash(format!("{:.0}/{:.0}/{:.0}", worker.0, worker.1, worker.2)),
                    dash(format!("{:.0}/{:.0}", wake.0, wake.1)),
                    dash(format!("{early}/{structural}")),
                    solver.deadline_aborts,
                    solver.backtracks,
                    solver.fallbacks,
                    solver.unsettled,
                    (cpu1 - cpu0) / wall,
                );
            }
        }
    }
}
