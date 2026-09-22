# Solver optimization: architecture, measurements and experiments

This file is the working record for performance work on the nonlinear circuit
solver. It exists so that the repository, and not a chat log, explains where
the work stands. `docs/SOLVER_EXPERIMENTS.md` holds the experiment-by-experiment
ledger, including everything that was rejected.

Everything here was measured on the development machine unless it says
otherwise: AMD Ryzen 7 5700G (Zen 3, 8C/16T), Linux, `--release`
(`opt-level = 3`, `lto = "thin"`), default `target-cpu` (x86-64 baseline, so
SSE2 and no AVX).

## The hot path

One audio sample through a modelled voice runs, per simulation in the chain:

```
prepare_rhs                     once per sample   (Schur RHS caches)
  repeat until settled:
    build_current_reduced       stamp nonlinear devices into the b x b boundary
    solve_stamped_with_delta    dense LU replay on b x b, then internal recovery
    [line search]               trial residual evaluations on rejected steps
```

`ReducedNonlinear` (`src/dsp/partition.rs`) is the shared structure underneath
all of it. A circuit's MNA unknowns are split into a *boundary* set `b` -- every
position a nonlinear device can write -- and an *internal* set `i`, which is
purely linear and is eliminated exactly:

```
S x_b = b_b - A_bi A_ii^-1 b_i        S = A_bb - A_bi A_ii^-1 A_ib
x_i   = A_ii^-1 b_i - (A_ii^-1 A_ib) x_b
```

`A_ii`, `A_ib`, `A_bi` cannot be touched by a device, so `A_ii^-1 A_ib` (the
*recovery matrix*) and `A_bi A_ii^-1 A_ib` (the *coupling*) are built once per
rebuild, outside the audio thread. A Newton pass therefore only stamps into
`A_bb`, subtracts the cached coupling, factorises `b x b`, and recovers `i`.

## Measurement instruments

### The phase profiler was measuring itself

`GAINSTAGEFX_PROFILE_TWIN_POWER_PHASES=1` times four phases with a clock that
was `clock_gettime(CLOCK_THREAD_CPUTIME_ID)`. That clock is **not** served by
the vDSO on Linux; it is a real syscall. Measured here:

| clock | ns per call |
|---|---|
| `CLOCK_THREAD_CPUTIME_ID` | 557.8 |
| `CLOCK_MONOTONIC` (vDSO) | 19.2 |
| `rdtsc` | 8.1 |

The phases being timed are 100-400 ns each, so every phase was charged several
times its own cost, and the four totals came out close together largely because
they were all close to `calls x syscall`. The profiler now reads the invariant
TSC, calibrated once against `CLOCK_MONOTONIC`
(`partition::test_thread_cpu_time_ns`). It is test-only; production builds are
unchanged, and the Twin deterministic hash is unaffected.

`dsp::partition::tests::the_phase_profiling_clock_is_far_cheaper_than_a_solver_phase`
now guards it: it measures the clock against itself and fails if a call costs
more than 150 ns, which is several times the TSC's 8 ns and the
`CLOCK_MONOTONIC` fallback's 19 ns, and well under the syscall's 558 ns.

Any phase number recorded before this change is not comparable with one after
it. The historical figures in earlier handoffs -- `stamp_us ~1.89 s`,
`dense_solve_us ~1.67 s`, `recovery_us ~1.30 s`, `trial_us ~0.53 s` -- are
instrument-dominated and should not be used to rank work.

### Honest phase split

Twin, 384,000-sample deterministic trace, accepted solver configuration, output
hash `ae73533fafbdafdb`:

| phase | total | calls | per call | share of profiled |
|---|---:|---:|---:|---:|
| nonlinear stamping | 0.650 s | 1,922,722 | 338 ns | 38.9 % |
| dense reduced solve | 0.579 s | 1,888,508 | 307 ns | 34.6 % |
| internal recovery | 0.221 s | 1,888,508 | 117 ns | 13.2 % |
| trial residual | 0.167 s | 598,209 | 279 ns | 10.0 % |
| Jacobian initializer | 0.051 s | -- | -- | 3.1 % |
| settled check | 0.004 s | 384,000 | 11 ns | 0.3 % |
| **profiled total** | **1.672 s** | | | |

