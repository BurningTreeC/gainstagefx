//! The reservoir on its own, with stand-in processors whose output is easy to
//! predict: the delay is exact, the controls travel with their frames, late
//! output is concealed without the alignment slipping, and the callback
//! neither allocates nor waits where it must not.
use super::*;
use crate::test_allocations::assert_no_heap;
use std::sync::atomic::AtomicUsize;

const RATE: f64 = 48_000.0;

/// Multiplies by the call's control; optionally takes `cost` per segment, or
/// `stall` on the segment numbered `stall_at`, or panics there.
struct Stand {
    resets: Arc<AtomicUsize>,
    segments: usize,
    cost: Duration,
    stall_at: Option<usize>,
    stall: Duration,
    panic_at: Option<usize>,
    /// Publish the frames the host waits for first, then take this long over
    /// the rest; and count the segments that asked for that.
    behind: Duration,
    splits: Arc<AtomicUsize>,
}

impl Stand {
    fn new() -> Self {
        Self {
            resets: Arc::new(AtomicUsize::new(0)),
            segments: 0,
            cost: Duration::ZERO,
            stall_at: None,
            stall: Duration::ZERO,
            panic_at: None,
            behind: Duration::ZERO,
            splits: Arc::new(AtomicUsize::new(0)),
        }
    }
}

fn busy(duration: Duration) {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        std::hint::spin_loop();
    }
}

impl Segments for Stand {
    type Controls = f32;
    type Reset = ();

    fn process(
        &mut self,
        channels: &mut [&mut [f32]],
        gain: &f32,
        timing: &Timing,
        publish: &mut dyn FnMut(&[&mut [f32]], usize),
    ) {
        if self.panic_at == Some(self.segments) {
            panic!("the stand-in processor fails on purpose");
        }
        if self.stall_at == Some(self.segments) {
            thread::sleep(self.stall);
        }
        busy(self.cost);
        self.segments += 1;
        let len = channels.first().map_or(0, |c| c.len());
        if timing.first < len {
            self.splits.fetch_add(1, Ordering::Relaxed);
        }
        for channel in channels.iter_mut() {
            for sample in channel[..timing.first].iter_mut() {
                *sample *= gain;
            }
        }
        publish(channels, timing.first);
        thread::sleep(self.behind);
        for channel in channels.iter_mut() {
            for sample in channel[timing.first..].iter_mut() {
                *sample *= gain;
            }
        }
    }

    fn reset(&mut self, _: &()) {
        self.resets.fetch_add(1, Ordering::Relaxed);
    }
}

fn config(delay: usize, channels: usize, max_block: usize, offline: bool) -> Config {
    Config {
        delay,
        channels,
        max_block,
        sample_rate: RATE,
        offline,
    }
}

/// A signal with two impulses and no two equal samples: any shift or repeat
/// is visible.
fn signal(len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| match i {
            0 => 1.0,
            1_000 => -1.0,
            _ => ((i as f32) * 0.000_731).sin() * 0.25 + (i % 7) as f32 * 0.01,
        })
        .collect()
}

/// Feed `input` in calls of the given lengths (cycled); gain per call from
/// `gain`. Returns the output.
fn run(
    reservoir: &mut Reservoir<Stand>,
    input: &[f32],
    blocks: &[usize],
    gain: impl Fn(usize) -> f32,
) -> Vec<f32> {
    let mut output = Vec::with_capacity(input.len());
    let mut at = 0;
    let mut call = 0;
    while at < input.len() {
        let n = blocks[call % blocks.len()].min(input.len() - at);
        let mut block = input[at..at + n].to_vec();
        reservoir.process(&mut [&mut block[..]], gain(call));
        output.extend_from_slice(&block);
        at += n;
        call += 1;
    }
    output
}

/// The tests that measure behaviour against the wall clock run one at a time,
/// so that they at least do not starve each other's workers. What the
/// machine's load can still change is checked with a tolerance; everything
/// the timeline guarantees whatever the load is checked exactly, offline.
static TIMING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn timing() -> std::sync::MutexGuard<'static, ()> {
    TIMING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// How many frames from `from` on are not the input `delay` frames late.
fn misaligned(output: &[f32], input: &[f32], delay: usize, from: usize) -> usize {
    (from.max(delay)..output.len())
        .filter(|&t| output[t] != input[t - delay])
        .count()
}

