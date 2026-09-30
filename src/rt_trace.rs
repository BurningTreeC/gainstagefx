//! Opt-in realtime telemetry: one fixed-size record per host callback.
//!
//! Armed by `GAINSTAGEFX_RT_TRACE=1` in the host's environment, written to
//! `GAINSTAGEFX_RT_TRACE_DIR` (the system temp directory otherwise) as
//! `gainstagefx-rt-<pid>-instance-<n>.csv`, one file per plugin instance, and
//! read by `tools/rt_trace.py`.
//!
//! The audio thread does three things and no more: reads the clock, fills a
//! `Copy` record, and pushes it into a preallocated single-producer ring. It
//! never allocates, locks, formats or touches a file. A writer thread, spawned
//! when the plugin instance is created and never from `process`, drains the
//! ring every 50 ms, formats rows and streams them to disk, so a capture
//! survives the host being killed and is not capped at a fixed length. If the
//! writer ever falls behind, records are dropped and counted, never waited for.
//!
//! What makes two instances' files comparable is the clock: every timestamp is
//! `CLOCK_MONOTONIC` in nanoseconds, the same clock in every thread and every
//! process on the machine, so `tools/rt_trace.py` can lay instances side by side
//! and say whether their callbacks overlapped, ran one after the other, or
//! neither. The thread id and CPU say where each one ran.

use std::cell::{Cell, UnsafeCell};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::params::GainStageParams;
use crate::voice::{
    ChainIdentity, PipelineUse, RealtimeStageTimings, SolverBreakdown, SolverHealth,
};

/// Records the ring holds: eleven seconds of 64-sample callbacks at 48 kHz,
/// against a writer that empties it twenty times a second.
const CAPACITY: usize = 8192;

static INSTANCES: AtomicUsize = AtomicUsize::new(1);

/// How the callback ran its channels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceMode {
    /// A one-channel layout.
    #[default]
    Mono,
    /// A stereo bus carrying one signal: one chain, both outputs.
    DualMono,
    /// Genuine stereo, the right chain on the persistent stereo worker.
    StereoParallel,
    /// Genuine stereo, both chains on the host thread.
    StereoSequential,
}

impl TraceMode {
    fn name(self) -> &'static str {
        match self {
            TraceMode::Mono => "mono",
            TraceMode::DualMono => "dual-mono",
            TraceMode::StereoParallel => "stereo-parallel",
            TraceMode::StereoSequential => "stereo-sequential",
        }
    }
}

/// One callback. Plain data: filled on the audio thread, formatted elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct TraceRecord {
    pub callback: u64,
    pub start_ns: u64,
    pub end_ns: u64,
    /// From the previous callback's start; zero on the first.
    pub interval_ns: u64,
    pub samples: u32,
    pub sample_rate: f32,
    pub tid: i32,
    pub cpu_start: i32,
    pub cpu_end: i32,
    pub mode: TraceMode,
    pub identity: Option<ChainIdentity>,
    pub oversampling: u32,
    /// The solver's wall-clock allowance from the callback's start; zero when
    /// none was armed (offline, or an empty block).
    pub budget_ns: u64,
    pub deadline_aborts: u32,
    /// Samples of the block in which some circuit hit the cutoff.
    pub abort_samples: u32,
    pub first_abort_sample: i32,
    pub stages: RealtimeStageTimings,
    /// Time the host thread spent waiting for the stereo worker.
    pub worker_wait_ns: u64,
    /// Where the chain's second half ran; see `Chain::process_block`.
    pub pipeline: PipelineUse,
    pub solver: SolverBreakdown,
    pub largest_output_delta: f32,
}

/// Single producer (the audio thread), single consumer (the writer).
struct Ring {
    slots: Box<[UnsafeCell<Option<TraceRecord>>]>,
    /// Next slot to write. Only the producer stores it.
    head: AtomicUsize,
    /// Next slot to read. Only the consumer stores it.
    tail: AtomicUsize,
    dropped: AtomicU64,
}

// SAFETY: a slot is written only by the producer while it is outside
// `[tail, head)` and read only by the consumer while it is inside it; the
// Release/Acquire pair on `head` and `tail` hands each slot across.
unsafe impl Sync for Ring {}

impl Ring {
    fn new() -> Self {
        Self {
            slots: (0..CAPACITY).map(|_| UnsafeCell::new(None)).collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicU64::new(0),
        }
    }

