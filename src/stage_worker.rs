//! A persistent helper that runs the second half of a chain while the audio
//! thread runs the first half of the next stretch of samples.
//!
//! `Chain::process` is one feed-forward line: pedal and preamplifier, then the
//! power stage, the iron and the cabinet. Nothing in the second half feeds
//! back into the first within a sample, so the two halves can run on two
//! cores a few samples apart -- inside the same host callback, with no added
//! latency, and with the output identical to the bit because each half runs
//! exactly the code it always ran, in the same order, on its own state.
//!
//! The worker is only ever *offered* a block's second half. The audio thread
//! and the worker race for it with one compare-and-swap; if the worker has not
//! woken by the time the first half is finished, the audio thread claims the
//! second half and runs it itself. So the audio thread never waits for a
//! worker that has not started: the worst case is today's serial block plus
//! one futex wake. Once the worker *has* claimed a block, the audio thread
//! waits for it -- which is why the worker takes the audio thread's own
//! realtime scheduling on Linux, captured from the first callback.
//!
//! Nothing here allocates after construction, and the worker parks between
//! blocks rather than spinning, so an idle plugin costs no CPU.
//!
//! There are two such workers, each a `Lane`. The first runs the chain's
//! second half; the second, when the chain has a pedal worth a core of its own,
//! runs the preamplifier while the audio thread runs the pedal, so the chain
//! is three stages on three cores (`Chain::process_block`). The `StageWorker`
//! methods are the first lane's; `second` is the other.

use std::cell::UnsafeCell;
use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

const IDLE: u32 = 0;
const OFFERED: u32 = 1;
const CLAIMED: u32 = 2;
const DONE: u32 = 3;
const RECLAIMED: u32 = 4;
const STOP: u32 = 5;

/// Runs samples `from..to` of the offered block.
pub type Run = unsafe fn(*mut (), usize, usize);

#[derive(Clone, Copy)]
struct Job {
    run: Run,
    ctx: *mut (),
    total: usize,
}

struct Shared {
    state: AtomicU32,
    /// Samples of the block whose first half is written.
    produced: AtomicUsize,
    job: UnsafeCell<Option<Job>>,
    /// The audio thread's scheduling, for the worker to adopt; -1 until known.
    policy: AtomicI32,
    priority: AtomicI32,
    /// Test-only behaviour: claim each block, then wait until the whole of it
    /// has been published before running any of it. See `lagging`.
    lag: bool,
}

// SAFETY: `job` is written by the audio thread only while the state is IDLE
// (no one else reads it then) and read by whichever side wins the OFFERED
// claim, after the Release store that published it.
unsafe impl Sync for Shared {}
unsafe impl Send for Shared {}

pub struct StageWorker {
    lanes: [Lane; 2],
}

/// One persistent worker thread and the block it is offered.
pub struct Lane {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    /// Whether the audio thread's scheduling has been read yet.
    scheduling_read: AtomicBool,
}

impl Default for StageWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl StageWorker {
    /// Workers with no threads behind them: every offer is reclaimed by the
    /// caller. The path a real worker takes when it has not woken in time,
    /// made deterministic for tests.
    #[doc(hidden)]
    pub fn inert() -> Self {
        Self {
            lanes: [Lane::inert(), Lane::inert()],
        }
    }

    pub fn new() -> Self {
        Self {
            lanes: [
                Lane::spawn(false, "gsfx-stage"),
                Lane::spawn(false, "gsfx-stage-2"),
            ],
        }
    }

    /// Workers that claim every block and then wait for all of it to be
    /// published before running any: the calling thread always finishes its
    /// own half first, which is the case where it takes work back from a
    /// worker already running. Made deterministic in *that* it happens, for
    /// tests; where in the block it happens still varies, and must not matter.
    #[doc(hidden)]
    pub fn lagging() -> Self {
        Self {
            lanes: [
                Lane::spawn(true, "gsfx-stage"),
                Lane::spawn(true, "gsfx-stage-2"),
            ],
        }
    }

    /// The second worker, for a chain's third stage.
    pub fn second(&self) -> &Lane {
        &self.lanes[1]
    }