/// The frames a reservoir's own counts say it did not play on time:
/// concealed, or crossfading back after a concealment. A machine's load
/// decides how many there are; if the delay ever slipped, every frame after
/// the slip would be out of place and far more than this.
fn accounted<P: Segments>(reservoir: &Reservoir<P>) -> usize {
    let c = &reservoir.stats().callback;
    (c.underrun_frames.load(Ordering::Relaxed)
        + RECOVERY as u64 * c.underrun_events.load(Ordering::Relaxed)) as usize
}

fn assert_delayed(output: &[f32], input: &[f32], delay: usize, label: &str) {
    for (t, &y) in output.iter().enumerate() {
        let expected = if t < delay { 0.0 } else { input[t - delay] };
        assert_eq!(y, expected, "{label}: frame {t}");
    }
}

#[test]
fn the_delay_is_exact_for_every_depth_and_block_size() {
    // Offline, so that the worker is always waited for and the comparison is
    // about the timeline, not about the machine.
    let input = signal(6_000);
    for delay in [21, 32, 64, 96, 128] {
        for block in [1, 8, 16, 32, 63, 64, 65, 96, 128, 256, 512] {
            let mut reservoir = Reservoir::new(config(delay, 1, 512, true), Box::new(Stand::new()));
            let output = run(&mut reservoir, &input, &[block], |_| 1.0);
            assert_delayed(&output, &input, delay, &format!("D={delay} N={block}"));
            assert_eq!(reservoir.stats().underruns(), 0);
            // The impulse at 0 comes out at D, the one at 1000 at 1000 + D.
            assert_eq!(output[delay], 1.0);
            assert_eq!(output[1_000 + delay], -1.0);
        }
    }
}

#[test]
fn a_varying_block_size_does_not_move_the_delay() {
    let input = signal(8_000);
    for delay in [21, 64, 128] {
        let mut reservoir = Reservoir::new(config(delay, 1, 256, true), Box::new(Stand::new()));
        let output = run(
            &mut reservoir,
            &input,
            &[32, 64, 17, 128, 48, 64, 1, 255],
            |_| 1.0,
        );
        assert_delayed(&output, &input, delay, &format!("D={delay} varying"));
    }
}

#[test]
fn controls_travel_with_their_own_frames() {
    // A different gain on every call: frame t must come out multiplied by the
    // gain of the call that *delivered* input t - D, not of the call it is
    // played in.
    let input = signal(4_000);
    let blocks = [37, 64, 5, 90];
    let gain = |call: usize| 1.0 + call as f32 * 0.5;
    let mut reservoir = Reservoir::new(config(64, 1, 128, true), Box::new(Stand::new()));
    let output = run(&mut reservoir, &input, &blocks, gain);
    let mut owner = Vec::new();
    let mut call = 0;
    while owner.len() < input.len() {
        owner.extend(std::iter::repeat_n(call, blocks[call % blocks.len()]));
        call += 1;
    }
    for (t, &y) in output.iter().enumerate().skip(64) {
        assert_eq!(y, input[t - 64] * gain(owner[t - 64]), "frame {t}");
    }
}

#[test]
fn stereo_channels_stay_apart() {
    let left = signal(3_000);
    let right: Vec<f32> = left.iter().map(|x| -0.5 * x + 0.1).collect();
    let mut reservoir = Reservoir::new(config(64, 2, 64, true), Box::new(Stand::new()));
    let (mut out_l, mut out_r) = (Vec::new(), Vec::new());
    for at in (0..left.len()).step_by(48) {
        let end = (at + 48).min(left.len());
        let mut l = left[at..end].to_vec();
        let mut r = right[at..end].to_vec();
        reservoir.process(&mut [&mut l[..], &mut r[..]], 1.0);
        out_l.extend_from_slice(&l);
        out_r.extend_from_slice(&r);
    }
    assert_delayed(&out_l, &left, 64, "left");
    assert_delayed(&out_r, &right, 64, "right");
}