    /// Realtime side: never blocks, never allocates.
    fn push(&self, record: TraceRecord) {
        let head = self.head.load(Ordering::Relaxed);
        if head.wrapping_sub(self.tail.load(Ordering::Acquire)) >= CAPACITY {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        // SAFETY: see `unsafe impl Sync`; this slot is outside the readable range.
        unsafe { *self.slots[head % CAPACITY].get() = Some(record) };
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }

    fn pop(&self) -> Option<TraceRecord> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: see `unsafe impl Sync`; this slot is inside the readable range.
        let record = unsafe { (*self.slots[tail % CAPACITY].get()).take() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        record
    }
}

/// A plugin instance's trace: the producer's bookkeeping and its writer.
pub struct RtTrace {
    ring: Arc<Ring>,
    stop: Arc<AtomicBool>,
    writer: Option<JoinHandle<()>>,
    callback: u64,
    last_start_ns: u64,
    previous_output: Option<f64>,
}

impl RtTrace {
    /// A trace if the environment asks for one. Spawns the writer, so this
    /// belongs in the plugin's constructor, never on the audio thread.
    pub fn from_env(params: Arc<GainStageParams>) -> Option<Self> {
        let armed = std::env::var_os("GAINSTAGEFX_RT_TRACE")
            .is_some_and(|value| !value.is_empty() && value != "0");
        if !armed {
            return None;
        }
        let dir = std::env::var_os("GAINSTAGEFX_RT_TRACE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Self::to_dir(&dir, params).map(|(trace, _)| trace)
    }

    /// A trace writing into `dir`, and the file it writes. What `from_env`
    /// does once it has decided to trace; tests call it directly.
    pub fn to_dir(
        dir: &std::path::Path,
        params: Arc<GainStageParams>,
    ) -> Option<(Self, std::path::PathBuf)> {
        let instance = INSTANCES.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(
            "gainstagefx-rt-{}-instance-{instance}.csv",
            std::process::id()
        ));
        let ring = Arc::new(Ring::new());
        let stop = Arc::new(AtomicBool::new(false));
        let writer = {
            let (ring, stop, path) = (ring.clone(), stop.clone(), path.clone());
            std::thread::Builder::new()
                .name(format!("gsfx-trace-{instance}"))
                .spawn(move || write_rows(&path, instance, &ring, &stop, &params))
                .ok()?
        };
        let trace = Self {
            ring,
            stop,
            writer: Some(writer),
            callback: 0,
            last_start_ns: 0,
            previous_output: None,
        };
        Some((trace, path))
    }

    /// The callback's opening clock read, its thread and CPU.
    #[inline]
    pub fn begin(&self) -> (u64, i32, i32) {
        (monotonic_ns(), thread_id(), current_cpu())
    }

    /// Track the largest step between consecutive output samples.
    #[inline]
    pub fn observe_output(&mut self, out: f64, largest: &mut f64) {
        if let Some(previous) = self.previous_output {
            *largest = largest.max((out - previous).abs());
        }
        self.previous_output = Some(out);
    }

