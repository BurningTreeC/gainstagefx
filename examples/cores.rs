//! Whether more cores help, and what a callback actually gets.
//!
//! Every timing figure in this repository is single-core, and that is not an
//! oversight. A plugin's `process()` is called on one host thread and must
//! return before the block's deadline. More cores let a host run more tracks
//! and more plugins at once; they do not make one callback faster. The
//! deadline is per callback, on one thread, and BUG-018 is a statement about
//! that thread.
//!
//! A plugin *can* fan out internally -- the two channels here are independent
//! circuits with no shared state -- so the question is whether the handoff is
//! cheaper than the work it takes away. Two shapes are measured:
//!
//! * spawning a thread per block, which is what the obvious implementation
//!   does and is not realtime safe at all: it allocates, it enters the kernel,
//!   and its worst case is at the scheduler's discretion. It is here to show
//!   how much of the cost is the spawn.
//! * a worker parked on a spinning atomic, allocated once at construction.
//!   That is the only shape a callback could actually use, and it is the
//!   honest floor for what a handoff costs.
//!
//! Neither is used by the plugin. This is a measurement, not a proposal --
//! see the conclusion the numbers print.
//!
//! `cargo run --release --example cores`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};

const RATE: f64 = 48_000.0;

fn chain(gain: Gain) -> Chain {
    let mut c = Chain::new(RATE);
    c.apply(&Settings {
        gain,
        drive: 0.8,
        tone: ToneSection::Scooping,
        cabinet: Cabinet::Stack,
        oversampling: 1,
        ..Settings::default()
    });
    c.settle();
    c
}

fn stimulus(r: usize, block: usize, k: usize) -> f64 {
    0.2 * ((r * block + k) as f64 * 0.03).sin()
}

/// Both channels in sequence, which is what `plugin.rs` does.
fn one_thread(gain: Gain, block: usize, rounds: usize) -> f64 {
    let (mut a, mut b) = (chain(gain), chain(gain));
    let start = std::time::Instant::now();
    for r in 0..rounds {
        for k in 0..block {
            let x = stimulus(r, block, k);
            std::hint::black_box(a.process(x));
            std::hint::black_box(b.process(x));
        }
    }
    start.elapsed().as_secs_f64() / rounds as f64 * 1e6
}

/// A thread per block. Joined every block, because the block has to be
/// finished before the callback returns -- there is nothing to overlap it
/// with.
fn spawned(gain: Gain, block: usize, rounds: usize) -> f64 {
    let (mut a, mut b) = (chain(gain), chain(gain));
    let start = std::time::Instant::now();
    for r in 0..rounds {
        std::thread::scope(|s| {
            let (a, b) = (&mut a, &mut b);
            s.spawn(move || {
                for k in 0..block {
                    std::hint::black_box(a.process(stimulus(r, block, k)));
                }
            });
            s.spawn(move || {
                for k in 0..block {
                    std::hint::black_box(b.process(stimulus(r, block, k)));
                }
            });
        });
    }
    start.elapsed().as_secs_f64() / rounds as f64 * 1e6
}

/// One worker, started once, parked on a spinning atomic. No allocation, no
/// syscall, no lock: the callback publishes a round number and spins until the
/// worker publishes it back. This is as cheap as a per-block handoff gets, and
/// it costs a core its whole life whether the plugin is passing audio or not.
fn parked(gain: Gain, block: usize, rounds: usize) -> f64 {
    let go = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicUsize::new(0));
    let mut here = chain(gain);

    let worker = {
        let (go, done) = (Arc::clone(&go), Arc::clone(&done));
        std::thread::spawn(move || {
            let mut there = chain(gain);
            loop {
                let mut r = go.load(Ordering::Acquire);
                while r == done.load(Ordering::Relaxed) {
                    std::hint::spin_loop();
                    r = go.load(Ordering::Acquire);
                }
                if r == usize::MAX {
                    return;
                }
                for k in 0..block {
                    std::hint::black_box(there.process(stimulus(r - 1, block, k)));
                }
                done.store(r, Ordering::Release);
            }
        })
    };

    // Let the worker reach its spin before the clock starts.
    go.store(1, Ordering::Release);
    while done.load(Ordering::Acquire) != 1 {
        std::hint::spin_loop();
    }

    let start = std::time::Instant::now();
    for r in 0..rounds {
        go.store(r + 2, Ordering::Release);
        for k in 0..block {
            std::hint::black_box(here.process(stimulus(r, block, k)));
        }
        while done.load(Ordering::Acquire) != r + 2 {
            std::hint::spin_loop();
        }
    }
    let took = start.elapsed().as_secs_f64() / rounds as f64 * 1e6;

    go.store(usize::MAX, Ordering::Release);
    done.store(usize::MAX - 1, Ordering::Release);
    let _ = worker.join();
    took
}

fn main() {
    println!("Two channels of one voice, one block, at 48 kHz.\n");
    println!(
        "  {:<12}{:>6}{:>10}{:>10}{:>10}{:>10}{:>9}",
        "voice", "block", "budget", "1 thread", "spawned", "parked", "handoff"
    );

    for gain in [Gain::Peavey, Gain::Boogie, Gain::Neve, Gain::Crunch] {
        for block in [16usize, 64, 256, 1024] {
            let rounds = (RATE as usize / block).max(8);
            let budget = block as f64 / RATE * 1e6;
            let one = one_thread(gain, block, rounds);
            let two = spawned(gain, block, rounds);
            let park = parked(gain, block, rounds);
            println!(
                "  {:<12}{block:>6}{budget:>8.0}us{one:>8.0}us{two:>8.0}us{park:>8.0}us{:>7.0}us",
                gain.name(),
                park - one / 2.0
            );
        }
        println!();
    }

    println!("`handoff` is what the second thread costs beyond the work it took");
    println!("away: the parked figure, less half of one thread's work.\n");
    println!("Two channels is the whole of the parallelism available inside one");
    println!("instance, so the ceiling here is 2x however cheap the handoff gets,");
    println!("and BUG-018 needs about 4.5x on the 5150. A parked worker also holds");
    println!("a core spinning for the life of the plugin -- per instance -- which");
    println!("is a core the host wanted for the other twenty tracks. Cores are how");
    println!("a session gets bigger, not how a callback gets faster.");
}
