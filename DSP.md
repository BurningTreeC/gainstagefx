# DSP implementation notes

Current source audit: 2026-09-20, version 0.19.0 (`8e09bb2` plus working-tree experiments). Older architecture audit sections
are historical; use this document and IMPLEMENTATION_RESUME.md for current behavior.

## Host and signal path

`plugin.rs` owns one `Chain` per negotiated host channel. Actual host sample rate
is passed at initialization; `Chain::set_rate` rebuilds rate-dependent sections.
Duplicated mono advances one chain until the first different stereo block, when
full runtime history is copied into the other preallocated chain. Genuine stereo
uses independent histories and a persistent worker. Its RUNNING wait is still
unbounded: zero allocations alone does not establish hard realtime safety.

`voice.rs` preconstructs gain circuits, pedals, independent complete power stages,
73P line driver, transformer coloration, tone, speaker and acoustic processors.
The chain is input volts -> pedal -> gain/preamp -> Mark graphic where selected ->
73P line driver where selected -> selected power (inverter, valves, supply,
transformer and feedback together) -> makeup -> host-rate effects/tone/acoustics.
Power bypass does not bypass the 73P studio output driver. Model selection preserves
serialized IDs; selections and state copies use preallocated storage.

FIR oversampling supports 1/2/4/8; the current modeled-circuit cap is 2, with
expensive-pedal policy in `effective_factor`. These are existing policies, not
new solver optimizations. Nonlinear companion timesteps follow the effective
inner rate; cabinet/microphone filters follow host rate. Reported latency is 66
host samples, with wet padding and dry alignment; acoustic common first-arrival
delay is removed. Rate/latency and mono/stereo regressions live in `tests/voice.rs`,
`tests/runtime_state.rs`, `tests/modular_state.rs`, and plugin callback tests.

## Circuit equations and Newton solve

`netlist.rs` constructs validated modified nodal analysis circuits. `device.rs`
contains Koren triodes/pentodes, Shockley diodes, bipolar/JFET devices, rail-aware
and dynamic op-amps, rectifier valves and integrated-flux transformer cores.
`time.rs` uses trapezoidal capacitor/inductor companions, a separate DC operating
point, analytic Jacobians, voltage limiting, predictor, Newton iteration and
residual line search. Reactive histories advance only through the existing
success/failure policy; an unsettled solve remains reported as unsettled.

`partition.rs` eliminates linear internal unknowns with an exact Schur complement.
For the common resistor-loaded Twin power system this is 31 full unknowns, 13
boundary and 18 internal. Speaker loads change the full system. Nonlinear devices
stamp directly into the boundary matrix. The constant internal factorization,
couplings and active-RHS responses are cached; every accepted full state recovers
the eliminated unknowns. Control/rate changes refresh numeric caches without
allocating Newton workspaces. Small circuits retain full MNA when reduction is
not worthwhile. HM-2/MT-2 already have nonlinear partitioning; do not assume they
are still the original monolithic implementations.

The common 13x13 dense LU specializes fixed dimensions, uses reciprocal pivots,
and checks replayed pivot plans. An unsound replay must invalidate its plan even
when fallback factorization succeeds. Boundary-major recovery keeps each node's
subtraction order. The precondensed stamp base caches the invariant Schur matrix;
its earlier change of floating-point ordering is protected by reference tests.

Accepted-trial nonlinear caches retain device intermediates only at an exactly
matching next evaluation point. They do not reuse line-search merit. Direct
physical residual and stamped `A*x-rhs` are mathematically equivalent but differ
in floating-point operation order; substituting one for the other can change
search decisions. The fixed-13 stamped-merit specialization must preserve every
row/column multiply/add and is compared with the generic path bit for bit. The
2026-09-20 Codex continuation adds the same boundary-gather idea to the *linear
trial residual* in `begin_residual`; it is independently switchable with
`GAINSTAGEFX_TEST_DISABLE_FIXED_13_TRIAL_RESIDUAL=1` and is not accepted until
repeated A/B shows identical counters/output plus a timing benefit.

