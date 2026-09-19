//! Running a circuit sample by sample.
//!
//! The same modified nodal analysis the AC solver uses, in real arithmetic and
//! stepped through time: reactances become companion models -- a conductance
//! and a current source standing in for what the part remembers -- and the
//! matrix is refactorised whenever something about it changes.
//!
//! Having both solvers is worth more than either alone. A linear network has
//! one right answer, and the two arrive at it by completely different routes:
//! one solves a complex system once per frequency, the other integrates
//! through time and has its spectrum taken. When they agree, both are working.
//! `the_two_solvers_agree` is the test that says so, and it is the closest
//! thing to an independent check either of them can have.

use super::device::{
    AnyDevice, Bipolar, Core, Device, Diode, Jfet, Linearisation, Mark, OpAmp, Pentode, Rectifier,
    ResidualStamper, Stamper, Transconductor, Triode, VariableResistor,
};
use super::netlist::{Adjust, Circuit, Part, GROUND};
use super::partition::{ReducedLinear, ReducedNonlinear};

/// Enable flush-to-zero and denormals-are-zero in the MXCSR register.
/// What one Newton pass achieved.
#[derive(Clone, Copy)]
enum Pass {
    /// The answer has stopped moving and every device agrees it is done.
    Settled,
    /// A step was taken and it improved matters. There is more to do.
    Moved,
    /// No step of any permitted length improved the residual. Stopping here
    /// and keeping the last good answer is the deterministic fallback; going
    /// round again would only compute the same refusal.
    Stuck,
}

#[cfg(test)]
const SOLVER_TRACE_CAPACITY: usize = 32;
#[cfg(test)]
const UNSETTLED_TRACE_CAPACITY: usize = 32;
#[cfg(test)]
const SEARCH_TRACE_TRIALS: usize = 9;
#[cfg(test)]
const TAIL_TRACE_PASSES: usize = 8;
const LAST_SETTLED_RESTART_PASSES: usize = 32;
const POST_RESTART_STAGE_PASSES: usize = 32;
/// Extra exact-target work permitted only after a full-budget solve has
/// demonstrated a sustained, full-Newton, rapidly decreasing residual tail.
///
/// This is the proven bounded rescue from the clean Twin solver baseline. The
/// topology work must stand on its own; do not hide circuit-model mistakes by
/// enlarging this emergency budget.
const EXHAUSTED_CONVERGENT_TAIL_PASSES: usize = 96;
const EXHAUSTED_TAIL_ARM_PASSES: usize = 4;
const EXHAUSTED_TAIL_STRONG_RATIO: f64 = 0.25;
const EXHAUSTED_TAIL_KEEP_RATIO: f64 = 0.95;
#[cfg(test)]
const TEST_LAST_SETTLED_RESTART_PASSES_DEFAULT: usize = LAST_SETTLED_RESTART_PASSES;

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub struct SolverTrace {
    pub solve: u64,
    pub input: f64,
    pub last_input: f64,
    pub ceiling: usize,
    pub used_passes: usize,
    pub target_passes: usize,
    pub moved: f64,
    pub before: f64,
    pub search_merit: f64,
    pub backtracks: u64,
    pub fallbacks: u64,
    pub continuation: bool,
    pub tail_deep_passes: usize,
    pub tail_deep_improvements: usize,
    pub tail_deep_weak_improvements: usize,
    pub deep_first_improvement_pass: usize,
    pub deep_last_improvement_pass: usize,
    pub deep_first_lambda: f64,
    pub deep_last_lambda: f64,
    pub deep_first_moved_before: f64,
    pub deep_first_moved_after: f64,
    pub deep_first_merit_before: f64,
    pub deep_first_merit_after: f64,
    pub deep_last_moved_before: f64,
    pub deep_last_moved_after: f64,
    pub deep_last_merit_before: f64,
    pub deep_last_merit_after: f64,
    pub post_deep_confirmation_passes: usize,
    pub post_low_residual_confirmation_passes: usize,
    pub repeat_cycle_rejections: usize,
    pub continuation_trigger_pass: usize,
    pub continuation_trigger_target_passes: usize,
    pub continuation_trigger_was_stuck: bool,
    pub continuation_trigger_moved: f64,
    pub continuation_trigger_merit: f64,
    pub continuation_midpoint: f64,
    pub continuation_midpoint_moved: f64,
    pub continuation_midpoint_stuck: bool,
    pub last_settled_restart_attempted: bool,
    pub last_settled_restart_passes: usize,
    pub last_settled_restart_backtracks: u64,
    pub last_settled_restart_fallbacks: u64,
    pub last_settled_restart_settled: bool,
    pub tail_trace_count: usize,
    pub tail_trace_passes: [usize; TAIL_TRACE_PASSES],
    pub tail_trace_here: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_reference: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_accepted_lambda: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_accepted_merit: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_trial_count: [usize; TAIL_TRACE_PASSES],
    pub tail_trace_trial_lambdas: [[f64; SEARCH_TRACE_TRIALS]; TAIL_TRACE_PASSES],
    pub tail_trace_trial_merits: [[f64; SEARCH_TRACE_TRIALS]; TAIL_TRACE_PASSES],
    pub tail_trace_moved_before: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_moved_after: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_fallback: [bool; TAIL_TRACE_PASSES],
    pub tail_trace_max_unknown: [usize; TAIL_TRACE_PASSES],
    pub tail_trace_max_norm: [f64; TAIL_TRACE_PASSES],
    pub tail_trace_unsettled_devices: [usize; TAIL_TRACE_PASSES],
    pub probe_pass: usize,
    pub probe_here: f64,
    pub probe_reference: f64,
    pub probe_trial_count: usize,
    pub probe_lambdas: [f64; SEARCH_TRACE_TRIALS],
    pub probe_merits: [f64; SEARCH_TRACE_TRIALS],
    pub probe_exact: [bool; SEARCH_TRACE_TRIALS],
    pub probe_max_correction_unknown: usize,
    pub probe_max_correction_norm: f64,
    pub probe_max_correction_abs: f64,
    pub probe_max_correction_voltage: f64,
    pub probe_max_correction_guess: f64,
    pub probe_unsettled_devices: usize,
    pub settled: bool,
}

#[cfg(test)]
impl SolverTrace {
    const EMPTY: Self = Self {
        solve: 0,
        input: 0.0,
        last_input: 0.0,
        ceiling: 0,
        used_passes: 0,
        target_passes: 0,
        moved: 0.0,
        before: 0.0,
        search_merit: 0.0,
        backtracks: 0,
        fallbacks: 0,
        continuation: false,
        tail_deep_passes: 0,
        tail_deep_improvements: 0,
        tail_deep_weak_improvements: 0,
        deep_first_improvement_pass: 0,
        deep_last_improvement_pass: 0,
        deep_first_lambda: 0.0,
        deep_last_lambda: 0.0,
        deep_first_moved_before: 0.0,
        deep_first_moved_after: 0.0,
        deep_first_merit_before: 0.0,
        deep_first_merit_after: 0.0,
        deep_last_moved_before: 0.0,
        deep_last_moved_after: 0.0,
        deep_last_merit_before: 0.0,
        deep_last_merit_after: 0.0,
        post_deep_confirmation_passes: 0,
        post_low_residual_confirmation_passes: 0,
        repeat_cycle_rejections: 0,
        continuation_trigger_pass: 0,
        continuation_trigger_target_passes: 0,
        continuation_trigger_was_stuck: false,
        continuation_trigger_moved: 0.0,
        continuation_trigger_merit: 0.0,
        continuation_midpoint: 0.0,
        continuation_midpoint_moved: 0.0,
        continuation_midpoint_stuck: false,
        last_settled_restart_attempted: false,
        last_settled_restart_passes: 0,
        last_settled_restart_backtracks: 0,
        last_settled_restart_fallbacks: 0,
        last_settled_restart_settled: false,
        tail_trace_count: 0,
        tail_trace_passes: [0; TAIL_TRACE_PASSES],
        tail_trace_here: [0.0; TAIL_TRACE_PASSES],
        tail_trace_reference: [0.0; TAIL_TRACE_PASSES],
        tail_trace_accepted_lambda: [0.0; TAIL_TRACE_PASSES],
        tail_trace_accepted_merit: [0.0; TAIL_TRACE_PASSES],
        tail_trace_trial_count: [0; TAIL_TRACE_PASSES],
        tail_trace_trial_lambdas: [[0.0; SEARCH_TRACE_TRIALS]; TAIL_TRACE_PASSES],
        tail_trace_trial_merits: [[0.0; SEARCH_TRACE_TRIALS]; TAIL_TRACE_PASSES],
        tail_trace_moved_before: [0.0; TAIL_TRACE_PASSES],
        tail_trace_moved_after: [0.0; TAIL_TRACE_PASSES],
        tail_trace_fallback: [false; TAIL_TRACE_PASSES],
        tail_trace_max_unknown: [0; TAIL_TRACE_PASSES],
        tail_trace_max_norm: [0.0; TAIL_TRACE_PASSES],
        tail_trace_unsettled_devices: [0; TAIL_TRACE_PASSES],
        probe_pass: 0,
        probe_here: 0.0,
        probe_reference: 0.0,
        probe_trial_count: 0,
        probe_lambdas: [0.0; SEARCH_TRACE_TRIALS],
        probe_merits: [0.0; SEARCH_TRACE_TRIALS],
        probe_exact: [false; SEARCH_TRACE_TRIALS],
        probe_max_correction_unknown: 0,
        probe_max_correction_norm: 0.0,
        probe_max_correction_abs: 0.0,
        probe_max_correction_voltage: 0.0,
        probe_max_correction_guess: 0.0,
        probe_unsettled_devices: 0,
        settled: false,
    };
}

/// Called once at startup. Without this, denormal f64 values cause 10-100x
/// slower arithmetic on x86/x64, producing crackling when signals get small.
pub fn enable_ftz_daz() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let mut mxcsr: i32 = 0;
        std::arch::asm!("stmxcsr [{0}]", in(reg) &mut mxcsr, options(nostack, preserves_flags));
        mxcsr |= 0x8040;
        std::arch::asm!("ldmxcsr [{0}]", in(reg) &mxcsr, options(nostack, preserves_flags));
    }
}

/// How still the solve has to get before it is called settled, in volts.
const TOLERANCE: f64 = 1e-6;

/// The relative part of the convergence test. See `iterate`.
const RELATIVE: f64 = 1e-6;
/// How many passes a sample may take before the last good answer is used
/// instead. A circuit driven somewhere absurd should go quiet, not explode.
///
/// This was briefly eight, on the reasoning that the extrapolation from the
/// previous two samples converges in two or three passes and the rest are
/// rare enough not to matter. The first half is true and the second is not.
/// Measured on the 5150 lead channel with `examples/drivecheck.rs`: at eight
/// passes a 0.82 V input reads 190 % distortion, 3.3 V reads 107 % and 6.6 V
/// reads 428 %, and at thirty-two the same three read 49 %, 48 % and 52 %. A
/// hundred per cent distortion is not a quality setting, it is an unconverged
/// solve leaving the callback. The cost is real and belongs in the
/// formulation, not in the iteration count -- see
/// `docs/experiments/solver-iteration-bound.md`.
///
/// Sixty-four rather than thirty-two. The 32 figure was measured on steady
/// sine waves and is enough for those; a pick attack is a genuine
/// discontinuity at the input, and the grid limiter in `device.rs` walks a
/// valve grid about a volt a pass above cutoff. A pick that takes the grid
/// from -30 V to +20 V is fifty passes of walking before the solve can even
/// start checking, and 32 runs out partway through. The extra passes only
/// fire on samples that were already failing -- an ordinary sample settles
/// in two to three and a half. The hard ceiling itself bounds that tail; the
/// plugin no longer hides solver bursts behind an internal audio FIFO.
const MAX_ITERATIONS: usize = 64;

/// The fewest passes the work budget may ever leave a sample.
///
/// Twelve was chosen so it would be "comfortably above where they normally
/// finish" -- and that sentence is the bug. The work budget's entire job is to
/// bind on a block that is running late, and a floor above where the solve
/// normally finishes cannot bind on anything. Measured: with the budget armed
/// the 5150 went from 5.4 % of callbacks missed to 6.0 %, and the Twin from
/// 0.7 % to 4.1 %. It could not save a single pass, and it still paid for
/// reading the clock.
///
/// BUG-008 is real and is why a floor exists at all: a permanent ceiling of
/// eight gave the 5150's lead channel 190, 107 and 428 per cent distortion.
/// But that was measured before the grid limiter and the line search took that
/// same channel from 7.69 passes a sample to 3.71. The circuits now average
/// 2.0 to 3.3 on real playing, so four is where a solve that has genuinely
/// stopped converging gets cut off, not where an ordinary one does.
///
/// The floor and the budget's steps are two different numbers for two
/// different jobs: this one stops a *permanent* starvation from a caller bug,
/// and `Budget::ceiling` decides how hard to lean on the tail of one late
/// block. Conflating them is what made the guarantee do nothing.
///
/// Twelve stands, because the budget is off -- see `plugin::Budget`. Lowering
/// this to four so the budget could bite was tried and measured: it made the
/// misses worse, because a capped solve that fails to converge spends its
/// whole allowance and hands the next sample a worse place to start from.
const PASS_FLOOR: usize = 12;

/// Maximum number of line-search trial lengths, including the full Newton
/// step.
///
/// The default, and what every circuit uses unless it has been measured to
/// want otherwise -- see `Simulation::set_backtracks`.
///
/// The full step is tried first and almost always taken, so this is the depth
/// of a path the solver rarely walks. With six trials the sequence is
/// 1, 1/2, 1/4, 1/8, 1/16 and 1/32. The cost of reaching the shortest trial is
/// therefore bounded and known.
/// How many plain Newton passes to take before the line search is worth its
/// cost.
///
/// A solve that has not converged by here is not going to on its own. Eight is
/// measured rather than chosen: the circuits here average between 3.6 and 3.9
/// passes, so a threshold of four fires on ordinary samples and charges every
/// circuit for a residual it did not need -- the 73P went from 36.1 % of
/// realtime to 41.4 % for nothing, its two unsettled solves a second unchanged.
/// Twelve is too late: the 5150's convergence failures come back.
///
/// At eight the 5150 keeps none, the Mark IIC+ and the Big Muff pay about a
/// point and a half, and the 73P pays two.
const FULL_STEPS: usize = 8;

/// How much a pass has to shrink the correction to count as converging.
///
/// Newton near the answer squares the error, so a healthy pass cuts `moved`
/// by orders of magnitude and a factor of two is a generous floor. A pass that
/// does not manage even that is not approaching the answer, it is oscillating
/// around it, and oscillation is exactly the case the line search exists for.
///
/// This is the test `FULL_STEPS` was standing in for. A count is a poor proxy:
/// tuned on the gain circuits, where a stalled solve is rare, it left the
/// 5150's power amplifier taking eight plain passes that got nowhere and then
/// converging on the ninth -- the first with the search on. A quarter of that
/// stage's samples landed on exactly nine passes, which is not a convergence
/// distribution, it is a wall.
const CONVERGING: f64 = 0.5;

/// Minimum meaningful progress for a deliberately nonmonotone line-search
/// step, expressed as a fraction of the window between the current residual
/// and the previous search residual.
///
/// The two-point reference is allowed to accept a short residual increase so
/// Newton can cross a device-conduction boundary.  A plain `there < reference`
/// test, however, also accepts a point that is numerically the *same* as the
/// previous search point by a few ulps.  The Twin power stage had one attack
/// sample that then alternated forever between two states: A -> B was accepted
/// for improving the stale reference by only ~6e-8 of the A/B residual gap,
/// and B -> A was a genuine decrease.
///
/// One millionth is the proven production threshold from v17/v20.  The v22
/// A/B recording test confirmed that tightening this to 1e-5 changes later
/// realtime trajectories and creates a different failure set, so keep the
/// proven production value.
const NONMONOTONE_MIN_PROGRESS: f64 = 1.0e-6;
#[cfg(test)]
const TEST_V21_NONMONOTONE_MIN_PROGRESS: f64 = 1.0e-5;
// Keep repeat-cycle gating deliberately narrow.  The Slaughter '95 / Twin
// solve-49465 trace has a slower 1.2e-5..1.7e-5 drift, but widening this window
// to 2e-5 changed the physical trajectory and increased the full recording
// from six to eight unsettled power solves.  Leave that case to the separate
// branch-preserving restart experiment instead of rejecting a still-useful
// nonmonotone step globally.
const REPEAT_CYCLE_MAX_PROGRESS: f64 = 1.0e-5;
const REPEAT_CYCLE_MATCH_REL: f64 = 1.0e-4;

#[inline]
fn repeat_cycle_edge(here: f64, reference: f64, there: f64) -> bool {
    if !there.is_finite() || there < here {
        return false;
    }
    let window = reference - here;
    if window <= 0.0 {
        return false;
    }
    let progress = (reference - there) / window;
    progress > NONMONOTONE_MIN_PROGRESS && progress <= REPEAT_CYCLE_MAX_PROGRESS
}

#[inline]
fn repeat_cycle_pair_matches(
    old_here: f64,
    old_reference: f64,
    here: f64,
    reference: f64,
) -> bool {
    fn close(a: f64, b: f64) -> bool {
        let scale = a.abs().max(b.abs()).max(1.0e-12);
        (a - b).abs() <= scale * REPEAT_CYCLE_MATCH_REL
    }
    close(old_here, here) && close(old_reference, reference)
}

#[inline]
fn search_trial_improves_with_progress(
    here: f64,
    reference: f64,
    there: f64,
    min_progress: f64,
) -> bool {
    if !there.is_finite() {
        return false;
    }
    if there < here {
        return true;
    }

    let window = reference - here;
    window > 0.0
        && there < reference
        && reference - there > window * min_progress
}

#[inline]
fn search_trial_improves(here: f64, reference: f64, there: f64) -> bool {
    search_trial_improves_with_progress(here, reference, there, NONMONOTONE_MIN_PROGRESS)
}

/// Pick the next damping length from a quadratic model of the exact residual
/// norm along a Newton direction.  For `phi(lambda) = ||F(x + lambda p)||^2`
/// and an exact Newton direction `J p = -F`, `phi'(0) = -2 phi(0)`.  One
/// rejected residual evaluation is therefore enough to fit the quadratic and
/// estimate its minimiser without another Jacobian or factorisation.
///
/// Keep the interpolation deliberately safeguarded: every rejection shrinks
/// by at least two and by at most ten, and a circuit may not go below the same
/// shortest step its configured dyadic search already allowed.  A bad/nonfinite
/// model therefore degenerates to the old half-step backtrack.
#[inline]
fn quadratic_backtrack_lambda(here: f64, there: f64, lambda: f64, floor: f64) -> f64 {
    let half = lambda * 0.5;
    if !here.is_finite()
        || !there.is_finite()
        || !lambda.is_finite()
        || here <= 0.0
        || lambda <= 0.0
    {
        return half.max(floor);
    }

    let denominator = there - here + 2.0 * here * lambda;
    if !denominator.is_finite() || denominator <= 0.0 {
        return half.max(floor);
    }

    let proposed = here * lambda * lambda / denominator;
    if !proposed.is_finite() || proposed <= 0.0 {
        return half.max(floor);
    }

    let upper = half;
    if upper <= floor {
        return floor;
    }
    let lower = (lambda * 0.1).max(floor);
    proposed.clamp(lower, upper)
}

/// Reuse useful damping information across consecutive line-search passes.
///
/// Starting every difficult Twin pass from lambda=1 repeats residual probes
/// that the preceding pass has just shown to be too aggressive. Re-expand by
/// a factor of two so a shortened step can recover toward full Newton quickly,
/// while never starting below the configured search floor.
#[inline]
fn line_search_warm_start_lambda(previous: f64, floor: f64) -> f64 {
    if !previous.is_finite() || previous <= 0.0 || !floor.is_finite() || floor <= 0.0 {
        return 1.0;
    }
    (previous * 2.0).clamp(floor, 1.0)
}

/// Whether the input slope is smooth enough for the ordinary one-sample
/// voltage extrapolator to remain a useful Newton starting point.
///
/// The predictor assumes the next source step resembles the previous one. On
/// the Twin attack trace the over-budget blocks coincide with large source
/// curvature: continuing the old voltage slope through a source reversal or a
/// >2x slope jump makes both the gain and power solves spend many passes
/// walking back from a deliberately bad starting point. Keep the proven
/// predictor on locally smooth audio and start from the last settled voltage
/// only when that assumption is visibly false.
///
/// This is a numerical starting-point policy only. It does not change the
/// circuit equations, convergence tolerance, source value or committed state.
#[inline]
fn predictor_input_slope_is_consistent(current: f64, last: f64, earlier: f64) -> bool {
    const MIN_RATIO: f64 = 0.5;
    const MAX_RATIO: f64 = 2.0;
    const QUIET: f64 = 1.0e-12;

    if !current.is_finite() || !last.is_finite() || !earlier.is_finite() {
        return false;
    }

    let previous_step = last - earlier;
    let current_step = current - last;
    if previous_step.abs() <= QUIET {
        return current_step.abs() <= QUIET;
    }

    let ratio = current_step / previous_step;
    ratio.is_finite() && (MIN_RATIO..=MAX_RATIO).contains(&ratio)
}

/// Scale the previous settled-state movement by the source's own local secant.
///
/// V3.4 proved that blindly extending `V[n-1] - V[n-2]` is expensive whenever
/// the source trajectory bends. A binary keep/drop gate fixes the worst cases,
/// but throws away useful direction information on reversals and decelerations.
///
/// Treat the last two settled samples as a secant parameterised by source
/// voltage. The current source-step ratio says how much of the previous state
/// movement is worth carrying into the next Newton starting point:
///
/// - equal source steps => 1.0, exactly the old predictor;
/// - a smaller same-direction step => a proportionally smaller state step;
/// - a reversal => a bounded step back along the settled state secant;
/// - a >2x source jump, non-finite value or first edge out of a flat source =>
///   0.0, i.e. start from the last settled state as V3.4 already does.
///
/// Never extrapolate farther than one previous state step in either direction.
/// That makes this strictly no more aggressive than the pre-V3.4 predictor.
#[inline]
fn predictor_source_scale(current: f64, last: f64, earlier: f64) -> f64 {
    const MAX_SOURCE_RATIO: f64 = 2.0;
    const QUIET: f64 = 1.0e-12;

    if !current.is_finite() || !last.is_finite() || !earlier.is_finite() {
        return 0.0;
    }

    let previous_step = last - earlier;
    let current_step = current - last;
    if previous_step.abs() <= QUIET {
        // With a flat source on both samples, keep the ordinary state predictor:
        // the reactive circuit can still be moving even when its source is not.
        return if current_step.abs() <= QUIET { 1.0 } else { 0.0 };
    }

    let ratio = current_step / previous_step;
    if !ratio.is_finite() || ratio.abs() > MAX_SOURCE_RATIO {
        return 0.0;
    }

    ratio.clamp(-1.0, 1.0)
}

#[cfg(test)]
#[test]
fn line_search_warm_start_reexpands_but_stays_bounded() {
    assert_eq!(line_search_warm_start_lambda(1.0, 0.125), 1.0);
    assert_eq!(line_search_warm_start_lambda(0.5, 0.125), 1.0);
    assert_eq!(line_search_warm_start_lambda(0.25, 0.125), 0.5);
    assert_eq!(line_search_warm_start_lambda(0.125, 0.125), 0.25);
    assert_eq!(line_search_warm_start_lambda(0.01, 0.125), 0.125);
    assert_eq!(line_search_warm_start_lambda(f64::NAN, 0.125), 1.0);
}

#[cfg(test)]
#[test]
fn input_curvature_predictor_gate_keeps_smooth_slopes_and_rejects_attacks() {
    // Constant and gently changing same-direction slopes keep the predictor.
    assert!(predictor_input_slope_is_consistent(3.0, 2.0, 1.0));
    assert!(predictor_input_slope_is_consistent(2.5, 2.0, 1.0));
    assert!(predictor_input_slope_is_consistent(4.0, 2.0, 1.0));

    // A source reversal, sudden acceleration or sudden deceleration starts
    // Newton from the last settled voltage instead of extending stale motion.
    assert!(!predictor_input_slope_is_consistent(1.5, 2.0, 1.0));
    assert!(!predictor_input_slope_is_consistent(5.0, 2.0, 1.0));
    assert!(!predictor_input_slope_is_consistent(2.25, 2.0, 1.0));

    // Silence remains predictable, while the first real edge out of silence
    // deliberately suppresses extrapolation.
    assert!(predictor_input_slope_is_consistent(0.0, 0.0, 0.0));
    assert!(!predictor_input_slope_is_consistent(1.0e-3, 0.0, 0.0));
}

#[cfg(test)]
#[test]
fn source_scaled_predictor_follows_bounded_input_secant() {
    // Equal source steps are exactly the historical one-sample predictor.
    assert_eq!(predictor_source_scale(3.0, 2.0, 1.0), 1.0);
    // A smaller source step carries only the matching fraction of state motion.
    assert_eq!(predictor_source_scale(2.25, 2.0, 1.0), 0.25);
    // A source reversal predicts back along the settled state secant.
    assert_eq!(predictor_source_scale(1.5, 2.0, 1.0), -0.5);
    // Acceleration is bounded to the old predictor's one-state-step reach.
    assert_eq!(predictor_source_scale(4.0, 2.0, 1.0), 1.0);
    // A very large jump and the first edge out of a flat source stay at V[n-1].
    assert_eq!(predictor_source_scale(5.0, 2.0, 1.0), 0.0);
    assert_eq!(predictor_source_scale(1.0e-3, 0.0, 0.0), 0.0);
    // A flat source can still have moving reactive state, so retain prediction.
    assert_eq!(predictor_source_scale(0.0, 0.0, 0.0), 1.0);
}

#[cfg(test)]
#[test]
fn quadratic_backtracking_is_safeguarded_and_can_skip_dyadic_trials() {
    // A wildly bad full step predicts a much shorter candidate, but never
    // below the configured 1/8 Twin search floor.
    assert_eq!(quadratic_backtrack_lambda(1.0, 100.0, 1.0, 0.125), 0.125);
    // A modest rejection stays inside the safeguard interval rather than
    // inventing a longer step.
    let next = quadratic_backtrack_lambda(1.0, 2.0, 1.0, 0.125);
    assert!((0.125..=0.5).contains(&next));
    // Non-finite merit falls straight back to ordinary halving.
    assert_eq!(quadratic_backtrack_lambda(1.0, f64::INFINITY, 1.0, 0.125), 0.5);
}

#[inline]
fn exact_tail_progress(
    searched: bool,
    line_search_failed: bool,
    backtracks: u64,
    previous_merit: f64,
    current_merit: f64,
    ratio: f64,
) -> bool {
    searched
        && !line_search_failed
        && backtracks == 0
        && previous_merit.is_finite()
        && current_merit.is_finite()
        && previous_merit > 0.0
        && current_merit > 0.0
        && current_merit <= previous_merit * ratio
}

#[cfg(test)]
#[test]
fn exhausted_tail_requires_sustained_full_newton_progress() {
    // The remaining Blackface Throb failures have this geometry near the
    // 64-pass wall: every line search accepts lambda=1 and the exact-target
    // merit falls by about 8-9x per pass.  That is a converging branch, not a
    // stalled solve, so it is eligible for bounded continuation.
    assert!(exact_tail_progress(
        true,
        false,
        0,
        1.088_800_617_574_289_4e72,
        1.221_162_349_945_562e71,
        EXHAUSTED_TAIL_STRONG_RATIO,
    ));

    // Any damping, fallback, increase, or A/B-style return keeps the ordinary
    // 64-pass wall in force.
    assert!(!exact_tail_progress(
        true,
        false,
        1,
        1.0,
        0.1,
        EXHAUSTED_TAIL_STRONG_RATIO,
    ));
    assert!(!exact_tail_progress(
        true,
        true,
        0,
        1.0,
        0.1,
        EXHAUSTED_TAIL_STRONG_RATIO,
    ));
    assert!(!exact_tail_progress(
        true,
        false,
        0,
        0.100_128,
        0.635_466,
        EXHAUSTED_TAIL_STRONG_RATIO,
    ));
}

#[cfg(test)]
#[test]
fn nonmonotone_search_rejects_roundoff_two_cycle_edge() {
    let here = 0.141_976_311_873_356_33;
    let reference = 0.162_927_833_355_882_32;
    let cycle_edge = 0.162_927_832_100_732_92;
    assert!(!search_trial_improves(here, reference, cycle_edge));
}

