# Realtime with several instances in REAPER — 29 September 2026

**The goal.** At 48 kHz / 64 samples in REAPER, two or three live-monitored
instances and four to six playback instances, with no crackle, stutter or
dropout.

**The finding that changes the target.** On this machine the deadline is not
1,333 µs. It is **~930 µs one callback in three**. The SSL 2 Mk II delivers
audio in 0.5 ms USB packets. PipeWire, driving the graph from that device at a
64-frame quantum, therefore wakes at 1.5, 1.5, 1.0 ms. That averages 1,333 µs,
but a graph still running when the next wake arrives is an xrun. Measured from
the 07:43 REAPER captures (`tools/rt_trace.py`), Puppet Master '86:

| next driver slot | ≤ 900 µs | 900–1,000 µs | ≥ 1,000 µs | ≥ 1,400 µs |
| --- | ---: | ---: | ---: | ---: |
| 1.0 ms | 0 xruns in 8,961 | 50 % | 100 % | 100 % |
| 1.5 ms | 0 % | 0 % | 0 % (to 1,300 µs) | 40–100 % |

Each xrun cost REAPER 8–10 periods (10.7–13.3 ms) of audio. There were 2,694
of them in 81.5 s, 33 a second, and they account exactly for the 28 s gap
between the capture's wall time and its audio time. That is the stutter.
A slow callback also makes the next one slower, because the input jumps across
the lost audio and caches are cold: 33 power-stage backtracks a callback after
a gap against 4 otherwise.

## What the host does with two instances

The two instances were **not serialised** on one thread. In the short slot,
Puppet callbacks under 900 µs never xrun (0 of 8,369), although Jazz plus
Puppet together took ≥ 1,100 µs in essentially every cycle. Serialised, every
one of those cycles would have xrun. Each instance's xrun risk depends only on
its own duration against the next driver wake. The Jazz track also ran some
callbacks back to back (intervals of 400–700 µs), which is REAPER rendering a
playback track ahead of time. The old captures had no timestamps, so this was
established by aligning the two files on their shared gaps (1,605 of 2,694
matched) and by consequence. The new trace records absolute timestamps, thread
and CPU, so the next capture answers it directly.

In the worst 1 % of callbacks, every stage ran 1.6–2.3× slower *per unit of
work*. That includes the linear cabinet, 158 → 357 µs for identical
arithmetic. This is interference (an SMT sibling, clock or cache), not solver
work, and it will grow with every instance added.

## The existing deadline protection

`Simulation::set_realtime_deadline`: the plugin sets one absolute cutoff,
callback start + 0.9 × host period (1,200 µs at 64/48k), for every nonlinear
circuit. A solve that wants a fourth pass after the cutoff is abandoned and
handled as an unsettled sample: the output is the bounded iterate, the
reactive and device state is **not advanced**, and the next sample starts
without the predictor. In the capture it fired 6,160 times in 737 callbacks.
It fires *after* the real deadline in the short slot, so it could not prevent
those xruns. It fires *before* it in the long slots, where 1,200–1,300 µs
callbacks never xrun. Each abort freezes the circuit's state for the rest of
that callback, which is a discontinuity of its own.

It is also wall-clock dependent: with it armed, any test that activates the
plugin in realtime mode had its outcome decided by machine load.
`tests/support/plugin_mono.rs` now disarms it in its fixture.

## Where Puppet Master '86's time went, and what changed

`perf` on the real take (`examples/rt_scenario`, dual-mono path, 1x):

| area | before | after |
| --- | ---: | ---: |
| acoustic rear paths (`DelayLine::read`, `pressure`) | 20 % | 8 % |
| reduced LU (`solve_dense_planned*`) | 26 % | 25 % of a smaller total |
| Schur recovery / RHS / Newton loop | 22 % | 22 % |
| device stamps (triode, pentode) | 6 % | 7 % |

1. **Dead rear paths skipped** (`acoustics/stage.rs`). Each capsule evaluated
   all 91 panel×route paths every sample. On the closed 4x12, 67 of them have
   exactly zero weight: routes round edges the capsule cannot see, plus the
   front panel's own thirteen, which are zero by construction. Each skipped
   path saves two Lagrange reads and a one-pole. Only the live paths are
   read now, in their original order. Guarded by
   `every_unlisted_rear_path_stays_exactly_silent`.
2. **Masked planned LU** (`dsp/partition.rs`). The replayed reduced
   factorisation visits only the entries that can be non-zero (19–37 %
   filled). Guarded by `masked_planned_replay_is_the_dense_solve_bit_for_bit`.