    /// Stamp the end of the callback and hand the record to the writer.
    #[inline]
    pub fn finish(&mut self, mut record: TraceRecord) {
        record.end_ns = monotonic_ns();
        record.cpu_end = current_cpu();
        record.callback = self.callback;
        record.interval_ns = if self.last_start_ns == 0 {
            0
        } else {
            record.start_ns.saturating_sub(self.last_start_ns)
        };
        self.last_start_ns = record.start_ns;
        self.callback += 1;
        self.ring.push(record);
    }
}

impl Drop for RtTrace {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

const HEADER: &str = "instance,callback,start_ns,end_ns,callback_interval_us,process_us,\
samples,sample_rate,tid,cpu_start,cpu_end,mode,voice,pedal,power,radiating,oversampling,\
budget_us,deadline_aborts,abort_samples,first_abort_sample,abort_stages,\
pedal_us,gain_us,power_us,iron_us,tone_us,cabinet_us,other_us,worker_wait_us,pipeline,\
pedal_passes,gain_passes,power_passes,iron_passes,\
pedal_backtracks,gain_backtracks,power_backtracks,iron_backtracks,\
pedal_fallbacks,gain_fallbacks,power_fallbacks,iron_fallbacks,\
pedal_unsettled,gain_unsettled,power_unsettled,iron_unsettled,\
largest_output_delta,ring_dropped,preset";

fn write_rows(
    path: &std::path::Path,
    instance: usize,
    ring: &Ring,
    stop: &AtomicBool,
    params: &GainStageParams,
) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(file) = std::fs::File::create(path) else {
        // Keep draining so the audio thread's ring never fills and so the
        // `dropped` counter keeps meaning "the writer fell behind".
        while !stop.load(Ordering::Acquire) {
            while ring.pop().is_some() {}
            std::thread::sleep(Duration::from_millis(50));
        }
        return;
    };
    let mut out = std::io::BufWriter::new(file);
    let _ = writeln!(out, "{HEADER}");
    let mut preset = String::new();
    loop {
        let stopping = stop.load(Ordering::Acquire);
        // The preset name lives behind the editor's mutex. This thread may
        // wait on it; the audio thread never touches it.
        if let Ok(name) = params.preset_name.try_lock() {
            if *name != preset {
                preset.clone_from(&name);
            }
        }
        let mut wrote = false;
        while let Some(record) = ring.pop() {
            let _ = write_row(&mut out, instance, &record, ring, &preset);
            wrote = true;
        }
        if wrote {
            let _ = out.flush();
        }
        if stopping {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = out.flush();
}

fn write_row(
    out: &mut impl Write,
    instance: usize,
    r: &TraceRecord,
    ring: &Ring,
    preset: &str,
) -> std::io::Result<()> {
    let us = |ns: u64| ns as f64 / 1000.0;
    let process_ns = r.end_ns.saturating_sub(r.start_ns);
    let s = &r.stages;
    let staged = s.pedal_ns + s.gain_ns + s.power_ns + s.iron_ns + s.tone_ns + s.cabinet_ns;
    let (voice, pedal, power, radiating) = match r.identity {
        Some(id) => (
            format!("{:?}", id.voice),
            format!("{:?}", id.pedal),
            id.power
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "none".into()),
            id.radiating,
        ),
        None => ("none".into(), "none".into(), "none".into(), false),
    };
    let stage = |h: &SolverHealth, bit: u32| if h.deadline_aborts > 0 { bit } else { 0 };
    let v = &r.solver;
    let abort_stages = stage(&v.pedal, 1)
        | stage(&v.gain, 2)
        | stage(&v.power, 4)
        | stage(&v.iron, 8)
        | stage(&v.line, 16);
    write!(
        out,
        "{instance},{},{},{},{:.3},{:.3},{},{},{},{},{},{},{voice},{pedal},{power},{},{},{:.3},{},{},{},{},",
        r.callback,
        r.start_ns,
        r.end_ns,
        us(r.interval_ns),
        us(process_ns),
        r.samples,
        r.sample_rate,
        r.tid,
        r.cpu_start,
        r.cpu_end,
        r.mode.name(),
        u8::from(radiating),
        r.oversampling,
        us(r.budget_ns),
        r.deadline_aborts,
        r.abort_samples,
        r.first_abort_sample,
        abort_stages,
    )?;
    let pipeline = match r.pipeline {
        PipelineUse::Serial => "serial",
        PipelineUse::Worker => "worker",
        PipelineUse::Reclaimed => "reclaimed",
        PipelineUse::Shared => "shared",
    };
    write!(
        out,
        "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{pipeline},",
        us(s.pedal_ns),
        us(s.gain_ns),
        us(s.power_ns),
        us(s.iron_ns),
        us(s.tone_ns),
        us(s.cabinet_ns),
        us(process_ns.saturating_sub(staged)),
        us(r.worker_wait_ns),
    )?;
    let gain = |f: fn(&SolverHealth) -> u64| f(&v.gain) + f(&v.line);
    writeln!(
        out,
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.9},{},{}",
        v.pedal.passes,
        gain(|h| h.passes),
        v.power.passes,
        v.iron.passes,
        v.pedal.backtracks,
        gain(|h| h.backtracks),
        v.power.backtracks,
        v.iron.backtracks,
        v.pedal.fallbacks,
        gain(|h| h.fallbacks),
        v.power.fallbacks,
        v.iron.fallbacks,
        v.pedal.unsettled,
        gain(|h| h.unsettled),
        v.power.unsettled,
        v.iron.unsettled,
        r.largest_output_delta,
        ring.dropped.load(Ordering::Relaxed),
        csv_field(preset),
    )
}

/// Quote a free-text field if it needs it.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

thread_local! {
    static TID: Cell<i32> = const { Cell::new(0) };
}

/// The kernel's id for the calling thread, looked up once per thread.
#[inline]
fn thread_id() -> i32 {
    TID.with(|tid| {
        if tid.get() == 0 {
            tid.set(sys::gettid());
        }
        tid.get()
    })
}

#[inline]
fn current_cpu() -> i32 {
    sys::sched_getcpu()
}

/// `CLOCK_MONOTONIC`, in nanoseconds.
#[inline]
pub fn monotonic_ns() -> u64 {
    sys::monotonic_ns()
}

#[cfg(target_os = "linux")]
mod sys {
    use std::os::raw::{c_int, c_long};