#[cfg(test)]
#[test]
fn nonmonotone_search_baseline_keeps_near_reference_realtime_step() {
    // This is intentionally accepted by the proven v20/v17 production
    // threshold.  v22's A/B mode can tighten the threshold without silently
    // changing the default solver trajectory.
    let here = 0.084_340_116_374_264_49;
    let reference = 0.633_248_107_882_544_5;
    let cycle_edge = 0.633_246_975_258_810_5;
    assert!(search_trial_improves(here, reference, cycle_edge));
}

#[cfg(test)]
#[test]
fn nonmonotone_search_rejects_near_reference_realtime_cycle() {
    // Twin recording solve 156871: the full step goes from the low-residual
    // side back almost exactly to the previous high-residual state.  It beats
    // that stale reference by only about 2e-6 of the available window and is
    // another numerical two-cycle, not useful boundary progress.
    let here = 0.084_340_116_374_264_49;
    let reference = 0.633_248_107_882_544_5;
    let cycle_edge = 0.633_246_975_258_810_5;
    assert!(!search_trial_improves_with_progress(
        here,
        reference,
        cycle_edge,
        TEST_V21_NONMONOTONE_MIN_PROGRESS,
    ));
}

#[cfg(test)]
#[test]
fn repeat_cycle_guard_requires_recurrence_of_same_pair() {
    let first_here = 0.114_945_277_989_292_97;
    let first_reference = 0.680_489_151_575_063_5;
    let first_return = 0.680_488_142_785_411_4;
    let second_here = 0.114_945_309_999_195_18;
    let second_reference = 0.680_488_142_785_411_4;
    let second_return = 0.680_487_432_007_531_6;

    assert!(repeat_cycle_edge(first_here, first_reference, first_return));
    assert!(repeat_cycle_edge(second_here, second_reference, second_return));
    assert!(repeat_cycle_pair_matches(
        first_here,
        first_reference,
        second_here,
        second_reference,
    ));

    // The known useful boundary crossing is much larger progress toward the
    // current residual, so it is not even eligible for repeat-cycle gating.
    assert!(!repeat_cycle_edge(
        0.033_794_202_049_355_44,
        0.073_825_018_786_463_82,
        0.071_505_251_319_160_08,
    ));
}

#[cfg(test)]
#[inline]
fn collect_post_restart_cycle_geometry(
    trace_unsettled: bool,
    post_restart_continuation: bool,
) -> bool {
    // The post-restart rescue uses the dominant Newton unknown as part of its
    // deliberately narrow A/B-cycle signature.  That geometry must therefore
    // be collected when the rescue experiment is enabled even if the verbose
    // GAINSTAGEFX_TRACE_UNSETTLED diagnostics are off.  Keep the deeper
    // passive line-search probes trace-only; this helper covers only the cheap
    // O(nodes) dominant-correction scan needed for the recovery predicate.
    trace_unsettled || post_restart_continuation
}

#[cfg(test)]
#[test]
fn post_restart_continuation_collects_cycle_geometry_without_trace() {
    assert!(!collect_post_restart_cycle_geometry(false, false));
    assert!(collect_post_restart_cycle_geometry(true, false));
    assert!(collect_post_restart_cycle_geometry(false, true));
    assert!(collect_post_restart_cycle_geometry(true, true));
}

#[cfg(test)]
#[inline]
fn late_continuation_rejection(
    target_passes: usize,
    min_target_passes: usize,
    line_search_failed: bool,
    ordinary_stuck: bool,
    stuck_only: bool,
) -> bool {
    target_passes >= min_target_passes
        && (ordinary_stuck || (!stuck_only && line_search_failed))
}

/// Decide whether an eligible late rejection has persisted long enough to
/// justify source continuation.
///
/// A genuine `Pass::Stuck` is still an immediate continuation trigger. A
/// recoverable line-search fallback is cheaper: the solver has already kept
/// the best finite trial and returned `Pass::Moved`. V3/V3.1 steered source
/// after one such fallback, which made the Twin's tail spend thousands of
/// midpoint solves that usually only confirmed a basin the exact-target solve
/// could recover on its next pass.
///
/// Production therefore requires two consecutive *eligible late* line-search
/// fallbacks. Any successful intervening pass clears the arm. `single_fallback`
/// is test-only A/B plumbing that reproduces the V3.1 policy.
#[inline]
fn persistent_late_continuation_rejection(
    target_passes: usize,
    min_target_passes: usize,
    line_search_failed: bool,
    ordinary_stuck: bool,
    stuck_only: bool,
    armed: &mut bool,
    single_fallback: bool,
) -> bool {
    if target_passes < min_target_passes {
        *armed = false;
        return false;
    }

    if ordinary_stuck {
        *armed = false;
        return true;
    }

    if stuck_only || !line_search_failed {
        *armed = false;
        return false;
    }

    if single_fallback || *armed {
        true
    } else {
        *armed = true;
        false
    }
}

/// Decide whether a smooth valve-grid limiter should hand the current pass to
/// the global line search.
///
/// V3 handed off on the very first limiter encounter. That cut average work,
/// but Ultra Lead telemetry showed more source-continuation entries and a
/// worse p99/max tail. One ordinary limiter correction is cheap and often
/// enough to move the predicted point back into the Newton basin. Production
/// therefore defers exactly the first limiter handoff of a Twin power solve;
/// a second limiter encounter uses the exact-Newton/global-search path.
///
/// `immediate` is test-only A/B plumbing for reproducing the V3 policy.
#[inline]
fn limiter_global_search_handoff(
    limiter_would_hold: bool,
    already_searching: bool,
    armed: &mut bool,
    immediate: bool,
) -> bool {
    if already_searching || !limiter_would_hold {
        return false;
    }
    if immediate || *armed {
        true
    } else {
        *armed = true;
        false
    }
}

#[cfg(test)]
#[test]
fn first_limiter_handoff_is_deferred_once_per_solve() {
    let mut armed = false;

    assert!(!limiter_global_search_handoff(true, false, &mut armed, false));
    assert!(armed);
    assert!(limiter_global_search_handoff(true, false, &mut armed, false));

    // An already-active line search owns the pass; the preflight adds nothing.
    assert!(!limiter_global_search_handoff(true, true, &mut armed, false));

    let mut immediate_armed = false;
    assert!(limiter_global_search_handoff(
        true,
        false,
        &mut immediate_armed,
        true,
    ));
    assert!(!immediate_armed);
}

#[cfg(test)]
#[test]
fn stuck_only_continuation_ignores_recoverable_line_search_fallbacks() {
    let late = LATE_CONTINUATION_MIN_TARGET_PASSES;

    // Production policy today: a late fallback is enough to try the midpoint.
    assert!(late_continuation_rejection(late, late, true, false, false));
    // Test policy: only an actual exhausted/stuck Newton step may steer source.
    assert!(!late_continuation_rejection(late, late, true, false, true));
    assert!(late_continuation_rejection(late, late, false, true, true));
    // Never steer early, even for a stuck step.
    assert!(!late_continuation_rejection(late - 1, late, true, true, true));
}

#[cfg(test)]
#[test]
fn persistent_late_fallback_requires_two_consecutive_rejections() {
    let late = LATE_CONTINUATION_MIN_TARGET_PASSES;
    let mut armed = false;

    // First recoverable late fallback only arms the midpoint rescue.
    assert!(!persistent_late_continuation_rejection(
        late,
        late,
        true,
        false,
        false,
        &mut armed,
        false,
    ));
    assert!(armed);

    // A second consecutive fallback is persistent enough to steer source.
    assert!(persistent_late_continuation_rejection(
        late + 1,
        late,
        true,
        false,
        false,
        &mut armed,
        false,
    ));

    // Any successful intervening pass clears the arm.
    assert!(!persistent_late_continuation_rejection(
        late + 2,
        late,
        false,
        false,
        false,
        &mut armed,
        false,
    ));
    assert!(!armed);
    assert!(!persistent_late_continuation_rejection(
        late + 3,
        late,
        true,
        false,
        false,
        &mut armed,
        false,
    ));

    // A genuinely stuck pass remains an immediate rescue trigger.
    assert!(persistent_late_continuation_rejection(
        late,
        late,
        false,
        true,
        false,
        &mut armed,
        false,
    ));

    // Test-only A/B mode reproduces V3.1's single-fallback trigger.
    let mut immediate = false;
    assert!(persistent_late_continuation_rejection(
        late,
        late,
        true,
        false,
        false,
        &mut immediate,
        true,
    ));
}

#[cfg(test)]
#[test]
fn continuation_min_target_pass_delays_fallback_steering_without_disabling_it() {
    let production = LATE_CONTINUATION_MIN_TARGET_PASSES;
    let delayed = production + 8;

    // A recoverable fallback that would steer under production remains on the
    // exact target until the configured delayed threshold is reached.
    assert!(late_continuation_rejection(
        production,
        production,
        true,
        false,
        false,
    ));
    assert!(!late_continuation_rejection(
        production,
        delayed,
        true,
        false,
        false,
    ));
    assert!(late_continuation_rejection(
        delayed,
        delayed,
        true,
        false,
        false,
    ));

    // A genuine stuck step obeys the same minimum-pass gate.
    assert!(!late_continuation_rejection(
        delayed - 1,
        delayed,
        false,
        true,
        false,
    ));
    assert!(late_continuation_rejection(
        delayed,
        delayed,
        false,
        true,
        false,
    ));
}

#[cfg(test)]
#[inline]
fn post_restart_two_cycle_signature(
    count: usize,
    here: &[f64; TAIL_TRACE_PASSES],
    accepted_lambda: &[f64; TAIL_TRACE_PASSES],
    fallback: &[bool; TAIL_TRACE_PASSES],
    max_unknown: &[usize; TAIL_TRACE_PASSES],
) -> bool {
    // This detector is deliberately failure-only.  It is evaluated only after
    // both the normal solve and the full last-settled restart have exhausted
    // their budgets, so it cannot perturb a successful realtime trajectory.
    // Do not hard-code Twin node numbers here: require two distinct dominant
    // unknowns to alternate with the measured 0.5/1.0 A/B line-search rhythm.
    if count < TAIL_TRACE_PASSES {
        return false;
    }

    let a = max_unknown[0];
    let b = max_unknown[1];
    if a == b {
        return false;
    }

    for i in 0..TAIL_TRACE_PASSES {
        if fallback[i] || !here[i].is_finite() || here[i] <= 0.0 {
            return false;
        }
        let expected_unknown = if i % 2 == 0 { a } else { b };
        let expected_lambda = if i % 2 == 0 { 0.5 } else { 1.0 };
        if max_unknown[i] != expected_unknown
            || (accepted_lambda[i] - expected_lambda).abs() > 1.0e-12
        {
            return false;
        }
    }

    let high0 = here[0];
    let low0 = here[1];
    if high0 <= low0 * 2.0 {
        return false;
    }

    fn close(a: f64, b: f64) -> bool {
        let scale = a.abs().max(b.abs()).max(1.0e-12);
        (a - b).abs() <= scale * 3.0e-4
    }

    (2..TAIL_TRACE_PASSES).all(|i| {
        if i % 2 == 0 {
            close(here[i], high0)
        } else {
            close(here[i], low0)
        }
    })
}

#[cfg(test)]
#[test]
fn post_restart_continuation_detects_slaughter_twin_two_cycle_only() {
    // Slaughter '95 / American Twin solve 49465 before the restart.  The
    // dominant correction alternates spk/pl_a in the trace, but the generic
    // solver deliberately keys on the two-state geometry instead of node IDs.
    let here = [
        0.635_474_905_366_872_9,
        0.100_128_195_774_345_97,
        0.635_465_566_573_893_9,
        0.100_120_717_560_601_84,
        0.635_457_844_752_295,
        0.100_114_531_889_541_35,
        0.635_451_456_236_436_2,
        0.100_109_412_716_881_83,
    ];
    let accepted_lambda = [0.5, 1.0, 0.5, 1.0, 0.5, 1.0, 0.5, 1.0];
    let fallback = [false; TAIL_TRACE_PASSES];
    let max_unknown = [14, 8, 14, 8, 14, 8, 14, 8];

    assert!(post_restart_two_cycle_signature(
        TAIL_TRACE_PASSES,
        &here,
        &accepted_lambda,
        &fallback,
        &max_unknown,
    ));

    // A normal descending Newton tail is not an A/B cycle and must not arm
    // the post-restart continuation rescue.
    let descending = [0.63, 0.31, 0.15, 0.07, 0.03, 0.01, 0.004, 0.001];
    assert!(!post_restart_two_cycle_signature(
        TAIL_TRACE_PASSES,
        &descending,
        &accepted_lambda,
        &fallback,
        &max_unknown,
    ));

    let mut same_unknown = max_unknown;
    same_unknown[1] = 14;
    assert!(!post_restart_two_cycle_signature(
        TAIL_TRACE_PASSES,
        &here,
        &accepted_lambda,
        &fallback,
        &same_unknown,
    ));
}

#[cfg(test)]
#[test]
fn repeat_cycle_guard_leaves_slaughter_twin_slow_drift_to_restart() {
    // Slaughter '95 / American Twin recording solve 49465.  v4.5 widened the
    // cycle window enough to reject this return, but that changed the later
    // trajectory and increased the full recording from six to eight unsettled
    // power solves.  Keep this slower drift outside the global cycle guard and
    // handle the rare failed sample with a branch-preserving restart instead.
    let first_here = 0.100_128_195_774_345_97;
    let first_reference = 0.635_474_905_366_872_9;
    let first_return = 0.635_465_566_573_893_9;
    let second_here = 0.100_120_717_560_601_84;
    let second_reference = 0.635_465_566_573_893_9;
    let second_return = 0.635_457_844_752_295;

    assert!(!repeat_cycle_edge(
        first_here,
        first_reference,
        first_return,
    ));
    assert!(!repeat_cycle_edge(
        second_here,
        second_reference,
        second_return,
    ));
    assert!(repeat_cycle_pair_matches(
        first_here,
        first_reference,
        second_here,
        second_reference,
    ));
}

#[cfg(test)]
#[inline]
fn deep_rescue_is_strong(here: f64, accepted_merit: f64) -> bool {
    here.is_finite()
        && accepted_merit.is_finite()
        && here > 0.0
        && accepted_merit <= here * 0.5
}

#[cfg(test)]
#[inline]
fn weak_deep_confirmation_limit(
    ceiling: usize,
    used_passes: usize,
    weak_deep_improvements: usize,
    search_merit: f64,
    devices_settled: bool,
    enabled: bool,
) -> usize {
    let base_limit = ceiling + POST_LOW_RESIDUAL_CONFIRMATION_PASSES;
    if enabled
        && weak_deep_improvements > 0
        && (used_passes < base_limit
            || (search_merit <= LOW_RESIDUAL_CONFIRMATION_MERIT && devices_settled))
    {
        ceiling + POST_WEAK_DEEP_CONFIRMATION_PASSES
    } else {
        base_limit
    }
}

#[cfg(test)]
#[test]
fn weak_deep_confirmation_extends_only_inside_low_residual_basin() {
    let ceiling = MAX_ITERATIONS;
    let base_limit = ceiling + POST_LOW_RESIDUAL_CONFIRMATION_PASSES;
    let extended_limit = ceiling + POST_WEAK_DEEP_CONFIRMATION_PASSES;
    assert_eq!(POST_WEAK_DEEP_CONFIRMATION_PASSES, 7);

    assert_eq!(
        weak_deep_confirmation_limit(ceiling, ceiling, 1, 5.0e-3, true, true),
        extended_limit,
    );
    assert_eq!(
        weak_deep_confirmation_limit(ceiling, base_limit, 1, 2.53e-4, true, true),
        extended_limit,
    );
    assert_eq!(
        weak_deep_confirmation_limit(ceiling, base_limit, 1, 2.0e-2, true, true),
        base_limit,
    );
    assert_eq!(
        weak_deep_confirmation_limit(ceiling, base_limit, 0, 2.53e-4, true, true),
        base_limit,
    );
}

#[cfg(test)]
#[test]
fn deep_rescue_latch_requires_material_merit_drop() {
    // Known-good deep rescues open a new basin immediately.  The hard Twin
    // synthetic cases reduce exact-target merit by roughly 74-76%.
    assert!(deep_rescue_is_strong(
        3.169_128_274_822_109e-2,
        8.331_689_401_027_355e-3,
    ));
    assert!(deep_rescue_is_strong(
        3.712_020_398_926_971e-2,
        9.000_915_312_593_347e-3,
    ));

    // v25 recording survivor 61223 accepts lambda=1/32 at pass 60, but the
    // merit only falls from about 0.09572 to 0.08983.  Accepting the step is
    // fine; treating that 6% nibble as a completed deep rescue is what this
    // experiment questions.
    assert!(!deep_rescue_is_strong(
        9.571_778_950_972_604e-2,
        8.982_900_248_648_22e-2,
    ));
}

#[cfg(test)]
#[test]
fn nonmonotone_search_keeps_real_boundary_crossing() {
    let here = 0.033_794_202_049_355_44;
    let reference = 0.073_825_018_786_463_82;
    let useful_crossing = 0.071_505_251_319_160_08;
    assert!(search_trial_improves(here, reference, useful_crossing));
}

/// Source continuation is a late rescue, not part of the normal solve. The
/// first FULL_STEPS target passes are left exactly as the pre-continuation
/// solver ran them. Only a later target pass whose complete line search fell
/// back to its best non-improving trial (or reports Stuck) may spend one of the
/// remaining pass slots on a single midpoint Newton correction.
const LATE_CONTINUATION_MIN_TARGET_PASSES: usize = 10;

/// How much bigger than the replayed pivot an entry below it may be before the
/// order is considered out of date.
///
/// Partial pivoting takes the largest, but it does not have to: any nonzero
/// pivot gives an exact LU, and the only thing at stake is how much the
/// entries grow during elimination. Circuit simulators therefore use a
/// threshold rather than the strict maximum -- KLU's default is a thousand --
/// and the value here is measured.
///
/// Taking the strict maximum makes the order look out of date constantly: the
/// 5150's power stage abandoned its plan on 34667 passes a second at a
/// threshold of one against 21643 at sixteen, and paid for a search each time.
/// Past sixteen there is nothing more to win -- what is left are real order
/// changes, tubes crossing between cutoff and conduction, which the other
/// guard catches -- so this is as loose as it has any reason to be.
///
/// The output is bit-identical at every threshold from one to a thousand. On
/// these matrices the pivot choice never changes the answer to double
/// precision; it only changes how often the search has to run.
const GROWTH: f64 = 16.0;

const MAX_BACKTRACKS: usize = 6;

/// Keep the Twin's measured four-trial search on every ordinary and early
/// continuation pass. The two extra damping trials are reserved for solves
/// already entering the final four Newton slots; broad continuation deepening
/// rescued non-failing samples around passes 12-13 and made realtime slower.
/// Tail-only deepening leaves those successful samples bit-identical while
/// giving genuinely pathological solves access to 1/16 and 1/32 steps.
const CONTINUATION_BACKTRACKS: usize = MAX_BACKTRACKS;
const CONTINUATION_DEEPENING_TAIL_PASSES: usize = 4;

/// Once a genuinely deeper continuation trial (1/16 or 1/32) has reduced the
/// exact target residual, allow a few ordinary Newton passes to finish the
/// trajectory it opened.  These are confirmation passes only: deep search is
/// latched off after the first accepted deep step, the circuit equations and
/// convergence test stay unchanged, and a host-reduced pass ceiling is never
/// exceeded.
///
/// v13 telemetry showed the useful Twin step at pass 61 for four hard solves
/// and at pass 63 for another.  In each case the following ordinary passes
/// collapsed the correction by orders of magnitude but the fixed 64-pass
/// ceiling arrived before `moved < 1`.  Three extra ordinary passes are enough
/// to test that convergence path without broadening the normal realtime path.
const POST_DEEP_CONFIRMATION_PASSES: usize = 3;

/// A different tail class reaches the normal 64-pass wall already inside a
/// low-residual basin without ever needing a deep continuation step.  The
/// recording traces for solves 145404 and 156871 ended with exact residuals
/// below 1e-2, every device locally settled, and a newly accepted improving
/// step.  Give only that state up to three ordinary confirmation passes.
///
/// This is deliberately not a general pass-budget increase: it is available
/// only at the full 64-pass ceiling, only after a successful searched pass,
/// and never overrides a host-reduced realtime ceiling.
const LOW_RESIDUAL_CONFIRMATION_MERIT: f64 = 1.0e-2;
const POST_LOW_RESIDUAL_CONFIRMATION_PASSES: usize = 3;
#[cfg(test)]
// v27 stopped solve 61223 one strict confirmation too early: pass 70 had
// exact merit 5.96e-10, accepted a full step to 2.22e-16, and left a
// correction of only ~9.00e2.  A seventh confirmation is therefore a
// deliberately tiny extension of this already test-only recovery path.
const POST_WEAK_DEEP_CONFIRMATION_PASSES: usize = 7;

/// The shortest step the line search will try, as a fraction of Newton's own.
const MIN_LAMBDA: f64 = 1.0 / 64.0;
/// The operating point is hunted once and can afford to be patient.
const DC_ITERATIONS: usize = 500;

/// What an inductor is stamped as in the direct-current matrix.
///
/// A tenth of a milliohm: a short by any measure that matters -- a hundred
/// milliamps through it drops nine microvolts -- while staying close enough to
/// the megohm grid leaks in the same matrix that the solve still converges.
/// It was a millionth of an ohm, and twelve orders of magnitude across one
/// matrix is more than the arithmetic has to give: the operating point came
/// out right and the residual never reached the tolerance, so a circuit
/// working at four hundred volts reported that it had not settled when it had.
///
/// It is also where an inductor's standing *current* comes from. See
/// `find_operating_point`.
const INDUCTOR_DC: f64 = 1e4;

/// A capacitor's companion, by the trapezoidal rule.
#[derive(Clone, Copy)]
struct Capacitor {
    a: usize,
    b: usize,
    conductance: f64,
    /// Current the companion source carries into the next step.
    history: f64,
    voltage: f64,
}

/// An inductor's, likewise.
#[derive(Clone, Copy)]
struct Inductor {
    a: usize,
    b: usize,
    conductance: f64,
    history: f64,
    current: f64,
}

