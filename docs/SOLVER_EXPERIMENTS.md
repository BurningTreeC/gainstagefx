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
preset, the five accepted switches at their exact values and no other
`GAINSTAGEFX_TEST_*` switch set -- and otherwise prints
`twin_solver_output_hash,checked=false,reason=...`. Switches that change speed
but not arithmetic are listed in `TRAJECTORY_NEUTRAL_SWITCHES` and leave it
armed, so a wide/narrow A/B run doubles as a proof of bit-identity.

Accepted trajectory: `newton_passes=1888508`, `search_trials=598209`,
`backtracks=106884`, `fallbacks=13568`, `cont_successes=813`,
`restart_attempts=8`, `pivot_replays=1824624`, `unsettled=0`, hash
`ae73533fafbdafdb`. The run takes about 5 s.

Timing on this machine, three consecutive baseline runs: CPU mean 763.5, 762.5,
760.2 us; p99 1637.0, 1636.3, 1625.9 us. Spread is about 0.4 %, so a timing
claim below roughly 1 % needs repeated runs.

## Accepted

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