Both are exact. All 78 shipped presets produce bit-identical output on the
real take. Pinned, three alternating runs on a quiet machine:

| | mean | p99 | p99.9 | callbacks > 930 µs |
| --- | ---: | ---: | ---: | ---: |
| Puppet Master '86, before | 929 µs | 1,277 | 1,396 | 45 % |
| Puppet Master '86, after | 753 µs | 1,083 | 1,224 | 9 % |
| Jazz Chorus, before | 617 µs | 860 | 1,294 | 0.7 % |
| Jazz Chorus, after | 506 µs | 788 | 1,200 | 0.5 % |

## The zero-latency stage pipeline

`Chain::process` is feed-forward: pedal and preamplifier, then power stage,
iron, make-up, tone and cabinet. Nothing in the second half feeds back into
the first within a sample, so the power stage at sample *n* and the
preamplifier at *n + 1* are independent. `Chain::split` gives the two halves
as views over disjoint fields. `process` is `back.sample(front.sample(x))`,
and `process_block` offers the back half to a `StageWorker`
(`src/stage_worker.rs`), which runs it 8 samples behind the front half on
another core, inside the same callback.

- **No added latency:** the pipeline lives inside the callback, not across
  callbacks.
- **Bit-identical:** `tests/pipeline.rs` checks every voice at 1x, 2x, 4x and
  8x, per sample, serial block, pipelined and reclaimed. The plugin test
  `a_pipelined_callback_is_the_serial_callback_and_does_not_allocate` does the
  same through the plugin, under `assert_no_heap`.
- **Never waits on a worker that has not started:** one compare-and-swap
  decides who runs the back half. If the worker has not woken when the front
  half is done, the audio thread reclaims it.
- **Adaptive:** used only when the chain's smoothed cost exceeds 30 % of the
  period (off below 20 %), so light presets never wake a second thread.
- **Priority:** on Linux the worker adopts the audio thread's scheduling
  policy and priority.

Measured on the real take, 48k/64, two cores:

| | mean | p99 | p99.9 | max | callbacks > 930 µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Puppet Master '86, session start | 931 µs | 1,284 | 1,432 | 1,877 | 46 % |
| Puppet Master '86, serial now | 727 µs | 959 | 1,100 | 1,290 | 1.9 % |
| Puppet Master '86, pipelined | 434 µs | 579 | 680 | 770–918 | 0 |
| Jazz Chorus, session start | 613 µs | 844 | 1,265 | 1,853 | 0.6 % |
| Jazz Chorus, pipelined | 399 µs | 608–640 | 1,024 | 1,592 | 0.16 % |

The Jazz Chorus gains less because its JC-120 transistor power stage alone is
~350 µs a callback, and one simulation cannot be split between threads.

**Considered and rejected, with reasons:**
- **A global Newton across stage boundaries.** The stages are one-way
  coupled, so the Jacobian is block-triangular. Solving them concurrently
  against guessed boundary values is block-Jacobi iteration: more sweeps, not
  fewer.
- **Parallelism inside a sample** (partition residuals, factorisation,
  speculative Newton). A reduced 13×13 solve is now ~0.1–0.3 µs, and one
  cross-core hand-off costs about as much;
  `examples/async_quantum_deadline` measured per-sample hand-offs losing.

## The deadline cutoff

Every abandoned solve is a click. Forced cutoffs on the last 8 samples of one
block in 50 (`rt_scenario --cut-from 56 --cut-every 50`) gave worst-sample
errors of −0.2 to −6.5 dB re peak, with disturbances lasting milliseconds to
over a second. That held whether the abandoned sample's state was frozen
(today), advanced from the bounded guess, or advanced with the predictor kept.
Each variant was worst on a different preset, and freezing let the Twin run
away. So the cutoff now fires only at `REALTIME_CUTOFF_PERIODS` = 1.5 periods,
once a callback has certainly missed its deadline. Its only remaining job is
to hold an overrun to one lost cycle.

## Why the Mark power stage backtracked, and the fix

The output valve that saturates swings its plate toward 0 V. The pentode model
has an edge there: at `vpk ≤ 0` every slope is zero, so the valve vanished
from the Jacobian. Newton then threw the plate to +400 V, then back below
zero, and the line search accepted both steps.

`Pentode::plate_edge_conductance` fixes the Jacobian only. A pass that
*arrives* at the edge from above is linearised with the slope from above,
taken at the previous point; the current there is still exactly zero, so
every solution is unchanged.

