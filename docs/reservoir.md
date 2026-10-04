# The reservoir — 4 October 2026

**What it is.** The plugin's DSP — every chain, solver, oversampler, cabinet
and piece of history — now runs on its own sequential worker thread, a fixed
number of host samples behind the host: **128 by default, 2.667 ms at
48 kHz**. The host callback hands the worker this call's input and takes
output the worker finished earlier. The delay is real (the audio is 128
samples late) and reported (the host is told, and compensates parallel tracks
for it). The brief asked for 64 first, then measurement; 128 is the smallest
depth that measured no underrun anywhere, and is the default since.

**Why.** In REAPER every callback that ran past the next driver wake was an
xrun, and each cost REAPER 8–10 periods of *everyone's* audio
(`docs/realtime-multi-instance.md`). A solver spike on the host thread is a
host xrun. On the worker it is a reservoir that runs low, and the host
callback that remains takes microseconds.

**What it is not.** Reported latency alone is not headroom, and a reservoir is
not throughput: a worker that needs 1.5 ms for every 1.333 ms of audio empties
any reservoir and stays empty. It smooths compute jitter. It does not make an
overloaded chain fit.

Code: `src/reservoir.rs` (the reservoir, generic over its DSP),
`src/plugin.rs` (`Controls`, `Engine`, the lifecycle). Tests:
`tests/support/reservoir_unit.rs`, `tests/support/reservoir_plugin.rs`.
Measurements: `examples/reservoir.rs`.

## The timeline

Within one epoch (a run between resets) frames are numbered from zero in the
order the host delivers them, across calls of any length. With a reservoir of
`D` frames:

```text
output frame t  =  silence                      for t < D   (priming)
                =  process(input frame t - D)   for t >= D
```

exactly, whatever the block sizes, because both sides count frames. There is
no "one block of latency": a 16-sample host gets 128 samples of delay, so does
a 256-sample one. Queue capacities are larger than `D` and say nothing about
the delay.

The latency budget is the reservoir plus what the chains already had, in host
samples:

| oversampling in use | chains' own | total at D = 128 | at 48 kHz |
|---|---:|---:|---:|
| 1x (every shipped modelled preset) | 0 | 128 | 2.667 ms |
| 2x (the cap for modelled circuits) | 56 | 184 | 3.833 ms |
| 4x | 64 | 192 | 4.000 ms |
| 8x | 66 | 194 | 4.042 ms |

The reservoir sits at the host-rate boundary, before the oversampler, so its
128 samples are host samples at every factor. 128 samples is 2.902 ms at
44.1 kHz, 1.333 ms at 96 kHz and 0.667 ms at 192 kHz: the depth is in
samples, not milliseconds.

How much time the delay buys depends on the host block `N` against `D`.
Output frame `t` needs input `t - D`, which arrived in the call holding it, and
the worker processes a whole call's segment before publishing it, so:

- **`N == D`** (64 at 64): the worker has one full period for each block, from
  the moment the (now cheap) callback hands it over until the next callback
  needs it. The deadline has left the host's graph for the worker's own
  thread; it has not grown.
- **`N < D`**: the worker may be `(D - N) / N` callbacks late and nobody
  hears it. At the default 128 and 64-sample blocks, one whole callback.
- **`N > D`**: the last `N - D` frames of a call's output are made from the
  first `N - D` of its own input, which no earlier work could have produced.
  The worker processes and publishes those first and the rest behind them, so
  the callback waits for `N - D` frames' work instead of `N`: half at 256 with
  D = 128, three quarters at 512 -- the least any design can wait at this
  latency. The other `D` frames absorb spikes as any reservoir does. A
  reservoir at least as long as the host's block removes the wait.

## What runs where

```text
host callback (process)                  DSP worker ("gainstagefx-dsp")
-----------------------                  ------------------------------
read the parameters -> Controls          pop a header, read its frames
push frames + Controls (rtrb SPSC)  -->  skip it if from an older epoch
wake the worker (unpark)                 reset the Engine if it says so
copy published output out           <--  Engine::process_segment(frames, Controls)
conceal anything late                    write output at timeline slots,
report latency if it changed             publish (epoch, count) with Release
```