pub struct Simulation {
    circuit: Circuit,
    rate: f64,
    /// What every supply is multiplied by. See `set_supply_scale`.
    supply_scale: f64,
    n: usize,
    /// Everything that does not change from sample to sample.
    base: Vec<f64>,
    /// The same, as the circuit stands at DC: capacitors open and inductors
    /// shorted, rather than carrying the companion conductances they have when
    /// time is running.
    base_dc: Vec<f64>,
    /// The working matrix, and its factorisation.
    matrix: Vec<f64>,
    pivots: Vec<usize>,
    /// Per row, the column its last nonzero sits in. See `factorise`.
    /// The circuit's *structure*, worked out once from the netlist and the
    /// devices' declared footprints rather than rescanned from the values on
    /// every factorisation.
    ///
    /// `reach_template[row]` is the last column that row can hold a nonzero
    /// in; `depth_template[col]` is the last row that column can. The
    /// factorisation used to derive the first by scanning each row from the
    /// right for its last nonzero -- which walks the trailing zeros, so on a
    /// banded matrix it read most of the row to find an entry near the
    /// diagonal -- and it never had the second at all, so the pivot search
    /// read the whole column below the diagonal every time. Between them
    /// those two scans were about half of the factorisation, and the
    /// factorisation is half of the plugin. Measured on the four preamplifier
    /// circuits, taking the structure from the netlist instead is worth 30 to
    /// 37 per cent of the whole solve, and it is bit for bit identical: the
    /// entries skipped are the ones that were zero.
    /// The most Newton passes a sample may take, when a caller has asked for
    /// less than `MAX_ITERATIONS`.
    ///
    /// This is the deliberate last resort of layer 5a, and it is **not** the
    /// mistake BUG-008 records. That was `MAX_ITERATIONS` cut from 32 to 8
    /// permanently, so every sample at high drive came out unconverged and the
    /// 5150 measured 190 per cent distortion. This is a ceiling the host's own
    /// clock lowers for the tail of a block that is running out of time, put
    /// back at the top of the next one, floored so it can never reach the
    /// figure that broke it, and counted so a session doing it is visible
    /// rather than quietly worse.
    ///
    /// The trade it makes: a handful of less-converged samples against a block
    /// the host does not get at all.
    ceiling: usize,
    /// Samples finished at the ceiling rather than by converging.
    pinched: u64,
    watching: bool,
    violations: usize,
    reach_template: Vec<usize>,
    depth_template: Vec<usize>,
    reach: Vec<usize>,
    /// The working column bounds, refreshed from `depth_template` each call.
    depth: Vec<usize>,
    /// Per row, the column its L part starts in. The other end of the same
    /// idea, used by the forward substitution.
    first: Vec<usize>,
    /// Fixed topology mask used to derive `reach_template` and
    /// `depth_template` exactly once.
    ///
    /// The circuit's wiring never changes when a pot moves or the host sample
    /// rate changes. Keeping this storage in the simulation avoids allocating
    /// the old temporary `Vec<bool>` from `map_structure()` during a rebuild,
    /// and `structure_mapped` lets later rebuilds reuse the cached bounds
    /// outright.
    structure_pattern: Vec<bool>,
    /// Matrix entries a nonlinear device may change. Built once from device
    /// footprints and used by line-search trials to restore only the dynamic
    /// part of the MNA matrix instead of copying the full n x n base again.
    device_matrix_slots: Vec<usize>,
    /// CSR-style row offsets/columns for the structurally nonzero MNA entries.
    /// The line-search merit only needs these coefficients, so it does not
    /// rescan the dense row envelope for zeros on every trial. `merit_slots`
    /// stores the already-multiplied row/column index beside each column, so
    /// the hot residual walk does not redo `row * n + col` for every entry.
    merit_row_offsets: Vec<usize>,
    merit_columns: Vec<usize>,
    merit_slots: Vec<usize>,
    structure_mapped: bool,
    /// The pivot row chosen for each column, learned once and replayed.
    ///
    /// This is KLU's refactorisation, and the reason it works here is that
    /// the entries which force a pivot come from the *linear* parts -- an
    /// inductor's or a transformer's current row, whose diagonal is zero or
    /// small -- and those do not change between Newton passes. What does
    /// change is the nonlinear device conductances, and those land on the
    /// conductance block, which a nodal matrix makes diagonally dominant by
    /// construction: the diagonal is the sum of the conductances at that node
    /// and the off-diagonals are the individual ones. So the order chosen the
    /// first time is still the order the search would choose the thousandth.
    ///
    /// Measured: removing the search outright left the 5150's power stage and
    /// the Mark IIC+'s both 19 % cheaper and their output bit-identical --
    /// identical because on these circuits the search never once picked
    /// anything but the diagonal.
    plan: Vec<usize>,
    /// Whether `plan` holds an order worth replaying. Cleared by `rebuild`,
    /// and by the guard below.
    planned: bool,
    /// How often a replayed pivot order had to be abandoned and searched
    /// again. Telemetry: a circuit that does this often is one the plan does
    /// not suit, and it should be paying for the search every time instead.
    replans: u64,
    rhs: Vec<f64>,
    /// Input and reactive contributions, constant throughout one Newton solve.
    /// Prepared before the solve; scratch rather than committed circuit state.
    fixed_rhs: Vec<f64>,
    /// Whether `rhs` currently contains this sample's complete fixed RHS.
    /// Successful reduced solves never need that full vector, so materialize
    /// it lazily only when a full-MNA fallback/debug solve is actually entered.
    rhs_fixed_loaded: bool,
    /// Previous line-search residual within this sample. Reset before each
    /// transient solve; scratch, not physical state to copy on stereo wake-up.
    search_merit: f64,
    voltage: Vec<f64>,
    capacitors: Vec<Capacitor>,
    inductors: Vec<Inductor>,
    /// What each reactance was carrying, held across a rebuild. See `rebuild`.
    carried_c: Vec<(f64, f64)>,
    carried_l: Vec<f64>,
    /// The rate the device list was built at. A device outlives a rebuild
    /// unless the timestep it was built with has moved. See `rebuild`.
    device_rate: f64,
    /// Whether there is any audio in flight to protect.
    ///
    /// True from construction, a reset, or an explicit hunt, until the first
    /// sample goes through. It decides what a moved control means: with
    /// nothing in flight the circuit should simply settle where the control
    /// now puts it, and mid-signal it must not, because settling means solving
    /// the circuit with no signal in it and then setting every capacitor from
    /// that answer. See `process`.
    at_rest: bool,
    controls: Vec<f64>,
    /// The running value of every adjustable part, by slot.
    values: Vec<f64>,
    /// The drive the primary input source injects for one volt in.
    source: Vec<f64>,
    /// Per auxiliary source, the Norton current coefficient for one volt.
    /// Flattened as [source * n + node] so no allocation is needed per sample.
    aux_source: Vec<f64>,
    /// Current voltage presented by each auxiliary input port.
    aux_values: Vec<f64>,
    /// What the rails inject, which does not depend on the signal.
    bias: Vec<f64>,
    /// Every device the solver linearises, stored inline rather than boxed.
    /// See `AnyDevice` for why.
    devices: Vec<AnyDevice>,
    /// Scratch for a Newton pass.
    work: Vec<f64>,
    /// Whether every matrix slot outside the nonlinear device footprint still
    /// matches the selected base matrix. `Some(dc)` means sparse Newton
    /// restamps may safely restore only the device footprint; `None` means a
    /// full LU factorisation has overwritten `work` and the next pass must
    /// refresh the dense base once.
    work_base_mode: Option<bool>,
    guess: Vec<f64>,
    /// Scratch for the substitution: the permuted right-hand side that
    /// `substitute` writes and reads while it solves.
    ///
    /// Held rather than allocated where it is used. This was a heap
    /// allocation on the audio thread, once per Newton pass -- about one
    /// million a second in stereo at four times oversampling. Allocating
    /// there is not merely slow: `malloc` can take a lock, and a lock on
    /// the audio thread is a dropout.
    scratch: Vec<f64>,
    /// The extrapolated Newton starting point for this sample. Kept for
    /// diagnosis and for `jump`'s log. The failure branch below does *not*
    /// use this for its bound any more -- see `recent_move` for why.
    predicted: Vec<f64>,
    /// The per-node movement from `V[n-2]` to `V[n-1]`, taken *before* the
    /// predictor runs.
    ///
    /// This is the scale the failed-iterate bound is measured in, and it
    /// has to be taken before the predictor because the predictor
    /// overwrites `voltage` with the extrapolation and shifts `earlier`
    /// up one -- after which `|voltage - earlier|` is
    /// `|V_pred - V[n-1]|`, which happens to equal the movement by
    /// accident of the extrapolation's arithmetic and stops equalling it
    /// the moment the extrapolation is skipped.
    ///
    /// It is per node because the nodes in these circuits do not share a
    /// scale -- a power tube's plate swings hundreds of volts and the
    /// speaker node swings a few -- and the bound has to be meaningful
    /// on both.
    recent_move: Vec<f64>,
    /// Whether the previous sample's solve failed to converge.
    ///
    /// When it did, extrapolating from its answer is worse than not
    /// extrapolating at all. A failed sample's answer is a bounded guess,
    /// not a converged value, and `2 V[n-1] - V[n-2]` built from it
    /// extends a guess by doubling its distance from the sample before it.
    /// On the sample after a failure the correct starting point is
    /// `V[n-1]` directly -- the bounded guess the previous sample already
    /// produced -- and letting Newton walk from there, rather than from a
    /// number that has been exaggerated by the extrapolation.
    last_was_unsettled: bool,
    /// Inputs of the last two samples whose nonlinear solve actually settled.
    /// Late source continuation must start from the same source value whose
    /// reactive/device state was committed, never from an unsettled sample.
    /// The Twin-only predictor gate also compares their source slope with the
    /// current sample before deciding whether voltage extrapolation is useful.
    last_input: f64,
    earlier_input: f64,
    /// Per-circuit numerical policy. Disabled by default; the voice catalogue
    /// currently enables it on the Twin gain and power simulations.
    late_continuation: bool,
    /// The answer before that, so the next sample can be started from where
    /// the last two were heading rather than from where the last one was.
    earlier: Vec<f64>,
    /// Whether the last stamp sits exactly at the point it was given, which
    /// is what says whether its residual may be believed.
    exact: bool,
    /// Whether the next solve may be started from an extrapolation. False if
    /// anything in the circuit switches rather than bends.
    predictable: bool,
    dirty: bool,
    /// How much work the solver is actually doing, which is the only way to
    /// tell a circuit that costs what it should from one iterating itself to
    /// a standstill.
    solves: u64,
    newton_passes: u64,
    unsettled: u64,
    rebuilds: u64,
    /// Where the devices are linearised for the stamp about to be taken.
    /// Either where the solve currently is, or the line-search trial itself.
    /// Keeping the trial here avoids a second full-vector copy on every
    /// backtracking attempt.
    point: Vec<f64>,
    /// Every device's linearisation, saved so a rejected trial can undo it.
    saved: Vec<Linearisation>,
    /// Compact boundary residual used by merit-only line-search probes. It is
    /// construction-time storage so rejected trials allocate nothing.
    trial_residual: Vec<f64>,
    /// How often the full Newton step was refused and a shorter one taken.
    backtrack_count: u64,
    /// Solves where no step of any length improved the residual.
    fallbacks: u64,
    /// Newton corrections that came back non-finite.
    nonfinite: u64,
    /// Twin gain/power samples where a source-slope discontinuity suppressed
    /// the ordinary voltage extrapolator and Newton started from the last
    /// settled state instead. Retained in the existing telemetry column.
    attack_predictor_suppressions: u64,
    /// Late one-step continuation telemetry.
    continuation_attempts: u64,
    continuation_midpoint_successes: u64,
    continuation_successes: u64,
    /// Attempts triggered by `Pass::Stuck` whose exact target solve then
    /// converged. These are true rescues: the pre-continuation solver would
    /// have stopped on that same `Stuck`.
    continuation_actual_rescues: u64,
    /// How far the last pass wanted to move, in convergence-test units. The
    /// solve loop watches it shrink; see `CONVERGING`.
    moved: f64,
    /// How short a step this circuit's line search will try. See
    /// `set_backtracks`.
    backtracks: usize,
    /// Initial damping length for the next consecutive searched pass on the
    /// same source RHS. Reset to one whenever `prepare_rhs()` changes the RHS
    /// or a pass returns to plain Newton.
    search_lambda_hint: f64,
    /// Zero right-hand side used while constructing/refactoring the cached
    /// linear partition. Stored here so a later rebuild never allocates a
    /// throwaway `vec![0.0; n]` on the audio thread.
    partition_zero_rhs: Vec<f64>,
    /// Whether the one allowed construction attempt for `linear_partition`
    /// has happened. If a topology cannot be reduced, later audio-thread
    /// rebuilds stay on the full factorised path instead of retrying an
    /// allocating constructor.
    partition_initialized: bool,
    /// Exact output-boundary reduction for linear circuits. Once created for
    /// a linear topology, its storage is refreshed in place rather than
    /// reconstructed.
    linear_partition: Option<ReducedLinear>,
    /// Fixed Schur boundary containing every terminal a nonlinear device can
    /// stamp. Linear branch-current/output unknowns are promoted only when the
    /// passive internal block requires them for an invertible partition.
    /// Device stamps therefore change only boundary coefficients/RHS, which is
    /// the structural proof that lets `ReducedNonlinear` eliminate everything
    /// else exactly before every Newton factorisation.
    nonlinear_boundary: Vec<usize>,
    /// Unknowns whose fixed RHS can change at audio rate: input/supply nodes
    /// and capacitor/inductor history injection nodes. Schur response columns
    /// for only these contributors are cached at rebuild time.
    rhs_active_nodes: Vec<usize>,
    /// Construction of a nonlinear reduction is attempted only once. If the
    /// passive internal block is singular or the reduction would not be worth
    /// its recovery cost, the simulation permanently retains the original full
    /// MNA path rather than allocating/retrying from an audio-thread rebuild.
    nonlinear_partition_initialized: bool,
    /// Transient companion-model reduction used by normal audio samples.
    nonlinear_partition: Option<ReducedNonlinear>,
    /// DC companion-model reduction. This may legitimately be unavailable even
    /// when the transient reduction works (for example if opening capacitors at
    /// DC makes the chosen internal block singular); DC then uses the original
    /// full MNA solve while realtime audio still gets the reduced path.
    nonlinear_partition_dc: Option<ReducedNonlinear>,
    #[cfg(test)]
    solver_trace: [SolverTrace; SOLVER_TRACE_CAPACITY],
    #[cfg(test)]
    solver_trace_len: usize,
    /// Dedicated failure-only trace storage. The general trace also records
    /// successful deep-rescue solves and can fill quickly on a long recording;
    /// keeping unsettled solves separate guarantees that a traced realtime pass
    /// retains every rare failure without changing production state or work.
    #[cfg(test)]
    unsettled_solver_trace: [SolverTrace; UNSETTLED_TRACE_CAPACITY],
    #[cfg(test)]
    unsettled_solver_trace_len: usize,
    #[cfg(test)]
    test_disable_continuation_deepening: bool,
    /// Test-only A/B switch for the Twin-specific handoff from smooth valve
    /// grid limiting to the global line search. Production always enables it.
    #[cfg(test)]
    test_disable_limiter_global_search: bool,
    /// Test-only A/B switch restoring V3's immediate limiter-to-global-search
    /// handoff. Production V3.1 defers exactly the first limiter encounter.
    #[cfg(test)]
    test_immediate_limiter_global_search: bool,
    /// Test-only A/B switch for safeguarded quadratic backtracking. Production
    /// always enables it on the Twin reduced line-search path.
    #[cfg(test)]
    test_disable_quadratic_backtracking: bool,
    /// Test-only A/B switch for V3.3's within-solve line-search warm start.
    /// Production keeps the bounded warm start enabled on Twin gain and power.
    #[cfg(test)]
    test_disable_line_search_warm_start: bool,
    /// Test-only A/B switch for V3.4's Twin input-curvature predictor gate.
    /// Production suppresses extrapolation only when consecutive source slopes
    /// reverse or differ by more than 2x.
    #[cfg(test)]
    test_disable_input_curvature_predictor_gate: bool,
    /// Test-only A/B switch for V3.5's bounded source-scaled state predictor.
    /// When disabled, production falls back exactly to V3.4's binary gate.
    #[cfg(test)]
    test_disable_source_scaled_predictor: bool,
    /// Test-only A/B switch for reusing a valve/core evaluation from an
    /// accepted reduced line-search point in the immediately following Newton
    /// stamp. Production enables this exact same-point cache.
    #[cfg(test)]
    test_disable_accepted_trial_device_cache: bool,
    /// Test-only cost experiment: ordinary late source continuation is allowed
    /// only after `iterate()` returns `Pass::Stuck`. A recoverable late
    /// line-search fallback stays on the exact target instead of paying for a
    /// midpoint pass. The proven 32-pass last-settled/staged rescue is
    /// independent and remains available.
    #[cfg(test)]
    test_continuation_stuck_only: bool,
    #[cfg(test)]
    test_single_late_rejection_continuation: bool,
    /// Test-only continuation cost sweep. Production begins ordinary late
    /// source continuation at `LATE_CONTINUATION_MIN_TARGET_PASSES`; this may
    /// delay that gate without changing the 32-pass last-settled/staged rescue.
    #[cfg(test)]
    test_continuation_min_target_passes: usize,
    /// Recurrence-based two-cycle guard for the late-continuation circuit
    /// policy. Unlike the rejected global 1e-5 threshold, this never rejects
    /// the first near-reference nonmonotone crossing. It acts only when
    /// essentially the same low/high residual pair reappears within two
    /// searched passes.
    cycle_armed: bool,
    cycle_age: usize,
    cycle_here: f64,
    cycle_reference: f64,
    /// Test-only weak-deep recovery experiment. A deep 1/16 or 1/32 step is
    /// still accepted, but only a material (>= 50%) exact-merit drop latches
    /// deep search off. If that alternate path reaches the ordinary
    /// low-residual confirmation window, it may spend up to three additional
    /// strict confirmations while the accepted exact-target merit remains in
    /// the low-residual basin.
    #[cfg(test)]
    test_weak_deep_recovery: bool,
    /// Test-only exact-target retry from the previous settled sample's voltage.
    /// The dynamic reactance/device history has not advanced on a failed sample,
    /// so this restarts Newton on the physically continuous branch rather than
    /// committing the alternate weak-deep trajectory.
    #[cfg(test)]
    test_last_settled_restart: bool,
    #[cfg(test)]
    test_last_settled_restart_passes: usize,
    /// Test-only follow-up for the one Slaughter/Twin sample that still fails
    /// after the full branch-preserving restart. It is allowed to steer only
    /// after restart exhaustion and only when the pre-restart tail is the
    /// measured alternating two-state cycle. The source is advanced through
    /// settled 25%, 50%, 75%, and 100% stages, each with the full test budget.
    #[cfg(test)]
    test_post_restart_continuation: bool,
    /// Transactional backups while post-restart source stepping runs. A failed
    /// recovery must not leave either voltage or junction-limiting state on a
    /// different branch for the next audio sample. Storage is reserved with
    /// the immutable topology.
    post_restart_saved_voltage: Vec<f64>,
    post_restart_saved_linearisation: Vec<Linearisation>,
    /// Nonlinear-device linearisation from the last sample that actually
    /// converged. A failed Newton solve does not advance any physical device
    /// state, and it must not advance the device limiter/linearisation state
    /// either. Keeping this tiny snapshot lets the failure path roll that
    /// purely numerical state back without restamping or allocating.
    last_settled_linearisation: Vec<Linearisation>,
    last_settled_linearisation_valid: bool,
    #[cfg(test)]
    last_search_cycle_rejected: bool,
    #[cfg(test)]
    last_search_here: f64,
    #[cfg(test)]
    last_search_accepted_lambda: f64,
    #[cfg(test)]
    last_search_accepted_merit: f64,
    #[cfg(test)]
    test_trace_search_geometry: bool,
    #[cfg(test)]
    last_search_reference: f64,
    #[cfg(test)]
    last_search_trial_count: usize,
    #[cfg(test)]
    last_search_trial_lambdas: [f64; SEARCH_TRACE_TRIALS],
    #[cfg(test)]
    last_search_trial_merits: [f64; SEARCH_TRACE_TRIALS],
    #[cfg(test)]
    last_search_trial_exact: [bool; SEARCH_TRACE_TRIALS],
    #[cfg(test)]
    last_search_max_correction_unknown: usize,
    #[cfg(test)]
    last_search_max_correction_norm: f64,
    #[cfg(test)]
    last_search_max_correction_abs: f64,
    #[cfg(test)]
    last_search_max_correction_voltage: f64,
    #[cfg(test)]
    last_search_max_correction_guess: f64,
    #[cfg(test)]
    last_search_unsettled_devices: usize,
}

impl Simulation {
    /// Resume an identically configured simulation at the source's current
    /// sample. This only copies into construction-time storage: no allocation,
    /// circuit rebuild, device construction or operating-point solve occurs.
    ///
    /// Controls/topology/rate must already agree. The destination may be dirty
    /// because it received controls while dormant, so copy the prepared numeric
    /// caches as well as the evolving physical and Newton state. Statistics
    /// remain local to each simulation and count only work it actually did.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.n, source.n);
        debug_assert_eq!(self.circuit.output, source.circuit.output);
        debug_assert_eq!(self.circuit.parts.len(), source.circuit.parts.len());
        debug_assert_eq!(self.rate, source.rate);
        debug_assert_eq!(self.controls, source.controls);
        debug_assert_eq!(self.values.len(), source.values.len());
        debug_assert_eq!(self.ceiling, source.ceiling);
        debug_assert_eq!(self.backtracks, source.backtracks);
        debug_assert_eq!(self.late_continuation, source.late_continuation);
        debug_assert_eq!(self.capacitors.len(), source.capacitors.len());
        debug_assert_eq!(self.inductors.len(), source.inductors.len());
        debug_assert_eq!(self.devices.len(), source.devices.len());

        self.base.copy_from_slice(&source.base);
        self.base_dc.copy_from_slice(&source.base_dc);
        // Adjustable values are usually static user settings, but the unified
        // Twin also stores its audio-rate LDR resistance here. Copy the values
        // with the prepared matrix/device state so a sleeping stereo channel
        // wakes at exactly the same optical-cell state as the active one.
        self.values.copy_from_slice(&source.values);
        self.source.copy_from_slice(&source.source);
        self.aux_source.copy_from_slice(&source.aux_source);
        self.aux_values.copy_from_slice(&source.aux_values);
        self.bias.copy_from_slice(&source.bias);
        self.matrix.copy_from_slice(&source.matrix);
        self.pivots.copy_from_slice(&source.pivots);
        self.reach.copy_from_slice(&source.reach);
        self.depth.copy_from_slice(&source.depth);
        self.first.copy_from_slice(&source.first);
        self.plan.copy_from_slice(&source.plan);
        self.planned = source.planned;
        self.voltage.copy_from_slice(&source.voltage);
        self.predicted.copy_from_slice(&source.predicted);
        self.earlier.copy_from_slice(&source.earlier);
        self.recent_move.copy_from_slice(&source.recent_move);
        self.last_was_unsettled = source.last_was_unsettled;
        self.last_input = source.last_input;
        self.earlier_input = source.earlier_input;
        self.last_settled_linearisation
            .copy_from_slice(&source.last_settled_linearisation);
        self.last_settled_linearisation_valid = source.last_settled_linearisation_valid;
        self.exact = source.exact;
        self.predictable = source.predictable;
        self.moved = source.moved;
        self.capacitors.copy_from_slice(&source.capacitors);
        self.inductors.copy_from_slice(&source.inductors);
        for (dst, src) in self.devices.iter_mut().zip(&source.devices) {
            dst.copy_runtime_state_from(src);
        }
        self.device_rate = source.device_rate;
        self.at_rest = source.at_rest;
        self.dirty = source.dirty;