- Puppet Master '86: power-stage passes 4.09 → 3.51 a sample, backtracks
  −87 %.
- Across all 78 presets: backtracks −25 %, no preset gains unsettled samples.
- Output nulls at −164 dB or better on every preset except Experienced '67,
  whose fuzz-driven Plexi has unsettled samples in both builds (11 → 9 on the
  full take).
- Both frozen baselines pass.

**Tried and rejected:**
- A pitch-synchronous, then nearest-neighbour, history predictor: passes
  −0.05 % and −0.5 %.
- A hybrid dense/sparse row update in the LU: +9 % slower.

**Open:** the JC-120's junction limiter (SPICE `pnjlim`) walks a junction
turning on in ~80 mV passes.

## The 11:19 REAPER capture: pipeline in, two instances

Two instances, one process. The Jazz Chorus ran for 160 s and was alone for
the first 95 s. The Puppet Master '86 was added for the last 65 s. Timestamps
are absolute, so this is measurement, not alignment.

| | 07:43 capture | 11:19 capture |
| --- | ---: | ---: |
| Puppet: mean / p99 / p99.9 | 784 / 1,336 / 1,576 µs | 426 / 723 / 859 µs |
| Puppet callbacks over 930 µs | 15.8 % | 0.05 % |
| dropout gaps with both running | 33 per second | 0.34 per second |
| Puppet deadline aborts | 6,160 | 0 |

- **The pipeline carried the load.** The Puppet's second half ran on the
  worker in 99.9 % of its callbacks.
- **The Jazz Chorus is now the limit.** Alone, every callback over
  ~930–1,000 µs in the short USB slot was a dropout (106 in 95 s), even
  pipelined. Its JC-120 transistor power stage cannot be split.
- **REAPER ran the two instances one after the other.** They used different
  threads, but the second started a median 18 µs after the first ended in 89 %
  of cycles. Check *Preferences → Audio → Buffering → Allow live FX
  multiprocessing*.
- **The adaptive switch started cold.** The three worst dropouts followed a
  preset load with pipelining still off, so it now starts on.

## The transistor Jacobian

`Bipolar::stamp`'s collector current carries the Early effect
(`Ic · (1 + vce / VA)`), but its derivatives did not. On an output transistor
carrying amps, Newton's linear model was short an output conductance of
`Ic / VA`, so solves crept toward the answer. With the derivative in:

- Neve 73P: preamp passes 2.86 → 2.05 a sample, output block 2.56 → 2.01.
- JC-120: preamp 2.94 → 2.22. The power stage's backtracks fell 2,186 → 36
  and its fallbacks 451 → 104 over 4 s.
- Fuzz Face 3.30 → 2.53, HM-2 −14 %.
- Across the catalogue, unsettled samples fell 12 → 7.

Where the solver converges, the answers are the same: quiet passages null at
−155 to −175 dB, and the frozen baseline still matches the 73P sample for
sample. In loud Jazz Chorus passages the two builds differ by up to −9 dB re
peak. That is not the fix: both builds leave a few solves unsettled there, at
different samples, and each such sample is a discontinuity of its own. Those
JC-120 unsettled samples are the next target.

## True latency

The chains now report and delay by the oversampler's own round trip:
**nothing at 1x**, 56 samples at a modelled 2x. Before, every setting was
padded to 66 samples, so every live instance was heard 1.375 ms late. The
padded mode remains `Chain`'s default, because the frozen fixtures render it.

- `activate` computes the figure from the parameters with
  `voice::true_latency`, the same rule `set_oversampling` applies. A restart
  therefore reports what the next block will produce, and
  `the_activation_latency_rule_is_what_the_chain_does` guards against a
  restart loop.
- A pending oversampling change is now installed by `delayed_dry` as well as
  `process`. The dry delay's length is the latency now, and it had been taken
  one sample, or one block, late.

## Tried after the 11:19 capture, and rejected

Each was measured on the real take across all 78 presets and reverted. The
tree was rebuilt afterwards and checked byte-identical to the build before
the experiments.