#[test]
fn a_reset_lets_nothing_from_before_through() {
    let input = signal(3_000);
    let stand = Stand::new();
    let resets = Arc::clone(&stand.resets);
    let mut reservoir = Reservoir::new(config(64, 1, 64, true), Box::new(stand));
    let _ = run(&mut reservoir, &input[..1_000], &[64], |_| 1.0);
    reservoir.reset(());
    // Different material after the reset: none of the first may appear.
    let after: Vec<f32> = input[1_000..].iter().map(|x| x + 2.0).collect();
    let output = run(&mut reservoir, &after, &[64], |_| 1.0);
    assert_delayed(&output, &after, 64, "after reset");
    assert_eq!(
        resets.load(Ordering::Relaxed),
        1,
        "the DSP was reset once, before the new audio"
    );
}

#[test]
fn priming_is_silence_and_is_counted_down() {
    let mut reservoir = Reservoir::new(config(96, 1, 32, true), Box::new(Stand::new()));
    let mut block = [1.0f32; 32];
    reservoir.process(&mut [&mut block[..]], 1.0);
    assert!(block.iter().all(|&x| x == 0.0));
    assert_eq!(
        reservoir
            .stats()
            .callback
            .priming_remaining
            .load(Ordering::Relaxed),
        64
    );
    for _ in 0..2 {
        let mut block = [1.0f32; 32];
        reservoir.process(&mut [&mut block[..]], 1.0);
    }
    assert_eq!(
        reservoir
            .stats()
            .callback
            .priming_remaining
            .load(Ordering::Relaxed),
        0
    );
    let mut block = [1.0f32; 32];
    reservoir.process(&mut [&mut block[..]], 1.0);
    assert!(block.iter().all(|&x| x == 1.0));
}

/// Calls paced like a host keeping real time: one period apart.
fn paced(reservoir: &mut Reservoir<Stand>, input: &[f32], block: usize) -> Vec<f32> {
    let period = Duration::from_secs_f64(block as f64 / RATE);
    let start = Instant::now();
    let mut output = Vec::with_capacity(input.len());
    for (call, chunk) in input.chunks(block).enumerate() {
        let due = start + period * call as u32;
        while Instant::now() < due {
            std::hint::spin_loop();
        }
        let mut buffer = chunk.to_vec();
        reservoir.process(&mut [&mut buffer[..]], 1.0);
        output.extend_from_slice(&buffer);
    }
    output
}

#[test]
fn in_real_time_a_late_worker_is_concealed_and_the_delay_does_not_slip() {
    // The worker stalls 8 ms on one segment: six periods at 64/48k. The
    // callback must not wait for it; the frames due meanwhile are concealed,
    // the late ones skipped when they come, and afterwards the output is the
    // input exactly 64 frames late again.
    let _timing = timing();
    let input = signal(48_000 / 2);
    let mut stand = Stand::new();
    stand.stall_at = Some(40);
    stand.stall = Duration::from_millis(8);
    let mut reservoir = Reservoir::new(config(64, 1, 64, false), Box::new(stand));
    let output = paced(&mut reservoir, &input, 64);
    let stats = reservoir.stats();
    assert!(stats.underruns() > 0, "the stall must show");
    assert_eq!(stats.callback.structural_waits.load(Ordering::Relaxed), 0);
    // No assertion on early waits: a test thread the machine holds up for
    // over 5 ms catches up with calls back to back, which is a host ahead of
    // real time, and is rightly waited for.
    // Every frame not concealed or crossfading is the input exactly 64
    // frames late: the late frames were skipped, not played late, so the
    // delay did not slip. (A loaded machine can make the worker late again;
    // that adds concealed frames, and they are counted.)
    let wrong = misaligned(&output, &input, 64, 0);
    assert!(
        wrong <= accounted(&reservoir),
        "{wrong} frames out of place, {} concealed",
        accounted(&reservoir)
    );
    // Nothing concealed is louder than what came before it.
    let peak = input.iter().fold(0.0f32, |p, x| p.max(x.abs()));
    assert!(output.iter().all(|x| x.abs() <= peak * 1.001));
}