        match (&mut self.linear_partition, &source.linear_partition) {
            (Some(dst), Some(src)) => dst.copy_runtime_state_from(src),
            (None, None) => {}
            _ => panic!("runtime copy requires identical linear partitions"),
        }
        for (dst, src) in [
            (&mut self.nonlinear_partition, &source.nonlinear_partition),
            (
                &mut self.nonlinear_partition_dc,
                &source.nonlinear_partition_dc,
            ),
        ] {
            match (dst, src) {
                (Some(dst), Some(src)) => dst.copy_runtime_state_from(src),
                (None, None) => {}
                _ => panic!("runtime copy requires identical nonlinear partitions"),
            }
        }
        // Topology masks and partition selection are immutable after `new`.
        // Work/RHS/fixed_rhs/guess/point/saved/carry buffers, RHS materialization
        // state and search_merit are overwritten before use; they contain no
        // committed sample state.
        // Force the destination through one dense refresh before it uses the
        // sparse Newton restamp path: `work` itself is deliberately not copied.
        self.work_base_mode = None;
        self.rhs_fixed_loaded = false;
    }

    pub fn new(circuit: Circuit, rate: f64) -> Self {
        let n = circuit.unknowns();
        let aux_count = circuit.aux_inputs;
        let initial_voltages = circuit.initial_voltages.clone();

        // The topology is immutable for the lifetime of a `Simulation`, so
        // size every rebuild-owned list once. `clear()` then retains exactly
        // the storage a later control/rate rebuild needs instead of growing a
        // carry/device buffer for the first time on the audio thread.
        let mut capacitor_count = 0usize;
        let mut inductor_count = 0usize;
        let mut device_count = 0usize;
        for part in &circuit.parts {
            match part {
                Part::Capacitor { .. }
                | Part::Adjustable {
                    kind: Adjust::Capacitor,
                    ..
                } => capacitor_count += 1,
                Part::Inductor { .. }
                | Part::Adjustable {
                    kind: Adjust::Inductor,
                    ..
                } => inductor_count += 1,
                Part::Diode { .. }
                | Part::Rectifier { .. }
                | Part::Triode { .. }
                | Part::Pentode { .. }
                | Part::Jfet { .. }
                | Part::Bipolar { .. }
                | Part::Transconductor { .. }
                | Part::Core { .. }
                | Part::OpAmp { .. }
                | Part::Adjustable { kind: Adjust::RealtimeResistor, .. } => device_count += 1,
                _ => {}
            }
        }

        // Build the smallest exact Schur boundary from topology. Every terminal
        // a nonlinear device can stamp must be present. An op-amp's branch
        // variable is part of that nonlinear stamp too. Linear transformer
        // branch variables and the output are deliberately left internal first;
        // rebuild will promote them only if the internal block proves singular.
        let mut nonlinear_boundary_mask = vec![false; n];
        let mut rhs_active_mask = vec![false; n];
        for (index, part) in circuit.parts.iter().enumerate() {
            if !part.is_linear() {
                for node in part.touches() {
                    if node != GROUND && node < n {
                        nonlinear_boundary_mask[node] = true;
                    }
                }
                if matches!(part, Part::OpAmp { .. }) {
                    let branch = circuit.branch_of(index);
                    if branch < n {
                        nonlinear_boundary_mask[branch] = true;
                    }
                }
            }
            match *part {
                Part::Input { node, .. } | Part::Supply { node, .. } => {
                    if node != GROUND && node < n {
                        rhs_active_mask[node] = true;
                    }
                }
                Part::Capacitor { a, b, .. }
                | Part::Inductor { a, b, .. }
                | Part::Adjustable {
                    a,
                    b,
                    kind: Adjust::Capacitor | Adjust::Inductor,
                    ..
                } => {
                    if a != GROUND && a < n {
                        rhs_active_mask[a] = true;
                    }
                    if b != GROUND && b < n {
                        rhs_active_mask[b] = true;
                    }
                }
                _ => {}
            }
        }
        let nonlinear_boundary: Vec<usize> = nonlinear_boundary_mask
            .iter()
            .enumerate()
            .filter_map(|(index, &selected)| selected.then_some(index))
            .collect();
        let rhs_active_nodes: Vec<usize> = rhs_active_mask
            .iter()
            .enumerate()
            .filter_map(|(index, &selected)| selected.then_some(index))
            .collect();

        // The middle, except where the circuit says otherwise: a front-panel
        // control the plugin does not expose still has to sit where a player
        // leaves it rather than at half its travel. See `Netlist::rest`.
        let mut controls = vec![0.5; circuit.controls.max(1)];
        for &(which, position) in &circuit.resting {
            if which < controls.len() {
                controls[which] = position;
            }
        }
        let values = circuit.adjustables.clone();
        let mut sim = Self {
            circuit,
            rate,
            supply_scale: 1.0,
            n,
            base: vec![0.0; n * n],
            base_dc: vec![0.0; n * n],
            matrix: vec![0.0; n * n],
            pivots: vec![0; n],
            ceiling: MAX_ITERATIONS,
            pinched: 0,
            watching: false,
            violations: 0,
            reach_template: vec![0; n],
            depth_template: vec![0; n],
            reach: vec![0; n],
            depth: vec![0; n],
            first: vec![0; n],
            structure_pattern: vec![false; n * n],
            device_matrix_slots: Vec::new(),
            merit_row_offsets: Vec::new(),
            merit_columns: Vec::new(),
            merit_slots: Vec::new(),
            structure_mapped: false,
            plan: vec![0; n],
            planned: false,
            replans: 0,
            rhs: vec![0.0; n],
            fixed_rhs: vec![0.0; n],
            rhs_fixed_loaded: false,
            search_merit: 0.0,
            voltage: vec![0.0; n],
            capacitors: Vec::with_capacity(capacitor_count),
            inductors: Vec::with_capacity(inductor_count),
            carried_c: Vec::with_capacity(capacitor_count),
            carried_l: Vec::with_capacity(inductor_count),
            device_rate: f64::NAN,
            at_rest: true,
            controls,
            values,
            source: vec![0.0; n],
            aux_source: vec![0.0; aux_count * n],
            aux_values: vec![0.0; aux_count],
            bias: vec![0.0; n],
            devices: Vec::with_capacity(device_count),
            work: vec![0.0; n * n],
            work_base_mode: None,
            guess: vec![0.0; n],
            scratch: vec![0.0; n],
            predicted: vec![0.0; n],
            recent_move: vec![0.0; n],
            last_was_unsettled: false,
            last_input: 0.0,
            earlier_input: 0.0,
            late_continuation: false,
            earlier: vec![0.0; n],
            exact: true,
            predictable: false,
            dirty: true,
            solves: 0,
            newton_passes: 0,
            unsettled: 0,
            rebuilds: 0,
            point: vec![0.0; n],
            saved: Vec::with_capacity(device_count),
            trial_residual: vec![0.0; n],
            post_restart_saved_voltage: vec![0.0; n],
            post_restart_saved_linearisation: Vec::with_capacity(device_count),
            last_settled_linearisation: Vec::with_capacity(device_count),
            last_settled_linearisation_valid: false,
            backtrack_count: 0,
            fallbacks: 0,
            nonfinite: 0,
            attack_predictor_suppressions: 0,
            continuation_attempts: 0,
            continuation_midpoint_successes: 0,
            continuation_successes: 0,
            continuation_actual_rescues: 0,
            moved: f64::INFINITY,
            backtracks: MAX_BACKTRACKS,
            search_lambda_hint: 1.0,
            partition_zero_rhs: vec![0.0; n],
            partition_initialized: false,
            linear_partition: None,
            nonlinear_boundary,
            rhs_active_nodes,
            nonlinear_partition_initialized: false,
            nonlinear_partition: None,
            nonlinear_partition_dc: None,
            #[cfg(test)]
            solver_trace: [SolverTrace::EMPTY; SOLVER_TRACE_CAPACITY],
            #[cfg(test)]
            solver_trace_len: 0,
            #[cfg(test)]
            unsettled_solver_trace: [SolverTrace::EMPTY; UNSETTLED_TRACE_CAPACITY],
            #[cfg(test)]
            unsettled_solver_trace_len: 0,
            #[cfg(test)]
            test_disable_continuation_deepening: std::env::var_os(
                "GAINSTAGEFX_DISABLE_CONTINUATION_DEEPENING",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_limiter_global_search: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_LIMITER_GLOBAL_SEARCH",
            )
            .is_some(),
            #[cfg(test)]
            test_immediate_limiter_global_search: std::env::var_os(
                "GAINSTAGEFX_TEST_IMMEDIATE_LIMITER_GLOBAL_SEARCH",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_quadratic_backtracking: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_QUADRATIC_BACKTRACKING",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_line_search_warm_start: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_LINE_SEARCH_WARM_START",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_input_curvature_predictor_gate: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_INPUT_CURVATURE_PREDICTOR_GATE",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_source_scaled_predictor: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_SOURCE_SCALED_PREDICTOR",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_accepted_trial_device_cache: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_ACCEPTED_TRIAL_DEVICE_CACHE",
            )
            .is_some(),
            #[cfg(test)]
            test_continuation_stuck_only: std::env::var_os(
                "GAINSTAGEFX_TEST_CONTINUATION_STUCK_ONLY",
            )
            .is_some(),
            #[cfg(test)]
            test_single_late_rejection_continuation: std::env::var_os(
                "GAINSTAGEFX_TEST_SINGLE_LATE_REJECTION_CONTINUATION",
            )
            .is_some(),
            #[cfg(test)]
            test_continuation_min_target_passes: std::env::var(
                "GAINSTAGEFX_TEST_CONTINUATION_MIN_TARGET_PASSES",
            )
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            // Test-only sweeps may move the trigger down to the first pass
            // where the bounded line search is active. Production still uses
            // LATE_CONTINUATION_MIN_TARGET_PASSES when the env var is absent.
            .map(|value| value.max(LATE_CONTINUATION_MIN_TARGET_PASSES))
            .unwrap_or(LATE_CONTINUATION_MIN_TARGET_PASSES),
            cycle_armed: false,
            cycle_age: 0,
            cycle_here: 0.0,
            cycle_reference: 0.0,
            #[cfg(test)]
            test_weak_deep_recovery: std::env::var_os(
                "GAINSTAGEFX_TEST_WEAK_DEEP_RECOVERY",
            )
            .is_some(),
            #[cfg(test)]
            test_last_settled_restart: std::env::var_os(
                "GAINSTAGEFX_TEST_LAST_SETTLED_RESTART",
            )
            .is_some(),
            #[cfg(test)]
            test_last_settled_restart_passes: std::env::var(
                "GAINSTAGEFX_TEST_LAST_SETTLED_RESTART_PASSES",
            )
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|passes| passes.clamp(1, TEST_LAST_SETTLED_RESTART_PASSES_DEFAULT))
            .unwrap_or(TEST_LAST_SETTLED_RESTART_PASSES_DEFAULT),
            #[cfg(test)]
            test_post_restart_continuation: std::env::var_os(
                "GAINSTAGEFX_TEST_POST_RESTART_CONTINUATION",
            )
            .is_some(),
            #[cfg(test)]
            last_search_cycle_rejected: false,
            #[cfg(test)]
            last_search_here: 0.0,
            #[cfg(test)]
            last_search_accepted_lambda: 0.0,
            #[cfg(test)]
            last_search_accepted_merit: 0.0,
            #[cfg(test)]
            test_trace_search_geometry: std::env::var_os("GAINSTAGEFX_TRACE_UNSETTLED")
                .is_some(),
            #[cfg(test)]
            last_search_reference: 0.0,
            #[cfg(test)]
            last_search_trial_count: 0,
            #[cfg(test)]
            last_search_trial_lambdas: [0.0; SEARCH_TRACE_TRIALS],
            #[cfg(test)]
            last_search_trial_merits: [0.0; SEARCH_TRACE_TRIALS],
            #[cfg(test)]
            last_search_trial_exact: [false; SEARCH_TRACE_TRIALS],
            #[cfg(test)]
            last_search_max_correction_unknown: 0,
            #[cfg(test)]
            last_search_max_correction_norm: 0.0,
            #[cfg(test)]
            last_search_max_correction_abs: 0.0,
            #[cfg(test)]
            last_search_max_correction_voltage: 0.0,
            #[cfg(test)]
            last_search_max_correction_guess: 0.0,
            #[cfg(test)]
            last_search_unsettled_devices: 0,
        };
        sim.rebuild();
        // Apply initial voltages from the circuit to help the DC solver
        // find the operating point for circuits with floating nodes.
        for &(node, volts) in &initial_voltages {
            if node != GROUND && node < sim.voltage.len() {
                sim.voltage[node] = volts;
            }
        }
        sim.find_operating_point();
        sim
    }

    /// Solves, Newton passes, solves that never settled, and matrix rebuilds.
    /// How big the matrix is: a row per node plus one per part that imposes a
    /// voltage. The factorisation costs the cube of this, so it is the first
    /// number to look at when a circuit turns out to be expensive.
    pub fn unknowns(&self) -> usize {
        self.n
    }

    /// Size of the realtime nonlinear solve after exact Schur reduction:
    /// `(boundary_unknowns, eliminated_internal_unknowns)`. `None` means this
    /// circuit stays on the original full-MNA path.
    pub fn nonlinear_reduction(&self) -> Option<(usize, usize)> {
        self.nonlinear_partition
            .as_ref()
            .map(|partition| (partition.boundary_len(), partition.internal_len()))
    }

    #[cfg(test)]
    pub fn nonlinear_boundary_names(&self) -> Vec<String> {
        self.nonlinear_boundary
            .iter()
            .map(|&unknown| self.circuit.node_name(unknown).to_owned())
            .collect()
    }

    /// What the line search had to do: steps refused, solves where nothing
    /// of any length helped, and non-finite Newton corrections.
    ///
    /// Separate from `statistics` so that adding to it does not change a
    /// tuple half the harnesses already destructure.
    pub fn health(&self) -> (u64, u64, u64) {
        (self.backtrack_count, self.fallbacks, self.nonfinite)
    }

    /// Tight fast-transient telemetry. The first element is retained for CSV
    /// compatibility with the abandoned predictor-suppression experiment and
    /// is always zero in this mechanism.
    pub fn continuation_health(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.attack_predictor_suppressions,
            self.continuation_attempts,
            self.continuation_midpoint_successes,
            self.continuation_successes,
            self.continuation_actual_rescues,
        )
    }

    #[cfg(test)]
    pub fn solver_trace(&self) -> &[SolverTrace] {
        &self.solver_trace[..self.solver_trace_len]
    }

    #[cfg(test)]
    pub fn unsettled_solver_trace(&self) -> &[SolverTrace] {
        &self.unsettled_solver_trace[..self.unsettled_solver_trace_len]
    }

    #[cfg(test)]
    pub fn solver_unknown_name(&self, at: usize) -> &str {
        self.circuit.node_name(at)
    }

    #[cfg(test)]
    fn push_solver_trace(&mut self, trace: SolverTrace) {
        if self.solver_trace_len < SOLVER_TRACE_CAPACITY {
            self.solver_trace[self.solver_trace_len] = trace;
            self.solver_trace_len += 1;
        }
    }

    #[cfg(test)]
    fn push_unsettled_solver_trace(&mut self, trace: SolverTrace) {
        if self.unsettled_solver_trace_len < UNSETTLED_TRACE_CAPACITY {
            self.unsettled_solver_trace[self.unsettled_solver_trace_len] = trace;
            self.unsettled_solver_trace_len += 1;
        }
    }

    #[inline(always)]
    fn limiter_global_search_enabled(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_limiter_global_search
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline(always)]
    fn immediate_limiter_global_search(&self) -> bool {
        #[cfg(test)]
        {
            self.test_immediate_limiter_global_search
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    #[inline(always)]
    fn valve_grid_limiter_would_hold(&self) -> bool {
        self.devices
            .iter()
            .any(|device| device.step_limiter_would_hold(&self.voltage))
    }

    #[inline]
    fn quadratic_backtracking_enabled(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_quadratic_backtracking
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline]
    fn line_search_warm_start_enabled(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_line_search_warm_start
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline]
    fn accepted_trial_device_cache_enabled(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_accepted_trial_device_cache
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline(always)]
    fn continuation_deepening_enabled(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_continuation_deepening
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    /// How often a replayed pivot order had to be thrown away and searched
    /// again. A circuit that does this often is one the plan does not suit.
    pub fn replans(&self) -> u64 {
        self.replans
    }

    pub fn statistics(&self) -> (u64, u64, u64, u64) {
        (
            self.solves,
            self.newton_passes,
            self.unsettled,
            self.rebuilds,
        )
    }

    pub fn is_linear(&self) -> bool {
        self.devices.is_empty()
    }

    /// Snapshot the assembled matrix and source vector of a linear circuit.
    /// This is intended for exact partitioning experiments; nonlinear
    /// simulations must remain on the normal Newton path.
    pub fn linear_system(&self) -> Option<(Vec<f64>, Vec<f64>, usize)> {
        self.is_linear()
            .then(|| (self.base.clone(), self.source.clone(), self.circuit.output))
    }

    /// Snapshot one raw nonlinear Jacobian before factorisation. This is a
    /// diagnostic surface for partitioning experiments and is not used by
    /// the realtime solver.
    pub fn jacobian_snapshot(&mut self, input: f64) -> Option<(Vec<f64>, Vec<f64>, usize)> {
        if self.is_linear() {
            return None;
        }
        if self.dirty {
            self.rebuild();
        }
        self.prepare_rhs(input, false);
        self.build_current(false, true, false);
        Some((self.work.clone(), self.rhs.clone(), self.circuit.output))
    }

    /// What a node is sitting at. Worth having: an operating point is the
    /// first thing to check when a stage sounds wrong, and it is a number the
    /// data sheet also has.
    pub fn voltage_at(&self, node: usize) -> f64 {
        self.voltage.get(node).copied().unwrap_or(0.0)
    }

    /// The operating point voltage vector. Used to share a DC solution between
    /// identical channels so the second one does not have to hunt it again.
    ///
    /// After the first sample this is the *running* state, not the operating
    /// point, so it is only meaningful to share it while `needs_operating_point`
    /// is still true. See `apply_operating_point`.
    pub fn operating_point(&self) -> &[f64] {
        &self.voltage
    }

    /// Whether this circuit still has its DC hunt in front of it: a control,
    /// sample rate, or deferred reset has made it dirty *and* there is no audio
    /// in flight to destroy.
    ///
    /// The one moment at which sharing a solution between identical channels
    /// is sound: before it, there is nothing new to share; after it, what
    /// would be shared is a running state and not an operating point.
    ///
    /// `at_rest` is the half that was missing, and leaving it out was audible.
    /// `dirty` means only that a pot has moved, and a pot moves whenever a
    /// player touches a knob -- so mid-playback this said yes once a block for
    /// as long as the knob was turning. `Plugin::process` takes that as
    /// permission to hunt channel 0's operating point and hand it to every
    /// other channel, and both halves of that are destructive with audio in
    /// flight: hunting solves the circuit *with no signal in it* and sets
    /// every capacitor from the answer, and sharing then replaces the other
    /// channels' running state with it. The audible result is the music with
    /// a block-rate transient laid over the top of it, worse the harder the
    /// circuit is driven, for exactly as long as a knob is held.
    ///
    /// `Simulation::process` has always drawn this distinction correctly --
    /// it rebuilds on `dirty` but only hunts `if self.at_rest`. This is the
    /// same distinction, which is why it is the same flag.
    pub fn needs_operating_point(&self) -> bool {
        self.dirty && self.at_rest
    }

    /// Apply an operating point from another identical simulation. Rebuilds
    /// the matrix (so component values are current), sets the node voltages,
    /// initialises the reactance state to steady state, and clears the dirty
    /// flag so the next `process` call skips the expensive DC hunt.
    ///
    /// This sets every capacitor's stored charge from the voltages it is
    /// handed, so it does not only seed a solve -- it *replaces* the running
    /// state. Calling it on a channel that is already running discards
    /// whatever that channel was doing. Measured with `examples/sharing.rs`:
    /// doing it once a block put the right channel of a TS808 three decibels
    /// from where it should have been, and made the output depend on the host
    /// buffer size. Call it only when `needs_operating_point` says there is
    /// nothing to discard.
    pub fn apply_operating_point(&mut self, voltage: &[f64]) {
        if self.dirty {
            self.rebuild();
        }
        self.voltage.copy_from_slice(voltage);
        for c in &mut self.capacitors {
            let v = across(&self.voltage, c.a, c.b);
            c.voltage = v;
            c.history = c.conductance * v;
        }
        for l in &mut self.inductors {
            l.current = INDUCTOR_DC * across(&self.voltage, l.a, l.b);
            l.history = -l.current;
        }
        self.predicted.copy_from_slice(&self.voltage);
        self.earlier.copy_from_slice(&self.voltage);
        self.last_was_unsettled = false;
        self.last_input = 0.0;
        self.earlier_input = 0.0;
        // The supplied node voltages do not carry the other simulation's
        // device limiter/linearisation bookkeeping. Let the first local
        // Newton stamp establish that state before it can become a rollback
        // point.
        self.last_settled_linearisation_valid = false;
        self.dirty = false;
    }

    pub fn controls(&self) -> usize {
        self.circuit.controls
    }

    /// Where the circuit rests a control that nobody turns, if it rests it at
    /// all. See `Netlist::rest`.
    pub fn resting_position(&self, which: usize) -> Option<f64> {
        self.circuit
            .resting
            .iter()
            .find(|&&(control, _)| control == which)
            .map(|&(_, position)| position)
    }

    /// Change an adjustable part's value. Like a control, this marks the matrix
    /// for one rebuild on the next sample; reactive state is carried across.
    pub fn set_value(&mut self, slot: usize, value: f64) {
        if let Some(current) = self.values.get_mut(slot) {
            if value.is_finite() && value > 0.0 && *current != value {
                *current = value;
                self.dirty = true;
            }
        }
    }

    /// Change an audio-rate resistor without rebuilding the circuit. The part
    /// is stamped with nonlinear devices, so Newton sees the new conductance
    /// immediately while the immutable linear matrix and reactive state stay intact.
    pub fn set_realtime_value(&mut self, slot: usize, value: f64) {
        if !value.is_finite() || value <= 0.0 {
            return;
        }
        if let Some(current) = self.values.get_mut(slot) {
            *current = value;
        }
        for device in &mut self.devices {
            if device.set_realtime_resistor(slot, value) {
                break;
            }
        }
    }

    /// Set an independently driven auxiliary source for the next sample.
    pub fn set_aux_input(&mut self, slot: usize, value: f64) {
        if let Some(target) = self.aux_values.get_mut(slot) {
            *target = if value.is_finite() { value } else { 0.0 };
        }
    }

    pub fn value(&self, slot: usize) -> Option<f64> {
        self.values.get(slot).copied()
    }

    pub fn set_control(&mut self, which: usize, position: f64) {
        if which < self.controls.len() && (self.controls[which] - position).abs() > 1e-12 {
            self.controls[which] = position.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    /// What every supply in this circuit is multiplied by.
    ///
    /// One is the amplifier on its own mains. Less than one is the amplifier on
    /// a variac -- which is a real thing players did, and on some records the
    /// whole point: every rail comes down together, so the valves run out of
    /// room sooner, the bias supply follows, and the amplifier goes soft where
    /// it used to be loud. It scales the supplies and nothing else, because
    /// that is all a variac does.
    pub fn set_supply_scale(&mut self, scale: f64) {
        let scale = scale.clamp(0.1, 2.0);
        if (self.supply_scale - scale).abs() > 1e-9 {
            self.supply_scale = scale;
            self.dirty = true;
        }
    }

    pub fn supply_scale(&self) -> f64 {
        self.supply_scale
    }

    pub fn set_rate(&mut self, rate: f64) {
        if (self.rate - rate).abs() > 1e-9 {
            self.rate = rate;
            self.dirty = true;
        }
    }

    /// Stamps everything that does not depend on the solution, and factorises
    /// it once. A pot moving or the rate changing is what invalidates this.
    fn rebuild(&mut self) {
        self.rebuilds += 1;
        let n = self.n;
        self.base.iter_mut().for_each(|x| *x = 0.0);
        self.base_dc.iter_mut().for_each(|x| *x = 0.0);
        self.source.iter_mut().for_each(|x| *x = 0.0);
        self.aux_source.iter_mut().for_each(|x| *x = 0.0);
        self.bias.iter_mut().for_each(|x| *x = 0.0);
        // Carry every reactance's state across the rebuild.
        //
        // A rebuild happens when a control moves, and a control moving is a
        // resistance changing. Nothing else about the circuit changes: the
        // parts are the same parts in the same order, and every capacitor
        // still holds the charge it held. Rebuilding the lists from scratch
        // and leaving them at zero threw that charge away, and the only thing
        // standing between that and silence was a DC hunt from `process`,
        // which replaced the running audio with the operating point.
        //
        // Measured with `examples/knobmove.rs`: a knob nudged by one part in a
        // million at every block boundary -- a move that cannot change the
        // circuit -- put an error *louder than the signal* through the Mark
        // IIC+ and the 73P, and moved every other voice by four to fifteen
        // decibels. That is what a knob sounded like while it was turning.
        //
        // What is carried is the branch current, not the companion term,
        // because the companion term depends on the conductance and the
        // conductance moves with the sample rate.
        self.carried_c.clear();
        for c in &self.capacitors {
            self.carried_c
                .push((c.voltage, c.history - c.conductance * c.voltage));
        }
        self.carried_l.clear();
        for l in &self.inductors {
            self.carried_l.push(l.current);
        }
        // A device outlives a rebuild unless the timestep has moved.
        //
        // Nothing a control does reaches a device: a diode is a diode, and a
        // pot is a conductance in the matrix. But a device can have a state --
        // a saturating core integrates the voltage across it and remembers the
        // flux -- and rebuilding the list threw that away. It is the same
        // fault as the reactances above and it showed up in the same
        // measurement: with the capacitors carried across but the devices
        // still rebuilt, the 73P, whose input transformer has a core in it,
        // was still 40 dB from where it should have been while its knob moved,
        // against 100 dB or better for every voice without one.
        //
        // Only while there is signal in flight, though. At rest the circuit is
        // being established from nothing -- and a device with a state settles
        // to a different answer depending on where it starts, so a Console
        // channel re-settled around a core carrying the *previous* setting's
        // flux read two decibels off its own calibration.
        let keep_devices = !self.at_rest
            && !self.devices.is_empty()
            && (self.rate - self.device_rate).abs() < 1e-9;
        self.capacitors.clear();
        self.inductors.clear();
        if !keep_devices {
            self.devices.clear();
        }
        let step = 1.0 / self.rate;

        let mut base = std::mem::take(&mut self.base);
        let mut base_dc = std::mem::take(&mut self.base_dc);
        let mut source = std::mem::take(&mut self.source);
        let mut aux_source = std::mem::take(&mut self.aux_source);
        let mut bias = std::mem::take(&mut self.bias);
        {
            // `into` says which of the two matrices a part belongs in. A
            // capacitor's companion conductance is 2C/T, which for a 22 uF
            // bypass at 96 kHz is four siemens -- a quarter of an ohm. Leaving
            // that in the matrix while hunting the operating point shorts the
            // cathode to ground and the stage comes up with no bias at all.

            for (index, part) in self.circuit.parts.iter().enumerate() {
                match *part {
                    Part::Resistor { a, b, ohms } => {
                        stamp_both(&mut base, &mut base_dc, n, a, b, 1.0 / ohms)
                    }
                    Part::Pot {
                        a,
                        wiper,
                        b,
                        ohms,
                        taper,
                        control,
                    } => {
                        let position = self.controls.get(control).copied().unwrap_or(0.5);
                        let f = taper.fraction(position).clamp(1e-4, 1.0 - 1e-4);
                        stamp_both(&mut base, &mut base_dc, n, wiper, b, 1.0 / (ohms * f));
                        stamp_both(
                            &mut base,
                            &mut base_dc,
                            n,
                            a,
                            wiper,
                            1.0 / (ohms * (1.0 - f)),
                        );
                    }
                    Part::Input { node, series, bias: at, source: which } => {
                        let g = 1.0 / series;
                        stamp_both(&mut base, &mut base_dc, n, node, GROUND, g);
                        if node != GROUND {
                            if which == 0 {
                                source[node] += g;
                                // A directly coupled primary source may carry a DC bias.
                                bias[node] += g * at * self.supply_scale;
                            } else {
                                let slot = which - 1;
                                aux_source[slot * n + node] += g;
                            }
                        }
                    }
                    Part::Capacitor { a, b, farads } => {
                        // Trapezoidal: 2C/T, with the history current carried
                        // between steps. Only into the running matrix -- at DC
                        // a capacitor is an open circuit.
                        let conductance = 2.0 * farads / step;
                        stamp_matrix(&mut base, n, a, b, conductance);
                        self.capacitors.push(Capacitor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            voltage: 0.0,
                        });
                    }
                    Part::Adjustable { a, b, kind, slot } => {
                        let value = self.values[slot];
                        match kind {
                            Adjust::Resistor => {
                                stamp_both(&mut base, &mut base_dc, n, a, b, 1.0 / value)
                            }
                            // The same companions as the fixed parts below.
                            Adjust::Capacitor => {
                                let conductance = 2.0 * value / step;
                                stamp_matrix(&mut base, n, a, b, conductance);
                                self.capacitors.push(Capacitor {
                                    a,
                                    b,
                                    conductance,
                                    history: 0.0,
                                    voltage: 0.0,
                                });
                            }
                            Adjust::Inductor => {
                                let conductance = step / (2.0 * value);
                                stamp_matrix(&mut base, n, a, b, conductance);
                                stamp_matrix(&mut base_dc, n, a, b, INDUCTOR_DC);
                                self.inductors.push(Inductor {
                                    a,
                                    b,
                                    conductance,
                                    history: 0.0,
                                    current: 0.0,
                                });
                            }
                            Adjust::RealtimeResistor => {
                                if !keep_devices {
                                    self.devices.push(AnyDevice::VariableResistor(
                                        VariableResistor::new(a, b, slot, value),
                                    ));
                                }
                            }
                        }
                    }
                    Part::Inductor { a, b, henry } => {
                        // And at DC an inductor is a piece of wire.
                        //
                        // A tenth of a milliohm of wire, not a millionth. The
                        // figure was 1e6 siemens, which is a short by any
                        // measure that matters -- a hundred milliamps through
                        // it drops ninety nanovolts -- and it sat in the same
                        // matrix as a megohm grid leak at 4.5e-6. Twelve orders
                        // of magnitude across one matrix is more than the
                        // solve's arithmetic has to give, and in a circuit
                        // working at four hundred and fifty volts the residual
                        // never came down to the tolerance: the operating point
                        // was right and the solver would not say so. At 1e4 the
                        // same current drops nine microvolts, which is still
                        // nothing, and the spread is a hundred times narrower.
                        let conductance = step / (2.0 * henry);
                        stamp_matrix(&mut base, n, a, b, conductance);
                        stamp_matrix(&mut base_dc, n, a, b, INDUCTOR_DC);
                        self.inductors.push(Inductor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            current: 0.0,
                        });
                    }
                    Part::Supply {
                        node,
                        series,
                        volts,
                    } => {
                        let g = 1.0 / series;
                        stamp_both(&mut base, &mut base_dc, n, node, GROUND, g);
                        if node != GROUND {
                            bias[node] += g * volts * self.supply_scale;
                        }
                    }
                    Part::Diode { a, k, spec } => {
                        if !keep_devices {
                            self.devices.push(AnyDevice::Diode(Diode::new(a, k, spec)));
                        }
                    }
                    Part::Rectifier { a, k, spec } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Rectifier(Rectifier::new(a, k, spec)));
                        }
                    }
                    Part::Triode { p, g, k, spec } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Triode(Triode::new(p, g, k, spec)));
                        }
                    }
                    Part::Pentode {
                        p,
                        g,
                        k,
                        s,
                        count,
                        spec,
                    } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Pentode(Pentode::new(p, g, k, s, count, spec)));
                        }
                    }
                    Part::Jfet {
                        d,
                        g,
                        s: source_pin,
                        spec,
                    } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Jfet(Jfet::new(d, g, source_pin, spec)));
                        }
                    }
                    Part::Bipolar { c, b, e, spec, pnp } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Bipolar(Bipolar::new(c, b, e, spec, pnp)));
                        }
                    }
                    Part::Transconductor {
                        plus,
                        minus,
                        out,
                        reference,
                        gm,
                        limit,
                    } => {
                        if !keep_devices {
                            self.devices.push(AnyDevice::Transconductor(Transconductor::new(
                                plus, minus, out, reference, gm, limit,
                            )));
                        }
                    }
                    Part::Core {
                        a,
                        b,
                        spec,
                        antialias,
                    } => {
                        // The only device that has to be told the rate: it
                        // integrates the voltage across it, so its answer
                        // depends on how long a sample lasts.
                        if !keep_devices {
                            let rate = self.rate;
                            self.devices
                                                                .push(AnyDevice::Core(if antialias {
                                    Core::new_antialiased(a, b, spec, rate)
                                } else {
                                    Core::new(a, b, spec, rate)
                                }));
                        }
                    }
                    Part::LinearOpAmp { out, plus, minus } => {
                        let branch = self.circuit.branch_of(index);
                        // Exact ideal-op-amp stamp: output branch current plus
                        // the constraint V+ - V- = 0. There is no rail state,
                        // so this belongs permanently to the cached linear base.
                        if out != GROUND {
                            base[out * n + branch] += 1.0;
                            base_dc[out * n + branch] += 1.0;
                        }
                        if plus != GROUND {
                            base[branch * n + plus] += 1.0;
                            base_dc[branch * n + plus] += 1.0;
                        }
                        if minus != GROUND {
                            base[branch * n + minus] -= 1.0;
                            base_dc[branch * n + minus] -= 1.0;
                        }
                    }
                    Part::OpAmp {
                        out,
                        plus,
                        minus,
                        reference,
                        rail,
                    } => {
                        let branch = self.circuit.branch_of(index);
                        if !keep_devices {
                            self.devices.push(AnyDevice::OpAmp(OpAmp::new(
                                out, plus, minus, reference, branch, rail,
                            )));
                        }
                    }
                    Part::Transformer {
                        p1,
                        p2,
                        s1,
                        s2,
                        ratio,
                    } => {
                        let branch = self.circuit.branch_of(index);
                        stamp_branch(&mut base, n, branch, p1, p2, s1, s2, ratio);
                        stamp_branch(&mut base_dc, n, branch, p1, p2, s1, s2, ratio);
                    }
                }
            }
        }
        // And put it back. `history = G v + i` for a capacitor and
        // `-(i + G v)` for an inductor, which is the shape the advance step
        // writes and the shape `inject` expects.
        if self.carried_c.len() == self.capacitors.len() {
            for (c, &(v, i)) in self.capacitors.iter_mut().zip(&self.carried_c) {
                c.voltage = v;
                c.history = c.conductance * v + i;
            }
        }
        if self.carried_l.len() == self.inductors.len() {
            for (l, &i) in self.inductors.iter_mut().zip(&self.carried_l) {
                let v = across(&self.voltage, l.a, l.b);
                l.current = i;
                l.history = -(i + l.conductance * v);
            }
        }

        self.device_rate = self.rate;
        self.predictable = !self.devices.iter().any(|d| d.switches());
        // Capacity was reserved from the immutable topology in `new`, so
        // this resize only changes the logical length. The line search may not
        // allocate on the audio thread.
        self.saved
            .resize(self.devices.len(), Linearisation::default());
        self.post_restart_saved_linearisation
            .resize(self.devices.len(), Linearisation::default());
        self.last_settled_linearisation
            .resize(self.devices.len(), Linearisation::default());
        if !keep_devices {
            self.last_settled_linearisation_valid = false;
        }
        self.base = base;
        self.base_dc = base_dc;
        self.source = source;
        self.aux_source = aux_source;
        self.bias = bias;
        // Wiring is immutable. Compute the elimination bounds on the first
        // assembly only and reuse them for every later control/rate rebuild.
        self.map_structure();

        // A linear topology stays linear for this simulation's lifetime. Its
        // condensed structure is therefore allocated once and only its numeric
        // coefficients need to change when a pot or sample rate moves.
        if self.devices.is_empty() {
            let output = self.circuit.output;
            if let Some(partition) = self.linear_partition.as_mut() {
                let _ = partition.refresh(&self.base, &self.partition_zero_rhs, output);
            } else if !self.partition_initialized {
                // The first rebuild is construction-time work. If reduction is
                // impossible, remember that result and use the full LU path on
                // all later rebuilds rather than retrying an allocating
                // constructor from the audio thread.
                self.linear_partition = ReducedLinear::new_with_rhs_active(
                    &self.base,
                    &self.partition_zero_rhs,
                    output,
                    &self.rhs_active_nodes,
                );
                self.partition_initialized = true;
            }
        } else {
            debug_assert!(self.linear_partition.is_none());

            // Nonlinear circuits get a different exact reduction. The boundary
            // contains every coefficient/RHS position a device can change, so
            // A_ii, A_ib and A_bi are purely linear and stay fixed between
            // rebuilds. Their Schur contribution is paid here once; a Newton
            // pass then factorises only the much smaller boundary matrix.
            if !self.nonlinear_partition_initialized {
                // Try the true nonlinear boundary first, then promote the
                // *smallest possible* subset of otherwise-linear MNA unknowns
                // needed to make A_ii invertible. Transformer branch currents
                // and the output used to be included unconditionally. That is
                // exact but expensive: dense reduced LU scales cubically with
                // boundary size. Construction happens once outside realtime, so
                // exhaustively trying the small optional set is cheap and gives
                // the smallest exact boundary the topology admits.
                let mut optional = Vec::new();
                for (index, part) in self.circuit.parts.iter().enumerate() {
                    if matches!(part, Part::Transformer { .. }) {
                        let branch = self.circuit.branch_of(index);
                        if branch < n
                            && !self.nonlinear_boundary.contains(&branch)
                            && !optional.contains(&branch)
                        {
                            optional.push(branch);
                        }
                    }
                }
                if self.circuit.output < n
                    && !self.nonlinear_boundary.contains(&self.circuit.output)
                    && !optional.contains(&self.circuit.output)
                {
                    optional.push(self.circuit.output);
                }
                optional.sort_unstable();

                let mut candidates = Vec::new();
                if optional.len() <= 10 {
                    let combinations = 1usize << optional.len();
                    for promoted in 0..=optional.len() {
                        for mask in 0..combinations {
                            if mask.count_ones() as usize != promoted {
                                continue;
                            }
                            let mut candidate = self.nonlinear_boundary.clone();
                            for (bit, &node) in optional.iter().enumerate() {
                                if mask & (1usize << bit) != 0 {
                                    candidate.push(node);
                                }
                            }
                            candidate.sort_unstable();
                            candidates.push(candidate);
                        }
                    }
                } else {
                    // Defensive fallback for an unexpectedly large MNA
                    // topology: still try the minimal and conservative full
                    // optional boundaries without exponential construction work.
                    candidates.push(self.nonlinear_boundary.clone());
                    let mut conservative = self.nonlinear_boundary.clone();
                    conservative.extend(optional.iter().copied());
                    conservative.sort_unstable();
                    conservative.dedup();
                    candidates.push(conservative);
                }

                for candidate in candidates {
                    if !nonlinear_reduction_worthwhile(n, candidate.len()) {
                        continue;
                    }
                    let transient = ReducedNonlinear::new_with_active_rhs(
                        &self.base,
                        &self.partition_zero_rhs,
                        &candidate,
                        &self.rhs_active_nodes,
                    );
                    let dc_partition = ReducedNonlinear::new_with_active_rhs(
                        &self.base_dc,
                        &self.partition_zero_rhs,
                        &candidate,
                        &self.rhs_active_nodes,
                    );
                    if transient.is_some() {
                        self.nonlinear_boundary = candidate;
                        self.nonlinear_partition = transient;
                        self.nonlinear_partition_dc = dc_partition;
                        break;
                    }
                }
                self.nonlinear_partition_initialized = true;
            } else {
                if let Some(partition) = self.nonlinear_partition.as_mut() {
                    let _ = partition.refresh(&self.base, &self.partition_zero_rhs);
                }
                if let Some(partition) = self.nonlinear_partition_dc.as_mut() {
                    let _ = partition.refresh(&self.base_dc, &self.partition_zero_rhs);
                }
            }
        }
        // Numeric values have changed even though the cached topology has not.
        // Conservatively relearn the pivot plan; factorisation will cache the
        // new order after this rebuild.
        self.planned = false;
        self.work_base_mode = None;

        // A linear circuit is factorised once and reused. A circuit with a
        // device in it has to be refactorised every Newton pass, because the
        // device's own conductance is part of the matrix and moves with the
        // answer.
        self.matrix.copy_from_slice(&self.base);
        factorise(
            &mut self.matrix,
            &mut self.pivots,
            &mut self.reach,
            &mut self.depth,
            &mut self.first,
            &self.reach_template,
            &self.depth_template,
            &mut self.plan,
            false,
            n,
        );
        self.dirty = false;
    }

    /// Work out which matrix positions can ever be nonzero, and turn that into
    /// the two bounds the factorisation needs.
    ///
    /// Everything the linear network stamps is already sitting in `base` and
    /// `base_dc`, so their nonzeros are read straight off. What is not there
    /// is the nonlinear devices, whose entries appear only once they are
    /// stamped and, for an op-amp, differ between its linear state and its
    /// railed one -- so each device declares its own footprint over all of its
    /// states. See `Mark`.
    ///
    /// The result is deliberately an **upper bound**. A conductance that
    /// happens to come out at exactly zero leaves a position marked that holds
    /// nothing, and the elimination then runs one column further and subtracts
    /// `factor * 0.0` from it. That changes no value, so the arithmetic stays
    /// bit for bit what a value-driven scan produced; it only declines to
    /// discover, a hundred thousand times a second, a shape that was decided
    /// when the parts were wired together.
    fn map_structure(&mut self) {
        if self.structure_mapped {
            return;
        }

        let n = self.n;
        // Start with the nonlinear device footprint so it can be retained as
        // a compact list for line-search restamps without allocating another
        // n x n topology mask. The full structural map is then completed with
        // the two immutable linear base matrices.
        self.structure_pattern.fill(false);
        {
            let mut mark = Mark {
                pattern: &mut self.structure_pattern,
                n,
            };
            for device in &self.devices {
                device.footprint(&mut mark);
            }
        }
        self.device_matrix_slots.clear();
        for (index, &marked) in self.structure_pattern.iter().enumerate() {
            if marked {
                self.device_matrix_slots.push(index);
            }
        }
        for (slot, marked) in self.base.iter().zip(self.structure_pattern.iter_mut()) {
            *marked |= *slot != 0.0;
        }
        for (slot, marked) in self.base_dc.iter().zip(self.structure_pattern.iter_mut()) {
            *marked |= *slot != 0.0;
        }
        // The diagonal is always in play: the factorisation pivots on it, and
        // a row whose reach fell short of its own diagonal would bound the
        // elimination to nothing.
        let pattern = &self.structure_pattern;
        let reach_template = &mut self.reach_template;
        let depth_template = &mut self.depth_template;
        for row in 0..n {
            reach_template[row] = (row..n)
                .rev()
                .find(|&k| pattern[row * n + k])
                .unwrap_or(row)
                .max(row);
            depth_template[row] = (row..n)
                .rev()
                .find(|&k| pattern[k * n + row])
                .unwrap_or(row)
                .max(row);
        }

        // The merit function is evaluated many times specifically on the
        // difficult samples that already threaten the realtime deadline.
        // Build a compact row map once so those evaluations touch only matrix
        // entries that can structurally be nonzero. Column order stays
        // ascending, preserving the arithmetic order of the old dense scan.
        self.merit_row_offsets.clear();
        self.merit_columns.clear();
        self.merit_slots.clear();
        self.merit_row_offsets.reserve(n + 1);
        self.merit_row_offsets.push(0);
        for (row, &row_reach) in reach_template.iter().enumerate().take(n) {
            let row_start = row * n;
            for col in 0..=row_reach {
                let slot = row_start + col;
                if pattern[slot] {
                    self.merit_columns.push(col);
                    self.merit_slots.push(slot);
                }
            }
            self.merit_row_offsets.push(self.merit_columns.len());
        }
        self.structure_mapped = true;
    }

    /// Check every stamp against the declared structure, and count what falls
    /// outside it.
    ///
    /// Off by default and never on in the audio path: this is `O(n^2)` a pass
    /// and exists so `tests/devices.rs` can hold the devices to their word.
    pub fn watch_structure(&mut self) {
        self.watching = true;
        self.violations = 0;
    }

    /// Stamps seen outside the declared structure since `watch_structure`.
    pub fn violations(&self) -> usize {
        self.violations
    }

    /// How short a step this circuit's line search may try before giving up
    /// and taking the full Newton step.
    ///
    /// Per circuit, because the right answer is per circuit and both halves of
    /// that are measured. The Twin Reverb's power amplifier spirals at six:
    /// a step shortened to a sixty-fourth barely moves the solve, so the next
    /// pass stalls too and searches again, and the stage spends 3713 solves a
    /// second running out of passes and 120,182 backtracks a second getting
    /// there. Stopping at a sixteenth breaks the spiral -- in the plugin, 3.6 %
    /// of callbacks missed becomes 0.1 %, the worst falls from 7509 us to 1447,
    /// and against a high-accuracy reference **the Twin does not move at all**:
    /// -205.4, -199.3 and -189.8 dB at three drives, identical either way.
    ///
    /// The 5150 is the reason this is not simply lowered for everyone. The same
    /// change takes its answer from 209.6 dB below that reference to **45.7 dB
    /// below**, which is audible. Its solve genuinely needs the short steps,
    /// and cutting them off leaves it somewhere it had not finished with --
    /// §58.4's fourth point, in a new place.
    ///
    /// So this is a numerical parameter tuned per matrix, the way a tolerance
    /// or a preconditioner is, and not a property of any circuit. Anything
    /// changed here has to be measured both ways: the deadline *and* the
    /// distance from an accurate reference.
    pub fn set_backtracks(&mut self, most: usize) {
        self.backtracks = most.clamp(1, MAX_BACKTRACKS);
    }

    /// Enable the bounded late source-continuation rescue for a circuit whose
    /// realtime tail has been measured to benefit from it. This does not alter
    /// the normal predictor or normal early Newton path.
    pub fn set_late_continuation(&mut self, enabled: bool) {
        self.late_continuation = enabled;
    }

    #[inline(always)]
    fn input_curvature_predictor_gate_enabled(&self) -> bool {
        if !self.late_continuation {
            return false;
        }
        #[cfg(test)]
        {
            !self.test_disable_input_curvature_predictor_gate
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline(always)]
    fn source_scaled_predictor_enabled(&self) -> bool {
        if !self.input_curvature_predictor_gate_enabled() {
            return false;
        }
        #[cfg(test)]
        {
            !self.test_disable_source_scaled_predictor
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    /// Cap the Newton passes a sample may take. See `ceiling`.
    ///
    /// Clamped to `PASS_FLOOR` at the bottom, so no caller -- and no bug in a
    /// caller -- can starve the solve past the point BUG-008 measured.
    pub fn set_pass_ceiling(&mut self, passes: usize) {
        self.ceiling = passes.clamp(PASS_FLOOR, MAX_ITERATIONS);
    }

    /// How many samples finished at the ceiling rather than by converging.
    pub fn pinched(&self) -> u64 {
        self.pinched
    }

    /// The two structural bounds `map_structure` worked out.
    ///
    /// Exposed so `tests/devices.rs` can check that nothing ever stamps
    /// outside it. A footprint that is wrong is not a crash: it is a matrix
    /// entry the elimination declines to touch, an answer that is quietly not
    /// the answer, and no test would notice without this.
    pub fn structure(&self) -> (Vec<usize>, Vec<usize>) {
        (self.reach_template.clone(), self.depth_template.clone())
    }

    /// Where the circuit sits with no signal on it.
    ///
    /// Hunted before any audio arrives, with the reactances held at their
    /// steady state, so the first sample starts from the operating point
    /// rather than charging up through it. Without this a valve stage spends
    /// its first tenth of a second climbing to its own bias, and whatever is
    /// listening hears that as a thump.
    pub fn find_operating_point(&mut self) -> bool {
        // The matrix has to be the one the controls currently describe. Asking
        // for an operating point after moving a control, without rebuilding
        // first, solves the circuit as it used to be -- and the answer looks
        // perfectly converged, because it is: it is the converged answer to
        // the wrong question. What gives it away is Kirchhoff's law, which the
        // returned voltages then quietly fail.
        if self.dirty {
            self.rebuild();
        }
        if self.devices.is_empty() {
            return true;
        }
        let mut settled = false;
        self.prepare_rhs(0.0, true);
        // At rest a capacitor carries no current and an inductor no voltage,
        // which is what the zeroed histories already say.
        for pass in 0..DC_ITERATIONS {
            match self.iterate(true, pass >= FULL_STEPS) {
                Pass::Settled => {
                    settled = true;
                    break;
                }
                Pass::Moved => {}
                Pass::Stuck => break,
            }
        }
        // The reactances take up whatever the operating point implies, so they
        // do not then charge through it.
        for c in &mut self.capacitors {
            let v = across(&self.voltage, c.a, c.b);
            c.voltage = v;
            c.history = c.conductance * v;
        }
        // An inductor's standing current, which is not readable the way a
        // capacitor's charge is.
        //
        // At rest a capacitor holds a voltage and no current, so its state is
        // right there in the node voltages. An inductor is the other way
        // about: it holds a current and drops no voltage, so the node voltages
        // say nothing about it and this left every inductor starting from
        // zero. Nothing noticed for a long time, because the only inductors in
        // the catalogue were in tone stacks and cabinets, where none of them
        // carries any standing current.
        //
        // The 73P's output transformer does. Its primary is T8's collector
        // load, so it carries the whole of that transistor's thirty-seven
        // milliamps -- and starting from zero, the amplifier spent a fifth of
        // a second climbing to its own operating point and put a thump of 1.5
        // out through the speaker on the way. The guitar amplifiers' output
        // chokes carry their plate current the same way.
        //
        // The direct-current matrix stamps an inductor as a conductance, so
        // the current is the small drop across it times that conductance.
        for l in &mut self.inductors {
            l.current = INDUCTOR_DC * across(&self.voltage, l.a, l.b);
            l.history = -l.current;
        }
        // The predictor state, reset to the operating point.
        //
        // Without this the first sample after a hunt runs the extrapolator
        // `2 * voltage - earlier` with `earlier` holding whatever it held
        // from before -- a previous circuit, a previous voice, a previous
        // session's running state. On a valve voice that means a first
        // Newton correction from four hundred volts at the plate to a value
        // some previous state suggested, which is a worse start than no
        // prediction at all. `apply_operating_point` already resets both
        // vectors; this is the same reset, in the hunt that does not go
        // through it.
        //
        // `last_was_unsettled` is cleared alongside, so the first sample
        // after a reset starts from a clean predictor state and is not
        // treated as following a failure.
        self.predicted.copy_from_slice(&self.voltage);
        self.earlier.copy_from_slice(&self.voltage);
        self.last_was_unsettled = false;
        self.last_input = 0.0;
        self.earlier_input = 0.0;
        if settled {
            for (saved, device) in self
                .last_settled_linearisation
                .iter_mut()
                .zip(self.devices.iter())
            {
                *saved = device.linearisation();
            }
            self.last_settled_linearisation_valid = true;
        } else {
            self.last_settled_linearisation_valid = false;
        }
        self.at_rest = true;
        settled
    }

    /// Cache the input, supply and reactive-history contributions once per
    /// solve. Newton trials change device stamps, never these contributions.
    fn prepare_rhs(&mut self, input: f64, dc: bool) {
        // A new source RHS starts a new Newton problem. Damping learned on the
        // previous RHS is not evidence about this one.
        self.search_lambda_hint = 1.0;

        // Only these positions can ever contain source/bias/reactive history.
        // Inactive entries remain permanently zero from construction, so a
        // reduced solve does not need an O(n) full-vector rebuild each sample.
        for &k in &self.rhs_active_nodes {
            let mut value = self.source[k] * input + self.bias[k];
            for (slot, &aux) in self.aux_values.iter().enumerate() {
                value += self.aux_source[slot * self.n + k] * aux;
            }
            self.fixed_rhs[k] = value;
        }
        if !dc {
            for c in &self.capacitors {
                inject(&mut self.fixed_rhs, c.a, c.b, c.history);
            }
            for l in &self.inductors {
                inject(&mut self.fixed_rhs, l.a, l.b, l.history);
            }
        }
        self.rhs_fixed_loaded = false;

        // The Schur reduction's internal RHS is also immutable throughout the
        // Newton solve. Cache its contribution once per sample instead of
        // recomputing the same boundary-by-internal dot products every pass.
        let partition = if dc {
            self.nonlinear_partition_dc.as_mut()
        } else {
            self.nonlinear_partition.as_mut()
        };
        if let Some(partition) = partition {
            let _ = partition.prepare_rhs(&self.fixed_rhs);
        }
    }

    #[inline]
    fn ensure_fixed_rhs_loaded(&mut self) {
        if !self.rhs_fixed_loaded {
            self.rhs.copy_from_slice(&self.fixed_rhs);
            self.rhs_fixed_loaded = true;
        }
    }

    /// Reset the RHS entries a nonlinear device may write. `prepare_rhs()`
    /// already copied the complete fixed RHS once for this sample/operating-
    /// point solve, so the internal/passive entries never need to be recopied
    /// on every Newton pass.
    #[inline]
    fn reset_device_rhs(&mut self) {
        for &row in &self.nonlinear_boundary {
            self.rhs[row] = self.fixed_rhs[row];
        }
    }

    /// Stamp the current accepted voltage. When the exact nonlinear Schur
    /// reduction is available, `work` is never factorised by a successful
    /// reduced solve. In that common path the immutable linear matrix therefore
    /// stays resident and only the nonlinear device footprint needs restoring.
    /// A full-MNA factorisation marks the cache dirty so the next pass performs
    /// one dense refresh before sparse restamps resume.
    #[inline]
    fn build_current(&mut self, dc: bool, limiting: bool, sparse_ok: bool) {
        // Full MNA is the fallback/diagnostic path. Only now pay to materialize
        // the complete fixed RHS; subsequent Newton passes reset just the
        // nonlinear boundary entries as before.
        let rhs_was_loaded = self.rhs_fixed_loaded;
        self.ensure_fixed_rhs_loaded();
        if rhs_was_loaded {
            self.reset_device_rhs();
        }
        let base = if dc { &self.base_dc } else { &self.base };
        if sparse_ok && self.work_base_mode == Some(dc) {
            for &slot in &self.device_matrix_slots {
                self.work[slot] = base[slot];
            }
        } else {
            self.work.copy_from_slice(base);
            self.work_base_mode = Some(dc);
        }

        let voltage = &self.voltage;
        let mut stamper = Stamper {
            matrix: &mut self.work,
            rhs: &mut self.rhs,
            n: self.n,
            map: None,
            mapping_failed: false,
            limiting,
            junction_held: false,
        };
        for device in &mut self.devices {
            device.stamp(&mut stamper, voltage);
        }
        self.exact = !stamper.junction_held;
        if self.watching {
            let n = self.n;
            for row in 0..n {
                for col in 0..n {
                    if self.work[row * n + col] != 0.0
                        && (col > self.reach_template[row] || row > self.depth_template[col])
                    {
                        self.violations += 1;
                    }
                }
            }
        }
    }

    /// Stamp nonlinear devices directly into the Schur boundary system. This
    /// bypasses the full n×n matrix entirely on the production reduced path.
    /// Returns false only if the partition is unavailable/unprepared or a
    /// device unexpectedly references an internal unknown.
    #[inline]
    fn build_current_reduced(&mut self, dc: bool, limiting: bool) -> bool {
        let voltage = &self.voltage;
        let partition = if dc {
            self.nonlinear_partition_dc.as_mut()
        } else {
            self.nonlinear_partition.as_mut()
        };
        let Some(partition) = partition else {
            return false;
        };
        let n = partition.boundary_len();
        let (exact, mapping_ok) = {
            let Some((matrix, rhs, map)) = partition.begin_stamp(&self.fixed_rhs) else {
                return false;
            };
            let mut stamper = Stamper {
                matrix,
                rhs,
                n,
                map: Some(map),
                mapping_failed: false,
                limiting,
                junction_held: false,
            };
            for device in &mut self.devices {
                device.stamp(&mut stamper, voltage);
            }
            (!stamper.junction_held, !stamper.mapping_failed)
        };
        if mapping_ok {
            partition.finish_stamp();
        }
        self.exact = exact;
        mapping_ok
    }

    /// Build only the nonlinear-boundary entries of a line-search candidate.
    /// A reduced merit probe never reads internal Schur-eliminated nodes, so
    /// touching the full 31/37-unknown Twin vector on every rejected trial is
    /// wasted work. The accepted step materialises the full vector once.
    #[inline]
    fn build_reduced_trial_point(&mut self, dc: bool, lambda: f64) -> bool {
        let partition = if dc {
            self.nonlinear_partition_dc.as_ref()
        } else {
            self.nonlinear_partition.as_ref()
        };
        let Some(partition) = partition else {
            return false;
        };
        let mut finite = true;
        for &at in partition.boundary_nodes() {
            let value = self.voltage[at] + lambda * self.scratch[at];
            self.point[at] = value;
            finite &= value.is_finite();
        }
        finite
    }

    #[inline]
    fn build_full_trial_point(&mut self, lambda: f64) -> bool {
        let mut finite = true;
        for ((point, &voltage), &delta) in self
            .point
            .iter_mut()
            .zip(&self.voltage)
            .zip(&self.scratch)
        {
            let value = voltage + lambda * delta;
            *point = value;
            finite &= value.is_finite();
        }
        finite
    }

    /// Reduced line-search trial residual and merit. A trial point is judged
    /// but never solved, so forming nonlinear Jacobian/tangent coefficients is
    /// pure overhead. The immutable Schur-linear contribution is evaluated
    /// first, then each nonlinear device adds only its physical equation.
    #[inline]
    fn reduced_trial_merit(&mut self, dc: bool, allow_cache_eval: bool) -> Option<f64> {
        // Keep this experiment on the Twin-specific transient path. The cache
        // is exact for every circuit, but the current optimization target is
        // the measured Twin tail and broadening scope would add risk without
        // improving this A/B. Trial probes still evaluate only the physical
        // current equations. When caching is allowed, devices merely retain
        // intermediates that current evaluation already computed; analytic
        // slopes are formed later only if the exact point becomes the next
        // Newton linearisation.
        let cache_eval = allow_cache_eval
            && self.late_continuation
            && !dc
            && self.accepted_trial_device_cache_enabled();
        let point = &self.point;
        let partition = if dc {
            self.nonlinear_partition_dc.as_ref()
        } else {
            self.nonlinear_partition.as_ref()
        }?;
        let n = partition.boundary_len();
        let map = partition.begin_residual(
            &self.fixed_rhs,
            point,
            &mut self.trial_residual[..n],
        )?;
        let (exact, mapping_ok) = {
            let mut residual = ResidualStamper {
                residual: &mut self.trial_residual[..n],
                map,
                mapping_failed: false,
                limiting: false,
                junction_held: false,
            };
            for device in &mut self.devices {
                device.trial_residual(&mut residual, point, cache_eval);
                // Junction-held merit is deliberately unusable. Stop the
                // device walk immediately rather than evaluating equations
                // that will be discarded before any step decision.
                if residual.junction_held {
                    break;
                }
            }
            (!residual.junction_held, !residual.mapping_failed)
        };
        self.exact = exact;
        if !mapping_ok {
            return None;
        }
        partition.residual_merit(&self.trial_residual[..n])
    }

    #[inline]
    fn current_reduced_merit(&self, dc: bool) -> Option<f64> {
        let partition = if dc {
            self.nonlinear_partition_dc.as_ref()
        } else {
            self.nonlinear_partition.as_ref()
        }?;
        Some(partition.merit_stamped(&self.voltage))
    }

    /// Full point stamp used only by structure-watch diagnostics. This keeps
    /// their exhaustive verification semantics without putting dense rebuilds
    /// back on the production line-search path.
    fn build_point_full(&mut self, dc: bool) {
        let base = if dc { &self.base_dc } else { &self.base };
        self.work.copy_from_slice(base);
        self.work_base_mode = Some(dc);
        self.ensure_fixed_rhs_loaded();
        self.reset_device_rhs();

        let point = &self.point;
        let mut stamper = Stamper {
            matrix: &mut self.work,
            rhs: &mut self.rhs,
            n: self.n,
            map: None,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        for device in &mut self.devices {
            device.stamp(&mut stamper, point);
        }
        self.exact = !stamper.junction_held;

        let n = self.n;
        for row in 0..n {
            for col in 0..n {
                if self.work[row * n + col] != 0.0
                    && (col > self.reach_template[row] || row > self.depth_template[col])
                {
                    self.violations += 1;
                }
            }
        }
    }

    /// Restore exactly the coefficients the line-search merit can read after
    /// a full-MNA Newton solve has overwritten `work` with LU factors.
    ///
    /// The first trial cannot use the ordinary sparse device restamp because
    /// factorisation has filled structurally-zero scratch slots. Restore every
    /// structural coefficient once; later trials again touch only the nonlinear
    /// footprint. The matrix is still not safe for a reduced solve afterwards,
    /// so `work_base_mode` deliberately remains `None`.
    fn restamp_first_trial(&mut self, dc: bool) {
        let base = if dc { &self.base_dc } else { &self.base };
        for &slot in &self.merit_slots {
            self.work[slot] = base[slot];
        }
        self.ensure_fixed_rhs_loaded();
        self.reset_device_rhs();

        let point = &self.point;
        let mut stamper = Stamper {
            matrix: &mut self.work,
            rhs: &mut self.rhs,
            n: self.n,
            map: None,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        for device in &mut self.devices {
            device.stamp(&mut stamper, point);
        }
        self.exact = !stamper.junction_held;
    }

    /// Rebuild only the coefficients and RHS entries a nonlinear device can
    /// change for a line-search trial. Physical state is untouched; device
    /// linearisation state is explicitly restored after rejected trials.
    #[inline]
    fn restamp_trial(&mut self, dc: bool) {
        let base = if dc { &self.base_dc } else { &self.base };
        for &slot in &self.device_matrix_slots {
            self.work[slot] = base[slot];
        }
        self.ensure_fixed_rhs_loaded();
        self.reset_device_rhs();

        let point = &self.point;
        let mut stamper = Stamper {
            matrix: &mut self.work,
            rhs: &mut self.rhs,
            n: self.n,
            map: None,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        for device in &mut self.devices {
            device.stamp(&mut stamper, point);
        }
        self.exact = !stamper.junction_held;
    }

    /// How badly `x` fails to satisfy the circuit as currently stamped.
    ///
    /// The companion models are built so that solving `A x = b` yields the
    /// next Newton iterate, which makes `A(x) x - b(x)` the residual of the
    /// nonlinear system itself: it is zero exactly at the solution and
    /// nowhere else. Summed and squared, that is the merit function -- one
    /// number saying how wrong a candidate is, which is what lets the line
    /// search compare two of them.
    ///
    /// Summing the squares rather than taking the largest matters: individual
    /// rows cancel each other, and a step that halves one row while doubling
    /// another is not progress.
    #[inline]
    fn merit(&self, x: &[f64], reduced_boundary_only: bool) -> f64 {
        let mut total = 0.0;
        // Once line search is active, the current point and the Newton target
        // have both already been obtained from the *same sample's* linear
        // internal equations. Those equations are affine and unchanged by the
        // nonlinear devices, so every point on the search segment satisfies
        // them exactly. Their residual is therefore identically zero. In the
        // exact Schur-reduced path the merit can evaluate only the boundary
        // rows without changing the value being compared. This removes the
        // passive-network residual walk from every backtracking trial.
        if reduced_boundary_only {
            for &row in &self.nonlinear_boundary {
                let mut sum = -self.rhs[row];
                let start = self.merit_row_offsets[row];
                let end = self.merit_row_offsets[row + 1];
                for (&col, &slot) in self.merit_columns[start..end]
                    .iter()
                    .zip(&self.merit_slots[start..end])
                {
                    sum += self.work[slot] * x[col];
                }
                total += sum * sum;
            }
        } else {
            for row in 0..self.n {
                let mut sum = -self.rhs[row];
                let start = self.merit_row_offsets[row];
                let end = self.merit_row_offsets[row + 1];
                for (&col, &slot) in self.merit_columns[start..end]
                    .iter()
                    .zip(&self.merit_slots[start..end])
                {
                    sum += self.work[slot] * x[col];
                }
                total += sum * sum;
            }
        }
        total
    }

    /// One Newton pass, with a bounded backtracking line search.
    ///
    /// `search` turns the line search on. With it off this is a plain full
    /// Newton step, which is what it should be while the solve is behaving:
    /// judging a step means stamping the circuit where it lands, and a stamp
    /// costs about what the factorisation does, so a pass that consults the
    /// residual costs roughly twice one that does not. Measured over a whole
    /// second the search buys about a quarter fewer passes, which does not
    /// pay for doubling them.
    ///
    /// What it does buy is the tail, and the tail is what misses deadlines.
    /// So the caller runs plain Newton first and turns this on only for a
    /// solve that is still going after `FULL_STEPS` -- one that is no longer
    /// converging but oscillating, where a shorter step is the only way out
    /// and the extra stamp is worth paying for because the alternative is
    /// thirty-two passes and a held sample.
    fn iterate(&mut self, dc: bool, search: bool) -> Pass {
        self.iterate_with_limiter_handoff(dc, search, true)
    }

    fn iterate_with_limiter_handoff(
        &mut self,
        dc: bool,
        search: bool,
        allow_limiter_handoff: bool,
    ) -> Pass {
        let n = self.n;

        // The Twin power stage previously paid for two globalization schemes
        // in series on a hard sample: first a valve-grid step limiter walked
        // toward the predicted point for several Newton passes, and only after
        // the correction stopped shrinking did the global line search take
        // over.  The local limiter changes the Newton linearisation; the line
        // search instead damps the exact Newton direction.  When a smooth
        // triode/pentode grid limiter would hold this pass, V3 may hand the
        // correction to the existing line search rather than spend many passes
        // walking to the same basin. V3.1's caller defers that handoff exactly
        // once per Twin power solve; all other call sites preserve immediate
        // V3 behavior. `late_continuation` is intentionally Twin-power-only.
        let limiter_requests_global_search = allow_limiter_handoff
            && !dc
            && !search
            && self.late_continuation
            && self.limiter_global_search_enabled()
            && self.valve_grid_limiter_would_hold();
        let search = search || limiter_requests_global_search;
        if !search && self.late_continuation {
            // Only consecutive searched passes share a hint. A successful
            // return to plain Newton means the old damping evidence is stale.
            self.search_lambda_hint = 1.0;
        }

        // Linearise where the solve is now.
        //
        // Re-centred every pass rather than inherited from the step that got
        // us here. A stamp taken during the last pass's trial had its devices
        // limited toward *that* pass's starting point, so it describes
        // somewhere slightly other than where the solve now is, and a
        // residual measured against it is not the residual of anything.
        // Carrying it forward to save the stamp cost the 73P forty-seven
        // thousand failed solves a second.
        let reduced_candidate = if dc {
            self.nonlinear_partition_dc.is_some()
        } else {
            self.nonlinear_partition.is_some()
        };
        // The production Schur path stamps nonlinear devices directly into
        // the small reduced boundary system. Full MNA is retained for
        // structure-watch diagnostics and as a numerical fallback only.
        let mut reduced_direct = reduced_candidate
            && !self.watching
            && self.build_current_reduced(dc, !search);
        if !reduced_direct {
            self.build_current(dc, !search, false);
        }
        let search = search && self.exact;
        let here = if search {
            if reduced_direct {
                self.current_reduced_merit(dc)
                    .unwrap_or_else(|| self.merit(&self.voltage, false))
            } else {
                self.merit(&self.voltage, false)
            }
        } else {
            0.0
        };
        // A two-point nonmonotone search lets Newton cross a short increase
        // in residual instead of getting trapped taking tiny steps around a
        // power tube's conduction boundary. A worsening step must make a tiny
        // but meaningful amount of progress from the previous residual toward
        // the current one; merely beating the stale reference by floating-point
        // noise is not enough (see `search_trial_improves`). The reference
        // rolls forward, is reset every sample, and never changes the full
        // correction/device convergence test below. DC keeps its old search.
        let reference = if dc { here } else { here.max(self.search_merit) };
        #[cfg(test)]
        {
            self.last_search_here = here;
            self.last_search_reference = reference;
            self.last_search_accepted_lambda = 0.0;
            self.last_search_accepted_merit = 0.0;
            self.last_search_trial_count = 0;
            self.last_search_trial_lambdas = [0.0; SEARCH_TRACE_TRIALS];
            self.last_search_trial_merits = [0.0; SEARCH_TRACE_TRIALS];
            self.last_search_trial_exact = [false; SEARCH_TRACE_TRIALS];
            self.last_search_max_correction_unknown = 0;
            self.last_search_max_correction_norm = 0.0;
            self.last_search_max_correction_abs = 0.0;
            self.last_search_max_correction_voltage = 0.0;
            self.last_search_max_correction_guess = 0.0;
            self.last_search_unsettled_devices = 0;
            self.last_search_cycle_rejected = false;
        }
        if search && self.cycle_armed {
            self.cycle_age = self.cycle_age.saturating_add(1);
            if self.cycle_age > 2 {
                self.cycle_armed = false;
                self.cycle_age = 0;
            }
        }
        if search {
            self.search_merit = here;
        }

        // Save the linearisation before the search disturbs it. See
        // `Linearisation`: a rejected trial must leave nothing behind.
        //
        // Only when the search can actually run. `linearisation` is a call
        // per device, and on the common path -- no stall, fewer than
        // `FULL_STEPS` passes -- the search never fires, so the save is pure
        // overhead. Gating it on `search` costs nothing on the passes that
        // need it and returns the whole cost on the passes that do not,
        // which is most of them: the catalogue's voices average two to three
        // and a half passes on real playing, and the search is only turned on
        // at eight or on the first pass that fails to shrink its correction.
        //
        // Full-MNA/watch-mode trials still mutate devices, so those paths
        // snapshot eagerly. Reduced residual probes are read-only; their rare
        // defensive full-MNA fallback snapshots lazily immediately before the
        // first operation that can mutate a device.
        let mut search_state_saved = false;
        if search && !(reduced_direct && !self.watching) {
            for (device, saved) in self.devices.iter().zip(self.saved.iter_mut()) {
                *saved = device.linearisation();
            }
            search_state_saved = true;
        }

        // Solve the Newton linearisation. The direct reduced path fuses full
        // recovery with delta/convergence measurement, so recovered nodes are
        // touched only once.
        let mut reduced_moved = if reduced_direct {
            if dc {
                self.nonlinear_partition_dc.as_mut().and_then(|partition| {
                    partition.solve_stamped_with_delta(
                        &mut self.guess,
                        &self.voltage,
                        &mut self.scratch,
                        TOLERANCE,
                        RELATIVE,
                    )
                })
            } else {
                self.nonlinear_partition.as_mut().and_then(|partition| {
                    partition.solve_stamped_with_delta(
                        &mut self.guess,
                        &self.voltage,
                        &mut self.scratch,
                        TOLERANCE,
                        RELATIVE,
                    )
                })
            }
        } else {
            None
        };

        if reduced_direct && reduced_moved.is_none() {
            // Rare numerical fallback to the original exact full MNA solve.
            reduced_direct = false;
            self.build_current(dc, !search, false);
        }

        if !reduced_direct {
            self.guess.copy_from_slice(&self.rhs);
            let sound = factorise(
                &mut self.work,
                &mut self.pivots,
                &mut self.reach,
                &mut self.depth,
                &mut self.first,
                &self.reach_template,
                &self.depth_template,
                &mut self.plan,
                self.planned,
                n,
            );
            if !sound {
                self.replans += 1;
            }
            self.planned = sound;
            self.work_base_mode = None;
            substitute(
                &self.work,
                &self.pivots,
                &self.reach,
                &self.first,
                &mut self.guess,
                n,
                &mut self.scratch,
            );
            reduced_moved = None;
        }
        // How far the solution wants to move, measured against the scale it is
        // moving *at*.
        //
        // This was an absolute figure: a millionth of a volt, everywhere. On a
        // grid sitting at a tenth of a volt that is a reasonable demand. On a
        // plate sitting at four hundred and fifty it asks for nine significant
        // digits, which is nine orders below anything the circuit does and
        // about six below what the arithmetic can even carry meaningfully --
        // so the solve kept iterating long after the answer had stopped
        // changing in any physical sense.
        //
        // The form is the one every circuit simulator uses: a relative part
        // for the nodes that are large and an absolute floor for the nodes
        // that are small. SPICE calls them RELTOL and VNTOL and ships defaults
        // of 1e-3 and 1e-6; this is a thousand times stricter on the relative
        // part than SPICE and keeps the same absolute floor, because the
        // absolute floor is what a small-signal node needs and there is no
        // reason to relax it.
        //
        // Measured on the *full* Newton correction, never on a shortened one.
        // The test asks whether the answer has stopped moving, and a step that
        // the line search deliberately made small is small for that reason and
        // not because the circuit has settled. Measuring the shortened step
        // instead let a solve declare itself converged at whatever value it
        // happened to be holding: damped to a sixty-fourth, the 73P reported
        // itself done after a single pass. See `R-013` in the regression log.
        let moved = if let Some(moved) = reduced_moved {
            moved
        } else {
            let mut moved: f64 = 0.0;
            for ((&guess, &voltage), delta) in self
                .guess
                .iter()
                .zip(&self.voltage)
                .zip(self.scratch.iter_mut())
            {
                if !guess.is_finite() {
                    self.nonfinite += 1;
                    return Pass::Stuck;
                }
                *delta = guess - voltage;
                let scale = TOLERANCE + RELATIVE * voltage.abs();
                moved = moved.max(delta.abs() / scale);
            }
            moved
        };
        // Kept so the caller can see whether this pass made progress, and turn
        // the line search on the moment one does not. See `CONVERGING`.
        self.moved = moved;
        #[cfg(test)]
        if search
            && collect_post_restart_cycle_geometry(
                self.test_trace_search_geometry,
                self.test_post_restart_continuation,
            )
        {
            let mut max_norm = 0.0f64;
            let mut max_at = 0usize;
            for (at, (&delta, &voltage)) in self.scratch.iter().zip(&self.voltage).enumerate() {
                let scale = TOLERANCE + RELATIVE * voltage.abs();
                let norm = delta.abs() / scale;
                if norm > max_norm {
                    max_norm = norm;
                    max_at = at;
                }
            }
            self.last_search_max_correction_unknown = max_at;
            self.last_search_max_correction_norm = max_norm;
            self.last_search_max_correction_abs = self.scratch[max_at].abs();
            self.last_search_max_correction_voltage = self.voltage[max_at];
            self.last_search_max_correction_guess = self.guess[max_at];
            self.last_search_unsettled_devices = self
                .devices
                .iter()
                .filter(|device| !device.settled(TOLERANCE))
                .count();
        }

        // Converged? Then stop here, before the line search, and take the
        // full step -- which is by definition a tiny one.
        //
        // This test has to come first, and getting that wrong is subtle. A
        // converged solve has a residual sitting at round-off, and a residual
        // at round-off cannot get any smaller: every trial the line search
        // offers is rejected for failing to improve on a number that is
        // already as small as arithmetic allows, and the search reports
        // failure at precisely the moment it has succeeded. Measured with the
        // test in the wrong place, the transformer core -- five unknowns, one
        // device, converging in two passes flat -- reported 7854 failed
        // solves a second.
        if moved < 1.0 && self.devices.iter().all(|d| d.settled(TOLERANCE)) {
            self.voltage.copy_from_slice(&self.guess);
            return Pass::Settled;
        }

        // Not there yet, so the step has to earn its place. A fresh search
        // starts with the full Newton step, and almost always takes it. On the
        // Twin's difficult tail only, consecutive searched passes may reuse a
        // bounded damping hint from the preceding pass so we do not re-probe
        // step lengths that were just shown to be too aggressive.
        if !search {
            self.voltage.copy_from_slice(&self.guess);
            return Pass::Moved;
        }

        // Preserve each circuit's existing shortest allowed trial. Twin uses
        // four trials, so its old dyadic floor was 1/8; the generic six-trial
        // path bottoms at 1/32. Polynomial interpolation may skip directly
        // toward that floor but may not search deeper than the old policy.
        let search_floor = (0.5f64)
            .powi(self.backtracks.saturating_sub(1) as i32)
            .max(MIN_LAMBDA);
        let mut lambda = if self.late_continuation
            && !dc
            && self.line_search_warm_start_enabled()
        {
            self.search_lambda_hint.clamp(search_floor, 1.0)
        } else {
            1.0
        };
        let mut taken = false;
        let mut best_lambda = 1.0;
        let mut best_merit = f64::INFINITY;
        let mut cycle_rejected = false;
        let mut accepted_pure_reduced = false;
        let mut devices_dirty = false;
        #[cfg(test)]
        let mut accepted_merit = 0.0;
        // `point` is both the device linearisation point and the candidate
        // voltage vector. Keeping one buffer removes the old trial->point copy
        // from every backtracking attempt.
        //
        // The first exact trial after a full-MNA solve sees LU factors in
        // `work`; restore only the structural coefficients that the merit can
        // observe. A reduced solve never factorises `work`, so it can take the
        // cheaper device-footprint restamp from the first trial. Every rejected
        // trial after that also leaves an unfactorised trial stamp behind.
        // Structure-watch mode deliberately retains the exhaustive full build.
        let mut first_trial = true;
        for _ in 0..self.backtracks {
            // The exact Schur merit reads only nonlinear-boundary unknowns.
            // Build those on the hot reduced path; full-MNA diagnostics and
            // defensive fallbacks still materialise every unknown.
            let mut pure_reduced_trial = reduced_direct && !self.watching;
            let mut interpolated_next = None;
            let finite = if pure_reduced_trial {
                self.build_reduced_trial_point(dc, lambda)
            } else {
                self.build_full_trial_point(lambda)
            };
            if finite {
                // Re-linearise where the step lands and ask whether the
                // circuit is any closer to satisfying itself there.
                let reduced_there = if reduced_direct && !self.watching {
                    match self.reduced_trial_merit(dc, true) {
                        Some(merit) => Some(merit),
                        None => {
                            // Defensive topology fallback. A correctly built
                            // boundary contains every nonlinear terminal, but
                            // if that invariant is ever violated do not judge
                            // a trial against stale full-MNA storage.
                            pure_reduced_trial = false;
                            if !self.build_full_trial_point(lambda) {
                                break;
                            }
                            if !search_state_saved {
                                for (device, saved) in
                                    self.devices.iter().zip(self.saved.iter_mut())
                                {
                                    *saved = device.linearisation();
                                }
                                search_state_saved = true;
                            }
                            self.build_point_full(dc);
                            devices_dirty = true;
                            None
                        }
                    }
                } else {
                    if self.watching {
                        self.build_point_full(dc);
                    } else if first_trial {
                        self.restamp_first_trial(dc);
                    } else {
                        self.restamp_trial(dc);
                    }
                    devices_dirty = true;
                    None
                };
                first_trial = false;
                if !self.exact {
                    // A junction was held where this step lands, so the
                    // residual there is somewhere else's. Stop searching and
                    // let the full step stand rather than decide on it.
                    break;
                }
                let there = reduced_there.unwrap_or_else(|| self.merit(&self.point, false));
                #[cfg(test)]
                if self.test_trace_search_geometry
                    && self.last_search_trial_count < SEARCH_TRACE_TRIALS
                {
                    let slot = self.last_search_trial_count;
                    self.last_search_trial_lambdas[slot] = lambda;
                    self.last_search_trial_merits[slot] = there;
                    self.last_search_trial_exact[slot] = self.exact;
                    self.last_search_trial_count += 1;
                }
                if there.is_finite() && there < best_merit {
                    best_merit = there;
                    best_lambda = lambda;
                }
                let improves = search_trial_improves(here, reference, there);
                // On the Twin's exact Schur path use the rejected residual to
                // fit a safeguarded quadratic along this same Newton ray. PETSc
                // and other production Newton solvers use polynomial
                // backtracking for this reason: halving is robust but often
                // spends multiple function evaluations walking past the useful
                // damping length.  The acceptance test itself is unchanged.
                if self.late_continuation
                    && self.quadratic_backtracking_enabled()
                    && reduced_direct
                    && pure_reduced_trial
                    && !dc
                    && !improves
                {
                    interpolated_next = Some(quadratic_backtrack_lambda(
                        here,
                        there,
                        lambda,
                        search_floor,
                    ));
                }
                let improves = if improves
                    && self.late_continuation
                    && !dc
                    && repeat_cycle_edge(here, reference, there)
                {
                    if self.cycle_armed
                        && self.cycle_age <= 2
                        && repeat_cycle_pair_matches(
                            self.cycle_here,
                            self.cycle_reference,
                            here,
                            reference,
                        )
                    {
                        cycle_rejected = true;
                        #[cfg(test)]
                        {
                            self.last_search_cycle_rejected = true;
                        }
                        false
                    } else {
                        self.cycle_armed = true;
                        self.cycle_age = 0;
                        self.cycle_here = here;
                        self.cycle_reference = reference;
                        true
                    }
                } else {
                    improves
                };
                if improves {
                    taken = true;
                    accepted_pure_reduced = pure_reduced_trial;
                    #[cfg(test)]
                    {
                        accepted_merit = there;
                    }
                    break;
                }
            }
            // Refused. Put every device back where it was, so the next,
            // shorter trial is measured from the same place this one was.
            // Reduced residual probes are read-only, so the overwhelmingly
            // common Twin/HM/MT path has nothing to restore here.
            if devices_dirty {
                for (device, saved) in self.devices.iter_mut().zip(self.saved.iter()) {
                    device.relinearise(*saved);
                }
                devices_dirty = false;
            }
            self.backtrack_count += 1;
            if lambda <= search_floor {
                break;
            }
            lambda = interpolated_next.unwrap_or(lambda * 0.5).max(search_floor);
        }

        #[cfg(test)]
        if !taken
            && self.test_trace_search_geometry
            && search
            && self.backtracks >= CONTINUATION_BACKTRACKS
            && self.last_search_trial_count < SEARCH_TRACE_TRIALS
        {
            // Diagnostic-only continuation of the same Newton ray. Production
            // still stops after its six bounded trials (through 1/32). These
            // passive probes measure 1/64, 1/128 and 1/256, restore every
            // device linearisation, and never participate in step selection.
            // That tells us whether a pathological refusal merely needs a
            // shorter damping length or whether the Newton direction itself
            // has flattened out.
            let exact_before_probe = self.exact;
            let mut probe_lambda = 0.5f64.powi(self.backtracks as i32);
            while self.last_search_trial_count < SEARCH_TRACE_TRIALS {
                let mut finite = true;
                for ((point, &voltage), &delta) in self
                    .point
                    .iter_mut()
                    .zip(&self.voltage)
                    .zip(&self.scratch)
                {
                    let value = voltage + probe_lambda * delta;
                    *point = value;
                    finite &= value.is_finite();
                }
                if !finite {
                    break;
                }

                let mut probe_devices_dirty = false;
                let reduced_there = if reduced_direct && !self.watching {
                    match self.reduced_trial_merit(dc, false) {
                        Some(merit) => Some(merit),
                        None => {
                            // The ordinary reduced probe is read-only and did
                            // not require a saved device state. Only this rare
                            // full-MNA diagnostic fallback mutates devices, so
                            // capture the state lazily before it does.
                            if !search_state_saved {
                                for (device, saved) in
                                    self.devices.iter().zip(self.saved.iter_mut())
                                {
                                    *saved = device.linearisation();
                                }
                                search_state_saved = true;
                            }
                            self.build_point_full(dc);
                            probe_devices_dirty = true;
                            None
                        }
                    }
                } else {
                    if self.watching {
                        self.build_point_full(dc);
                    } else {
                        self.restamp_trial(dc);
                    }
                    probe_devices_dirty = true;
                    None
                };
                let exact_probe = self.exact;
                if !exact_probe {
                    if probe_devices_dirty {
                        for (device, saved) in self.devices.iter_mut().zip(self.saved.iter()) {
                            device.relinearise(*saved);
                        }
                    }
                    break;
                }
                let there = reduced_there.unwrap_or_else(|| self.merit(&self.point, false));
                let slot = self.last_search_trial_count;
                self.last_search_trial_lambdas[slot] = probe_lambda;
                self.last_search_trial_merits[slot] = there;
                self.last_search_trial_exact[slot] = true;
                self.last_search_trial_count += 1;

                if probe_devices_dirty {
                    for (device, saved) in self.devices.iter_mut().zip(self.saved.iter()) {
                        device.relinearise(*saved);
                    }
                }
                probe_lambda *= 0.5;
            }
            self.exact = exact_before_probe;
        }

        if cycle_rejected {
            self.cycle_armed = false;
            self.cycle_age = 0;
        }

        if !taken {
            // Nothing of any length improved the residual. That is not a
            // reason to stop: abandoning the solve here leaves it at a point
            // it had not finished with, and measured against an accurate
            // reference solve that was *worse* than the plain bounded Newton
            // this replaced -- the 5150 went from 48.8 dB below the reference
            // to 31.7 dB below it, which is a solver getting less right, not
            // more.
            //
            // Use the best finite trial that was actually measured, even
            // though it did not beat the reference. Unconditionally taking
            // the full jump here discarded useful search results and made
            // the 5150 oscillate through its iteration allowance on attacks.
            // If no finite exact merit was measured, retain the full step.
            self.fallbacks += 1;
            if self.late_continuation && !dc && self.line_search_warm_start_enabled() {
                self.search_lambda_hint =
                    line_search_warm_start_lambda(best_lambda, search_floor);
            }
            if devices_dirty {
                for (device, saved) in self.devices.iter_mut().zip(self.saved.iter()) {
                    device.relinearise(*saved);
                }
            }
            if dc || best_lambda == 1.0 {
                self.voltage.copy_from_slice(&self.guess);
            } else {
                for (voltage, &delta) in self.voltage.iter_mut().zip(&self.scratch) {
                    *voltage += best_lambda * delta;
                }
            }
            return Pass::Moved;
        }

        if self.late_continuation && !dc && self.line_search_warm_start_enabled() {
            self.search_lambda_hint = line_search_warm_start_lambda(lambda, search_floor);
        }

        #[cfg(test)]
        {
            self.last_search_accepted_lambda = lambda;
            self.last_search_accepted_merit = accepted_merit;
        }
        if accepted_pure_reduced {
            // The probe itself deliberately did not mutate any device. Commit
            // the winner once so limiter, op-amp rail and core state are the
            // same state the old mutating trial left for continuation logic.
            for device in &mut self.devices {
                device.commit_trial_state(&self.point);
            }
            // Materialise the eliminated/internal entries once, preserving the
            // exact arithmetic expression the old full candidate loop used.
            for (voltage, &delta) in self.voltage.iter_mut().zip(&self.scratch) {
                let value = *voltage + lambda * delta;
                *voltage = value;
            }
        } else {
            self.voltage.copy_from_slice(&self.point);
        }
        Pass::Moved
    }

    /// One sample in, one out.
    pub fn process(&mut self, input: f64) -> f64 {
        self.solves += 1;
        // A control has moved. What that means depends on whether there is
        // anything to lose.
        //
        // With nothing in flight -- a circuit just built, just reset, or set
        // up before playback starts -- settle it where the control now puts
        // it. That is what makes a plugin start at its operating point instead
        // of climbing to it, and it is what every measurement harness here
        // relies on to mean "this circuit, at this setting".
        //
        // Mid-signal it must not. Hunting the operating point solves the
        // circuit *with no signal in it* and then sets every capacitor from
        // that answer, which is to say it deletes the audio in flight. Done
        // once a block for as long as a knob is turning -- see
        // `examples/knobmove.rs` and `tests/knobs.rs`.
        if self.dirty {
            self.rebuild();
            if self.at_rest {
                self.find_operating_point();
            }
            self.dirty = false;
        }
        self.at_rest = false;
        let n = self.n;

        // Whether this sample's solve failed to converge. It gates the
        // reactance advance at the end of this function; see the advance
        // block for why that gate is necessary.
        let mut failed = false;

        if self.devices.is_empty() {
            // Nothing bends, so the matrix from `rebuild` still stands and one
            // substitution is the whole solve.
            for k in 0..n {
                let mut value = self.source[k] * input + self.bias[k];
                for (slot, &aux) in self.aux_values.iter().enumerate() {
                    value += self.aux_source[slot * n + k] * aux;
                }
                self.rhs[k] = value;
            }
            for c in &self.capacitors {
                inject(&mut self.rhs, c.a, c.b, c.history);
            }
            for l in &mut self.inductors {
                inject(&mut self.rhs, l.a, l.b, l.history);
            }
            let partitioned = if let Some(partition) = &self.linear_partition {
                partition.solve_into(
                    &self.rhs,
                    &mut self.voltage,
                    &mut self.scratch,
                    &mut self.guess,
                )
            } else {
                false
            };
            if !partitioned {
                substitute(
                    &self.matrix,
                    &self.pivots,
                    &self.reach,
                    &self.first,
                    &mut self.rhs,
                    n,
                    &mut self.scratch,
                );
                self.voltage.copy_from_slice(&self.rhs);
            }
        } else {
            self.prepare_rhs(input, false);
            // Take the recent movement before the predictor runs.
            //
            // `|V[n-1] - V[n-2]|` is the per-node scale the failure
            // branch's bound is measured in. Taken before the predictor
            // because the predictor overwrites `voltage` with the
            // extrapolation and shifts `earlier` up one -- after which
            // `|voltage - earlier|` is `|V_pred - V[n-1]|`, which happens
            // to equal the movement by accident of the extrapolation's
            // arithmetic and stops equalling it the moment the
            // extrapolation is skipped.
            // Start from where the last two samples were heading, not from
            // where the last one was. Audio is smooth over a sample, so a
            // straight line through the last two lands much closer to the
            // answer than the last one does, and Newton then has less to do:
            // measured, the passes drop from 2.9 a sample to the floor of 2.
            //
            // Only where nothing in the circuit switches. Carried through an
            // op-amp deciding which rail it is against, the prediction put its
            // output at -90 V against a 3.5 V rail and left it there: two
            // passes is enough for a device that bends and not for one that
            // steps, and a settled wrong state still reports itself settled.
            //
            // And only when the previous sample converged. A failed solve's
            // answer is a bounded guess, and extending a bounded guess by
            // doubling its distance from the sample before it produces a
            // number that is further from the truth than the guess was. On
            // the sample after a failure the correct starting point is
            // `V[n-1]` directly -- the bounded guess the previous sample
            // already produced -- and letting Newton walk from there.
            // `recent_move`, predictor bookkeeping and the predictor itself
            // used to be three separate full-vector operations here. These
            // vectors are touched for every nonlinear sample, so fuse them
            // into one walk without changing any arithmetic or state order.
            // On the predictable path each element still observes the old
            // voltage/earlier pair before either is updated.
            let predictor_scale = if self.predictable && !self.last_was_unsettled {
                if self.source_scaled_predictor_enabled() {
                    predictor_source_scale(input, self.last_input, self.earlier_input)
                } else if self.input_curvature_predictor_gate_enabled() {
                    if predictor_input_slope_is_consistent(
                        input,
                        self.last_input,
                        self.earlier_input,
                    ) {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    1.0
                }
            } else {
                0.0
            };
            let suppress_predictor = self.predictable
                && !self.last_was_unsettled
                && self.input_curvature_predictor_gate_enabled()
                && predictor_scale == 0.0;
            if suppress_predictor {
                self.attack_predictor_suppressions += 1;
            }

            if self.predictable && !self.last_was_unsettled && predictor_scale != 0.0 {
                for ((voltage, earlier), recent) in self
                    .voltage
                    .iter_mut()
                    .zip(self.earlier.iter_mut())
                    .zip(self.recent_move.iter_mut())
                {
                    let last = *voltage;
                    let previous = *earlier;
                    *recent = (last - previous).abs();
                    *earlier = last;
                    *voltage = last + predictor_scale * (last - previous);
                }
            } else {
                for ((&voltage, earlier), recent) in self
                    .voltage
                    .iter()
                    .zip(self.earlier.iter_mut())
                    .zip(self.recent_move.iter_mut())
                {
                    *recent = (voltage - *earlier).abs();
                    *earlier = voltage;
                }
            }
            // `predicted` used to receive a copy of this starting point on
            // every nonlinear sample. Continuation overwrites the buffer
            // immediately before it needs a backup, and the failure bound uses
            // `recent_move`, so this write was dead work in the realtime path.
            let mut settled = false;
            let ceiling = self.ceiling.clamp(PASS_FLOOR, MAX_ITERATIONS);
            self.cycle_armed = false;
            self.cycle_age = 0;
            self.cycle_here = 0.0;
            self.cycle_reference = 0.0;
            #[cfg(test)]
            {
                self.last_search_cycle_rejected = false;
            }

            if !self.late_continuation {
                // Exact pre-continuation fast path. Keep this branch structurally
                // identical to the measured-good solver so circuits that do not
                // opt in pay no continuation bookkeeping at all.
                self.moved = f64::INFINITY;
                self.search_merit = 0.0;
                let mut before = f64::INFINITY;
                for pass in 0..ceiling {
                    self.newton_passes += 1;
                    let stalled = self.moved > before * CONVERGING;
                    before = self.moved;
                    match self.iterate(false, stalled || pass >= FULL_STEPS) {
                        Pass::Settled => {
                            settled = true;
                            break;
                        }
                        Pass::Moved => {}
                        Pass::Stuck => break,
                    }
                }
            } else {
                // Bounded late rescue for the measured 5150/Twin power stages.
                // The normal predictor and all early target passes are unchanged.
                // A midpoint correction is permitted only after a late target
                // pass exhausts its line-search improvement attempts (or goes
                // non-finite), and it consumes one slot from the same ceiling.
                self.moved = f64::INFINITY;
                self.search_merit = 0.0;
                let mut before = f64::INFINITY;
                let mut used_passes = 0usize;
                let mut target_passes = 0usize;
                let mut continuation_from_stuck = None;
                // A recoverable late line-search fallback must repeat before
                // source continuation is worth a midpoint solve. Genuine
                // `Pass::Stuck` still continues immediately.
                let mut late_rejection_armed = false;
                // V3.1 lets the first smooth valve-grid limiter correction run
                // locally before handing a repeated large step to the global
                // line search. This is per exact-target solve and allocation-free.
                let mut limiter_handoff_armed = false;
                let mut source_changed = false;
                // A deep continuation step that actually beats the exact-target
                // merit opens a useful Newton basin.  Once that happens, stop
                // paying for 1/16 and 1/32 trials and spend any remaining work
                // on ordinary Newton confirmation instead.
                let mut deep_rescue_accepted = false;
                let mut low_residual_confirmation = false;
                // A full-budget solve may be making excellent exact-target
                // progress even though a device limiter made the original jump
                // too large to finish in 64 passes.  Track only consecutive
                // undamped full-Newton residual collapses.  If four of those
                // reach the wall, preserve that branch for a bounded tail
                // instead of immediately restarting it from `earlier`.
                let mut strong_exact_tail_passes = 0usize;
                let mut exhausted_exact_tail = false;
                // Explicit test-only restart/staging experiments must retain
                // their old 64-pass entry point so their measurements remain
                // comparable. Normal tests and release builds use the same
                // production recovery policy.
                let production_recovery_enabled = {
                    #[cfg(test)]
                    {
                        !self.test_last_settled_restart
                            && !self.test_post_restart_continuation
                    }
                    #[cfg(not(test))]
                    {
                        true
                    }
                };
                #[cfg(test)]
                let mut post_deep_confirmation_passes = 0usize;
                #[cfg(test)]
                let mut post_low_residual_confirmation_passes = 0usize;
                #[cfg(test)]
                let mut repeat_cycle_rejections = 0usize;
                #[cfg(test)]
                let mut continuation_trigger_pass = 0usize;
                #[cfg(test)]
                let mut continuation_trigger_target_passes = 0usize;
                #[cfg(test)]
                let mut continuation_trigger_was_stuck = false;
                #[cfg(test)]
                let mut continuation_trigger_moved = 0.0f64;
                #[cfg(test)]
                let mut continuation_trigger_merit = 0.0f64;
                #[cfg(test)]
                let mut continuation_midpoint = 0.0f64;
                #[cfg(test)]
                let mut continuation_midpoint_moved = 0.0f64;
                #[cfg(test)]
                let mut continuation_midpoint_stuck = false;
                #[cfg(test)]
                let mut last_settled_restart_attempted = false;
                #[cfg(test)]
                let mut last_settled_restart_passes = 0usize;
                #[cfg(test)]
                let mut last_settled_restart_backtracks = 0u64;
                #[cfg(test)]
                let mut last_settled_restart_fallbacks = 0u64;
                #[cfg(test)]
                let mut last_settled_restart_settled = false;
                #[cfg(test)]
                let sample_backtracks_start = self.backtrack_count;
                #[cfg(test)]
                let sample_fallbacks_start = self.fallbacks;
                #[cfg(test)]
                let mut continuation_deep_passes = 0usize;
                #[cfg(test)]
                let mut continuation_deep_rescues = 0usize;
                #[cfg(test)]
                let mut continuation_weak_deep_improvements = 0usize;
                #[cfg(test)]
                let mut deep_first_improvement_pass = 0usize;
                #[cfg(test)]
                let mut deep_last_improvement_pass = 0usize;
                #[cfg(test)]
                let mut deep_first_lambda = 0.0f64;
                #[cfg(test)]
                let mut deep_last_lambda = 0.0f64;
                #[cfg(test)]
                let mut deep_first_moved_before = 0.0f64;
                #[cfg(test)]
                let mut deep_first_moved_after = 0.0f64;
                #[cfg(test)]
                let mut deep_first_merit_before = 0.0f64;
                #[cfg(test)]
                let mut deep_first_merit_after = 0.0f64;
                #[cfg(test)]
                let mut deep_last_moved_before = 0.0f64;
                #[cfg(test)]
                let mut deep_last_moved_after = 0.0f64;
                #[cfg(test)]
                let mut deep_last_merit_before = 0.0f64;
                #[cfg(test)]
                let mut deep_last_merit_after = 0.0f64;
                #[cfg(test)]
                let mut probe_pass = 0usize;
                #[cfg(test)]
                let mut probe_here = 0.0f64;
                #[cfg(test)]
                let mut probe_reference = 0.0f64;
                #[cfg(test)]
                let mut probe_trial_count = 0usize;
                #[cfg(test)]
                let mut probe_lambdas = [0.0; SEARCH_TRACE_TRIALS];
                #[cfg(test)]
                let mut probe_merits = [0.0; SEARCH_TRACE_TRIALS];
                #[cfg(test)]
                let mut probe_exact = [false; SEARCH_TRACE_TRIALS];
                #[cfg(test)]
                let mut probe_max_correction_unknown = 0usize;
                #[cfg(test)]
                let mut probe_max_correction_norm = 0.0f64;
                #[cfg(test)]
                let mut probe_max_correction_abs = 0.0f64;
                #[cfg(test)]
                let mut probe_max_correction_voltage = 0.0f64;
                #[cfg(test)]
                let mut probe_max_correction_guess = 0.0f64;
                #[cfg(test)]
                let mut probe_unsettled_devices = 0usize;
                #[cfg(test)]
                let mut tail_trace_count = 0usize;
                #[cfg(test)]
                let mut tail_trace_passes = [0usize; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_here = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_reference = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_accepted_lambda = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_accepted_merit = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_trial_count = [0usize; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_trial_lambdas = [[0.0f64; SEARCH_TRACE_TRIALS]; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_trial_merits = [[0.0f64; SEARCH_TRACE_TRIALS]; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_moved_before = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_moved_after = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_fallback = [false; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_max_unknown = [0usize; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_max_norm = [0.0f64; TAIL_TRACE_PASSES];
                #[cfg(test)]
                let mut tail_trace_unsettled_devices = [0usize; TAIL_TRACE_PASSES];

                loop {
                    // The normal path is still capped by `ceiling`. At the
                    // full 64-pass ceiling, proven confirmation windows may add
                    // a few ordinary passes, and a solve that reaches the wall on
                    // a sustained undamped residual collapse may keep that exact
                    // branch for the separately bounded convergent tail. A
                    // host-pinched ceiling is never exceeded.
                    let confirmation_limit = if ceiling == MAX_ITERATIONS {
                        if exhausted_exact_tail {
                            ceiling + EXHAUSTED_CONVERGENT_TAIL_PASSES
                        } else if deep_rescue_accepted {
                            ceiling + POST_DEEP_CONFIRMATION_PASSES
                        } else if low_residual_confirmation {
                            #[cfg(test)]
                            {
                                weak_deep_confirmation_limit(
                                    ceiling,
                                    used_passes,
                                    continuation_weak_deep_improvements,
                                    self.search_merit,
                                    self.devices.iter().all(|device| device.settled(TOLERANCE)),
                                    self.test_weak_deep_recovery,
                                )
                            }
                            #[cfg(not(test))]
                            {
                                ceiling + POST_LOW_RESIDUAL_CONFIRMATION_PASSES
                            }
                        } else {
                            ceiling
                        }
                    } else {
                        ceiling
                    };
                    if used_passes >= confirmation_limit {
                        break;
                    }
                    #[cfg(test)]
                    if used_passes >= ceiling {
                        if deep_rescue_accepted {
                            post_deep_confirmation_passes += 1;
                        } else if low_residual_confirmation {
                            post_low_residual_confirmation_passes += 1;
                        }
                    }
                    // A continuation midpoint solves a different source RHS.
                    // Invalidate every progress/merit comparison that belonged
                    // to that RHS before returning to the exact target.  The
                    // target pass may still line-search immediately: iterate()
                    // rebuilds the exact-target system first and measures a
                    // fresh `here` merit there, so no residual from the
                    // midpoint is compared with a target residual.
                    if source_changed {
                        before = f64::INFINITY;
                        self.moved = f64::INFINITY;
                        self.search_merit = 0.0;
                        source_changed = false;
                    }
                    let stalled = self.moved > before * CONVERGING;
                    before = self.moved;
                    self.newton_passes += 1;
                    used_passes += 1;
                    let fallbacks_before = self.fallbacks;
                    let backtracks_before = self.backtrack_count;
                    let previous_search_merit = self.search_merit;
                    let search = stalled || target_passes >= FULL_STEPS;

                    // Keep the Twin's four-trial realtime search until a
                    // continuation solve is genuinely running out of Newton
                    // budget. Only the final few slots may try 1/16 and 1/32;
                    // this avoids charging successful pass-12/13 rescues for
                    // work that the v9 timing run proved they did not need.
                    let normal_backtracks = self.backtracks;
                    let deep_continuation_search = search
                        && continuation_from_stuck.is_some()
                        && !deep_rescue_accepted
                        && normal_backtracks < CONTINUATION_BACKTRACKS
                        && self.continuation_deepening_enabled()
                        && used_passes < ceiling
                        && used_passes
                            .saturating_add(CONTINUATION_DEEPENING_TAIL_PASSES)
                            >= ceiling;
                    if deep_continuation_search {
                        self.backtracks = CONTINUATION_BACKTRACKS;
                        #[cfg(test)]
                        {
                            continuation_deep_passes += 1;
                        }
                    }
                    #[cfg(test)]
                    let deep_moved_before = if deep_continuation_search { before } else { 0.0 };
                    let limiter_would_hold = !search
                        && self.late_continuation
                        && self.limiter_global_search_enabled()
                        && self.valve_grid_limiter_would_hold();
                    let allow_limiter_handoff = limiter_global_search_handoff(
                        limiter_would_hold,
                        search,
                        &mut limiter_handoff_armed,
                        self.immediate_limiter_global_search(),
                    );
                    let pass = self.iterate_with_limiter_handoff(
                        false,
                        search,
                        allow_limiter_handoff,
                    );
                    self.backtracks = normal_backtracks;
                    target_passes += 1;
                    let line_search_failed = self.fallbacks > fallbacks_before;
                    let backtracks_this_pass = self.backtrack_count - backtracks_before;
                    let strong_exact_progress = exact_tail_progress(
                        search,
                        line_search_failed,
                        backtracks_this_pass,
                        previous_search_merit,
                        self.search_merit,
                        EXHAUSTED_TAIL_STRONG_RATIO,
                    );
                    if strong_exact_progress {
                        strong_exact_tail_passes = strong_exact_tail_passes.saturating_add(1);
                    } else if search {
                        strong_exact_tail_passes = 0;
                    }

                    if used_passes == ceiling
                        && ceiling == MAX_ITERATIONS
                        && production_recovery_enabled
                        && self.late_continuation
                        && !deep_rescue_accepted
                        && strong_exact_tail_passes >= EXHAUSTED_TAIL_ARM_PASSES
                    {
                        exhausted_exact_tail = true;
                        if continuation_from_stuck.is_none() {
                            self.continuation_attempts += 1;
                        }
                    } else if exhausted_exact_tail && used_passes > ceiling {
                        // Once armed, keep going only while the same exact-target
                        // full-Newton branch continues to make meaningful residual
                        // progress.  A backtrack, fallback, or flattening tail
                        // returns immediately to the existing restart/staging path.
                        exhausted_exact_tail = exact_tail_progress(
                            search,
                            line_search_failed,
                            backtracks_this_pass,
                            previous_search_merit,
                            self.search_merit,
                            EXHAUSTED_TAIL_KEEP_RATIO,
                        );
                    }
                    #[cfg(test)]
                    if self.last_search_cycle_rejected {
                        repeat_cycle_rejections += 1;
                    }
                    #[cfg(test)]
                    if collect_post_restart_cycle_geometry(
                        self.test_trace_search_geometry,
                        self.test_post_restart_continuation,
                    ) && search
                    {
                        // Keep a rolling, allocation-free history of the last
                        // few *actual* line searches.  Earlier diagnostics only
                        // recorded the final deep-continuation window, which
                        // left short 3-20 pass failures opaque.  This is test
                        // instrumentation only and does not alter solver policy.
                        let slot = if tail_trace_count < TAIL_TRACE_PASSES {
                            let slot = tail_trace_count;
                            tail_trace_count += 1;
                            slot
                        } else {
                            tail_trace_passes.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_here.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_reference.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_accepted_lambda.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_accepted_merit.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_trial_count.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_trial_lambdas.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_trial_merits.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_moved_before.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_moved_after.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_fallback.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_max_unknown.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_max_norm.copy_within(1..TAIL_TRACE_PASSES, 0);
                            tail_trace_unsettled_devices.copy_within(1..TAIL_TRACE_PASSES, 0);
                            TAIL_TRACE_PASSES - 1
                        };
                        tail_trace_passes[slot] = used_passes;
                        tail_trace_here[slot] = self.last_search_here;
                        tail_trace_reference[slot] = self.last_search_reference;
                        tail_trace_accepted_lambda[slot] = self.last_search_accepted_lambda;
                        tail_trace_accepted_merit[slot] = self.last_search_accepted_merit;
                        tail_trace_trial_count[slot] = self.last_search_trial_count;
                        tail_trace_trial_lambdas[slot] = self.last_search_trial_lambdas;
                        tail_trace_trial_merits[slot] = self.last_search_trial_merits;
                        tail_trace_moved_before[slot] = before;
                        tail_trace_moved_after[slot] = self.moved;
                        tail_trace_fallback[slot] = line_search_failed;
                        tail_trace_max_unknown[slot] = self.last_search_max_correction_unknown;
                        tail_trace_max_norm[slot] = self.last_search_max_correction_norm;
                        tail_trace_unsettled_devices[slot] = self.last_search_unsettled_devices;
                    }
                    #[cfg(test)]
                    if deep_continuation_search && line_search_failed && probe_pass == 0 {
                        probe_pass = used_passes;
                        probe_here = self.last_search_here;
                        probe_reference = self.last_search_reference;
                        probe_trial_count = self.last_search_trial_count;
                        probe_lambdas = self.last_search_trial_lambdas;
                        probe_merits = self.last_search_trial_merits;
                        probe_exact = self.last_search_trial_exact;
                        probe_max_correction_unknown = self.last_search_max_correction_unknown;
                        probe_max_correction_norm = self.last_search_max_correction_norm;
                        probe_max_correction_abs = self.last_search_max_correction_abs;
                        probe_max_correction_voltage = self.last_search_max_correction_voltage;
                        probe_max_correction_guess = self.last_search_max_correction_guess;
                        probe_unsettled_devices = self.last_search_unsettled_devices;
                    }
                    if deep_continuation_search
                        && !line_search_failed
                        && self.backtrack_count - backtracks_before
                            >= normal_backtracks as u64
                    {
                        // A 1/16 or 1/32 trial beat the exact-target merit.
                        // Production retains the measured v14 policy: any
                        // accepted deep step latches deep search off.  The
                        // test-only v27 experiment keeps accepting weak deep
                        // steps but requires a material merit drop before it
                        // calls the basin opened and spends confirmation work.
                        #[cfg(test)]
                        let latch_deep_rescue = !self.test_weak_deep_recovery
                            || deep_rescue_is_strong(
                                self.last_search_here,
                                self.last_search_accepted_merit,
                            );
                        #[cfg(not(test))]
                        let latch_deep_rescue = true;

                        if latch_deep_rescue {
                            deep_rescue_accepted = true;
                            #[cfg(test)]
                            {
                                continuation_deep_rescues += 1;
                                let lambda = self.last_search_accepted_lambda;
                                if deep_first_improvement_pass == 0 {
                                    deep_first_improvement_pass = used_passes;
                                    deep_first_lambda = lambda;
                                    deep_first_moved_before = deep_moved_before;
                                    deep_first_moved_after = self.moved;
                                    deep_first_merit_before = self.last_search_here;
                                    deep_first_merit_after = self.last_search_accepted_merit;
                                }
                                deep_last_improvement_pass = used_passes;
                                deep_last_lambda = lambda;
                                deep_last_moved_before = deep_moved_before;
                                deep_last_moved_after = self.moved;
                                deep_last_merit_before = self.last_search_here;
                                deep_last_merit_after = self.last_search_accepted_merit;
                            }
                        } else {
                            #[cfg(test)]
                            {
                                continuation_weak_deep_improvements += 1;
                            }
                        }
                    }

                    // Some solves arrive at the 64-pass wall only one or
                    // two strict Newton confirmations short of convergence,
                    // without having used the deep-continuation rescue.  The
                    // exact pre-step merit is already tiny, the searched step
                    // was accepted (not a fallback), and every nonlinear
                    // device agrees locally.  Latch a three-pass ordinary
                    // confirmation window for only that case.
                    if used_passes == ceiling
                        && ceiling == MAX_ITERATIONS
                        && !deep_rescue_accepted
                        && !line_search_failed
                        && search
                        && self.search_merit <= LOW_RESIDUAL_CONFIRMATION_MERIT
                        && self.devices.iter().all(|device| device.settled(TOLERANCE))
                    {
                        low_residual_confirmation = true;
                    }

                    if matches!(pass, Pass::Settled) {
                        settled = true;
                        let exact_tail_rescue =
                            exhausted_exact_tail && used_passes > ceiling;
                        if exact_tail_rescue {
                            self.continuation_actual_rescues += 1;
                            if continuation_from_stuck.is_none() {
                                self.continuation_successes += 1;
                            }
                        }
                        if let Some(from_stuck) = continuation_from_stuck {
                            self.continuation_successes += 1;
                            if from_stuck && !exact_tail_rescue {
                                self.continuation_actual_rescues += 1;
                            }
                        }
                        break;
                    }

                    let ordinary_stuck = matches!(pass, Pass::Stuck);
                    #[cfg(test)]
                    let continuation_stuck_only = self.test_continuation_stuck_only;
                    #[cfg(not(test))]
                    let continuation_stuck_only = false;
                    #[cfg(test)]
                    let continuation_min_target_passes =
                        self.test_continuation_min_target_passes;
                    #[cfg(not(test))]
                    let continuation_min_target_passes =
                        LATE_CONTINUATION_MIN_TARGET_PASSES;
                    #[cfg(test)]
                    let single_late_rejection_continuation =
                        self.test_single_late_rejection_continuation;
                    #[cfg(not(test))]
                    let single_late_rejection_continuation = false;
                    let late_rejection = persistent_late_continuation_rejection(
                        target_passes,
                        continuation_min_target_passes,
                        line_search_failed,
                        ordinary_stuck,
                        continuation_stuck_only,
                        &mut late_rejection_armed,
                        single_late_rejection_continuation,
                    );

                    if continuation_from_stuck.is_none()
                        && used_passes < ceiling
                        && late_rejection
                    {
                        #[cfg(test)]
                        {
                            continuation_trigger_pass = used_passes;
                            continuation_trigger_target_passes = target_passes;
                            continuation_trigger_was_stuck = ordinary_stuck;
                            continuation_trigger_moved = self.moved;
                            continuation_trigger_merit = self.search_merit;
                        }
                        self.continuation_attempts += 1;
                        continuation_from_stuck = Some(ordinary_stuck);

                        // `predicted` is no longer needed after the predictor;
                        // reuse it as an allocation-free backup of the current
                        // exact-target iterate in case the steering correction
                        // itself cannot produce an improving step.
                        self.predicted.copy_from_slice(&self.voltage);
                        let midpoint = self.last_input + 0.5 * (input - self.last_input);
                        #[cfg(test)]
                        {
                            continuation_midpoint = midpoint;
                        }
                        self.prepare_rhs(midpoint, false);
                        self.newton_passes += 1;
                        used_passes += 1;
                        let midpoint_pass = self.iterate(false, false);
                        let midpoint_usable = !matches!(midpoint_pass, Pass::Stuck);
                        #[cfg(test)]
                        {
                            continuation_midpoint_moved = self.moved;
                            continuation_midpoint_stuck = !midpoint_usable;
                        }
                        if midpoint_usable {
                            self.continuation_midpoint_successes += 1;
                        } else {
                            self.voltage.copy_from_slice(&self.predicted);
                        }
                        self.prepare_rhs(input, false);

                        // Mark the source transition.  The next loop
                        // iteration discards midpoint progress/merit history,
                        // then evaluates any line search against a freshly
                        // stamped exact-target merit.
                        source_changed = true;

                        if ordinary_stuck && !midpoint_usable {
                            break;
                        }
                        continue;
                    }

                    if ordinary_stuck {
                        break;
                    }
                }

                // Diagnostic-only branch-preserving retry.  If the proven v25
                // path has exhausted its bounded work, the physical companion
                // state is still the previous settled sample because failed
                // solves do not advance capacitors, inductors or devices.
                // `earlier` is that sample's voltage vector.  Restart the same
                // exact target from there with the full six-trial line search
                // and measure how much extra work the continuous branch needs.
                // This is deliberately test-only: it is a root/trajectory
                // diagnostic, not a production pass-budget increase.
                #[cfg(test)]
                if !settled
                    && self.test_last_settled_restart
                    && ceiling == MAX_ITERATIONS
                    && !self.last_was_unsettled
                {
                    last_settled_restart_attempted = true;
                    let restart_backtracks_before = self.backtrack_count;
                    let restart_fallbacks_before = self.fallbacks;
                    let normal_backtracks = self.backtracks;

                    self.voltage.copy_from_slice(&self.earlier);
                    self.prepare_rhs(input, false);
                    self.moved = f64::INFINITY;
                    self.search_merit = 0.0;
                    before = f64::INFINITY;
                    self.cycle_armed = false;
                    self.cycle_age = 0;
                    self.cycle_here = 0.0;
                    self.cycle_reference = 0.0;
                    self.backtracks = MAX_BACKTRACKS;

                    for restart_pass in 0..self.test_last_settled_restart_passes {
                        let stalled = self.moved > before * CONVERGING;
                        before = self.moved;
                        self.newton_passes += 1;
                        used_passes += 1;
                        target_passes += 1;
                        last_settled_restart_passes += 1;
                        let search = stalled || restart_pass >= FULL_STEPS;
                        match self.iterate(false, search) {
                            Pass::Settled => {
                                settled = true;
                                last_settled_restart_settled = true;
                                break;
                            }
                            Pass::Moved => {}
                            Pass::Stuck => break,
                        }
                    }

                    self.backtracks = normal_backtracks;
                    last_settled_restart_backtracks =
                        self.backtrack_count - restart_backtracks_before;
                    last_settled_restart_fallbacks = self.fallbacks - restart_fallbacks_before;
                    if settled && continuation_from_stuck.is_some() {
                        self.continuation_successes += 1;
                    }
                }

                // Test-only staged source-stepping rescue for the one
                // remaining Slaughter '95 / Twin failure. v4.8 proved that a
                // fully settled 50% midpoint is reachable, but the direct
                // 50% -> 100% jump still falls back into the measured spk/pl_a
                // two-cycle. Keep the complete 32-pass branch-preserving
                // restart, then walk the source along the already continuous
                // physical branch in 25% increments. Every stage gets its own
                // full 32-pass Newton budget and must settle before advancing.
                //
                // The experiment is transactional on failure. In particular,
                // do not borrow `predicted` as scratch: that vector is part of
                // the normal predictor state and v4.8 could therefore perturb
                // later samples even after restoring the failed-restart volts.
                #[cfg(test)]
                if !settled
                    && self.test_post_restart_continuation
                    && last_settled_restart_attempted
                    && !last_settled_restart_settled
                    && last_settled_restart_passes == self.test_last_settled_restart_passes
                    && self.test_last_settled_restart_passes
                        == TEST_LAST_SETTLED_RESTART_PASSES_DEFAULT
                    && continuation_from_stuck.is_none()
                    && post_restart_two_cycle_signature(
                        tail_trace_count,
                        &tail_trace_here,
                        &tail_trace_accepted_lambda,
                        &tail_trace_fallback,
                        &tail_trace_max_unknown,
                    )
                {
                    continuation_trigger_pass = used_passes;
                    continuation_trigger_target_passes = target_passes;
                    continuation_trigger_was_stuck = false;
                    continuation_trigger_moved = self.moved;
                    continuation_trigger_merit = self.search_merit;
                    self.continuation_attempts += 1;
                    continuation_from_stuck = Some(false);

                    self.post_restart_saved_voltage.copy_from_slice(&self.voltage);
                    for (device, saved) in self
                        .devices
                        .iter()
                        .zip(self.post_restart_saved_linearisation.iter_mut())
                    {
                        *saved = device.linearisation();
                    }
                    let failed_restart_moved = self.moved;
                    let failed_restart_search_merit = self.search_merit;
                    let failed_restart_before = before;
                    let failed_restart_exact = self.exact;
                    let failed_restart_cycle_armed = self.cycle_armed;
                    let failed_restart_cycle_age = self.cycle_age;
                    let failed_restart_cycle_here = self.cycle_here;
                    let failed_restart_cycle_reference = self.cycle_reference;
                    let normal_backtracks = self.backtracks;
                    self.backtracks = MAX_BACKTRACKS;

                    self.voltage.copy_from_slice(&self.earlier);
                    self.cycle_armed = false;
                    self.cycle_age = 0;
                    self.cycle_here = 0.0;
                    self.cycle_reference = 0.0;
                    self.moved = f64::INFINITY;
                    self.search_merit = 0.0;
                    before = f64::INFINITY;

                    let source_delta = input - self.last_input;
                    let stage_fractions = [0.25f64, 0.5, 0.75, 1.0];
                    let mut staged_path_settled = true;

                    for fraction in stage_fractions {
                        let stage_input = self.last_input + fraction * source_delta;
                        let exact_stage = fraction == 1.0;
                        self.prepare_rhs(stage_input, false);
                        self.moved = f64::INFINITY;
                        self.search_merit = 0.0;
                        before = f64::INFINITY;
                        self.cycle_armed = false;
                        self.cycle_age = 0;
                        self.cycle_here = 0.0;
                        self.cycle_reference = 0.0;

                        let mut stage_settled = false;
                        for stage_pass in 0..POST_RESTART_STAGE_PASSES {
                            let stalled = self.moved > before * CONVERGING;
                            before = self.moved;
                            self.newton_passes += 1;
                            used_passes += 1;
                            if exact_stage {
                                target_passes += 1;
                            }
                            let search = stalled || stage_pass >= FULL_STEPS;
                            match self.iterate(false, search) {
                                Pass::Settled => {
                                    stage_settled = true;
                                    break;
                                }
                                Pass::Moved => {}
                                Pass::Stuck => break,
                            }
                        }

                        // Keep the existing trace fields useful without
                        // changing the telemetry schema: they describe the
                        // most recently attempted intermediate source stage.
                        if !exact_stage {
                            continuation_midpoint = stage_input;
                            continuation_midpoint_moved = self.moved;
                            continuation_midpoint_stuck = !stage_settled;
                            if fraction == 0.5 && stage_settled {
                                self.continuation_midpoint_successes += 1;
                            }
                        }

                        if !stage_settled {
                            staged_path_settled = false;
                            break;
                        }
                    }

                    if staged_path_settled {
                        settled = true;
                        self.continuation_successes += 1;
                        self.continuation_actual_rescues += 1;
                    }

                    self.backtracks = normal_backtracks;
                    if !settled {
                        self.voltage
                            .copy_from_slice(&self.post_restart_saved_voltage);
                        for (device, saved) in self
                            .devices
                            .iter_mut()
                            .zip(self.post_restart_saved_linearisation.iter())
                        {
                            device.relinearise(*saved);
                        }
                        self.prepare_rhs(input, false);
                        self.moved = failed_restart_moved;
                        self.search_merit = failed_restart_search_merit;
                        before = failed_restart_before;
                        self.exact = failed_restart_exact;
                        self.cycle_armed = failed_restart_cycle_armed;
                        self.cycle_age = failed_restart_cycle_age;
                        self.cycle_here = failed_restart_cycle_here;
                        self.cycle_reference = failed_restart_cycle_reference;
                    }
                }

                // Production branch-preserving recovery for the late-continuation
                // circuit policy (currently the Twin power stage). The normal 64-pass
                // solve remains unchanged. Only a fully exhausted solve is retried
                // from the previous settled sample, whose dynamic/device history is
                // still the committed physical state. If that exact-target restart
                // also fails, walk the input source along the same branch in settled
                // 25% increments. A failed staged attempt is transactional: recovery
                // must restore the exact pre-recovery iterate and device
                // linearisation, not a later failed restart state.
                // Use the same branch-preserving production recovery in normal
                // tests and release builds. Explicit solver-diagnostic tests keep
                // their env-controlled restart/staging path above so their
                // measurements remain reproducible and do not double-recover.
                if !settled
                    && production_recovery_enabled
                    && self.late_continuation
                    && ceiling == MAX_ITERATIONS
                    && !self.last_was_unsettled
                {
                    // Snapshot the original exhausted target solve before any
                    // restart/staged rescue mutates Newton or device-limiter state.
                    // If recovery itself fails, this is the state the ordinary
                    // failed-sample containment path must see. Saving after a
                    // failed restart is too late: that restart may already have
                    // gone non-finite.
                    self.post_restart_saved_voltage.copy_from_slice(&self.voltage);
                    for (device, saved) in self
                        .devices
                        .iter()
                        .zip(self.post_restart_saved_linearisation.iter_mut())
                    {
                        *saved = device.linearisation();
                    }
                    let pre_recovery_moved = self.moved;
                    let pre_recovery_search_merit = self.search_merit;
                    #[cfg(test)]
                    let pre_recovery_before = before;
                    let pre_recovery_exact = self.exact;
                    let pre_recovery_cycle_armed = self.cycle_armed;
                    let pre_recovery_cycle_age = self.cycle_age;
                    let pre_recovery_cycle_here = self.cycle_here;
                    let pre_recovery_cycle_reference = self.cycle_reference;

                    let normal_backtracks = self.backtracks;
                    self.voltage.copy_from_slice(&self.earlier);
                    self.prepare_rhs(input, false);
                    self.moved = f64::INFINITY;
                    self.search_merit = 0.0;
                    before = f64::INFINITY;
                    self.cycle_armed = false;
                    self.cycle_age = 0;
                    self.cycle_here = 0.0;
                    self.cycle_reference = 0.0;
                    self.backtracks = MAX_BACKTRACKS;

                    for restart_pass in 0..LAST_SETTLED_RESTART_PASSES {
                        let stalled = self.moved > before * CONVERGING;
                        before = self.moved;
                        self.newton_passes += 1;
                        let search = stalled || restart_pass >= FULL_STEPS;
                        match self.iterate(false, search) {
                            Pass::Settled => {
                                settled = true;
                                break;
                            }
                            Pass::Moved => {}
                            Pass::Stuck => break,
                        }
                    }

                    self.backtracks = normal_backtracks;

                    if !settled {
                        self.continuation_attempts += 1;
                        self.backtracks = MAX_BACKTRACKS;
                        self.voltage.copy_from_slice(&self.earlier);
                        self.cycle_armed = false;
                        self.cycle_age = 0;
                        self.cycle_here = 0.0;
                        self.cycle_reference = 0.0;

                        let source_delta = input - self.last_input;
                        let mut staged_path_settled = true;
                        for fraction in [0.25f64, 0.5, 0.75, 1.0] {
                            let stage_input = self.last_input + fraction * source_delta;
                            self.prepare_rhs(stage_input, false);
                            self.moved = f64::INFINITY;
                            self.search_merit = 0.0;
                            before = f64::INFINITY;
                            self.cycle_armed = false;
                            self.cycle_age = 0;
                            self.cycle_here = 0.0;
                            self.cycle_reference = 0.0;

                            let mut stage_settled = false;
                            for stage_pass in 0..POST_RESTART_STAGE_PASSES {
                                let stalled = self.moved > before * CONVERGING;
                                before = self.moved;
                                self.newton_passes += 1;
                                let search = stalled || stage_pass >= FULL_STEPS;
                                match self.iterate(false, search) {
                                    Pass::Settled => {
                                        stage_settled = true;
                                        break;
                                    }
                                    Pass::Moved => {}
                                    Pass::Stuck => break,
                                }
                            }

                            if fraction == 0.5 && stage_settled {
                                self.continuation_midpoint_successes += 1;
                            }
                            if !stage_settled {
                                staged_path_settled = false;
                                break;
                            }
                        }

                        if staged_path_settled {
                            settled = true;
                            self.continuation_successes += 1;
                            self.continuation_actual_rescues += 1;
                        } else {
                            self.voltage
                                .copy_from_slice(&self.post_restart_saved_voltage);
                            for (device, saved) in self
                                .devices
                                .iter_mut()
                                .zip(self.post_restart_saved_linearisation.iter())
                            {
                                device.relinearise(*saved);
                            }
                            self.prepare_rhs(input, false);
                            self.moved = pre_recovery_moved;
                            self.search_merit = pre_recovery_search_merit;
                            #[cfg(test)]
                            {
                                before = pre_recovery_before;
                            }
                            self.exact = pre_recovery_exact;
                            self.cycle_armed = pre_recovery_cycle_armed;
                            self.cycle_age = pre_recovery_cycle_age;
                            self.cycle_here = pre_recovery_cycle_here;
                            self.cycle_reference = pre_recovery_cycle_reference;
                        }
                        self.backtracks = normal_backtracks;
                    }
                }

                #[cfg(test)]
                if !settled || continuation_deep_passes > 0 || last_settled_restart_attempted {
                    let trace = SolverTrace {
                        solve: self.solves,
                        input,
                        last_input: self.last_input,
                        ceiling,
                        used_passes,
                        target_passes,
                        moved: self.moved,
                        before,
                        search_merit: self.search_merit,
                        backtracks: self.backtrack_count - sample_backtracks_start,
                        fallbacks: self.fallbacks - sample_fallbacks_start,
                        continuation: continuation_from_stuck.is_some(),
                        tail_deep_passes: continuation_deep_passes,
                        tail_deep_improvements: continuation_deep_rescues,
                        tail_deep_weak_improvements: continuation_weak_deep_improvements,
                        deep_first_improvement_pass,
                        deep_last_improvement_pass,
                        deep_first_lambda,
                        deep_last_lambda,
                        deep_first_moved_before,
                        deep_first_moved_after,
                        deep_first_merit_before,
                        deep_first_merit_after,
                        deep_last_moved_before,
                        deep_last_moved_after,
                        deep_last_merit_before,
                        deep_last_merit_after,
                        post_deep_confirmation_passes,
                        post_low_residual_confirmation_passes,
                        repeat_cycle_rejections,
                        continuation_trigger_pass,
                        continuation_trigger_target_passes,
                        continuation_trigger_was_stuck,
                        continuation_trigger_moved,
                        continuation_trigger_merit,
                        continuation_midpoint,
                        continuation_midpoint_moved,
                        continuation_midpoint_stuck,
                        last_settled_restart_attempted,
                        last_settled_restart_passes,
                        last_settled_restart_backtracks,
                        last_settled_restart_fallbacks,
                        last_settled_restart_settled,
                        tail_trace_count,
                        tail_trace_passes,
                        tail_trace_here,
                        tail_trace_reference,
                        tail_trace_accepted_lambda,
                        tail_trace_accepted_merit,
                        tail_trace_trial_count,
                        tail_trace_trial_lambdas,
                        tail_trace_trial_merits,
                        tail_trace_moved_before,
                        tail_trace_moved_after,
                        tail_trace_fallback,
                        tail_trace_max_unknown,
                        tail_trace_max_norm,
                        tail_trace_unsettled_devices,
                        probe_pass,
                        probe_here,
                        probe_reference,
                        probe_trial_count,
                        probe_lambdas,
                        probe_merits,
                        probe_exact,
                        probe_max_correction_unknown,
                        probe_max_correction_norm,
                        probe_max_correction_abs,
                        probe_max_correction_voltage,
                        probe_max_correction_guess,
                        probe_unsettled_devices,
                        settled,
                    };
                    if !settled {
                        self.push_unsettled_solver_trace(trace);
                    }
                    self.push_solver_trace(trace);
                }
            }
            if !settled {
                self.unsettled += 1;
                if ceiling < MAX_ITERATIONS {
                    self.pinched += 1;
                }
                // Bound the failed iterate and use it, rather than throwing
                // it away.
                //
                // The failed iterate is the Newton loop's last answer, and
                // it is a linear response to the *current* input. It
                // therefore follows the signal; on a pick attack that is
                // exactly what is wanted, because the pick is a genuine
                // jump and the failed iterate's scale reflects it.
                //
                // What the iterate cannot be trusted for is *magnitude*: a
                // Newton solve that ran out of passes has no convergence
                // guarantee, and its answer can be a large overshoot on any
                // node. Keeping it unbounded is what produced the +2.3 dB
                // spike on the Twin before the reversion was added.
                //
                // So: keep the iterate, and cap how far any node may move
                // from the last settled value. The cap has to be
                // *signal-scaled*, because the nodes in these circuits do
                // not share a scale. A power tube's plate sits at four
                // hundred and fifty volts and swings by hundreds; a
                // speaker node sits at a few volts and swings by a few. One
                // absolute cap cannot admit the pick on the plate and
                // reject the click on the speaker at the same time.
                //
                // `recent_move[k]` is how much node `k` moved between the
                // two samples before this one, taken above before the
                // predictor overwrote anything. `STEP_MULTIPLE` times that
                // admits a continuation of the signal with room to overshoot
                // it somewhat, and rejects a Newton overshoot that is many
                // times the signal's own scale. `STEP_CEILING` is a
                // last-resort absolute cap so a numerical accident cannot
                // escape by way of a large recent move.
                //
                // STEP_MULTIPLE was 4 until a pick attack was reported as
                // "distorted and stuttering". A pick is a genuine jump --
                // the grid has to travel from cutoff to hard clipping in a
                // few samples -- and 4 times the previous sample's movement
                // is not enough of a jump. 16 admits the transient while
                // still bounding the answer: a real Newton overshoot is
                // many orders larger than sixteen times the signal, and the
                // absolute cap catches those.
                //
                // A non-predictable circuit -- one with an op-amp in it --
                // does not extrapolate, so `recent_move` is stale on it and
                // it takes `STEP_CEILING` alone. The 5150 and the Twin do
                // not have op-amps in the signal path, so this branch does
                // not fire on them; it is here for the pedal voices.
                const STEP_MULTIPLE: f64 = 16.0;
                const STEP_CEILING: f64 = 1000.0;
                for k in 0..n {
                    let base = self.earlier[k];
                    if !base.is_finite() {
                        // `earlier` is supposed to be the previous finite
                        // sample. Do not propagate a broken invariant into
                        // another audio sample. Zero is only a last-ditch
                        // numerical quarantine; normal recovery never reaches
                        // this branch.
                        self.voltage[k] = 0.0;
                        continue;
                    }
                    let iter = self.voltage[k] - base;
                    let bound = if self.predictable && !self.last_was_unsettled {
                        (self.recent_move[k] * STEP_MULTIPLE).min(STEP_CEILING)
                    } else {
                        STEP_CEILING
                    };
                    if !iter.is_finite() {
                        // A comparison against NaN is false, so the old clamp
                        // silently let NaN escape into `voltage` and poisoned
                        // every following sample. A failed solve has no trusted
                        // non-finite answer: fall back to the last finite sample.
                        self.voltage[k] = base;
                    } else if iter.abs() > bound {
                        self.voltage[k] = base + iter.signum() * bound;
                    }
                }
                // `iterate()` relinearises devices at every Newton/trial
                // point. If the solve ends on an overflowed or otherwise
                // rejected trajectory, those limiter coordinates can be just
                // as poisoned as the node voltages were. Merely clamping the
                // voltages is therefore not enough: the next sample's first
                // limited stamp can start from an infinite grid/core value
                // and fail in one pass, poisoning every sample after it.
                //
                // Physical device state is already transactional --
                // `advance()` is skipped below. Restore the matching purely
                // numerical linearisation state as well. This costs one tiny
                // device loop only on failed samples; the realtime success
                // path remains unchanged apart from remembering its snapshot.
                if self.last_settled_linearisation_valid {
                    for (device, saved) in self
                        .devices
                        .iter_mut()
                        .zip(self.last_settled_linearisation.iter())
                    {
                        device.relinearise(*saved);
                    }
                }
                failed = true;
                self.last_was_unsettled = true;
            } else {
                self.last_was_unsettled = false;
            }
        }

        // Advance what each reactance remembers -- but only when this sample
        // actually converged.
        //
        // The advance is what changes the state: a capacitor's stored charge,
        // an inductor's current, the transformer core's flux. It has to
        // integrate the sample's *answer*, and a failed sample's answer is
        // not the answer -- it is a bounded guess. Advancing from a bounded
        // guess lets the state move by up to `STEP_MULTIPLE` times the
        // signal's own per-sample movement, and doing that for a run of
        // consecutive failed samples walks the state far enough from any
        // plausible operating point that the circuit stops returning to it.
        // On the Twin Reverb, whose power stage is the largest solver in the
        // catalogue and the one whose transformer core integrates the most,
        // that is what produced a loud pulsating that continued after the
        // input stopped: the core had walked into a limit cycle of its own.
        //
        // So on a failed sample the state is left exactly where the last
        // converged sample left it, and only the *output* -- which reads
        // `voltage`, not the state -- follows the bounded failed iterate.
        // That is what lets the sound follow the pick even while the solver
        // is struggling: the sample goes out, the state does not commit, and
        // the next sample's solve starts from the same operating point the
        // last one trusted.
        //
        // The convention throughout is that a branch's current leaving node
        // `a` is `G * v - history`, so `history` is exactly what gets injected
        // into the right hand side. Both companions are written to that shape
        // so one `inject` serves them, and getting the shape wrong is easy: a
        // capacitor whose recurrence carries the wrong term reads 24 dB down
        // on a network that should be flat, and an inductor's diverges to NaN
        // within a few samples.
        if !failed {
            for c in &mut self.capacitors {
                let v = across(&self.voltage, c.a, c.b);
                // Trapezoidal: i = 2C/T (v - v_prev) - i_prev, which rearranges to
                // i = G v - (G v_prev + i_prev).
                let current = c.conductance * v - c.history;
                c.history = c.conductance * v + current;
                c.voltage = v;
            }
            for l in &mut self.inductors {
                let v = across(&self.voltage, l.a, l.b);
                // i = i_prev + T/2L (v + v_prev) = G v + (i_prev + G v_prev), so
                // the injected term is the negative of that bracket.
                let current = l.conductance * v - l.history;
                l.history = -(current + l.conductance * v);
                l.current = current;
            }
            // And what each device remembers. Most of them remember nothing --
            // a diode's current depends on its voltage now and on nothing
            // else -- so this went uncalled for a long time without any of
            // them noticing. A saturating core is the first device here with a
            // state, and it integrates the voltage across it: left unadvanced
            // its flux never accumulated past a single sample, so it drew a
            // hundred-thousandth of the current it should have and iron in the
            // signal path did nothing whatsoever.
            for device in &mut self.devices {
                device.advance();
            }
            if !self.devices.is_empty() {
                for (saved, device) in self
                    .last_settled_linearisation
                    .iter_mut()
                    .zip(self.devices.iter())
                {
                    *saved = device.linearisation();
                }
                self.last_settled_linearisation_valid = true;
            }
        }

        // Keep the continuation source synchronized with the dynamic state
        // committed above. An unsettled sample advances neither.
        if self.late_continuation && !failed {
            self.earlier_input = self.last_input;
            self.last_input = input;
        }
        self.voltage[self.circuit.output]
    }

    /// Clears every dynamic state and marks the circuit as resting, but does
    /// not hunt the operating point yet.
    ///
    /// This is the realtime-safe reset primitive for a caller that is about to
    /// apply more controls. `Chain::apply` can switch a voice and then move its
    /// Drive, Master, EQ and other controls in the same host block. Settling
    /// inside the reset would solve an old/intermediate setting and force a
    /// second DC hunt after the final controls were installed.
    ///
    /// The simulation is therefore left `dirty && at_rest`. The caller can
    /// apply every pending control, then call `find_operating_point()` once
    /// (and share that DC solution with identical stereo channels). `process()`
    /// retains the safe fallback: if a caller does not prepare it explicitly,
    /// the first sample rebuilds and settles before audio is advanced.
    ///
    /// Marking the circuit dirty also resets stateful nonlinear devices
    /// correctly. A transformer core owns flux history that `Device::advance()`
    /// commits; advancing it is not a reset. The next rebuild occurs with
    /// `at_rest == true`, so `rebuild()` reconstructs the device list from its
    /// pristine constructors without allocating, using the capacity reserved
    /// in `new`.
    pub fn reset_deferred(&mut self) {
        self.at_rest = true;

        for c in &mut self.capacitors {
            c.history = 0.0;
            c.voltage = 0.0;
        }
        for l in &mut self.inductors {
            l.history = 0.0;
            l.current = 0.0;
        }

        self.voltage.fill(0.0);
        self.predicted.fill(0.0);
        self.earlier.fill(0.0);
        self.recent_move.fill(0.0);
        self.point.fill(0.0);
        self.guess.fill(0.0);
        self.last_was_unsettled = false;
        self.last_input = 0.0;
        self.earlier_input = 0.0;
        self.aux_values.fill(0.0);
        self.last_settled_linearisation_valid = false;
        self.exact = true;
        self.moved = f64::INFINITY;

        // Force one rebuild after the caller has finished applying the new
        // setting. With `at_rest` true that rebuild also gives stateful devices
        // a genuine fresh state. Do not rebuild or DC-hunt here.
        self.dirty = true;
    }

    /// Reset and establish the operating point immediately.
    ///
    /// This preserves the original public `reset()` contract for preparation,
    /// tests and any caller that explicitly wants a fully settled simulation
    /// before returning. Realtime voice/model switching uses `reset_deferred`
    /// so it can apply the complete new setting before paying for this hunt.
    pub fn reset(&mut self) {
        self.reset_deferred();
        self.find_operating_point();
    }
}

/// An ideal transformer's rows: the primary is `ratio` times the secondary in
/// volts, and one over that in amps.
#[allow(clippy::too_many_arguments)]
fn stamp_branch(
    m: &mut [f64],
    n: usize,
    branch: usize,
    p1: usize,
    p2: usize,
    s1: usize,
    s2: usize,
    ratio: f64,
) {
    for (node, sign) in [(p1, 1.0), (p2, -1.0), (s1, -ratio), (s2, ratio)] {
        if node != GROUND {
            m[node * n + branch] += sign;
            m[branch * n + node] += sign;
        }
    }
}

/// Adds a conductance between two nodes.
fn stamp_matrix(m: &mut [f64], n: usize, a: usize, b: usize, g: f64) {
    if a != GROUND {
        m[a * n + a] += g;
    }
    if b != GROUND {
        m[b * n + b] += g;
    }
    if a != GROUND && b != GROUND {
        m[a * n + b] -= g;
        m[b * n + a] -= g;
    }
}

/// Into both the running matrix and the one the operating point is solved on.
/// Only the reactances differ between them.
fn stamp_both(base: &mut [f64], dc: &mut [f64], n: usize, a: usize, b: usize, g: f64) {
    stamp_matrix(base, n, a, b, g);
    stamp_matrix(dc, n, a, b, g);
}

fn across(v: &[f64], a: usize, b: usize) -> f64 {
    let va = if a == GROUND { 0.0 } else { v[a] };
    let vb = if b == GROUND { 0.0 } else { v[b] };
    va - vb
}

fn inject(rhs: &mut [f64], a: usize, b: usize, current: f64) {
    if a != GROUND {
        rhs[a] += current;
    }
    if b != GROUND {
        rhs[b] -= current;
    }
}

/// LU with partial pivoting, in place.
///
/// Pivoting is not optional here. The diagonal carries zeros wherever a node's
/// own admittance cancels, and an unpivoted elimination divides by whatever
/// happens to be sitting there.
#[allow(clippy::too_many_arguments)]
/// The largest entry at or below the diagonal in `col`, recorded in `plan`.
fn find_pivot(m: &[f64], plan: &mut [usize], col: usize, bottom: usize, n: usize) -> usize {
    let mut best = col;
    let mut magnitude = m[col * n + col].abs();
    for row in (col + 1)..=bottom {
        let candidate = m[row * n + col].abs();
        if candidate > magnitude {
            best = row;
            magnitude = candidate;
        }
    }
    plan[col] = best;
    best
}

/// Exchange two rows, and the per-row bookkeeping that travels with them.
fn swap_rows(
    m: &mut [f64],
    pivots: &mut [usize],
    reach: &mut [usize],
    first: &mut [usize],
    col: usize,
    best: usize,
    n: usize,
) {
    let width = reach[col].max(reach[best]).saturating_add(1).min(n);
    for k in 0..width {
        m.swap(col * n + k, best * n + k);
    }
    pivots.swap(col, best);
    reach.swap(col, best);
    first.swap(col, best);
}

/// Whether exact nonlinear condensation is likely to beat the already sparse
/// full LU. Tiny circuits are deliberately left alone: forming/recovering a
/// Schur system has fixed O(n²) work and is meant for the large valve models,
/// not a two-diode pedal. This is only a performance gate; either path solves
/// the same equations.
fn nonlinear_reduction_worthwhile(n: usize, boundary: usize) -> bool {
    if n < 6 || boundary == 0 || boundary >= n {
        return false;
    }
    let internal = n - boundary;
    if internal < 2 {
        return false;
    }

    let n = n as f64;
    let b = boundary as f64;
    let i = internal as f64;
    // The reduced path now stamps directly in boundary space, replays a pivot
    // plan, combines only active RHS response columns, and recovers internals
    // with O(i*b) work. The old i^2-per-pass estimate is obsolete. Keep a
    // conservative allowance for full-vector recovery and device evaluation,
    // but let small nonlinear return stages participate when their boundary is
    // genuinely much smaller than full MNA.
    let full = n * n * n;
    let reduced = b * b * b + 2.0 * i * b + b * b;
    reduced < 0.85 * full
}

#[allow(clippy::too_many_arguments)]
fn factorise(
    m: &mut [f64],
    pivots: &mut [usize],
    reach: &mut [usize],
    depth: &mut [usize],
    first: &mut [usize],
    reach_template: &[usize],
    depth_template: &[usize],
    plan: &mut [usize],
    planned: bool,
    n: usize,
) -> bool {
    // How far right each row goes, and how far down each column does.
    //
    // A circuit matrix is nearly all zeros -- a part only ever stamps the nodes
    // it is connected to, so a row typically holds three or four entries out of
    // thirty -- and the elimination's inner loop was running the full width of
    // the matrix regardless, multiplying by zero for most of it. So each row
    // carries the column index of its last nonzero and the inner loop stops
    // there; each column carries the row index of its last, and the pivot
    // search stops there.
    //
    // Both come from the netlist, not from the values. They used to be
    // rediscovered on every call by scanning: the row scan walked in from the
    // right, which on a banded matrix means reading the whole tail of zeros to
    // find an entry sitting near the diagonal, and there was no column bound at
    // all, so the pivot search read every row below the diagonal. Measured on
    // the Boogie preamplifier that was 325 strided column reads and about 230
    // row reads a factorisation, against 59 actual eliminations -- the search
    // for the shape cost four times the arithmetic done on it.
    //
    // The shape is fixed when the parts are wired together. `map_structure`
    // works it out once. See `reach_template`.
    reach.copy_from_slice(reach_template);
    depth.copy_from_slice(depth_template);

    for (k, p) in pivots.iter_mut().enumerate() {
        *p = k;
    }
    // Where each row's L part starts. A row with nothing below the diagonal
    // keeps its own index, which makes the forward substitution's loop empty.
    for (k, f) in first.iter_mut().enumerate() {
        *f = k;
    }
    // Whether the order being replayed is still the order a search would have
    // chosen, and whether this call has had to give up on it partway.
    //
    // Giving up partway is exact, and that is the point. The elimination is
    // sequential: at column `col` every column to its left is finished, and
    // each of those was confirmed to have used the largest entry available.
    // So the matrix here is precisely what a searching factorisation would
    // have produced, and switching to a search from this column on completes a
    // proper partial-pivoted LU. No copy of the matrix is needed and nothing
    // has to be done twice -- which matters, because keeping a copy to retry
    // from cost more than the search it was there to avoid: the 5150's power
    // stage went from 64.9 % of realtime to 76.5 %.
    let mut sound = true;
    let mut searching = !planned;
    for col in 0..n {
        // Nothing below this column's last possible entry can be the pivot.
        let bottom = depth[col];
        let mut best = if searching {
            find_pivot(m, plan, col, bottom, n)
        } else {
            // Replayed. Whether it was the right choice is checked twice
            // below: for a pivot that has gone to nothing, before anything is
            // eliminated with it, and for a pivot that is merely no longer the
            // largest, out of loads the elimination performs anyway.
            plan[col]
        };
        if best != col {
            swap_rows(m, pivots, reach, first, col, best, n);
        }
        // The swap may have carried the old pivot row's entries down to row
        // `best`, so every column that row reaches into can now hold a nonzero
        // that far down. `best <= depth[col]`, so bounding the widening by
        // this column's own depth below covers it.

        let mut pivot = m[col * n + col];
        if !searching && pivot.abs() < 1e-30 {
            // The plan named a row that no longer holds anything here.
            //
            // This is the check that was missing, and leaving it out did not
            // fail quietly by a decibel: the `continue` below steps over the
            // whole elimination of this column, so the factorisation is of a
            // different matrix from the one that was stamped, and the
            // Distortion voice fell to 162 dB below the rest of the catalogue
            // -- silence. `tests/presets.rs::the_presets_are_level_matched`
            // is what caught it.
            //
            // The other guard cannot catch this, because it lives inside the
            // elimination loop this skips.
            searching = true;
            sound = false;
            best = find_pivot(m, plan, col, bottom, n);
            if best != col {
                swap_rows(m, pivots, reach, first, col, best, n);
            }
            pivot = m[col * n + col];
        }
        if pivot.abs() < 1e-30 {
            // Genuinely nothing to eliminate with: the matrix is singular in
            // this column.
            continue;
        }
        // Nothing to the right of the pivot row's reach can be changed by it,
        // and nothing below this column's own last entry has anything in this
        // column to eliminate. The second half of that was missing: the loop
        // ran to `n` and read `m[row * n + col]` for every row below the
        // diagonal just to find a zero and skip it -- and that read is
        // strided across rows, so on a twenty-eight unknown circuit it was
        // about four hundred loads a factorisation, each on its own cache
        // line. Bounding it costs nothing and skips only entries that were
        // zero, so the arithmetic is unchanged.
        let stop = reach[col];
        let ceiling = pivot.abs();
        for row in (col + 1)..=bottom {
            // Tested before the division, not after it.
            //
            // Most rows have nothing below the pivot at all, and eliminating
            // with a zero multiplier subtracts nothing from every remaining
            // column -- but the old form divided first and asked afterwards,
            // so a thirty unknown circuit paid about nine hundred divisions a
            // factorisation to compute `0.0 / pivot`, store it back over the
            // zero it came from, and then skip. A division is twenty odd
            // cycles where the loads either side are one, so those were a
            // sizeable share of the factorisation on their own.
            let entry = m[row * n + col];
            if entry == 0.0 {
                continue;
            }
            // Partial pivoting's own test, applied to a load already made: was
            // the pivot the largest in the column? If a replayed order says no,
            // the factorisation stands -- it is still an exact LU of this
            // matrix, only a less well conditioned one -- and the caller
            // searches again next time.
            if !searching && entry.abs() > ceiling * GROWTH {
                // Not the largest in the column any more. Unlike the check
                // above this is not a correctness problem -- the result is
                // still an exact LU of this matrix, only a less well
                // conditioned one -- so the pass stands and the next
                // factorisation searches and learns the order again.
                sound = false;
            }
            let factor = entry / pivot;
            m[row * n + col] = factor;
            // Where this row's L part starts, for the forward substitution.
            if col < first[row] {
                first[row] = col;
            }
            for k in (col + 1)..=stop {
                m[row * n + k] -= factor * m[col * n + k];
            }
            // The fill-in this just created reaches as far as the pivot row did.
            if stop > reach[row] {
                reach[row] = stop;
            }
        }
        // And it goes as far down as this column's own entries did, which is
        // what keeps the pivot search's bound honest for the columns to come.
        for value in depth.iter_mut().take(stop + 1).skip(col + 1) {
            if *value < bottom {
                *value = bottom;
            }
        }
    }
    sound
}

/// Solves with the factorisation `factorise` left behind.
///
/// Both triangles are walked between the bounds that function recorded rather
/// than across the full width: `first[row]` is where the row's L part starts
/// and `reach[row]` is where its U part ends, fill-in included. Everything
/// outside them is an exact zero, so skipping it subtracts nothing and the
/// arithmetic is what it was. It is worth doing because a substitution is
/// otherwise two full triangles -- about nine hundred multiply-adds on a
/// thirty unknown circuit, of which a circuit matrix means perhaps a tenth
/// are not multiplying by zero.
fn substitute(
    m: &[f64],
    pivots: &[usize],
    reach: &[usize],
    first: &[usize],
    rhs: &mut [f64],
    n: usize,
    permuted: &mut [f64],
) {
    for (row, &from) in pivots.iter().enumerate() {
        permuted[row] = rhs[from];
    }
    for row in 1..n {
        let mut sum = permuted[row];
        for k in first[row]..row {
            sum -= m[row * n + k] * permuted[k];
        }
        permuted[row] = sum;
    }
    for row in (0..n).rev() {
        let mut sum = permuted[row];
        for k in (row + 1)..=reach[row] {
            sum -= m[row * n + k] * permuted[k];
        }
        let pivot = m[row * n + row];
        permuted[row] = if pivot.abs() < 1e-30 {
            0.0
        } else {
            sum / pivot
        };
    }
    rhs.copy_from_slice(&permuted[..n]);
}