Source-scaled prediction uses the last two solution states and input-step ratio,
bounded to one previous state movement; invalid/large ratios suppress prediction.
Line search uses exact residual checks, safeguarded quadratic backtracking,
warm-started damping and a two-point nonmonotone reference with cycle rejection.
Twin-specific late continuation/restart and convergent-tail recovery retain their
original convergence tests. The provisional convergent-search release returns to
plain Newton only after repeated full-step strong contraction and rearms search
when contraction stops; recovery windows exclude it. See the progress log for
measured KEEP/REJECT decisions rather than inferring acceptance from presence.

## Instrumentation and validation

`GAINSTAGEFX_PROFILE_TWIN_POWER_PHASES=1` measures stamp/LU/recovery/residual/check
CPU time. `GAINSTAGEFX_PROFILE_SOLVER_CONTROL_TAIL=1` counts Newton/search/limiter/
predictor/continuation/pivot work and retains worst samples in fixed arrays.
Both are test-only, configured at construction; production has neither counters
nor per-sample environment access. Time measurements with profiling enabled must
be reported separately from unprofiled runs. `tools/solver_ab.py` performs
alternating repeated unprofiled A/B runs from one release test executable and can
require exact control-counter and complete-output-hash equality for arithmetic-only
experiments.

Use the release-only `twin_realtime_recording_solver_trace` for the bundled
recording at 48 kHz/64 samples. The target remains p99 and maximum below 1333.33 us,
zero CPU overruns and zero unsettled solves. Linux thread CPU time excludes
preemption and other threads; wall time still matters to real callback deadlines.
Neither an unpaced mono trace nor a median of repeated maxima proves DAW realtime
safety. Protect full-reference comparisons, runtime-state continuity, five common
sample rates, and callback allocation guards. Never regenerate the frozen legacy
fixture to mask a regression.

## Research and next decisions

Sources read on 2026-09-19:

