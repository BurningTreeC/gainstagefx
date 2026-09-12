//! Persistent right-channel worker for true stereo processing.
//!
//! The host audio thread owns channel zero. Once a stereo signal has actually
//! diverged, channel one can be processed independently on this persistent
//! worker. The callback never allocates, locks a mutex, or creates a thread.
//! A single atomic state machine publishes one job at a time. If the worker has
//! not claimed a job by the time the host thread finishes the left channel,
//! the host thread steals that still-unstarted job and executes it itself.
//!
//! The raw pointers in `StereoJob` are valid only for the duration of one
//! plugin callback. `finish_or_steal()` is called before that callback returns,
//! and `Drop` waits for any in-flight job before terminating the worker.

use std::cell::UnsafeCell;
use std::hint::spin_loop;
use std::mem::MaybeUninit;
use std::slice;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};

use crate::voice::Chain;

const IDLE: u8 = 0;
const READY: u8 = 1;
const RUNNING: u8 = 2;
const DONE: u8 = 3;
const STOP: u8 = 4;

/// Everything the right-channel worker needs for one host block.
///
/// All pointed-to storage is owned by the plugin/host callback and remains
/// alive until `finish_or_steal()` returns.
#[derive(Clone, Copy)]
pub(crate) struct StereoJob {
    pub chain: *mut Chain,
    pub samples: *mut f32,
    pub input_trim: *const f32,
    pub output_trim: *const f32,
    pub mix: *const f32,
    pub len: usize,
    pub bypassed: bool,
    pub silence_linear: f64,
}

struct Shared {
    state: AtomicU8,
    job: UnsafeCell<MaybeUninit<StereoJob>>,
}

// `job` is written only while IDLE and read only after the Release->Acquire
// transition to READY. The audio and worker threads race only to claim READY;
// exactly one winner transitions it to RUNNING and becomes the sole user of
// every raw pointer in that job until DONE is published.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

pub(crate) struct StereoWorker {
    shared: Arc<Shared>,
    thread: Thread,
    handle: Option<JoinHandle<()>>,
}

impl StereoWorker {
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: AtomicU8::new(IDLE),
            job: UnsafeCell::new(MaybeUninit::uninit()),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::Builder::new()
            .name("gainstagefx-right".to_owned())
            .spawn(move || worker_loop(worker_shared))
            .expect("create GainStageFx stereo worker");
        let worker_thread = handle.thread().clone();
        Self {
            shared,
            thread: worker_thread,
            handle: Some(handle),
        }
    }

    /// Publish one true-stereo right-channel block.
    ///
    /// The caller must not touch any storage referenced by `job` until
    /// `finish_or_steal()` returns.
    pub unsafe fn submit(&self, job: StereoJob) {
        debug_assert_eq!(self.shared.state.load(Ordering::Acquire), IDLE);
        // SAFETY: IDLE means neither thread can be reading the job slot.
        unsafe { (*self.shared.job.get()).write(job) };
        self.shared.state.store(READY, Ordering::Release);
        // Wake a parked worker. If it is already spinning/running this simply
        // leaves an unpark token that makes the next park return immediately.
        self.thread.unpark();
    }

    /// Wait for the right channel, stealing it if the worker never started it.
    ///
    /// The steal path is important on an oversubscribed machine: a descheduled
    /// worker must not force the host audio thread to wait for a job that has
    /// not begun. Once the worker has transitioned READY->RUNNING, however,
    /// only that thread may touch the right chain and samples and the callback
    /// has to wait for DONE.
    pub fn finish_or_steal(&self) {
        loop {
            match self.shared.state.load(Ordering::Acquire) {
                DONE => {
                    self.shared.state.store(IDLE, Ordering::Release);
                    return;
                }
                READY => {
                    if self
                        .shared
                        .state
                        .compare_exchange(READY, RUNNING, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        // SAFETY: this thread won the only READY->RUNNING claim.
                        let job = unsafe { (*self.shared.job.get()).assume_init_read() };
                        unsafe { process_job(job) };
                        self.shared.state.store(DONE, Ordering::Release);
                    }
                }
                RUNNING => spin_loop(),
                IDLE => return,
                STOP => return,
                _ => unreachable!("invalid stereo worker state"),
            }
        }
    }
}

impl Drop for StereoWorker {
    fn drop(&mut self) {
        // Plugin teardown/reinitialization is not concurrent with process(), but
        // finish defensively in case a host violates that expectation.
        loop {
            match self.shared.state.load(Ordering::Acquire) {
                IDLE | DONE => break,
                READY => {
                    if self
                        .shared
                        .state
                        .compare_exchange(READY, RUNNING, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        let job = unsafe { (*self.shared.job.get()).assume_init_read() };
                        unsafe { process_job(job) };
                        self.shared.state.store(DONE, Ordering::Release);
                    }
                }
                RUNNING => spin_loop(),
                STOP => break,
                _ => unreachable!("invalid stereo worker state"),
            }
        }
        self.shared.state.store(STOP, Ordering::Release);
        self.thread.unpark();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn worker_loop(shared: Arc<Shared>) {
    crate::dsp::time::enable_ftz_daz();
    loop {
        match shared.state.load(Ordering::Acquire) {
            READY => {
                if shared
                    .state
                    .compare_exchange(READY, RUNNING, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    // SAFETY: this thread won the only READY->RUNNING claim.
                    let job = unsafe { (*shared.job.get()).assume_init_read() };
                    unsafe { process_job(job) };
                    shared.state.store(DONE, Ordering::Release);
                }
            }
            STOP => return,
            IDLE | DONE => {
                // Continuous stereo arrives one callback apart. Spin briefly so
                // the next block normally avoids a scheduler wakeup, then park
                // so a mono/idle plugin does not burn an entire CPU core.
                let mut changed = false;
                for _ in 0..100_000 {
                    let state = shared.state.load(Ordering::Acquire);
                    if state != IDLE && state != DONE {
                        changed = true;
                        break;
                    }
                    spin_loop();
                }
                if !changed {
                    thread::park();
                }
            }
            RUNNING => spin_loop(),
            _ => unreachable!("invalid stereo worker state"),
        }
    }
}

unsafe fn process_job(job: StereoJob) {
    // SAFETY: publication/claim rules documented on `Shared` guarantee unique
    // access to the right chain and sample slice until DONE is published.
    let chain = unsafe { &mut *job.chain };
    let samples = unsafe { slice::from_raw_parts_mut(job.samples, job.len) };
    let input_trim = unsafe { slice::from_raw_parts(job.input_trim, job.len) };
    let output_trim = unsafe { slice::from_raw_parts(job.output_trim, job.len) };
    let mix = unsafe { slice::from_raw_parts(job.mix, job.len) };

    for i in 0..job.len {
        let raw = samples[i] as f64;
        let trimmed = raw * input_trim[i] as f64;
        let input = if trimmed.abs() < job.silence_linear {
            0.0
        } else {
            trimmed
        };
        let dry = chain.delayed_dry(input);
        let wet = chain.process(input);
        if !job.bypassed {
            samples[i] =
                ((dry * (1.0 - mix[i] as f64) + wet * mix[i] as f64) * output_trim[i] as f64)
                    as f32;
        }
    }
}
