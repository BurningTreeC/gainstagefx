//! A computational reservoir: the DSP runs on its own sequential worker a
//! fixed number of samples behind the host, so that a hard stretch of solving
//! costs the worker time it has rather than the host callback time it lacks.
//!
//! # Why reported latency alone is not headroom
//!
//! A host callback has until the next one to return. The nonlinear solvers do
//! not cost the same every sample: a clip transition can take ten times the
//! passes of the samples round it, and in REAPER every such callback that ran
//! past the next driver wake was an xrun costing 8-10 periods of everyone's
//! audio (`docs/realtime-multi-instance.md`). Telling the host the plugin has
//! 64 samples of latency changes none of that by itself. What changes it is
//! *delaying the audio by those 64 samples* and spending the delay: the
//! callback hands the worker this block's input and takes output the worker
//! finished earlier, and the worker -- which owns every piece of DSP state --
//! has until that output is due to produce it.
//!
//! # The timeline
//!
//! Within one *epoch* (a run between resets) host frames are numbered from
//! zero in the order the host delivers them, across callbacks of any length,
//! split or not. With a reservoir of `D` samples:
//!
//! ```text
//! output frame t  =  silence                      for t < D   (priming)
//!                 =  process(input frame t - D)   for t >= D
//! ```
//!
//! exactly, whatever the block sizes, because both sides count frames and
//! nothing else. The worker processes input frames strictly in order, one
//! segment (one host `process` call) at a time, with that call's own controls,
//! so the DSP sees the sequence it would have seen synchronously, `D` samples
//! later. Queue capacities are larger than `D` and have nothing to do with the
//! delay; nothing here infers latency from a container's size.
//!
//! How much time that buys depends on the block size `N` against `D`. Output
//! frame `t` needs input `t - D`, which arrived in the callback holding it, so
//! the first frame of a block has `D` samples' worth of time from its arrival
//! and the last `D - N + 1`. Processing in whole segments, a block of `N` must
//! be finished `D - N` samples after the callback that delivered it ends, plus
//! one callback: at `N == D` that is one full period, from the moment the
//! (now cheap) callback hands it over until the next callback needs it -- the
//! deadline has left the host's graph for the worker's own thread, but it has
//! not grown. At `N < D` the worker can be `(D - N) / N` callbacks late and
//! nobody hears it. At `N > D` some of a callback's own output depends on its
//! own input; see *Waiting*.
//!
//! # What the callback does
//!
//! Push this call's frames and controls into fixed-capacity SPSC rings
//! (`rtrb`), wake the worker, copy the output frames the worker has published
//! out of an index-addressed ring, update a few counters, return. It never
//! allocates, locks, logs or touches a file. Output frame `t` sits at slot
//! `(t - D) mod C` of a ring of `C > D + N_max` frames, and the worker
//! publishes how many frames of the current epoch it has written with one
//! `Release` store; the callback reads only below that count. The worker can
//! only ever be processing input the callback has already pushed, so it is
//! never more than `D + N_max` frames ahead of what the callback still reads,
//! and with `C` above that it can never overwrite a slot the callback is about
//! to read. (`Stats::output_overflows` checks it anyway.)
//!
//! # Late output
//!
//! A frame the worker has not finished when it is due is *concealed*: the last
//! output decays toward silence with a 0.5 ms time constant, and the real
//! signal is crossfaded back in over 32 samples when it returns. The late
//! frames, when they arrive, are skipped: alignment never slips, so the delay
//! stays exactly `D` and an underrun cannot quietly become latency. Each one
//! is counted (`underrun_events`, `underrun_frames`). The worker keeps
//! processing every input in order -- the circuits' history must stay
//! sequential -- and catches up on cheaper samples. If it falls so far behind
//! that the input ring fills (`input_overflows`), the stale input is
//! abandoned: a new epoch starts, the circuits keep their state, and the
//! output is concealed and re-primed.
//!
//! # Waiting
//!
//! In real time the callback never waits for audio a *previous* callback
//! delivered. Two cases are different, because the alternative to waiting is
//! wrong output rather than a missed deadline:
//!
//! - **The host is not running in real time.** An offline bounce, or a host
//!   rendering ahead (REAPER's anticipative FX processing, which CLAP still
//!   reports as realtime), calls `process` back to back. No worker can keep
//!   up with that, and none needs to: nothing is due yet. `Pace` measures how
//!   far ahead of a real-time pace the host's calls are, and a late frame is
//!   waited for only up to that lead, less a margin wider than real-time
//!   driver jitter. An offline render always waits.
//! - **The block is longer than the reservoir** (`N > D`). Then the last
//!   `N - D` frames of this call's output are made from the first `N - D` of
//!   its input, and no amount of earlier work could have produced them. The
//!   worker finishes and publishes those first (`Timing::first`), and the
//!   callback waits for them alone, bounded in real time by the block's own
//!   duration; the other `D` are processed behind it and absorb spikes as
//!   any reservoir does. That wait is the least any design can have at this
//!   latency: a reservoir at least as long as the host's block removes it.
//!
//! Every wait is counted with its reason.
//!
//! # What this cannot do
//!
//! It smooths jitter. It does not create throughput: a worker that needs
//! 1.5 ms for every 1.333 ms of audio empties any reservoir and stays empty.
//! `Stats` shows which is happening: a busy, recovering reservoir has a low
//! minimum fill and no underruns; an overloaded one has a stream of them.

