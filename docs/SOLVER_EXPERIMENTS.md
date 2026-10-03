# Solver optimization experiments

One entry per experiment, accepted or rejected, so that nobody repeats a
rejected one without new evidence. See `docs/SOLVER_OPTIMIZATION.md` for the
architecture and the measurement instruments.

## The regression oracle

The Twin deterministic trace is the reference. Any change that claims to be
exact must reproduce both the counters and the hash:

```bash
tools/twin_trace.sh              # the accepted configuration
tools/twin_trace.sh --counters   # + the solver control counters
tools/twin_trace.sh --phases     # + the per-phase TSC profile
tools/twin_trace.sh --narrow     # scalar kernels, for a timing A/B
```

The configuration lives in that script rather than in this file, because it is
a dozen environment variables and getting one wrong silently produces a
different trajectory. It clears every rejected experiment's switch as well, so
an inherited shell cannot change the answer.

**The hash is asserted, not just printed.** `twin_realtime_recording_solver_trace`
fails when the trajectory moves, and says so. The assertion arms itself only
when the run *is* the accepted configuration -- 384,000 frames, no attack
preset and no `GAINSTAGEFX_TEST_*` switch set -- and otherwise prints
`twin_solver_output_hash,checked=false,reason=...`. Switches that change speed
but not arithmetic are listed in `TRAJECTORY_NEUTRAL_SWITCHES` and leave it
armed, so a wide/narrow A/B run doubles as a proof of bit-identity.

**The accepted configuration is the solver the plugin ships** (2026-09-30). Until
then it was five switches turning on a Jacobian-assisted initialisation that only
test builds contain, so the oracle guarded a solver nobody played -- and a slower
one (rejected below). The test is `#[ignore]`d, so nothing ran it, and its hash had
moved twice without being updated, both times by intended changes:
3286bb6 (the saturated pentode's plate slope and the transistor's Early-effect
derivative, and the true-latency path) and 47bdd0e (the source-scaled predictor
for every circuit). Run it after any solver change: `tools/twin_trace.sh`.

Accepted trajectory: `newton_passes=1854355`, `search_trials=478907`,
`backtracks=72967`, `fallbacks=12374`, `cont_successes=601`,
`restart_attempts=3`, `pivot_replays=1797328`, `unsettled=0`, hash
`fb4f55063fe2e350`. The run takes about 3 s.

Timing on this machine (2026-09-30, 6,000 callbacks): mean 393 us, p99 848 us,
max 1,141 us, none over the deadline. Timing claims below roughly 1 % need
repeated runs.

## Accepted

### The stamps' writes inlined, and the transistor's on its own block (2026-10-03)

Exact: every preset's output hash unchanged over 4 s of the take. Profiled, a
transistor stage spent about a third of `Stamper::transconductance_at` on the
call, the ground tests and the bounds compares around each write, and the
compiler had declined its `#[inline]`. Forced inline (`#[inline(always)]` on
every `Stamper` write): cycles -2.6 % on Jazz Chorus, -2.1 % on the 800RB,
-2.1 % on the SVT, -1.9 % on Brown '84 (user-space cycles, `perf stat`,
interleaved).