    #[repr(C)]
    struct Timespec {
        tv_sec: c_long,
        tv_nsec: c_long,
    }

    unsafe extern "C" {
        fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
        #[link_name = "sched_getcpu"]
        fn sched_getcpu_c() -> c_int;
        #[link_name = "gettid"]
        fn gettid_c() -> c_int;
    }

    const CLOCK_MONOTONIC: c_int = 1;

    #[inline]
    pub fn monotonic_ns() -> u64 {
        let mut time = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: a valid, writable timespec and a clock every Linux has.
        if unsafe { clock_gettime(CLOCK_MONOTONIC, &mut time) } != 0 {
            return 0;
        }
        (time.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(time.tv_nsec as u64)
    }

    #[inline]
    pub fn sched_getcpu() -> c_int {
        // SAFETY: no arguments; -1 on failure.
        unsafe { sched_getcpu_c() }
    }

    #[inline]
    pub fn gettid() -> c_int {
        // SAFETY: no arguments; cannot fail.
        unsafe { gettid_c() }
    }
}

#[cfg(not(target_os = "linux"))]
mod sys {
    use std::sync::OnceLock;
    use std::time::Instant;

    static EPOCH: OnceLock<Instant> = OnceLock::new();

    /// Monotonic within this process only: enough to compare instances that
    /// share a host process, which is the common case.
    #[inline]
    pub fn monotonic_ns() -> u64 {
        EPOCH.get_or_init(Instant::now).elapsed().as_nanos() as u64
    }

    #[inline]
    pub fn sched_getcpu() -> i32 {
        -1
    }

    #[inline]
    pub fn gettid() -> i32 {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(callback: u64) -> TraceRecord {
        TraceRecord {
            callback,
            start_ns: 0,
            end_ns: 0,
            interval_ns: 0,
            samples: 64,
            sample_rate: 48_000.0,
            tid: 0,
            cpu_start: 0,
            cpu_end: 0,
            mode: TraceMode::Mono,
            identity: None,
            oversampling: 1,
            budget_ns: 0,
            deadline_aborts: 0,
            abort_samples: 0,
            first_abort_sample: -1,
            stages: RealtimeStageTimings::default(),
            worker_wait_ns: 0,
            pipeline: PipelineUse::Serial,
            solver: SolverBreakdown::default(),
            largest_output_delta: 0.0,
        }
    }

    /// Full is full: the producer drops and counts rather than overwriting a
    /// record the writer has not read, and everything it kept comes out in
    /// order.
    #[test]
    fn a_full_ring_drops_new_records_and_keeps_order() {
        let ring = Ring::new();
        for i in 0..CAPACITY as u64 + 5 {
            ring.push(record(i));
        }
        assert_eq!(ring.dropped.load(Ordering::Relaxed), 5);
        for i in 0..CAPACITY as u64 {
            assert_eq!(ring.pop().map(|r| r.callback), Some(i));
        }
        assert!(ring.pop().is_none());
        ring.push(record(99));
        assert_eq!(ring.pop().map(|r| r.callback), Some(99));
    }

    /// The clock the files are compared on moves forward, in nanoseconds.
    #[test]
    fn the_trace_clock_is_monotonic_nanoseconds() {
        let a = monotonic_ns();
        std::thread::sleep(Duration::from_millis(2));
        let b = monotonic_ns();
        assert!(b > a && b - a >= 1_000_000, "{a} -> {b}");
    }
}