- **The worker owns the `Engine`** exclusively: chains, solvers, stage and
  stereo workers, ramps, noise reduction, meters, trace. Nothing else touches
  it while it runs. Activation takes it back (stop, join, rebuild) on the main
  thread; deactivation and `Drop` join the worker. A worker per instance; no
  global state.
- **Input**: two `rtrb` rings (the wait-free SPSC ring nice-plug's standalone
  backend uses): frames, and one header per host call. Capacity
  `max(4·(D + N_max), 1024)` frames rounded up to a power of two — 1024 at
  D = 128 with 64-sample blocks, 21 ms — and up to 512 headers.
- **Output**: an index-addressed ring of `(D + 2·N_max + 64)` frames rounded
  up (512 at 128/64). Frame `j` lives at slot `j mod C`; the worker publishes
  `(epoch, frames written)` in one `AtomicU64` with `Release`, the callback
  reads only below it after `Acquire`. Because the worker can only process
  input the callback has already pushed, it is never more than `D + N_max`
  ahead of what the callback still reads, and with `C` above that it cannot
  overwrite a slot about to be read (`output_overflows` counts it anyway; zero
  in every run).
- **Priority**: the first callback reads the host thread's scheduling, and the
  worker adopts it (Linux, as the stage worker does); the stage workers then
  take it from the DSP worker. Nothing needs root: without an rtprio limit the
  call is refused and the worker keeps normal scheduling.
- **Idle**: the worker parks when its queue is empty and the callback unparks
  it; an idle instance costs no CPU, and nothing spins.

## Automation stays on its frames

nice-plug calls `process` once per stretch between parameter changes
(`SAMPLE_ACCURATE_AUTOMATION = true`), and the plugin always read every
parameter and stepped every smoother at the top of each call. That read is
now `GainStageFx::controls`, still on the host thread at the call, and its
result — `Controls`, a `Copy` of `Settings` plus trims, mix, bypass, noise
reduction, split and oversampling — is queued with that call's frames. The
worker never reads a parameter. So the frames played after an automation
point meet the new value and the ones before it the old, `D` samples later,
and a circuit or preset change lands exactly on its own frames: the old input
never meets the new device. GainStageFx reads no transport and no note
events, so there is nothing else to carry.

## Reset, activation, lifecycle

- `activate` (main thread): stop and join a running worker, rebuild the chains
  for the layout, start a new worker with the engine, report
  `D + chains' latency`.
- `reset` (any thread, including audio; never concurrent with `process`):
  starts a new epoch — the worker skips everything queued from before it,
  output from before it is ignored, the engine is reset before the first new
  frame, and the delay primes again with silence. It waits for nothing.
- `deactivate` and `Drop`: join the worker. Never from `process`.
- **A worker that panics** is caught, marked dead (`worker.alive`,
  `worker.panicked`), and returns its engine; the callback conceals from then
  on and never waits on it.

## Late output, overflow

**Late frames are concealed and skipped, never played late.** A frame not
ready when due is replaced by the last output decaying with a 0.5 ms time
constant, and the real signal is crossfaded back over 32 samples. When the
late frames arrive they are skipped, so the delay never slips and an underrun
cannot quietly become latency. Silence would be a step to zero, and holding
the last sample a DC step; a short decay is neither, and costs a multiply.
Each event and frame is counted.

The worker meanwhile processes **every** input in order — the circuits'
history must stay sequential, and abandoning samples is what made the cutoff
experiments ring for up to a second — and catches up on cheaper samples. If
it falls a whole input ring behind (`input_overflows`), the stale input is
abandoned: a new epoch, the circuits keep their state, the output is
concealed and re-primed.

## When the callback waits

In real time the callback never waits for audio an *earlier* callback
delivered. It waits in three cases, each counted:

1. **Offline** (`ProcessMode::Offline`): always, for everything. A bounce has
   no deadline and must be complete. It gives up only if the worker has died,
   or after 60 s on one block, which no block comes near.