Whole-chain CPU for the same trace is about 4.57 s (762 us mean over 6,000
64-sample blocks), so the *power stage's* Newton phases are about 37 % of Twin
CPU. The rest is the preamp voice -- itself a `ReducedNonlinear` Newton solve,
just not instrumented by this profiler -- plus tone, cabinet, oversampling and
solver control.

The ranking this gives is: **stamping and the dense solve are the targets**.
Internal recovery is 13 % of the profiled phases and under 5 % of Twin CPU;
halving it would be worth well under 1 % end to end.

## Partition structure across every model

`cargo run --release --example partition_survey` prints this. `fill` is the
fraction of the `i x b` recovery matrix that is not positive zero; `span` is
what a per-column leading/trailing-zero skip would still have to touch; `runs`
is the number of contiguous nonzero runs summed over the boundary columns.

```
model                  full    b     i     i*b     fill     span   runs  cpl_fill
markiic                  35   18    17     306    10.8%    10.8%     14      8.6%
evh5150                  30   18    12     216     9.7%     9.7%     12      7.4%
twin (preamp)            37   18    19     342    17.0%    35.4%     23     19.4%
heavy_metal (HM-2)       52   26    26     676     7.0%    13.8%     26      6.2%
metal_zone (MT-2)        50   22    28     616     7.6%     9.3%     22      6.4%
ts808                    25   14    11     154    11.0%    26.0%     14      7.1%
bigmuff                  27   14    13     182    11.0%    11.0%     14      7.1%
TWIN + spk               35   13    22     286    36.7%    73.1%     59     50.3%
MARKIIC + spk            31   13    18     234    28.6%    44.0%     31     28.4%
EVH5150 + spk            31   13    18     234    28.6%    44.0%     31     28.4%
AC30_EL84 + spk          30   17    13     221    12.7%    12.7%     13     14.9%
```

Findings that matter:

* **Boundary dimensions cluster.** Every 6L6/EL34 power stage except the AC30
  and the tube-rectified Recto reduces to exactly `b = 13`. Preamps and pedals
  spread from 3 to 26, with 14 and 18 common.
* **`A_ii` is block diagonal.** A cascade's stages are separated by their own
  nonlinear device nodes, so an internal node in one stage never couples to an
  internal node in another. That is why the recovery matrix is 63-93 % zeros
  rather than dense.
* **The sparsity is scattered, not banded.** For the production Twin power
  stage the nonzeros form 59 runs averaging 1.8 entries. A contiguous-span skip
  recovers only 27 % of the work there, and a run-compressed kernel would pay
  59 loop set-ups to save 181 multiply-subtracts. For the cascades
  (`markiic`, `evh5150`, `bigmuff`) span equals fill exactly, so a span skip is
  free sparsity -- but those partitions are not where the time goes.

## Where the cycles actually are

`perf record -F 3000` over the whole Twin deterministic trace, release build with
symbols. Flat profile, self time, everything at or above 1 % of total process
CPU:

```
22.69%  ReducedNonlinear::solve_stamped_with_delta   (fixed-13 solve + recovery, inlined)
20.76%  partition::solve_dense_planned               (the generic reduced solve)
 7.25%  Triode::stamp
 2.86%  Simulation::process
 2.36%  Simulation::iterate_with_limiter_handoff
 2.35%  ReducedNonlinear::prepare_rhs
 2.30%  Chain::process
 2.13%  Pentode::stamp
 1.78%  ReducedNonlinear::finish_stamp
 1.34%  Core::stamp
 1.34%  AnyDevice::stamp
 1.27%  Stamper::conductance
 1.20%  Stamper::transconductance
 1.11%  Simulation::prepare_rhs
 ~2.7%  libm exp / pow / log
```

**Dense linear algebra is 43 % of the plugin's CPU on this trace.** Device
stamping, transcendentals included, is about 17 %.

Two things in that list are worth stating plainly because they are easy to get
wrong from the phase profiler alone:

* `solve_dense_planned` is the *generic* reduced solve, so it is not the Twin
  power stage at all -- that goes through the specialised fixed-13 kernel
  inlined into `solve_stamped_with_delta`. The 20.76 % is mostly the Twin
  **preamp**, `b = 18`, which the power-stage phase profiler never sees. Any
  reduced-solver work must therefore be judged on both.
* Divisions are not a cost. The convergence test divides once per unknown per
  Newton pass, which looks alarming and is not: summed over
  `solve_stamped_with_delta`, every `divsd` in it accounts for **0.37 %** of
  that function. They overlap with the surrounding multiply/add work.