use std::cell::UnsafeCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};

/// The most channels a reservoir carries. The plugin's layouts have one or two.
pub const MAX_CHANNELS: usize = 2;

/// What the worker runs: a sequential DSP that processes one host call's
/// frames at a time, in place, with that call's controls.
pub trait Segments: Send + 'static {
    /// Everything a call reads from the host besides audio, captured on the
    /// host thread when the call happened.
    type Controls: Copy + Send + 'static;
    /// What a reset needs, captured on the host thread when it happened.
    type Reset: Copy + Send + 'static;

    /// Process `channels` in place. `publish(channels, n)` may be called
    /// whenever the first `n` frames are final, so that a host waiting for
    /// them (`Timing::first`) gets them before the rest are done; whatever is
    /// not published by the return is published then.
    fn process(
        &mut self,
        channels: &mut [&mut [f32]],
        controls: &Self::Controls,
        timing: &Timing,
        publish: &mut dyn FnMut(&[&mut [f32]], usize),
    );
    fn reset(&mut self, reset: &Self::Reset);
}

/// When a segment's audio is due, for a processor that holds itself to it.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// When a host keeping real time would have made this call. The call's
    /// own time in real time; later than it while the host renders ahead.
    pub due: Instant,
    /// How much later than `due` the segment's first frame is played.
    pub delay: Duration,
    /// False when there is no deadline at all: an offline render.
    pub realtime: bool,
    /// Frames the host is waiting for from this segment, at its start: the
    /// segment's length, except when the host's block is longer than the
    /// reservoir, when it is `len - delay` and the rest can follow later.
    pub first: usize,
}

/// How a reservoir is built. `delay` is the latency it adds, in host frames,
/// and need not be a power of two.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub delay: usize,
    pub channels: usize,
    pub max_block: usize,
    pub sample_rate: f64,
    /// An offline render: every late frame is waited for.
    pub offline: bool,
}

impl Config {
    /// Output ring frames: above `delay + max_block`, which is what the
    /// timeline needs, with room for one more block.
    fn output_capacity(&self) -> usize {
        (self.delay + 2 * self.max_block + 64).next_power_of_two()
    }

    /// Input ring frames: how far behind the worker may fall before its stale
    /// input is abandoned. Four reservoirs-and-blocks, at least 1024: at
    /// 64/48k that is 21 ms of concealed audio at the very worst.
    fn input_capacity(&self) -> usize {
        (4 * (self.delay + self.max_block))
            .max(1024)
            .next_power_of_two()
    }