#[test]
fn a_host_rendering_ahead_is_waited_for_and_nothing_is_lost() {
    // Back-to-back calls, as REAPER's anticipative processing makes them,
    // against a worker that needs a fifth of a period a block. Without waiting
    // nearly every block would be late; with the host's lead measured, none.
    let _timing = timing();
    let input = signal(48_000 / 4);
    let mut stand = Stand::new();
    stand.cost = Duration::from_micros(250);
    let mut reservoir = Reservoir::new(config(64, 1, 64, false), Box::new(stand));
    // Let the worker thread start: a reservoir is built at activation, long
    // before the first call, not in the same breath as it.
    thread::sleep(Duration::from_millis(5));
    let output = run(&mut reservoir, &input, &[64], |_| 1.0);
    let stats = reservoir.stats();
    assert!(
        stats.callback.early_waits.load(Ordering::Relaxed) > 0,
        "the host's lead was used"
    );
    // Without the waits, every block's output would be late. With them, the
    // only frames lost are any a loaded machine took more than the host's
    // lead to schedule.
    let wrong = misaligned(&output, &input, 64, 0);
    assert!(
        wrong <= accounted(&reservoir),
        "{wrong} frames out of place"
    );
    assert!(
        wrong * 10 < output.len(),
        "{wrong} of {} frames lost rendering ahead",
        output.len()
    );
}

#[test]
fn only_a_block_longer_than_the_reservoir_is_split_and_the_delay_is_exact() {
    let input = signal(6_000);
    for (delay, block, split) in [
        (64, 64, false),
        (64, 32, false),
        (64, 65, true),
        (21, 256, true),
    ] {
        let stand = Stand::new();
        let splits = Arc::clone(&stand.splits);
        let mut reservoir = Reservoir::new(config(delay, 1, 256, true), Box::new(stand));
        let output = run(&mut reservoir, &input, &[block], |_| 1.0);
        assert_delayed(&output, &input, delay, &format!("D={delay} N={block}"));
        assert_eq!(
            splits.load(Ordering::Relaxed) > 0,
            split,
            "D={delay} N={block}"
        );
    }
}

#[test]
fn a_long_block_waits_only_for_the_frames_it_returns() {
    // N = 256 against D = 64: each call returns 192 frames made from its own
    // input. The worker publishes those and then takes 3 ms over the other
    // 64, which the next call needs. Waiting for the whole segment would
    // cost every call 3 ms; waiting for what it returns costs it nearly
    // nothing.
    let _timing = timing();
    let input = signal(48_000 / 2);
    let mut stand = Stand::new();
    stand.behind = Duration::from_millis(3);
    let mut reservoir = Reservoir::new(config(64, 1, 256, false), Box::new(stand));
    thread::sleep(Duration::from_millis(5));
    let output = paced(&mut reservoir, &input, 256);
    let stats = reservoir.stats();
    let wait = &stats.callback.wait;
    assert!(stats.callback.structural_waits.load(Ordering::Relaxed) > 0);
    assert!(
        wait.quantile(0.5) < 1_500_000,
        "a call waited {} us at the median: for the whole segment, not its own frames",
        wait.quantile(0.5) / 1_000
    );
    let wrong = misaligned(&output, &input, 64, 0);
    assert!(
        wrong <= accounted(&reservoir),
        "{wrong} frames out of place"
    );
}

#[test]
fn a_block_longer_than_the_reservoir_waits_for_its_own_frames() {
    // N = 256 against D = 64: three quarters of each call's output is its own
    // input. Paced in real time, the callback waits for the worker (bounded)
    // and the output is still exactly 64 frames late.
    let _timing = timing();
    let input = signal(48_000 / 4);
    let mut reservoir = Reservoir::new(config(64, 1, 256, false), Box::new(Stand::new()));
    thread::sleep(Duration::from_millis(5));
    let output = paced(&mut reservoir, &input, 256);
    let stats = reservoir.stats();
    assert!(stats.callback.structural_waits.load(Ordering::Relaxed) > 0);
    let wrong = misaligned(&output, &input, 64, 0);
    assert!(
        wrong <= accounted(&reservoir),
        "{wrong} frames out of place with N > D"
    );
    assert!(wrong * 10 < output.len(), "{wrong} frames lost with N > D");
}