2. **The host is ahead of real time.** REAPER's anticipative FX processing
   renders playback tracks ahead in bursts of back-to-back calls, and CLAP
   reports it as realtime. No worker can keep up with that, and none needs to.
   `Pace` keeps the schedule a real-time host would follow (each call due one
   duration after the last, drained by 200 ppm against clock drift; only a
   gap over 5 ms restarts it), and a late frame is waited for only for the
   host's lead over that schedule, less a margin of 0.4 ms (wider than a USB
   interface's 0.33 ms jitter at 64/48k) or 30 % of a block. A host keeping
   real time is never ahead, so it is never waited for.
3. **`N > D`**, as above: for the frames the call returns, bounded by the
   call's own period.

Spin and `sched_yield` only, in real time; sleep only offline.

## The worker's solver cutoff

The wall-clock cutoff (`REALTIME_CUTOFF_PERIODS`, 1.5 periods) keeps its
meaning: a segment gets the same allowance a host callback got, measured from
the moment the **worker starts it**, plus any reservoir depth beyond one
period. Measured from the hand-over instead, one slow segment — an operating
point hunt after a preset change — spent the budget of every segment queued
behind it, and the cutoff abandoned their solves in a cascade, for frames that
were going to be late anyway: in the first scaling run that doubled to
quadrupled the aborts against the synchronous plugin.

## nice-plug integration

- **Latency** is `ActivateContext::set_latency_samples` in `activate` and
  `ProcessContext::set_latency_samples` in `process` when the oversampling
  moves it; nice-plug compares it with what it last reported. CLAP: the
  wrapper requests a restart when activated and calls
  `clap_host_latency.changed` on reactivation; VST3: `restartComponent
  (kLatencyChanged)`. The AU wrapper reports `kAudioUnitProperty_Latency` as
  the same samples over the sample rate (it does not notify the host of a
  change mid-session; that predates this).
- **Lifecycle**: `activate` may come more than once before `deactivate`;
  `reset` follows every activation and may come from the audio thread; a
  realtime `process` and `reset` are serialised by the wrapper's lock, which is
  what makes the reservoir's producer single.
- nice-plug has no worker or ring buffer of its own for plugins; `rtrb` is the
  one its standalone backend uses.

## Configuration

- `GAINSTAGEFX_RESERVOIR=<samples>` in the host's environment, read when an
  instance is created: any number of samples (21 works as well as 64); `0`
  runs the DSP on the host thread exactly as before. Takes effect at the next
  activation.
- `GainStageFx::set_reservoir_delay` does the same from code (tests,
  examples).
- With `GAINSTAGEFX_RT_TRACE=1`, rows behind a reservoir are the worker's
  segments, marked by a non-zero `reservoir_samples` column, with `queue_us`
  (hand-over to start: wake-up latency plus backlog); `tools/rt_trace.py`
  says so.

## Measurements