Then `Stamper::block`: a device's k x k block gathered into a local array, its
own write code run there on the constant positions `0..k`, and put back once,
so the writes fold to register arithmetic. A transistor makes 24 writes into
a block of 9: Jazz Chorus -5.5 %, the 800RB -5.7 % against the start. The
first try was 8 % *slower* -- the write code was a closure used on both paths,
so the compiler kept it out of line and nothing folded; written as an
always-inlined method with a closure per path, it folds. The triode, pentode
and JFET on blocks measured slower than inlined (SVT +3.4 %, Brown '84 level):
a triode with its cathode on ground writes three or four entries, a JFET eight
into nine, and the gather and scatter cost more than they save, so those keep
the direct writes. `tests`: `a_block_writes_what_the_direct_writes_do` (signed
zeros, both polarities, ground on each terminal, the refusals).

### Quadratic termination (2026-10-03)

Not exact: a solve may stop one pass before the convergence test would have
said so. Newton's corrections shrink quadratically once it is close, so with
`moved` the latest correction in tolerances and `previous` the one before, the
next would be about `moved^3 / previous^2`; when that is under a tenth of a
tolerance, the solve stops at the current point, committing the saturating
cores' flux there (`Device::commit`). Guarded: never on the DC solve or after a
line search, every device settled, and only within `QUADRATIC_REACH` = 30
tolerances, where Newton is quadratic. Without that reach the Plexi Cranked's
power stage went from 3.9 to 33.8 passes a solve with 194,000 unsettled samples:
a correction a thousand tolerances long is not yet in the quadratic regime. Two
thirds of all solves take exactly three passes, the third only confirming the
second; over 4 s of every preset, passes -7.3 % (power stages -9.0 %), no
unsettled samples, worst preset -117.5 dB re peak, median -161 dB; both frozen
baselines pass. 6 dB hotter, passes 119.0 M -> 109.6 M (-7.9 %), fallbacks
+0.3 %, still none unsettled, worst preset -101.9 dB (*Thrash Rhythm*), median
-161 dB. A reach of 100 saved -9.6 % at -102 dB, 1000 saved -12.5 % at -40 dB,
which is not a solver's tolerance any more.

The same pass was tried before as Hairer-Wanner contraction stopping (kappa =
0.1, no reach) and rejected (`docs/realtime-multi-instance.md`): the physical
cabinet differentiates cone displacement at the sample rate, and its presets
nulled only at -73 to -89 dB with unsettled samples doubled. That test
estimates the remaining error as `theta / (1 - theta)` times the step, which
stops a linearly converging solve; this one stops only where the next
correction, estimated quadratically, is a tenth of a tolerance, and only within
reach. 74 of the 100 presets measured above run the physical cabinet.

### GMIN across every junction (2026-10-03)

Not exact, and not meant to be: a physical teraohm across each diode and
transistor junction, in the current and the slope alike, where the stamps had
floored a reverse junction's slope at 1e-12 S in the Jacobian only. A node held
only by off junctions (the 800RB's limiter collector) had no answer within reach
and was walked half a volt a pass from 468,000 V. Over 4 s of every preset, 65 of
100 bit-identical, the rest -87.5 to -137 dB re peak; both frozen baselines pass;
unsettled samples catalogue-wide at +6 dB 83,046 -> 0. `dsp::device::GMIN` and
`docs/realtime-catalogue.md` have the figures.

### Step 32b switched on in the plugin

Step 32b below (skip `x -= +0.0` in the Schur coupling subtraction) was accepted
as exact, but `use_sparse_coupling_subtraction` returned `false` outside test
builds, so the plugin never ran it. On since 2026-09-30, in test builds by default
too (`GAINSTAGEFX_TEST_DISABLE_SPARSE_COUPLING_SUBTRACTION` turns it off).
Output hashes identical on the four presets checked; per-sample callback mean,
A-B-B-A pinned to one core: Puppet Master '86 -2.5 %, Recto Lead -1.6 %, Ultra
Lead -1.1 %, Chime Edge -1.0 %, Hi-Headroom Pushed -0.9 %, Machine Rage '92
-0.7 %, Jazz Chorus -0.6 %, Brown '84 level.

### The power stage's second half, speculated on the other core

What is left in the tail after the exact speedups is a cluster of hard
power-stage solves, and those are sequential: each starts from the last. A
shadow copy of the power stage (`SpecBlock` in `voice.rs`) solves each block's
second half from the block's *first* state -- skipping the first half -- on
whichever thread would otherwise wait, and where the shadow found a solve hard
the real stage starts Newton at the shadow's unknowns and device
linearisations instead of the predictor's extrapolation. Nothing the shadow
computes is output, and the real stage converges to the same test on the same
equations: a block that speculated agrees with the per-sample chain to the
solver's tolerance (-163 dB relative to peak or better on all 78 presets), and
every block path -- serial, serial with the stage worker taking the shadow,
pipelined, reclaimed, cabinet handed over -- agrees with every other to the
bit, at all five rates (`tests/pipeline.rs`). Whether a block speculates, and
every answer, depends on the signal alone.

How each choice was made:

- **Which chains.** Only those whose first half is light next to the power
  stage. The shadow's inputs are the first half's output, and it runs on the
  thread that computes them: a preamplifier as costly as its power stage hands
  the shadow its inputs no faster than the power stage consumes them, and
  leaves no thread free to run it, so the power stage waits on every solve.
  Ungated, over the catalogue (A-B-B-A, pinned, 8 s each), every preset whose
  front measured under 0.45 of its power stage gained (p99.9 -9 to -23 %) and
  every one over 0.8 lost -- Puppet Master '86 p99 +27 %, p99.9 +28 %; Texas
  Storm '83 p99.9 +32 %; the Blackface and Boutique presets +10 to +23 %. The
  gate compares the two halves' Newton passes weighted by an estimate of what
  a pass of each circuit costs (`Simulation::pass_weight`, fitted from size
  over the catalogue by `hard_samples::pass_cost_by_size`, worst error 40 %),
  averaged over ~0.7 s, and opens below 0.5. A 40 ms average followed the
  playing and was still closed for the first blocks of an attack after a quiet
  passage. Blizzard '80, which gained ungated, sits on the edge and mostly
  does not speculate.
- **Which blocks, which starts.** A block speculates when its first half had a
  solve of 10+ passes, which the power stage's thread knows when it reaches the
  split; the real stage takes the shadow's answer where the shadow needed 4+
  passes. Swept with `hard_samples::speculative_second_half` over 8/10/14/18
  and 1/4/6/10.
- **The follow.** The shadow is re-copied from the real stage at every block's
  first solve. `copy_runtime_state_from` cost 3 us a block, and that was the
  whole of an early p50 regression. The rebuild-only matrices and Schur caches
  are a pure function of the configuration, so the per-block copy takes only
  the per-sample state and the learned pivot plans (~90 ns), with a full copy
  when a control moves and once more after a source's pending rebuild. Exact:
  `a_followed_shadow_solves_exactly_as_its_source` in `tests/runtime_state.rs`
  follows every power stage at five rates across control moves, a reset and
  blocks the shadow sits out.
- **Who runs it.** Pipelined, the calling thread, in what was its spin; the
  power half waits for an answer rather than computing it, which had made the
  typical speculated block 10 us slower than not speculating. The shadow's
  progress counters sit on a cache line of their own, and the power half
  re-reads them only once it has caught up. The trailing cabinet goes after the
  shadow, not before: cabinet-first doubled the waits.
- **When.** Once the first half has shown a hard solve -- or, within 512 host
  samples of the last block that did, as soon as the calling thread is free.
  Hard attacks cluster: that window covered every speculated block among the
  thirty heaviest on Jazz Chorus and Brown '84 while starting early on 9 % and
  25 % of their blocks. Waiting for the hard solve every time left the typical
  speculated Jazz block about 10 us slower.
- **Serial blocks.** With pipelining off, the shadow on the audio thread alone
  cost 100-200 us a speculated block (Jazz Chorus serial p99.9 847 -> 1,052 us).
  The stage worker is idle then, so it is handed the shadow at the first hard
  solve (`run_shadow`); the plugin now always passes its worker and says
  separately whether to pipeline. Serial with the helper: Jazz Chorus p99.9
  -8 %, Brown '84 p99 -14 %, p99.9 -20 %.

Pipelined over the take (`tools/preset_ab.py`: A-B-B-A, pinned to two cores,
8 s each; the max is a single callback and noisy -- presets that never
speculate moved by up to 3x between identical runs):

| | mean | p50 | p99 | p99.9 |
| --- | ---: | ---: | ---: | ---: |
| Jazz Chorus | 0 % | 0 % | +1 % | -16 % |
| Jazz Clean | -1 % | 0 % | -2 % | -15 % |
| Brown '84 (full take, 3 runs) | -1 % | 0 % | -11 % | -14 % |
| Brown '78 | -1 % | 0 % | -11 % | -15 % |
| Plexi Cranked | -1 % | 0 % | -13 % | -20 % |
| Plexi Crunch | -2 % | 0 % | -16 % | -18 % |
| Blackout '80 | -1 % | 0 % | -17 % | -13 % |
| Experienced '67 | 0 % | +1 % | -8 % | -11 % |
| gated off (Puppet, Recto, Blackface, Boutique, ...) | 0 % | 0 % | 0 to +1 % | -7 to +1 % |

Across all 78 presets ungated, the median moved by at most 0.3 % in every
column, which is how the regressions above hid: a median over the catalogue is
not a check on any one preset.

The estimator's first numbers were twice too good. It copied the power stage
before the chain's first call had installed the preset's oversampling, so it
solved at 192 kHz for inputs recorded at 48 (222 passes a block against the
chain's 208): Jazz Chorus -16 % / -27 % on the heaviest 1 % / 0.1 %, against
-8 % / -23 % once corrected. `the_estimators_copies_follow_the_chain` now checks
its copies against the chain solve for solve.

### Device stamps map their nodes once; the reduced RHS accumulates four wide

Two exact changes to the per-pass pipeline of a transistor power stage,
bit-identical on all 78 presets:

- Every `Stamper` term mapped its nodes through the reduced boundary map again
  (a ground test, a lookup, a failure flag), so a transistor's six terms on
  three nodes made twenty lookups a pass. Devices now `locate` their nodes once
  and stamp through the `_at` forms; the terms, their order and their
  arithmetic are unchanged. Jazz Chorus: instructions -4.7 %, cycles -2.4 %.
- `ReducedNonlinear::prepare_rhs` spent two thirds of its time in
  column-by-column vector updates that ran two lanes wide. They now have an AVX
  build (`accumulate_rhs_responses`), elementwise within a column and in the
  same column order, so the bits are the scalar loop's. A further -6 %
  instructions, -2.3 % cycles.

Pipelined over the take: Jazz Chorus mean 264 -> 249 us, p99.9 738 -> 697 us;
Puppet Master '86 mean 340 -> 327 us.

### Kernels for the plans hard samples use

Counted on the JC-120's hard solves (10+ passes; `hard_samples.rs`), the
line-search trials are cheap -- 4.2 a hard solve and under 0.3 % of the
profile even 12 dB hot. What a hard solve pays for is its reduced LU: its plan
turns unsound (1.9 invalidations and re-learns a hard solve) and half its
replays (6.6 of 11.4) ran on plans with no kernel, through the masked replay.
Those plans are concentrated (`uncovered_plans`: the top 10 of 386 carry 87 %)
but the generator stopped at eight plans a pattern and counted them only at the
take's level. It now keeps up to sixteen, down to 0.2 % of a pattern's replays
or 5,000, and also counts every preset 6 dB hot: 123 kernels, 99.9 % of 580 M
replayed solves. Masked replays per hard solve 6.6 -> 3.7. Serially, Jazz
Chorus 6 dB hot: p99 782 -> 756 us, p99.9 1,242 -> 1,182 us; at the take's level
the tail moves ~1 %. Exact, as every kernel is.

### The source-scaled predictor, for every circuit

The Twin's predictor policy, which scales the extrapolation of the last state
step by the source's own secant and drops it through reversals and jumps, was
gated on the Twin's continuation policy. It now holds for every circuit.

Why: on the real take, the JC-120's power stage is handed source steps of
0.8 V a sample (median over its hard samples; p90 2.2 V), three times its
rated input with ~10 kHz in it. Its hard solves (10 or more passes, 0.18 % of
solves, clustered) spend 54 % of their stamps with a junction limiter holding a
transistor back. Extending the last state step through such a reversal starts
Newton on the wrong side of the edge.

Measured over the catalogue: passes -2.9 %, fallbacks -8 %, backtracks -10 %,
half-step rescues 36 -> 18. The per-callback pass tail is shorter on 41
presets and longer on 5 (the lightest: Console and Valve Mic Pre, +7-10 % on
~350 passes a block). Outputs change as two converged answers to the same
equations do: 76 of 78 presets below -120 dB re peak, the worst -117 and -101
dB. Both frozen baselines pass. Jazz Chorus pipelined: p99 ~500 -> ~475 us,
p99.9 ~900 -> ~830 us, max ~1,200 -> ~1,030 us.

### Compiled reduced LU kernels

Annotated on the JC-120's power stage, the replayed reduced LU
(`solve_masked_planned`, 34 % of Jazz Chorus and 28 % of Puppet Master '86)
spent only about 30 % of its time on arithmetic. The rest went on seeding
row masks from the values (7 %), walking their bits (15 %), testing entries
for zero, swapping, and checks. The rejected symbolic LU below failed for the
same reason: at these sizes, any bookkeeping done at run time costs as much as
the arithmetic it saves. None of this bookkeeping changes between passes. The
pattern is fixed until the next rebuild, and a learned plan is replayed for
thousands of solves.

So `examples/kernels.rs` plays the catalogue over the take and counts which
(pattern, plan) pairs are replayed. It writes `src/dsp/partition/kernels.rs`:
the masked elimination unrolled for each pair that carries traffic, with
constant indices and nothing else. Each preset is played at its own
oversampling factor and at the other one, because the learned plans depend on
the step size. 88 kernels cover 99.4 % of 402 M replayed solves. With the
shipped factors alone, 76 kernels covered those factors, but Jazz Chorus
switched to 2x lost 15 % (739 → 627 µs mean once covered). A partition works out, at each refresh, every entry a
pass can hold: its device footprint plus whatever `boundary_base`,
`merit_linear` and `coupling` hold. It uses a kernel only if the kernel's
pattern covers that, and only for the plan it has learned. A pattern entry
that is zero on a pass is multiplied through where the masked path skips it,
which leaves every finite value unchanged, so the answer is identical to the
bit.

All 78 presets' output hashes are identical over the 19 s take. Serial means
fall 7–17 %: Jazz Chorus 466 → 392 µs, Puppet Master '86 718 → 619 µs,
Blackface Normal 583 → 484 µs. Pipelined, as the plugin runs, Puppet goes
432 → 354 µs mean and 585 → 485 µs p99; its p99.9 and max are unchanged.
Tests:
- `every_compiled_kernel_is_the_masked_replay`: every kernel against the
  masked replay, including zeros in the pattern, unsound plans, vanishing
  pivots and non-finite entries;
- `the_guaranteed_pattern_holds_across_the_catalogue`: every test build checks
  the pattern on every reduced solve.

A stale table costs speed, not correctness: a kernel that no longer matches
is simply not used.

### Half-step rescue of failed solves

A failed solve used to be bounded and used as it stood. It is a click: 69
samples across the catalogue on the real take, up to the signal's own peak.
All of them failed because the step was too long. At twice the rate the
JC-120's power stage, where 22 of them were, fails none.

Every nonlinear `Simulation` now keeps a twin at twice the rate. A sample that
would end unsettled carries its state across (capacitor voltage and branch
current, inductor current, core flux and volts), takes two half steps with the
source at its midpoint on the first, then re-solves the full step from the
twin's answer. That leaves the discretisation unchanged: only the starting
point improved. If the full step still fails, the twin's converged answer and
state are committed instead. Unsettled 69 → 0. The 71 presets that never
failed are bit-identical, and the change is local on the other seven. No
measurable realtime cost. Tests: `src/dsp/time/half_step.rs`,
`tests/half_step_rescue.rs`.

### Step 32b -- exact sparse Schur-coupling subtraction

Skips `matrix[slot] -= coupling[slot]` for coupling entries that are canonical
`+0.0`, using a slot list built at rebuild time. Unconditionally bit-exact:
subtracting `+0.0` cannot change any IEEE-754 value, including `-0.0`, the
infinities and NaN. `-0.0` coupling entries are deliberately *not* skipped.
Applies to every `ReducedNonlinear` partition. Hash preserved.

### Step 33 -- wide (AVX) dense-solve and recovery kernels

**Hypothesis.** The plugin is built for the x86-64 baseline so it loads
anywhere, so every automatically vectorised loop in it is SSE2, two doubles
wide. The elimination's row update and the boundary-major recovery are
elementwise multiply-and-subtract over contiguous doubles. Widening them to
four lanes is *bit-identical*, because each lane is a separately rounded
IEEE-754 operation and no reduction is reassociated.

**Evidence that it is exact.** Before writing anything, the whole crate was
rebuilt with `-C target-cpu=x86-64-v3` (AVX2 *and* FMA on everywhere) and the
Twin trace run on it. The hash was `ae73533fafbdafdb`, unchanged. That settles
both questions at once: Rust does not contract multiply-add into FMA, and it
does not reassociate FP reductions. The shipped kernels enable only `avx`, not
`fma`, so contraction is impossible rather than merely absent.

**What that build was worth.** Three runs each, quiet machine:

| | CPU mean | CPU p99 | CPU deadline misses |
|---|---:|---:|---:|
| baseline (SSE2) | 762.1 us | 1633.1 us | 374 |
| whole crate at `x86-64-v3` | 740.6 us | 1587.8 us | 315 |
| | **-2.8 %** | **-2.8 %** | **-16 %** |

**Implementation.** `partition::avx_available()` resolves
`is_x86_feature_detected!("avx")` once, at partition construction, into a
`bool` field. Three kernels get a second instantiation behind
`#[target_feature(enable = "avx")]` around a shared `#[inline(always)]` body:
`solve_dense_planned`, `solve_dense_planned_fixed_13_reciprocal` and
`recover_boundary_major_in_place`. The scalar bodies remain and are what a
non-AVX CPU runs. Nothing about pivot order, swap semantics, elimination order
or invalidation changes.

**Results.** Same binary, `GAINSTAGEFX_TEST_DISABLE_WIDE_KERNELS=1` for the
narrow side, runs interleaved so that machine noise hits both equally. Four
paired Twin traces:

| | CPU mean | CPU p99 | CPU deadline misses |
|---|---:|---:|---:|
| narrow | 761.8 us | 1635.9 us | 376 |
| wide | 754.9 us | 1618.6 us | 355 |
| | **-0.9 %** | **-1.1 %** | **-5.4 %** |

Hash `ae73533fafbdafdb` and every counter unchanged. `dsp::partition::tests`
25/25, `--test solver` 4/4, HM-2 distortion sweep clean at every setting.

**Shared across models**, which was the requirement. Two paired runs of
`plugin::channel_layout_tests::realtime_recording` at
`GAINSTAGEFX_REALTIME_SECONDS=4`, CPU mean per 64-sample callback:

| model | mode | narrow | wide | delta |
|---|---|---:|---:|---:|
| Cali IIC+ | playback_ahead_mono | 561.75 | 553.30 | -1.50 % |
| Cali IIC+ | live_paced_mono | 557.42 | 557.89 | +0.08 % |
| Cali IIC+ | live_paced_auto_dual_mono | 564.12 | 558.45 | -1.00 % |
| Cali IIC+ | live_paced_stereo_left_only | 566.26 | 569.06 | +0.49 % |
| Cali IIC+ | live_paced_stereo_both_driven | 601.19 | 566.73 | -5.73 % |
| American 5150 | playback_ahead_mono | 585.52 | 567.62 | -3.06 % |
| American 5150 | live_paced_mono | 587.33 | 575.04 | -2.09 % |
| American 5150 | live_paced_auto_dual_mono | 578.75 | 577.18 | -0.27 % |
| American 5150 | live_paced_stereo_left_only | 579.49 | 575.63 | -0.67 % |
| American 5150 | live_paced_stereo_both_driven | 603.37 | 595.17 | -1.36 % |
| American Twin | playback_ahead_mono | 800.39 | 788.88 | -1.44 % |
| American Twin | live_paced_mono | 802.52 | 787.82 | -1.83 % |
| American Twin | live_paced_auto_dual_mono | 800.55 | 787.27 | -1.66 % |
| American Twin | live_paced_stereo_left_only | 807.04 | 790.99 | -1.99 % |
| American Twin | live_paced_stereo_both_driven | 818.21 | 805.24 | -1.58 % |
| **all** | **mean of means** | **654.26** | **643.75** | **-1.61 %** |

Fourteen of fifteen improve; the two positives are on Cali IIC+ at two runs
apiece and are inside that harness's noise.

**Accepted.** It is less than the 2.8 % the whole-crate build gets, and that
gap is the point: the rest of it is in code these three kernels do not cover --
the acoustics and cabinet filters, the oversampling stages, the tone section.
Extending the same treatment there, or raising the shipped baseline, is the
open question below.

**Open: the shipped instruction-set baseline.** 2.8 % mean and 16 % fewer
deadline misses, bit-exact, is available for the price of requiring AVX2 at
load time. That is a product decision -- a v3 binary SIGILLs on a pre-2013
Intel or pre-2017 AMD machine -- and it is not taken here.

### Profiler clock -- `CLOCK_THREAD_CPUTIME_ID` to invariant TSC

Not an optimization; a correction to the instrument. See
`docs/SOLVER_OPTIMIZATION.md`. Test-only, hash preserved.

## Rejected (do not reintroduce without new evidence)

### Jacobian-assisted initialisation (`jacobian_init.rs`) -- rejected

The previous sample's reduced Jacobian proposes the starting point, gated on the
source's step ratio (3) and fused with the first Newton pass. It was the Twin
oracle's accepted configuration but only ever compiled into test builds. Measured
2026-09-30 on the Twin oracle against the shipping solver: mean 401 against 393 us,
p99 901 against 848 us, max 1,443 against 1,141 us, and 3 callbacks over the
deadline against none. It stays test-only; the oracle no longer turns it on.

### Speculation variants -- rejected

Measured while building the speculative second half above:

- **The power half computing the shadow itself when it is behind.** Both jobs
  on the critical path: the typical speculated block ran 10 us slower than not
  speculating.
- **The cabinet before the shadow** in the calling thread's trailing loop:
  twice the waits, 5 us more a speculated block.
- **Starting the shadow only at the first hard solve, always.** Correct, but
  the power half then waits on a shadow that started late; see the eager
  window above.
- **Taking the shadow's answer where the real stage's *previous* solve was
  hard,** which needs no wait on an easy stretch: a quarter to a third of the
  gain lost against taking it where the shadow's own solve was hard (Brown '84
  heaviest 1 %: -17 % against -24 %).
- **Bounding the wait by a pass budget** (an answer usable only if the
  shadow's passes up to it fit in the real stage's first half): 1-3 points of
  the gain lost on the presets it helps, and no help for the presets it hurt,
  whose problem was the front's cost rather than the shadow's.
- **Speculating on every chain.** See the gate above.
- **A per-block (40 ms) work average for the gate.** See the gate above.
- **A shadow with its own half-step twin:** no measurable difference.
- **The full state copy each block:** 3 us a block; see the follow above.

### Coupling subtracted only over the device footprint -- rejected

Precomputing `boundary_base - coupling` outside the device footprint (exact:
the same subtraction of the same two numbers) and subtracting only at the
footprint slots each pass left the instruction count flat and cost 3-4 % of
cycles on the JC-120: its footprint covers much of the 18 x 18 matrix, and
scalar updates through a slot list are slower than the contiguous loop they
replace.

### AVX coupling subtraction -- rejected

Exact and 2.4 % fewer instructions, but no change in cycles: the loop is bound
by memory traffic, not arithmetic.

### Speculating in time on more cores -- measured, not built (2026-10-03)

`SpecBlock` speculates on a block's second half with one helper. Cut into K
segments instead, each after the first with its own shadow from the block's
start, and optionally solved a second time from the end of the previous
segment's first sweep (`dsp::time::parallel_in_time`, critical path counted in
Newton passes, every shadow on its own core). Heaviest 1 % of blocks, as a
fraction of today's passes; the floor starts every real solve at its own answer:

| rig | floor | 2 segments | 4 | 4, two sweeps | 8, two sweeps | 16, two sweeps |
|---|---:|---:|---:|---:|---:|---:|
| American 800RB Clank | 15 % | 83 % | 79 % | 73 % | 69 % | 71 % |
| Round Fuzz + 800RB, 2x | 30 % | 83 % | 80 % | 73 % | 69 % | 72 % |
| Brown '84 | 15 % | 75 % | 64 % | 65 % | 58 % | 56 % |
| Jazz Chorus | 21 % | 91 % | 93 % | 84 % | 84 % | 89 % |
| Jazz Chorus, +6 dB | 22 % | 77 % | 71 % | 70 % | 67 % | 67 % |
| Treble Boost + High Gain + 6550, 8x | 25 % | 90 % | 88 % | 83 % | 81 % | 81 % |
| helpers | | 1 | 3 | 5 | 13 | 29 |

The mean block does not shorten at all (93-107 %), the helpers' passes are 1.3
to 2 times the stage's own, and past today's two segments the heavy blocks gain
another 10-20 % for 5 to 29 more cores. The floor says the headroom is real and
large, but only for a start at the answer: a shadow that skips ahead starts from
a state too far off. And the transistor stages have more than one solution where
they clip: on the JC-120 at the take's level, plain 4 and 16 segments moved the
output by -49 dB re peak where 2 segments keep -172, a shadow's far start
landing on the other one. (At +6 dB that stage, and the 800RB at 2x, move by
-53 to -74 dB from *any* change of start, the floor included: they amplify a
perturbation, whatever made it.) Not built.

### Starting a hard solve from its analogue a period ago -- rejected

A guitar note is nearly periodic, so a hard power-stage solve was usually solved
once already, a period earlier: find the past moment whose last 24 inputs best
match now's, within 50 ms, and start Newton from the answer reached then (the
*method of analogues*; `dsp::time::analogue`, an upper bound that picks with
hindsight). On the hard solves themselves passes fell 18-30 %, but they are few
enough that the whole take saved 1-2 %, and on the 800RB at +6 dB the output
moved by -57.7 dB re peak: its op-amp clamp has more than one solution there,
and a start from a different period lands on the other one (2026-10-03).

### A three-point predictor -- rejected

Extrapolating the next solve's start from the last three settled points
(second order) instead of two was slightly worse on every rig tried: a guitar
signal turns too often for the curvature of the last three samples to say
anything about the next (2026-10-03).

### Landing a valve grid's step on its conduction kink -- rejected

A grid draws nothing below 0 V and a straight line above it, and in an SVT stage
driven 33 dB past full power a driver grid crossed that kink back and forth while
the line search crawled at a tenth of a step (`dsp::time::power_hard`). Stopping
any grid step that would cross the kink on it, held, so the next pass is
linearised there: on that rig hard solves 12,239 -> 11,952 but fallbacks 12,388
-> 14,857 and passes a solve 4.355 -> 4.361 (2026-10-03). The extra pass a
crossing costs is more than the crawl it saves.

### Junction limiting from the critical voltage -- rejected

`pnjlim` log-compresses a forward step from the old junction voltage, and on
the JC-120's hard samples 95 % of the compressed steps start below the
critical voltage, asking for 1-3 V and more, so a junction walks ~0.1 V a pass.
Taking the step up to the critical voltage whole (which `pnjlim` already allows
for a target below it) and compressing only the rest halved the limiter-held
stamps but doubled the hard solves (1,662 -> 3,007) and multiplied their
fallbacks by six: the larger steps overshoot in the feedback loop, and the line
search pays for them. The slow walk was protecting convergence.

### The Twin's continuation policy for every circuit -- rejected

Across the catalogue it matches the source-scaled predictor's tail (it
includes it), removes the half-step rescues entirely and cuts backtracks by
two thirds, but its bookkeeping costs 1-5 % of the mean (5 % on the Puppet
Master '86) for no further gain in the tail.

### No predictor -- rejected

Starting every sample from the last settled state removes some rescues but
costs 1.4 % more passes and 18 % more fallbacks across the catalogue.

### Symbolic (structurally bounded) reduced LU -- rejected

**Hypothesis.** The full-MNA `factorise` bounds its elimination with
`reach`/`depth` templates taken from the netlist, and the comment on them
records that as worth "30 to 37 per cent of the whole solve" on the preamps.
The reduced boundary solve had no such thing: it eliminates `b x b` densely.
The reduced Jacobian is only 14-57 % structurally filled, so most of a dense
`b^3/3` elimination multiplies by zero.

**The flop counts are real.** `Simulation::reduced_elimination_cost`, reported
by `examples/partition_survey`, counts multiply-subtract pairs for both,
propagating fill-in exactly the way `factorise` propagates `reach` and widening
`depth` the same way:

| model | b | dense | bounded | saved |
|---|---:|---:|---:|---:|
| Twin power stage | 13 | 650 | 275 | 58 % |
| Twin preamp | 18 | 1785 | 362 | 80 % |
| Mark IIC+ preamp | 18 | 1785 | 52 | 97 % |
| 5150 preamp | 18 | 1785 | 25 | 99 % |
| HM-2 | 26 | 5525 | 384 | 93 % |

**It was implemented and it is bit-exact.** Same pivot order, same swaps, same
reciprocal pivots, same soundness checks; only three loop bounds change, and
every operation skipped had a zero operand. The Twin hash came back
`ae73533fafbdafdb` on every variant tried.

**And it is slower.** Interleaved against the same-binary baseline of 762 us:

| variant | CPU mean |
|---|---:|
| baseline (dense) | 762 us |
| bounded, all sizes | 846 us |
| bounded, `b = 13` only | 838 us |
| bounded, `b != 13` only | 800 us |

**Why.** These matrices are too small for sparse bookkeeping, and the dense
kernel was never flop-bound to begin with. For the Twin power stage the bounds
cut 375 multiply-subtracts but the *row* loop barely shortens -- 72 rows
scanned against 78 -- so the saving comes out of the long, contiguous,
vectorised inner loops while the cost is added to the short branchy ones. On
top of that the `depth` widening pass is about 71 extra iterations per solve
and the template copies another 26, against 275 flops of actual work. The
overhead is the same order as the arithmetic it removes.

**The general lesson.** Flop counts do not rank optimizations for `b` of 13 to
26. The dense 13x13 solve runs about 1350 cycles for roughly 730 flops, which
is already ~2 cycles a flop: it is overhead- and dependency-bound, not
throughput-bound. Removing arithmetic from it does not help; widening the
arithmetic it already does (Step 33) does. The code was removed rather than
left behind a flag. `Simulation::reduced_elimination_cost` and the survey
example are kept, because they are what makes this measurable again.


Carried forward from earlier work; the measurements behind these are in the
handoff history rather than in this file.

| id | idea | why it was rejected |
|---|---|---|
| -- | broad LM / globalization | catastrophic; narrow LM rescue was correct but no realtime win and new convergence concerns |
| -- | deeper residual ray search | local wins, global regression and unsettled solves |
| -- | narrow residual cycle gate | no useful acceptance rate |
| 17 | limiter preflight | worse timing |
| 18 | fallback stamp reuse | changed exact output immediately |
| 19 | skip pre-stamp restore | exact, but slower (cache behaviour) |
| 21 | preserve baseline cache + skip restore | exact, slower |
| 22 | skip redundant candidate copy | exact, slower |
| 23 | blind chord13 | fewer exact solves, damaged convergence and timing |
| 24 | verified chord13 | slower |
| 25 | contraction chord13 | far fewer exact solves, worse mean |
| 26 | deferred internal Schur recovery | almost no Newton reduction, worse timing |
| 27 | previous-J reduced-RHS tangent predictor | -3.2 % Newton passes, worse realtime timing |
| 28 | verified current-J linear refinement | Newton count rose sharply |
| 32 | generalized precondensed Schur stamp base | ~2-2.5 % end to end, but changed FP ordering and moved the trajectory (hash `0a9be6953296c1f9`) |

Step 28's lesson is the one to keep: a better linear backward error does not
imply a better nonlinear solve. Measure Newton behaviour, not residual norms.