#[test]
fn a_worker_a_whole_ring_behind_is_resynchronised() {
    // A 60 ms stall fills the input ring (1024 frames, 16 calls). The calls
    // that do not fit are counted and concealed, the stale input is dropped,
    // and the reservoir primes afresh and runs exactly again.
    let _timing = timing();
    let input = signal(48_000 / 2);
    let mut stand = Stand::new();
    stand.stall_at = Some(20);
    stand.stall = Duration::from_millis(60);
    let mut reservoir = Reservoir::new(config(64, 1, 64, false), Box::new(stand));
    let output = paced(&mut reservoir, &input, 64);
    let stats = reservoir.stats();
    assert!(stats.input_overflows() > 0);
    assert_eq!(stats.output_overflows(), 0);
    assert!(stats.worker.stale_segments.load(Ordering::Relaxed) > 0);
    // At the end the output is the input 64 frames late, within the new
    // epoch -- which, the overflowing calls having been outside every
    // timeline, is the same 64 frames of the stream.
    let tail = output.len() - 2_000;
    let wrong = misaligned(&output, &input, 64, tail);
    assert!(
        wrong <= accounted(&reservoir),
        "{wrong} frames out of place after resynchronising"
    );
}

#[test]
fn a_worker_that_dies_is_seen_and_never_hangs_the_callback() {
    let input = signal(4_000);
    let mut stand = Stand::new();
    stand.panic_at = Some(3);
    let mut reservoir = Reservoir::new(config(64, 1, 64, true), Box::new(stand));
    let started = Instant::now();
    // Even offline, which waits for everything, the callback returns.
    let output = run(&mut reservoir, &input, &[64], |_| 1.0);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!reservoir.worker_alive());
    assert!(reservoir.stats().worker.panicked.load(Ordering::Relaxed));
    assert!(reservoir.stats().underruns() > 0);
    assert!(output.iter().all(|x| x.is_finite()));
    // The processor comes back even so.
    assert!(reservoir.into_processor().is_some());
}

#[test]
fn the_callback_does_not_allocate() {
    let _timing = timing();
    let input = signal(48_000 / 8);
    let mut reservoir = Reservoir::new(config(64, 2, 64, false), Box::new(Stand::new()));
    let period = Duration::from_secs_f64(64.0 / RATE);
    let mut left = [0.0f32; 64];
    let mut right = [0.0f32; 64];
    let start = Instant::now();
    for (call, chunk) in input.chunks(64).enumerate() {
        while Instant::now() < start + period * call as u32 {
            std::hint::spin_loop();
        }
        left[..chunk.len()].copy_from_slice(chunk);
        right[..chunk.len()].copy_from_slice(chunk);
        assert_no_heap(|| {
            reservoir.process(
                &mut [&mut left[..chunk.len()], &mut right[..chunk.len()]],
                0.5,
            );
            if call == 30 {
                reservoir.reset(());
            }
        });
    }
}

#[test]
fn instances_share_nothing() {
    let input = signal(3_000);
    let mut a = Reservoir::new(config(64, 1, 64, true), Box::new(Stand::new()));
    let mut b = Reservoir::new(config(32, 1, 64, true), Box::new(Stand::new()));
    let (mut out_a, mut out_b) = (Vec::new(), Vec::new());
    for chunk in input.chunks(64) {
        let mut x = chunk.to_vec();
        let mut y = chunk.to_vec();
        a.process(&mut [&mut x[..]], 1.0);
        b.process(&mut [&mut y[..]], 2.0);
        out_a.extend_from_slice(&x);
        out_b.extend_from_slice(&y);
    }
    assert_delayed(&out_a, &input, 64, "a");
    let doubled: Vec<f32> = input.iter().map(|x| x * 2.0).collect();
    assert_delayed(&out_b, &doubled, 32, "b");
}

#[test]
fn the_histogram_brackets_its_quantiles() {
    let histogram = Histogram::new();
    for value in 1..=10_000u64 {
        histogram.record(value * 1_000);
    }
    for p in [0.5, 0.95, 0.99, 0.999] {
        let exact = (p * 10_000.0f64).ceil() as u64 * 1_000;
        let estimate = histogram.quantile(p);
        assert!(estimate >= exact, "p{p}: {estimate} < {exact}");
        assert!(
            (estimate as f64) < exact as f64 * 1.15,
            "p{p}: {estimate} vs {exact}"
        );
    }
    assert_eq!(histogram.max(), 10_000_000);
    for index in 0..BUCKETS - 1 {
        assert_eq!(Histogram::bucket(Histogram::floor(index)), index);
    }
}