- [KINSOL mathematics](https://sundials.readthedocs.io/en/latest/kinsol/Mathematics_link.html),
  sections on Jacobian refresh, globalization and stopping: full Newton steps become
  useful near a root; small steps alone can mean stagnation. Modified Newton needs
  explicit refresh after failed globalization. For this tiny system, blanket
  Jacobian reuse risks more iterations precisely where tail cost matters.
- [PETSc SNES manual](https://petsc.org/main/manual/snes/) and
  [backtracking implementation](https://petsc.org/main/src/snes/linesearch/impls/bt/linesearchbt.c.html):
  polynomial backtracking still checks the actual residual. Existing quadratic
  interpolation already follows that pattern; importing a large solver framework
  or changing acceptance rules needs evidence beyond average throughput.
- [taskset](https://man7.org/linux/man-pages/man1/taskset.1.html) and
  [clock_gettime](https://man7.org/linux/man-pages/man3/clock_gettime.3.html):
  affinity is inherited by test threads; thread CPU and wall latency answer
  different questions. Record affinity, governor, fixture and compiler. Do not
  benchmark concurrently with builds or suite runs.

Investigate repeated full-step tails and damped-step sequences before more policy
changes. Exact boundary gathers/fixed-size residual kernels can reduce indexing
without changing numerical trajectory. Chord/quasi-Newton, multiple pivot plans,
trust regions and generated topology kernels remain research candidates, not
accepted optimizations. Generic kernels remain the reference.


## Ceres / LM13 source audit — 2026-09-20

The current production baseline is V3.5. V3.6–V3.8.1 are historical rejected
experiments, not a starting point for new policy. Test-only dogleg and LM kernels
in the working tree do not exist in a production build. Dogleg now requires
`GAINSTAGEFX_TEST_NLSOLVE_DOGLEG_TRUST_REGION=1`; the older
`GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1` still overrides it.
Previously dogleg was inadvertently the default for library tests, so old
unqualified test timings did not necessarily describe production V3.5.

The audio path is host sample -> bounded source/history predictor -> companion
RHS -> nonlinear device stamp -> exact Schur boundary (Twin power 31 -> 13,
18 internal) -> Newton LU -> physical correction/device convergence check ->
bounded nonlinear-residual line search -> continuation/restart if needed ->
state commit. On failure, unsettled is reported and integration histories are
not advanced. Physical convergence uses the **full normalized Newton correction**
and device-settled state; residual evidence additionally controls recovery
windows. There is no Ceres gradient-, cost-, or parameter-tolerance success path.
LM acceptance returns `Pass::Moved`, never `Pass::Settled`.

The accepted evaluation cache, reciprocal-pivot LU, boundary-major recovery,
checked fixed-13 pivot replay, precondensed stamp, fixed-13 stamped merit and
trial residual remain in place. Accepted-search-merit reuse is absent. No circuit
values, tolerances, oversampling, or production iteration allowances changed.

### Sources actually inspected

The local Ceres checkout was `/home/simon/Work/GitHub/ceres-solver`, not a nested
copy in this repository. It was clean at
`fe351d5ab8cbc574f33456376bf5aa90d3162d5d`. Source links below pin that revision:

| Source | Relevant behavior and transfer decision |
|---|---|
| [LM strategy](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/levenberg_marquardt_strategy.cc) | Clamp squared Jacobian column norms to [1e-6, 1e32], use square root of diagonal/radius in the augmented solve. Rejection preserves the diagonal, divides radius by an increasing factor; acceptance resets reuse and updates radius from quality. Keep tiny fixed storage and bounded retries; do not adopt generic optimizer termination. |
| [Trust-region minimizer](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/trust_region_minimizer.cc) | Validate positive model reduction before nonlinear evaluation. Compute predicted cost change as `-Jp dot (F + Jp/2)`. Rejected candidates leave accepted x/F/J unchanged; accepted candidates rebuild F/J. Initial Jacobi scaling is `1/(1+sqrt(column norm squared))`, then held fixed. Separate scaling from LM damping and physical voltage normalization. |
| [Step evaluator](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/trust_region_step_evaluator.cc) | Nonmonotone quality uses current and reference costs plus accumulated predicted reductions and a bounded history counter. It is not `max(current,previous)`. Research only; do not combine with a new LM trigger. |
| [Dogleg strategy](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/dogleg_strategy.cc) | Rejected radii reuse GN/Cauchy/subspace work; invalid steps increase regularization and invalidate reuse. Useful caching lesson, not evidence to revive the rejected dogleg policy. |
| [Dense normal Cholesky](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/dense_normal_cholesky_solver.cc), [factorization](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/dense_cholesky.cc), [refinement](https://github.com/ceres-solver/ceres-solver/blob/fe351d5ab8cbc574f33456376bf5aa90d3162d5d/internal/ceres/iterative_refiner.cc) | Validate factorization/finite output; bounded refinement computes residual against the original matrix and reuses factors. Normal equations square conditioning. No observed invalid LM models in these traces, so mixed precision/refinement is not the next optimization. |

[KINSOL mathematical considerations](https://sundials.readthedocs.io/en/latest/kinsol/Mathematics_link.html)
separates solution and residual scaling and treats a small step without a small
residual as possible stagnation. This supports keeping root validation independent
of a least-squares strategy. The Ceres documentation website returned HTTP 502;
the actual local source above was used instead.

### Rescue transaction and activation

LM13-v2 already bounded work to two entries per **sample**, including invalid
captures, and two nonlinear probes per entry. The model contains F, J, JᵀJ,
-JᵀF and damping norms; rejected radii reuse it. Acceptance requires rho >= 0.25,
at least 20% squared-residual decrease and a 2% improvement over the best finite
Newton-ray trial. A zero-residual Newton trial is now protected too.

A lazy model restamp changes limiter/correction history and consumes device
caches. A rejected residual probe also clears cached evaluation state. Restore
both with `DeviceCheckpoint`; `relinearise()` alone deliberately clears caches
and is insufficient. Scratch is allocated at construction, not in processing.
Accepted rescue commits device state once and immediately returns to exact Newton.
All checkpoint code is test-only along with LM.

Two narrower activation experiments were tested and removed: requiring a cycle
rejection plus complete search failure never entered LM; restricting LM to a
failed Newton ray inside the existing restart made one useful rescue but did not
improve repeated tail timing. Neither gate nor its A/B switch remains. The user's
original LM13-v2 mechanism remains explicitly opt-in for research, with rollback
fixed; it is **rejected for production** because both recording fixtures produce
one unsettled solve against V3.5's zero. See IMPLEMENTATION_PROGRESS.md.