    /// Queued calls. A call is at least a frame, so this is the input ring's
    /// length in calls, capped: a host automating every sample of a 64-frame
    /// block makes 64 calls a period.
    fn header_capacity(&self) -> usize {
        self.input_capacity().min(512)
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// A log-linear histogram of nanoseconds: eight buckets an octave, about 9 %
/// wide. One thread records; any thread reads.
pub struct Histogram {
    counts: Box<[AtomicU64]>,
    max: AtomicU64,
    sum: AtomicU64,
}

const SUB_BUCKETS: u64 = 8;
const BUCKETS: usize = 8 + 61 * 8;

impl Histogram {
    fn new() -> Self {
        Self {
            counts: (0..BUCKETS).map(|_| AtomicU64::new(0)).collect(),
            max: AtomicU64::new(0),
            sum: AtomicU64::new(0),
        }
    }

    fn bucket(value: u64) -> usize {
        if value < SUB_BUCKETS {
            return value as usize;
        }
        let exponent = 63 - value.leading_zeros() as u64;
        let mantissa = (value >> (exponent - 3)) & (SUB_BUCKETS - 1);
        (SUB_BUCKETS + (exponent - 3) * SUB_BUCKETS + mantissa) as usize
    }

    /// The smallest value in bucket `index`.
    fn floor(index: usize) -> u64 {
        let index = index as u64;
        if index < SUB_BUCKETS {
            return index;
        }
        let exponent = 3 + (index - SUB_BUCKETS) / SUB_BUCKETS;
        let mantissa = (index - SUB_BUCKETS) % SUB_BUCKETS;
        (SUB_BUCKETS + mantissa) << (exponent - 3)
    }

    /// Single-writer record. Relaxed: these are counters, not synchronisation.
    #[inline]
    fn record(&self, value: u64) {
        let count = &self.counts[Self::bucket(value).min(BUCKETS - 1)];
        count.store(count.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
        self.sum
            .store(self.sum.load(Ordering::Relaxed) + value, Ordering::Relaxed);
        if value > self.max.load(Ordering::Relaxed) {
            self.max.store(value, Ordering::Relaxed);
        }
    }

    pub fn count(&self) -> u64 {
        self.counts.iter().map(|c| c.load(Ordering::Relaxed)).sum()
    }

    pub fn max(&self) -> u64 {
        self.max.load(Ordering::Relaxed)
    }

    pub fn mean(&self) -> f64 {
        let count = self.count();
        if count == 0 {
            0.0
        } else {
            self.sum.load(Ordering::Relaxed) as f64 / count as f64
        }
    }

    /// The upper edge of the bucket holding the `p`-quantile (0..=1): never
    /// below the true quantile, at most a bucket above it.
    pub fn quantile(&self, p: f64) -> u64 {
        let counts: Vec<u64> = self
            .counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
        let total: u64 = counts.iter().sum();
        if total == 0 {
            return 0;
        }
        let target = ((p.clamp(0.0, 1.0) * total as f64).ceil() as u64).max(1);
        let mut seen = 0;
        for (index, count) in counts.iter().enumerate() {
            seen += count;
            if seen >= target {
                let upper = Self::floor(index + 1).saturating_sub(1);
                return upper.min(self.max());
            }
        }
        self.max()
    }

    fn clear(&self) {
        for count in self.counts.iter() {
            count.store(0, Ordering::Relaxed);
        }
        self.max.store(0, Ordering::Relaxed);
        self.sum.store(0, Ordering::Relaxed);
    }
}

/// What a reservoir has done, readable from any thread while it runs. The
/// callback's counters and the worker's are written by one thread each and
/// kept on separate cache lines.
pub struct Stats {
    pub callback: CallbackStats,
    pub worker: WorkerStats,
}

#[repr(align(64))]
pub struct CallbackStats {
    pub callbacks: AtomicU64,
    pub frames: AtomicU64,
    /// Calls in which at least one due frame was not ready and was concealed.
    pub underrun_events: AtomicU64,
    pub underrun_frames: AtomicU64,
    /// Calls whose input did not fit: the worker was a whole input ring behind.
    pub input_overflows: AtomicU64,
    pub resets: AtomicU64,
    /// Calls that waited, by reason.
    pub structural_waits: AtomicU64,
    pub early_waits: AtomicU64,
    pub offline_waits: AtomicU64,
    /// Waits that ran out of time and concealed anyway.
    pub waits_expired: AtomicU64,
    /// Frames ready, beyond what this call needed first, when it read: the
    /// reservoir's fill. `fill_min` excludes priming.
    pub fill_last: AtomicU64,
    pub fill_min: AtomicU64,
    pub fill_max: AtomicU64,
    pub fill_sum: AtomicU64,
    pub fill_count: AtomicU64,
    /// Calls that read with the fill under 3/4, 1/2, 1/4 and 1/8 of the delay.
    pub low_water: [AtomicU64; 4],
    pub priming_remaining: AtomicU64,
    /// How long before a call needed them the worker had finished its frames:
    /// the margin the reservoir actually had, counting frames ready beyond
    /// the call's needs as the time they represent. Recorded only when
    /// everything due was ready.
    pub slack: Histogram,
    /// How long calls waited.
    pub wait: Histogram,
}

#[repr(align(64))]
pub struct WorkerStats {
    pub segments: AtomicU64,
    pub frames: AtomicU64,
    /// Segments from an abandoned epoch, skipped unprocessed.
    pub stale_segments: AtomicU64,
    /// Frames the worker would have had to write over unread output. Zero by
    /// construction; counted so that a broken invariant is seen, not heard.
    pub output_overflows: AtomicU64,
    /// Headers whose frames were not in the input ring. Zero by construction.
    pub protocol_errors: AtomicU64,
    /// Time to process one segment, and per frame of it.
    pub segment: Histogram,
    pub per_frame: Histogram,
    /// From the callback pushing a segment to the worker starting it: wake-up
    /// latency plus any queue ahead of it.
    pub queue: Histogram,
    pub alive: AtomicBool,
    pub panicked: AtomicBool,
}

impl Stats {
    fn new() -> Self {
        Self {
            callback: CallbackStats {
                callbacks: AtomicU64::new(0),
                frames: AtomicU64::new(0),
                underrun_events: AtomicU64::new(0),
                underrun_frames: AtomicU64::new(0),
                input_overflows: AtomicU64::new(0),
                resets: AtomicU64::new(0),
                structural_waits: AtomicU64::new(0),
                early_waits: AtomicU64::new(0),
                offline_waits: AtomicU64::new(0),
                waits_expired: AtomicU64::new(0),
                fill_last: AtomicU64::new(0),
                fill_min: AtomicU64::new(u64::MAX),
                fill_max: AtomicU64::new(0),
                fill_sum: AtomicU64::new(0),
                fill_count: AtomicU64::new(0),
                low_water: [
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                ],
                priming_remaining: AtomicU64::new(0),
                slack: Histogram::new(),
                wait: Histogram::new(),
            },
            worker: WorkerStats {
                segments: AtomicU64::new(0),
                frames: AtomicU64::new(0),
                stale_segments: AtomicU64::new(0),
                output_overflows: AtomicU64::new(0),
                protocol_errors: AtomicU64::new(0),
                segment: Histogram::new(),
                per_frame: Histogram::new(),
                queue: Histogram::new(),
                alive: AtomicBool::new(true),
                panicked: AtomicBool::new(false),
            },
        }
    }

    /// Forget everything measured so far (a benchmark's warm-up). Racy against
    /// a running callback or worker only in the sense that a count in flight
    /// may land on either side.
    pub fn clear(&self) {
        let c = &self.callback;
        for counter in [
            &c.callbacks,
            &c.frames,
            &c.underrun_events,
            &c.underrun_frames,
            &c.input_overflows,
            &c.resets,
            &c.structural_waits,
            &c.early_waits,
            &c.offline_waits,
            &c.waits_expired,
            &c.fill_max,
            &c.fill_sum,
            &c.fill_count,
        ] {
            counter.store(0, Ordering::Relaxed);
        }
        c.fill_min.store(u64::MAX, Ordering::Relaxed);
        for counter in &c.low_water {
            counter.store(0, Ordering::Relaxed);
        }
        c.slack.clear();
        c.wait.clear();
        let w = &self.worker;
        for counter in [
            &w.segments,
            &w.frames,
            &w.stale_segments,
            &w.output_overflows,
            &w.protocol_errors,
        ] {
            counter.store(0, Ordering::Relaxed);
        }
        w.segment.clear();
        w.per_frame.clear();
        w.queue.clear();
    }

    /// The lowest fill a call read with, outside priming; `None` before any.
    pub fn fill_min(&self) -> Option<u64> {
        let min = self.callback.fill_min.load(Ordering::Relaxed);
        (min != u64::MAX).then_some(min)
    }

    pub fn fill_mean(&self) -> f64 {
        let count = self.callback.fill_count.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            self.callback.fill_sum.load(Ordering::Relaxed) as f64 / count as f64
        }
    }

    pub fn underruns(&self) -> u64 {
        self.callback.underrun_events.load(Ordering::Relaxed)
    }

    pub fn input_overflows(&self) -> u64 {
        self.callback.input_overflows.load(Ordering::Relaxed)
    }

    pub fn output_overflows(&self) -> u64 {
        self.worker.output_overflows.load(Ordering::Relaxed)
    }
}

#[inline]
fn add(counter: &AtomicU64, value: u64) {
    counter.store(counter.load(Ordering::Relaxed) + value, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Bits of a published count that carry the epoch; the rest carry frames.
const EPOCH_SHIFT: u32 = 40;
const EPOCH_MASK: u32 = (1 << (64 - EPOCH_SHIFT)) - 1;
const COUNT_MASK: u64 = (1 << EPOCH_SHIFT) - 1;

#[inline]
fn pack(epoch: u32, count: u64) -> u64 {
    ((epoch & EPOCH_MASK) as u64) << EPOCH_SHIFT | (count & COUNT_MASK)
}

#[inline]
fn unpack(value: u64) -> (u32, u64) {
    ((value >> EPOCH_SHIFT) as u32, value & COUNT_MASK)
}

/// Whether epoch `a` is older than `b`, across wrap-around.
#[inline]
fn older(a: u32, b: u32) -> bool {
    let difference = b.wrapping_sub(a) & EPOCH_MASK;
    difference != 0 && difference < EPOCH_MASK / 2
}

/// Output frames by timeline index. One writer (the worker) and one reader
/// (the callback), never on the same slot at once: see the module
/// documentation for why the index arithmetic guarantees that.
struct OutputRing {
    slots: Box<[UnsafeCell<f32>]>,
    mask: usize,
    channels: usize,
}

// SAFETY: access is partitioned by the publish protocol: the worker writes a
// slot before publishing its index with Release, the callback reads only
// published indices after an Acquire load, and the worker cannot reach a slot
// again until the callback is past it.
unsafe impl Sync for OutputRing {}

impl OutputRing {
    fn new(frames: usize, channels: usize) -> Self {
        debug_assert!(frames.is_power_of_two());
        Self {
            slots: (0..frames * channels)
                .map(|_| UnsafeCell::new(0.0))
                .collect(),
            mask: frames - 1,
            channels,
        }
    }

    #[inline]
    fn slot(&self, index: u64, channel: usize) -> *mut f32 {
        self.slots[(index as usize & self.mask) * self.channels + channel].get()
    }
}

struct Shared {
    /// The callback's epoch; the worker skips segments from older ones.
    epoch: AtomicU32,
    /// `pack(epoch, frames)`: how many frames of an epoch the worker has
    /// written to the output ring.
    produced: AtomicU64,
    /// When `produced` last moved, in ns since `origin`.
    published: AtomicU64,
    /// `pack(epoch, index)`: the lowest output index the callback may still
    /// read, for the worker's overflow check.
    floor: AtomicU64,
    stop: AtomicBool,
    /// The host thread's scheduling, for the worker to adopt; policy -1 until
    /// known.
    policy: AtomicI32,
    priority: AtomicI32,
    origin: Instant,
    output: OutputRing,
    stats: Stats,
}

impl Shared {
    fn now_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos() as u64
    }
}

enum Kind<R> {
    Normal,
    /// The first segment of an epoch begun by a host reset.
    Reset(R),
    /// The first segment of an epoch begun because the worker fell an input
    /// ring behind: the stale input is dropped, the DSP keeps its state.
    Resync,
}

struct Header<P: Segments> {
    epoch: u32,
    kind: Kind<P::Reset>,
    len: u32,
    controls: P::Controls,
    pushed: Instant,
    timing: Timing,
}

// ---------------------------------------------------------------------------
// The host side
// ---------------------------------------------------------------------------

/// How far ahead of real time the host's calls are.
///
/// A host keeping real time calls once a period. One rendering ahead calls
/// back to back. `due` is when a real-time host would have made this call:
/// the last call's `due` plus its audio's duration, drained by 200 ppm so
/// that a device clock a little fast against the system's cannot accumulate
/// into a false lead. `due - now` is then how far ahead the host is.
///
/// A call that comes late does not move the schedule: the device keeps time,
/// and the call after a late one is on time, not early. Only a gap longer
/// than `GAP` -- transport stopped, the stream restarted -- starts a new one.
struct Pace {
    due: Option<Instant>,
    previous: Duration,
}

/// Lead the host must have before a late frame is waited for, at least: wider
/// than real-time driver jitter, such as the 0.33 ms a USB interface's 0.5 ms
/// packets put into 64-sample periods at 48 kHz.
const EARLY_MARGIN: Duration = Duration::from_micros(400);
const PACE_DRAIN: f64 = 0.000_2;
/// Lateness beyond which the stream is taken to have restarted.
const GAP: Duration = Duration::from_millis(5);
/// How long an offline render waits for a worker that is alive but has not
/// finished before it gives up on a block and conceals it (`waits_expired`).
const OFFLINE_PATIENCE: Duration = Duration::from_secs(60);

impl Pace {
    fn new() -> Self {
        Self {
            due: None,
            previous: Duration::ZERO,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// Note a call at `now` of `duration`; returns when it was due.
    fn observe(&mut self, now: Instant, duration: Duration) -> Instant {
        let due = match self.due {
            Some(due) => {
                let next = due + self.previous.mul_f64(1.0 - PACE_DRAIN);
                if now > next + GAP.max(self.previous * 3) {
                    now
                } else {
                    next
                }
            }
            None => now,
        };
        self.due = Some(due);
        self.previous = duration;
        due
    }

    /// How long a call due at `due` may wait at `now` without a host keeping
    /// real time noticing.
    fn allowance(&self, now: Instant, due: Instant) -> Duration {
        let margin = EARLY_MARGIN.max(self.previous.mul_f64(0.3));
        due.saturating_duration_since(now).saturating_sub(margin)
    }
}

/// Per-channel concealment of late frames.
#[derive(Clone, Copy)]
struct Conceal {
    /// The last sample handed to the host.
    last: f32,
    /// The decaying stand-in, carried through the crossfade back.
    ghost: f32,
    /// Frames of crossfade left; `RECOVERY` right after a concealed frame.
    recover: u32,
    concealing: bool,
}

const RECOVERY: u32 = 32;

impl Conceal {
    const fn new() -> Self {
        Self {
            last: 0.0,
            ghost: 0.0,
            recover: 0,
            concealing: false,
        }
    }

    #[inline]
    fn silence(&mut self) -> f32 {
        *self = Self::new();
        0.0
    }

    #[inline]
    fn late(&mut self, decay: f32) -> f32 {
        if !self.concealing {
            self.concealing = true;
            self.ghost = self.last;
        }
        self.ghost *= decay;
        self.last = self.ghost;
        self.ghost
    }

    #[inline]
    fn ready(&mut self, sample: f32, decay: f32) -> f32 {
        if self.concealing {
            self.concealing = false;
            self.recover = RECOVERY;
        }
        let out = if self.recover > 0 {
            let w = 1.0 - self.recover as f32 / (RECOVERY + 1) as f32;
            self.ghost *= decay;
            self.recover -= 1;
            sample * w + self.ghost * (1.0 - w)
        } else {
            sample
        };
        self.last = out;
        out
    }
}

/// The host side of a reservoir, and the owner of its worker.
pub struct Reservoir<P: Segments> {
    config: Config,
    shared: Arc<Shared>,
    frames: rtrb::Producer<f32>,
    headers: rtrb::Producer<Header<P>>,
    worker: Option<JoinHandle<Box<P>>>,
    thread: Thread,
    epoch: u32,
    /// Frames delivered this epoch: both pushed and output.
    timeline: u64,
    pending: Option<Kind<P::Reset>>,
    conceal: [Conceal; MAX_CHANNELS],
    decay: f32,
    pace: Pace,
    scheduling_read: bool,
}

impl<P: Segments> Reservoir<P> {
    /// Allocates everything and starts the worker with `processor`. Not for
    /// the audio thread.
    pub fn new(config: Config, processor: Box<P>) -> Self {
        assert!(config.delay > 0, "a reservoir needs a delay");
        assert!((1..=MAX_CHANNELS).contains(&config.channels));
        let max_block = config.max_block.max(1);
        let config = Config {
            max_block,
            ..config
        };
        let input = config.input_capacity();
        let (frames, frames_out) = rtrb::RingBuffer::new(input * config.channels);
        let (headers, headers_out) = rtrb::RingBuffer::new(config.header_capacity());
        let shared = Arc::new(Shared {
            epoch: AtomicU32::new(1),
            produced: AtomicU64::new(pack(0, 0)),
            published: AtomicU64::new(0),
            floor: AtomicU64::new(pack(0, 0)),
            stop: AtomicBool::new(false),
            policy: AtomicI32::new(-1),
            priority: AtomicI32::new(0),
            origin: Instant::now(),
            output: OutputRing::new(config.output_capacity(), config.channels),
            stats: Stats::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::Builder::new()
            .name("gainstagefx-dsp".to_owned())
            .spawn(move || worker_main(worker_shared, processor, frames_out, headers_out, config))
            .expect("create GainStageFx DSP worker");
        let thread = handle.thread().clone();
        shared
            .stats
            .callback
            .priming_remaining
            .store(config.delay as u64, Ordering::Relaxed);
        Self {
            config,
            shared,
            frames,
            headers,
            worker: Some(handle),
            thread,
            epoch: 1,
            timeline: 0,
            pending: None,
            conceal: [Conceal::new(); MAX_CHANNELS],
            decay: (-1.0 / (0.000_5 * config.sample_rate)).exp() as f32,
            pace: Pace::new(),
            scheduling_read: false,
        }
    }

    pub fn delay(&self) -> usize {
        self.config.delay
    }

    pub fn config(&self) -> Config {
        self.config
    }

    pub fn stats(&self) -> &Stats {
        &self.shared.stats
    }

    /// Whether the worker is still running: false after it panicked.
    pub fn worker_alive(&self) -> bool {
        self.shared.stats.worker.alive.load(Ordering::Acquire)
    }

    /// Start a new epoch: nothing queued or produced before this reaches the
    /// output, the worker resets its DSP with `reset` before the next frame,
    /// and the delay is primed again. Real-time safe: it waits for nothing.
    pub fn reset(&mut self, reset: P::Reset) {
        self.begin_epoch();
        self.pending = Some(Kind::Reset(reset));
        self.pace.reset();
        add(&self.shared.stats.callback.resets, 1);
    }

    fn begin_epoch(&mut self) {
        self.epoch = self.epoch.wrapping_add(1) & EPOCH_MASK;
        if self.epoch == 0 {
            self.epoch = 1;
        }
        self.shared.epoch.store(self.epoch, Ordering::Release);
        self.timeline = 0;
        for conceal in &mut self.conceal {
            *conceal = Conceal::new();
        }
        self.shared
            .stats
            .callback
            .priming_remaining
            .store(self.config.delay as u64, Ordering::Relaxed);
    }

    /// Frames of this epoch the worker has published.
    #[inline]
    fn produced(&self) -> u64 {
        let (epoch, count) = unpack(self.shared.produced.load(Ordering::Acquire));
        if epoch == self.epoch {
            count
        } else {
            0
        }
    }

    /// One host call: hand over `channels`' input with `controls`, and
    /// replace it with the output `delay` frames behind. Never allocates or
    /// locks; waits only as the module documentation describes.
    pub fn process(&mut self, channels: &mut [&mut [f32]], controls: P::Controls) {
        let n = channels.first().map_or(0, |channel| channel.len());
        if n == 0 {
            return;
        }
        let started = Instant::now();
        let rate = self.config.sample_rate;
        let duration = Duration::from_secs_f64(n as f64 / rate);
        add(&self.shared.stats.callback.callbacks, 1);
        add(&self.shared.stats.callback.frames, n as u64);
        if !self.scheduling_read {
            self.scheduling_read = true;
            if let Some((policy, priority)) = crate::stage_worker::sys::current_scheduling() {
                self.shared.priority.store(priority, Ordering::Relaxed);
                self.shared.policy.store(policy, Ordering::Release);
            }
        }
        let due = self.pace.observe(started, duration);
        let width = self.config.channels;
        let ch = width.min(channels.len());

        // 1. Hand the input over, all of it or none.
        let samples = n * width;
        let fits = n <= self.config.max_block
            && self.frames.slots() >= samples
            && self.headers.slots() >= 1;
        if !fits {
            // The worker is an input ring behind, or the host broke its own
            // maximum block. Abandon what is queued: a new epoch, primed
            // afresh, the DSP's state kept. This call's input is lost, so it
            // is outside the new timeline: concealed, not counted as frames.
            add(&self.shared.stats.callback.input_overflows, 1);
            self.begin_epoch();
            if !matches!(self.pending, Some(Kind::Reset(_))) {
                self.pending = Some(Kind::Resync);
            }
            for (channel, conceal) in channels.iter_mut().zip(self.conceal.iter_mut()) {
                for sample in channel.iter_mut() {
                    *sample = conceal.late(self.decay);
                }
            }
            return;
        }
        match self.frames.write_chunk_uninit(samples) {
            Ok(chunk) => {
                let input = &*channels;
                let written = chunk.fill_from_iter(
                    (0..n)
                        .flat_map(|i| (0..width).map(move |c| input.get(c).map_or(0.0, |s| s[i]))),
                );
                debug_assert_eq!(written, samples);
            }
            Err(_) => unreachable!("slots were checked"),
        }
        let kind = self.pending.take().unwrap_or(Kind::Normal);
        let header = Header {
            epoch: self.epoch,
            kind,
            len: n as u32,
            controls,
            pushed: started,
            timing: Timing {
                due,
                delay: Duration::from_secs_f64(self.config.delay as f64 / rate),
                realtime: !self.config.offline,
                // All of it, unless the block is longer than the reservoir:
                // then the frames whose output this very call returns.
                first: if n > self.config.delay {
                    n - self.config.delay
                } else {
                    n
                },
            },
        };
        if self.headers.push(header).is_err() {
            unreachable!("a slot was checked");
        }
        self.thread.unpark();

        // 2. Take the output. Frame `t0 + k` is input `t0 + k - D`.
        let stats = &self.shared.stats.callback;
        let delay = self.config.delay as u64;
        let t0 = self.timeline;
        self.timeline += n as u64;
        let first = t0 as i64 - delay as i64;
        let end = first + n as i64;
        self.shared
            .floor
            .store(pack(self.epoch, first.max(0) as u64), Ordering::Release);
        let primed = (-first).clamp(0, n as i64) as usize;
        stats.priming_remaining.store(
            (delay as i64 - self.timeline as i64).max(0) as u64,
            Ordering::Relaxed,
        );

        let mut produced = self.produced();
        if end > 0 && (produced as i64) < end {
            // Something due is not there yet.
            let structural = end > t0 as i64;
            let limit = if self.config.offline {
                // Nothing is due offline: wait as long as it takes. The limit
                // only stops a stuck worker hanging a render for ever; a dead
                // one ends the wait at once.
                add(&stats.offline_waits, 1);
                Some(started + OFFLINE_PATIENCE)
            } else if structural {
                // Bounded by the call's own period: the synchronous plugin
                // would have needed at least this long, and a call that waits
                // past its period has missed its deadline anyway.
                add(&stats.structural_waits, 1);
                Some(started + duration)
            } else {
                let allowance = self.pace.allowance(started, due);
                if allowance > Duration::ZERO {
                    add(&stats.early_waits, 1);
                    Some(started + allowance)
                } else {
                    None
                }
            };
            if let Some(limit) = limit {
                let waited = Instant::now();
                produced = self.wait(end as u64, limit, self.config.offline);
                stats.wait.record(waited.elapsed().as_nanos() as u64);
                if (produced as i64) < end {
                    add(&stats.waits_expired, 1);
                }
            }
        }

        // Fill: how many frames were ready from the first one this call
        // needed. Priming frames are not the worker's, and do not count.
        if end > 0 {
            let needed_from = first.max(0) as u64;
            let fill = produced.saturating_sub(needed_from);
            stats.fill_last.store(fill, Ordering::Relaxed);
            add(&stats.fill_sum, fill);
            add(&stats.fill_count, 1);
            if fill < stats.fill_min.load(Ordering::Relaxed) {
                stats.fill_min.store(fill, Ordering::Relaxed);
            }
            if fill > stats.fill_max.load(Ordering::Relaxed) {
                stats.fill_max.store(fill, Ordering::Relaxed);
            }
            for (level, counter) in [6, 4, 2, 1].iter().zip(&stats.low_water) {
                if fill * 8 < delay * level {
                    add(counter, 1);
                }
            }
            if (produced as i64) >= end {
                // How long before this call the worker had what it needed:
                // since its last publish, plus the frames it had ready beyond
                // this call's, in time -- the margin a deeper reservoir has
                // over a shallow one even when both are idle.
                let published = self.shared.published.load(Ordering::Relaxed);
                let now = self.shared.now_ns();
                let ahead = (produced as i64 - end) as f64 / rate * 1e9;
                stats
                    .slack
                    .record(now.saturating_sub(published) + ahead as u64);
            }
        }

        let ready = (produced as i64 - first).clamp(primed as i64, n as i64) as usize;
        let late = n - ready;
        if late > 0 {
            add(&stats.underrun_events, 1);
            add(&stats.underrun_frames, late as u64);
        }
        let output = &self.shared.output;
        for (c, (channel, conceal)) in channels
            .iter_mut()
            .take(ch)
            .zip(self.conceal.iter_mut())
            .enumerate()
        {
            let (silent, rest) = channel.split_at_mut(primed);
            for sample in silent.iter_mut() {
                *sample = conceal.silence();
            }
            let (on_time, behind) = rest.split_at_mut(ready - primed);
            for (k, sample) in on_time.iter_mut().enumerate() {
                let index = (first + (primed + k) as i64) as u64;
                // SAFETY: `index < produced`, published with Release after the
                // worker wrote it and read here after Acquire; the worker
                // cannot reach this slot again while the callback is here.
                let value = unsafe { *output.slot(index, c) };
                *sample = conceal.ready(value, self.decay);
            }
            for sample in behind.iter_mut() {
                *sample = conceal.late(self.decay);
            }
        }
        for channel in channels.iter_mut().skip(ch) {
            channel.fill(0.0);
        }
    }

    fn wait(&self, target: u64, until: Instant, may_sleep: bool) -> u64 {
        let mut spins = 0u32;
        loop {
            let produced = self.produced();
            if produced >= target
                || !self.shared.stats.worker.alive.load(Ordering::Acquire)
                || Instant::now() >= until
            {
                return produced;
            }
            spins = spins.saturating_add(1);
            if spins < 64 {
                std::hint::spin_loop();
            } else if may_sleep && spins > 256 {
                thread::sleep(Duration::from_micros(20));
            } else {
                thread::yield_now();
            }
        }
    }

    /// Stop the worker and take the processor back. Joins a thread: never on
    /// the audio thread.
    pub fn into_processor(mut self) -> Option<Box<P>> {
        self.stop()
    }

    fn stop(&mut self) -> Option<Box<P>> {
        self.shared.stop.store(true, Ordering::Release);
        let handle = self.worker.take()?;
        handle.thread().unpark();
        handle.join().ok()
    }
}

impl<P: Segments> Drop for Reservoir<P> {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// ---------------------------------------------------------------------------
// The worker
// ---------------------------------------------------------------------------

fn worker_main<P: Segments>(
    shared: Arc<Shared>,
    mut processor: Box<P>,
    mut frames: rtrb::Consumer<f32>,
    mut headers: rtrb::Consumer<Header<P>>,
    config: Config,
) -> Box<P> {
    crate::dsp::time::enable_ftz_daz();
    let channels = config.channels;
    let mut scratch: Vec<Vec<f32>> = (0..channels).map(|_| vec![0.0; config.max_block]).collect();
    let mut epoch = 0u32;
    let mut produced = 0u64;
    let mut applied = (-1, 0);
    let stats = &shared.stats.worker;
    let result = catch_unwind(AssertUnwindSafe(|| {
        while !shared.stop.load(Ordering::Acquire) {
            let Ok(header) = headers.pop() else {
                thread::park();
                continue;
            };
            let len = header.len as usize;
            let Ok(chunk) = frames.read_chunk(len * channels) else {
                add(&stats.protocol_errors, 1);
                continue;
            };
            if older(header.epoch, shared.epoch.load(Ordering::Acquire)) {
                chunk.commit_all();
                add(&stats.stale_segments, 1);
                continue;
            }
            let wanted = (
                shared.policy.load(Ordering::Acquire),
                shared.priority.load(Ordering::Relaxed),
            );
            if wanted.0 >= 0 && wanted != applied {
                crate::stage_worker::sys::set_scheduling(wanted.0, wanted.1);
                applied = wanted;
            }
            if header.epoch != epoch {
                epoch = header.epoch;
                produced = 0;
            }
            if let Kind::Reset(reset) = &header.kind {
                processor.reset(reset);
            }
            {
                let (a, b) = chunk.as_slices();
                for (i, value) in a.iter().chain(b.iter()).enumerate() {
                    scratch[i % channels][i / channels] = *value;
                }
            }
            chunk.commit_all();
            let started = Instant::now();
            stats
                .queue
                .record(started.saturating_duration_since(header.pushed).as_nanos() as u64);
            let (floor_epoch, floor) = unpack(shared.floor.load(Ordering::Acquire));
            let capacity = shared.output.mask as u64 + 1;
            if floor_epoch == epoch && produced + len as u64 > floor + capacity {
                add(&stats.output_overflows, len as u64);
            }
            let base = produced;
            let mut published = 0usize;
            // Write frames `published..upto` of this segment to the output
            // ring and make them visible to the callback.
            let mut publish = |done: &[&mut [f32]], upto: usize| {
                let upto = upto.min(len);
                if upto <= published {
                    return;
                }
                for (c, channel) in done.iter().enumerate().take(channels) {
                    for (k, value) in channel[published..upto].iter().enumerate() {
                        // SAFETY: the callback reads an index only after the
                        // Release below has published it, and cannot be
                        // reading the slot an index `capacity` earlier: see
                        // `OutputRing`.
                        unsafe { *shared.output.slot(base + (published + k) as u64, c) = *value };
                    }
                }
                published = upto;
                shared.published.store(shared.now_ns(), Ordering::Relaxed);
                shared
                    .produced
                    .store(pack(epoch, base + upto as u64), Ordering::Release);
            };
            {
                let mut slices: [&mut [f32]; MAX_CHANNELS] = Default::default();
                for (slot, buffer) in slices.iter_mut().zip(scratch.iter_mut()) {
                    *slot = &mut buffer[..len];
                }
                processor.process(
                    &mut slices[..channels],
                    &header.controls,
                    &header.timing,
                    &mut publish,
                );
                publish(&slices[..channels], len);
            }
            let spent = started.elapsed().as_nanos() as u64;
            stats.segment.record(spent);
            stats.per_frame.record(spent / len.max(1) as u64);
            add(&stats.segments, 1);
            add(&stats.frames, len as u64);
            produced = base + len as u64;
        }
    }));
    if result.is_err() {
        stats.panicked.store(true, Ordering::Release);
    }
    stats.alive.store(false, Ordering::Release);
    processor
}

#[cfg(test)]
#[path = "../tests/support/reservoir_unit.rs"]
mod tests;