    /// See `Lane::offer`; the first worker's.
    ///
    /// # Safety
    /// As `Lane::offer`.
    pub unsafe fn offer(&self, run: Run, ctx: *mut (), total: usize) {
        unsafe { self.lanes[0].offer(run, ctx, total) }
    }

    /// See `Lane::publish`.
    #[inline]
    pub fn publish(&self, produced: usize) {
        self.lanes[0].publish(produced);
    }

    /// See `Lane::try_reclaim`.
    ///
    /// # Safety
    /// As `Lane::try_reclaim`.
    pub unsafe fn try_reclaim(&self) -> bool {
        unsafe { self.lanes[0].try_reclaim() }
    }

    /// See `Lane::published`.
    #[inline]
    pub fn published(&self) -> usize {
        self.lanes[0].published()
    }

    /// See `Lane::is_done`.
    #[inline]
    pub fn is_done(&self) -> bool {
        self.lanes[0].is_done()
    }

    /// See `Lane::release`.
    pub fn release(&self) {
        self.lanes[0].release();
    }

    /// See `Lane::finish`.
    ///
    /// # Safety
    /// As `Lane::finish`.
    pub unsafe fn finish(&self) -> bool {
        unsafe { self.lanes[0].finish() }
    }
}

impl Lane {
    fn inert() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: AtomicU32::new(IDLE),
                produced: AtomicUsize::new(0),
                job: UnsafeCell::new(None),
                policy: AtomicI32::new(-1),
                priority: AtomicI32::new(0),
                lag: false,
            }),
            thread: None,
            scheduling_read: AtomicBool::new(false),
        }
    }

    fn spawn(lag: bool, name: &str) -> Self {
        let shared = Arc::new(Shared {
            state: AtomicU32::new(IDLE),
            produced: AtomicUsize::new(0),
            job: UnsafeCell::new(None),
            policy: AtomicI32::new(-1),
            priority: AtomicI32::new(0),
            lag,
        });
        let worker_shared = shared.clone();
        let thread = thread::Builder::new()
            .name(name.into())
            .spawn(move || worker_loop(&worker_shared))
            .expect("spawn stage worker");
        Self {
            shared,
            thread: Some(thread),
            scheduling_read: AtomicBool::new(false),
        }
    }

    /// Offer the second half of a `total`-sample block to the worker.
    ///
    /// # Safety
    /// `ctx` must stay valid, and `run(ctx, ..)` sound to call from another
    /// thread, until `finish` returns. One offer at a time.
    pub unsafe fn offer(&self, run: Run, ctx: *mut (), total: usize) {
        if !self.scheduling_read.swap(true, Ordering::Relaxed) {
            if let Some((policy, priority)) = sys::current_scheduling() {
                self.shared.priority.store(priority, Ordering::Relaxed);
                self.shared.policy.store(policy, Ordering::Release);
            }
        }
        debug_assert_eq!(self.shared.state.load(Ordering::Relaxed), IDLE);
        // SAFETY: IDLE, so the worker is not reading the job.
        unsafe { *self.shared.job.get() = Some(Job { run, ctx, total }) };
        self.shared.produced.store(0, Ordering::Relaxed);
        self.shared.state.store(OFFERED, Ordering::Release);
        if let Some(thread) = &self.thread {
            thread.thread().unpark();
        }
    }

    /// The first half has written samples `0..produced`. Callable from any
    /// thread: a third stage's worker publishes to the second's.
    #[inline]
    pub fn publish(&self, produced: usize) {
        self.shared.produced.store(produced, Ordering::Release);
    }

    /// How much of the block has been published.
    #[inline]
    pub fn published(&self) -> usize {
        self.shared.produced.load(Ordering::Acquire)
    }

    /// If the worker has not claimed the block, take it back and return
    /// `true`: the caller then runs the whole job itself, no further than
    /// `published` at any moment, and calls `release`. `false` means the
    /// worker has it; the caller waits for `is_done` and then calls
    /// `release`.
    ///
    /// # Safety
    /// Must follow `offer`.
    pub unsafe fn try_reclaim(&self) -> bool {
        self.shared
            .state
            .compare_exchange(OFFERED, RECLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Whether the worker has run all of the block it claimed.
    #[inline]
    pub fn is_done(&self) -> bool {
        self.shared.state.load(Ordering::Acquire) == DONE
    }

    /// End the block, after `try_reclaim` and, if the worker had it, once
    /// `is_done`.
    pub fn release(&self) {
        self.shared.state.store(IDLE, Ordering::Release);
    }

    /// The first half is done. Returns once the whole block's second half
    /// has run: on the worker (`true`) or, if the worker never claimed it,
    /// here (`false`).
    ///
    /// # Safety
    /// Must follow `offer`, with the same `ctx` still valid.
    pub unsafe fn finish(&self) -> bool {
        // SAFETY: we are the only writer of the job, and it is still live.
        let job = unsafe { (*self.shared.job.get()).expect("finish after offer") };
        self.shared.produced.store(job.total, Ordering::Release);
        let ran_on_worker = match self.shared.state.compare_exchange(
            OFFERED,
            RECLAIMED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // SAFETY: the claim is ours; the caller guarantees `ctx`.
                unsafe { (job.run)(job.ctx, 0, job.total) };
                false
            }
            Err(_) => {
                while self.shared.state.load(Ordering::Acquire) != DONE {
                    spin_loop();
                }
                true
            }
        };
        self.shared.state.store(IDLE, Ordering::Release);
        ran_on_worker
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        self.shared.state.store(STOP, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

fn worker_loop(shared: &Shared) {
    crate::dsp::time::enable_ftz_daz();
    let mut applied = (-1, 0);
    loop {
        let state = shared.state.load(Ordering::Acquire);
        if state == STOP {
            return;
        }
        if state != OFFERED {
            thread::park();
            continue;
        }
        let wanted = (
            shared.policy.load(Ordering::Acquire),
            shared.priority.load(Ordering::Relaxed),
        );
        if wanted.0 >= 0 && wanted != applied {
            sys::set_scheduling(wanted.0, wanted.1);
            applied = wanted;
        }
        if shared
            .state
            .compare_exchange(OFFERED, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            continue;
        }
        // SAFETY: the claim is ours, and the offer's Release made the job visible.
        let job = unsafe { (*shared.job.get()).expect("an offered job") };
        if shared.lag {
            while shared.produced.load(Ordering::Acquire) < job.total {
                spin_loop();
            }
        }
        let mut done = 0;
        while done < job.total {
            let produced = shared.produced.load(Ordering::Acquire);
            if produced > done {
                // SAFETY: `offer`'s contract; `produced` was published after
                // the first half wrote those samples.
                unsafe { (job.run)(job.ctx, done, produced) };
                done = produced;
            } else {
                spin_loop();
            }
        }
        shared.state.store(DONE, Ordering::Release);
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod sys {
    use std::os::raw::{c_int, c_ulong};

    #[repr(C)]
    struct SchedParam {
        sched_priority: c_int,
    }

    unsafe extern "C" {
        fn pthread_self() -> c_ulong;
        fn pthread_getschedparam(
            thread: c_ulong,
            policy: *mut c_int,
            param: *mut SchedParam,
        ) -> c_int;
        fn pthread_setschedparam(thread: c_ulong, policy: c_int, param: *const SchedParam)
            -> c_int;
    }

    /// The calling thread's policy and priority.
    pub fn current_scheduling() -> Option<(i32, i32)> {
        let mut policy = 0;
        let mut param = SchedParam { sched_priority: 0 };
        // SAFETY: valid out-pointers for the calling thread.
        let ok = unsafe { pthread_getschedparam(pthread_self(), &mut policy, &mut param) } == 0;
        ok.then_some((policy, param.sched_priority))
    }

    /// Adopt a policy and priority, if the process is allowed to. A plain
    /// thread that is not allowed realtime scheduling simply stays as it is.
    pub fn set_scheduling(policy: i32, priority: i32) {
        let param = SchedParam {
            sched_priority: priority,
        };
        // SAFETY: a valid parameter block for the calling thread.
        let _ = unsafe { pthread_setschedparam(pthread_self(), policy, &param) };
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) mod sys {
    pub fn current_scheduling() -> Option<(i32, i32)> {
        None
    }

    pub fn set_scheduling(_policy: i32, _priority: i32) {}
}