| idea | result | why rejected |
| --- | --- | --- |
| Contraction stopping (Hairer–Wanner, κ = 0.1): skip the confirming Newton pass | passes −8.8 % | Physical-cabinet presets null only at −73 to −89 dB re peak: the speaker path differentiates cone velocity at 48 kHz, which amplifies an error below tolerance into one measurable at the output. Unsettled samples doubled (7 → 14). The confirming pass is doing fidelity work. |
| Transistor junction limiter off on searched passes | every JC-120 power and 73P preamp solve unsettled | The limiter is load-bearing: capped-exponent conductances wreck the linear solve. |
| Transistor trial residuals evaluated unlimited | JC-120 unsettled 22 → 19 | Fuzz Face fallbacks 328 → 1,705. |
| Halve steps in a detected solver orbit | JC-120 unsettled 22 → 21 | Experienced '67 unsettled 16 → 87. |
| Schur recovery and RHS restricted to each column's non-zero span | exact, ~1–2 % faster | Within run-to-run noise; the loops were already short and vectorised. |
| Contraction stopping on pedal and preamp only (power stage and iron keep the confirming pass) | preamp passes −6 to −7 %, power unchanged | Brit Crunch and Brit Lead still null only at −73 to −76 dB re peak, so the error is amplified downstream of the preamp too, and the gain is small. |
| Four pipeline stages (pedal \| preamp \| power \| cabinet) instead of two | exact | Slower: Puppet 433 → 466 µs, Jazz 385 → 439 µs, and 3–5 ms spikes when the workers are oversubscribed. Each extra stage moves a working set of tens to hundreds of kB between cores and lengthens the pipeline's fill within a 64-sample block. Two stages is the sweet spot. |

The JC-120's remaining unsettled samples are approximate orbits of full Newton
steps. The line search accepts them under its non-monotone rule, while a
transistor walks its junction up under `pnjlim`. None of the three generic
remedies above was a net win across the catalogue.

### The JC-120's unsettled samples are clicks, and a smaller step removes them

Any change upstream, even at the level of rounding, moves where the JC-120 power stage ends a
solve unsettled. Two builds of Jazz Chorus then differ by −186 dB everywhere
except in 3-sample bursts reaching 34 % of peak, in the blocks
`rt_scenario --events` reports as unsettled or falling back. So each
unsettled sample leaves an output error of that size: a click inside loud
playing. `rt_scenario --oversampling 2` runs the same 19 s take at h/2:
unsettled 22 → 0, fallbacks 805 → 331, backtracks 1,691 → 272 (over twice
as many solves). The failures come from the step size. Source continuation
keeps h and does not remove them.

That is now fixed for every circuit (`Simulation::enable_half_step_rescue`).
A failing sample takes two half steps on a twin built at twice the rate, and
the full step is then solved again from that answer. Across the catalogue on
the take, unsettled samples fall 69 → 0. The 71 presets that never failed are
bit-identical, the other seven change only around their former failures, and
callback times are unchanged within noise.

## Earlier notes on the Mark power stage

The Newton pass count on the real take is 2–3 for 73 % of samples. 13 % leave
quadratic convergence at pass 2–3, and 8 % take 8–21 passes, which is 28 % of
all power-stage passes. The output pentodes' grids are asked to move 16–64+ V
in one Newton pass 285,000 times in 19 s, and `limit(raw, vgk, 4.0)` allows
8 V. A clamped pass cannot halve its correction, so it is `stalled`. That turns
the line search on, and from pass 8 on the search stays on and the limiter is
off, so every rejected trial is a backtrack. 82,000 of the clamps are steps up
across the grid-current kink at 0 V, where the linearisation below it has no
grid conductance. The remaining slow convergence happens with the limiter off
and full steps accepted, so the next thing to find is which non-smooth edge of
the pentode model it is crossing: `vpk.max(0.0)`, the zero Jacobian at
`vpk ≤ 0`, or `gp.max(1e-12)`.

## What the goal requires

- A live instance must finish inside ~930 µs **every** time, with its SMT
  sibling busy. Puppet's p99.9 is 1,224 µs uncontended, so the tail has to
  come down by about half.
- Playback instances run ahead on REAPER's worker threads. They need
  throughput and must not starve the live threads' cores.
- Adding instances only helps if the host keeps them on separate threads. The
  next capture, with the new trace, shows which threads and cores each
  instance used.

## Capturing in REAPER

```sh
GAINSTAGEFX_RT_TRACE=1 GAINSTAGEFX_RT_TRACE_DIR=/tmp/gainstagefx-trace reaper
python3 tools/rt_trace.py /tmp/gainstagefx-trace/gainstagefx-rt-*-instance-*.csv
```

Rows stream to disk every 50 ms from a writer thread (`src/rt_trace.rs`). The
audio thread only fills a fixed-size record and pushes it into a preallocated
ring. The trace is covered by
`an_armed_trace_does_not_allocate_and_writes_every_callback`.