`cargo run --release --example reservoir`, on this machine (16 threads), the
real take through the whole plugin -- parameters, reservoir, worker -- loaded
with shipped presets the way a host restores them, on a stereo bus carrying
the mono guitar, paced at 64 samples / 48 kHz, host threads `SCHED_FIFO` 70
(as PipeWire runs REAPER's), the workers adopting it. "Callback" is the host
callback the plugin's `process` takes; ">930" counts callbacks finishing past
the 930 us the short cycle of a USB interface leaves; underruns are reservoir
events. D = 0 is the synchronous plugin as it was. The machine had a browser
running (load average 1-8), so compare within a table, not across.

### Depth, one instance (20 s each)

| preset | D | ms | callback p50 / p99 / p99.9 / max us | >930 | underruns | worker p50 / p99 / max us | slack p0.1 us | vs D = 0 |
|---|---:|---:|---|---:|---:|---|---:|---|
| Jazz Chorus | 0 | 0 | 262 / 397 / 584 / 775 | 0 | - | - | - | reference |
| | 32 | 0.667 | 284 / 514 / 769 / 975 | 2 | 0 | 295 / 524 / 970 | 671 | exact |
| | 64 | 1.333 | **1 / 3 / 4 / 7** | 0 | 0 | 295 / 426 / 777 | 786 | exact |
| | 96 | 2.000 | 1 / 3 / 4 / 14 | 0 | 0 | 262 / 426 / 937 | 1,442 | exact |
| | 128 | 2.667 | 1 / 3 / 5 / 13 | 0 | 0 | 262 / 426 / 1,207 | 2,097 | exact |
| Puppet Master '86 | 0 | 0 | 285 / 394 / 439 / 522 | 0 | - | - | - | reference |
| | 32 | 0.667 | 288 / 399 / 441 / 577 | 0 | 0 | 295 / 426 / 568 | 677 | exact |
| | 64 | 1.333 | 1 / 6 / 15 / 45 | 0 | 0 | 328 / 590 / 774 | 721 | exact |
| | 96 | 2.000 | 1 / 3 / 4 / 12 | 0 | 0 | 295 / 426 / 604 | 1,573 | exact |
| | 128 | 2.667 | 1 / 3 / 4 / 11 | 0 | 0 | 295 / 426 / 607 | 2,359 | exact |
| Twin, Modern Purple | 0 | 0 | 362 / 595 / 699 / 777 | 0 | - | - | - | reference |
| | 32 | 0.667 | 477 / 954 / 1,153 / 1,335 | 213 | 2 | 492 / 983 / 1,349 | 672 | 128 frames differ |
| | 64 | 1.333 | 2 / 3 / 4 / 9 | 0 | 0 | 360 / 655 / 1,002 | 655 | exact |
| | 96 | 2.000 | 1 / 3 / 5 / 16 | 0 | 0 | 393 / 655 / 800 | 1,311 | exact |
| | 128 | 2.667 | 1 / 3 / 4 / 6 | 0 | 0 | 393 / 655 / 782 | 1,966 | exact |

D = 32 at a 64-sample block is the `N > D` case: every call waits for its
own audio (14,999 structural waits in 15,000 calls), which is the synchronous
plugin plus a hand-off, and where the worker needed more than the period the
wait expired and two blocks were concealed. A reservoir shorter than the
host's block is not a reservoir.

Solver work is identical at every depth (Puppet Master: 46,216 backtracks,
1,574 fallbacks, 0 unsettled, every run). The worker's wake-up latency, from
hand-over to start: p50 4-9 us, p99 6-27 us.

### Instances, one host thread each (10 s each)

Callbacks over 930 us out of the run's total (instances x 7,500); underrun
events behind the reservoir. Several runs where they were repeated.

| preset | instances | D = 0: >930 (>period) | D = 64 | D = 96 | D = 128 |
|---|---:|---|---|---|---|
| Puppet Master '86 | 1 | 0 | 0 | - | 0 |
| | 4 | 0 | 0 | - | 0 |
| | 8 | 0 | 0 | - | 0 |
| | 16 | 18,610 (0); 46,384 (14); 41,425 (11); 19,624 (0) | 0; 6; 0 | 0 | 0; 0 |
| Jazz Chorus | 1 | 7; 19; 16 (1); 0; 0 | 0; 1; 0; 0; 0 | 0; 0 | 0 |
| | 4 | 8 (1); 3; 6; 2; 5 | 0; 0; 0; 0 | 0; 0 | 0 |
| | 8 | 12; 12; 7; 6; 6 | 2; 0; 0; 0 | 4; 1 | 0 |
| | 16 | 717 (48); 717 (49); 933 (93); 580 (31) | 40; 55; 34 | 32 | 0; 0 |

With a reservoir at least as long as the block, the host callbacks stayed at
1-6 us p50 and under 60 us p99.9 at every count, and **no callback in any of
those runs crossed 930 us**.
The remaining risk moved to the worker: an underrun is the worker later than
the reservoir, from a stall of 1.4-3.0 ms -- the JC-120's hard samples, or
`SCHED_FIFO` threads queueing behind each other on a core with 16 instances on
16 hardware threads. It is concealed (a 64-sample fade, counted) instead of a
host xrun costing 8-10 periods of the session's audio.

### Instances, one host thread for all (10 s each)

The host calling every instance in turn, as REAPER did in 41 % of the 20:23
capture's cycles. Cycles finishing past the period, of 7,500:

| preset | instances | D = 0 | D = 64 | D = 128 |
|---|---:|---:|---:|---:|
| Jazz Chorus | 1 / 2 | 0 / 0 | 0 / 0 | 0 / 0 |
| | 4 | 107 | **0** | 0 |
| | 8 | **7,500** (every cycle) | **0** | 0 |
| Puppet Master '86 | 1 / 2 | 0 / 0 | 0 / 0 | 0 / 0 |
| | 4 | 904 | **0** | 0 |
| | 8 | **7,500** | **0** | 0 |

with no underrun in any reservoir run. This is where the architecture changes
what is possible rather than what is likely: eight chains of 250-330 us do not
fit one 1,333 us thread, and behind reservoirs the thread only hands audio
over while the workers run side by side.

### Output, against the synchronous plugin

Every run with no solver deadline aborts is **exact**: the reservoir's output
is the synchronous output 64 (96, 128) samples later, to the bit. Runs with
aborts differ -- by -130 to -165 dB on Puppet Master, by up to -26 dB in
JC-120 bursts -- and so do two *synchronous* runs of the same configuration:
the wall-clock cutoff (`REALTIME_CUTOFF_PERIODS`) abandons solves at
load-dependent samples in both. In tests, with the cutoff disarmed, the
comparison is bit-exact across circuits, layouts, block sizes, automation and
device changes (`tests/support/reservoir_plugin.rs`).

### Host blocks longer than the reservoir (D = 128, 10 s each)

The callback waits for the frames it returns, `N - D` of them, and the worker
publishes those before processing the rest:

| preset | block | callback p50 / p99.9 us, D = 0 -> D = 128 | worker p50 / max us | underruns |
|---|---:|---|---|---:|
| Jazz Chorus | 256 | 1,098 / 2,287 -> **587 / 1,334** | 1,180 / 2,783 | 0 |
| Puppet Master '86 | 256 | 1,095 / 1,583 -> **575 / 940** | 1,180 / 1,775 | 0 |
| Twin, Modern Purple | 256 | 1,361 / 2,871 -> **713 / 1,497** | 1,442 / 3,075 | 0 |
| Jazz Chorus | 512 | 2,175 / 3,922 -> 1,654 / 2,814 | 2,359 / 3,695 | 0 |
| Puppet Master '86 | 512 | 2,156 / 3,570 -> 1,769 / 2,612 | 2,621 / 3,477 | 0 |
| Twin, Modern Purple | 512 | 2,897 / 7,798 -> 2,023 / 3,515 | 2,884 / 4,801 | 0 |

Halved at 256 and down by a quarter at 512, as the arithmetic says; no call
past its period in either. Four instances behave the same. Puppet Master and
the Twin are bit-exact. The split is another block boundary, which matters
only to the power stage's speculation (per block): the JC-120's rounds a unit
in the last place differently, and its power stage can carry that into a
different, equally converged fallback -- bursts of up to -48 dB against the
unsplit render at 256, -165 dB at 512. The synchronous plugin does the same
between buffer sizes.

### Which depth

| D | at 48 kHz | underruns measured |
|---:|---:|---|
| 32 | 0.667 ms | no reservoir at 64-sample blocks (`N > D`) |
| 64 | 1.333 ms | none at 1-8 instances except the JC-120 (1 and 2 events, in 2 of 13 runs); 0-55 at 16 |
| 96 | 2.000 ms | JC-120: 1-4 at 8 instances, 32 at 16 |
| 128 | 2.667 ms | **none, in any run** (1-16 instances, both hosts) |

The smallest depth with no underrun anywhere measured is **128**, and it is the
default (the owner's choice, 4 October). 64 removes every host-side overrun
measured too, at half the latency, and conceals a rare 1.3 ms of the JC-120
under load; `GAINSTAGEFX_RESERVOIR=64` chooses it per session.