## Answers to the standing questions

Measured, not guessed. Sources: `examples/partition_survey`, the phase profiler
and `perf`.

1. **Reduced boundary dimensions.** 3 to 26. Every 6L6/EL34 power stage except
   the AC30 and the tube-rectified Recto is exactly 13; preamps and pedals
   cluster at 14 and 18.
2. **Which dimensions dominate.** 13 (every power stage) and 18 (the Twin and
   Mark IIC+ preamps) between them account for essentially all reduced solve
   time on the Twin trace.
3. **Schur coupling sparsity.** 5-50 % filled; 6-8 % for the big pedals.
   Step 32b already exploits this.
4. **Reduced Jacobian sparsity.** 14-57 % structurally filled. See the
   *rejected* symbolic-LU experiment: sparse does not pay at this size.
5. **Which device classes dominate stamping.** Triode 7.25 %, pentode 2.13 %,
   transformer core 1.34 %, dispatch 1.34 %.
6. **Internal recovery.** 13 % of the profiled power-stage phases, under 5 % of
   whole-chain CPU. It is not a major cost and never was; the figure that said
   otherwise was the instrument.
7. **Transcendentals.** About 2.7 % of total CPU in libm (`exp`, `pow`, `log`).
   Real, but an order of magnitude below the linear algebra, and cutting it
   means changing device equations.
8. **The largest realistic upside.** The reduced dense solve, at 43 %. Not by
   removing flops from it -- see the rejected experiment -- but by making the
   flops it does wider or fewer in number of *solves*.

## Ranked opportunities, after the measurements above

1. **The reduced dense solve, 43 % of CPU.** Not by removing flops (measured:
   does not pay at these sizes), but by widening them (Step 33, done) or by
   needing fewer solves. The Twin trace takes **4.9 Newton passes per sample**
   plus 598,209 line-search trials over 384,000 samples. Anything that lowers
   the pass count is worth several times what the kernel work is, which is why
   so many rejected experiments aimed there -- and why the next attempt should
   still aim there, just with a better predictor rather than a cheaper linear
   algebra step.
2. **The instruction-set baseline.** 2.8 % mean and 16 % fewer deadline misses,
   bit-exact, for requiring AVX2 at load. A product decision, not a technical
   one. Runtime-dispatched kernels (Step 33) recover about a third of it
   without that cost; extending the same dispatch to the acoustics, cabinet,
   oversampling and tone filters would recover more.
3. **Device stamping, 17 %.** Dominated by the triode. Worth a dedicated
   profile of `Triode::stamp` before anything is attempted; the transcendentals
   inside it are only 2.7 % of total CPU, so the other 14 % is the stamping
   itself -- index translation, matrix writes and dispatch.
4. **Internal recovery, under 5 %.** Already boundary-major and now four lanes
   wide. There is little left here, and the earlier belief that there was came
   from the profiler clock.

## The transcendentals are 19 % of CPU

Grouped by shared object rather than by symbol, the same `perf` recording says:

```
77.7%  gainstagefx
19.1%  libm.so.6
 1.0%  libc and everything else
```

`libm` does not appear that large in the flat symbol profile because glibc
resolves `exp`, `pow` and `log` through ifuncs and `perf` cannot name the
selected variants, so the cost is spread over dozens of unnamed addresses.

Nineteen per cent is the *second* largest item in the plugin after the reduced
dense solve, and it comes almost entirely from the Koren tube models: a pentode
evaluation is one `exp`, one `ln_1p`, one `powf` and one `atan`, and the
`powf` exponent is the published `ex` (1.3 to 1.4 across the catalogue), so it
is a genuine general-case `pow`.

This is worth knowing and it is **not** a free win. Every route to it --
a cheaper `softplus`, a polynomial `pow` for a fixed exponent, a shared
exponential, a table -- changes the device equations and therefore the
nonlinear trajectory. It falls squarely under the "mathematically equivalent
but floating-order changed" rule: it needs a null test, spectra, cross-model
convergence and a stated argument, not a benchmark. The measurement is recorded
here so the next attempt starts from the right number and not from a guess.

Note that `ex` is never an exponent with a cheap exact identity. The catalogue
uses 1.3, 1.3328, 1.35 and 1.4; not one of them is 1.5, so `x * sqrt(x)` is not
available even as an inexact substitute for the common case.
