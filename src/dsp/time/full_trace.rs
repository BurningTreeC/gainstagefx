//! Exact-sample diagnostics. This module is compiled only for library tests.
//! Storage is reserved by the harness after warm-up; formatting happens after
//! processing. Extra residual probes restore device caches and never feed back
//! into a solver decision.

use super::{Device, Pass, ResidualStamper, Simulation, SolverControlProfile, RELATIVE, TOLERANCE};

const PASS_CAPACITY: usize = 512;
const TRIAL_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug)]
struct Trial {
    kind: &'static str,
    lambda: f64,
    merit: f64,
    exact: bool,
    accepted: bool,
}

impl Default for Trial {
    fn default() -> Self {
        Self {
            kind: "none",
            lambda: f64::NAN,
            merit: f64::NAN,
            exact: false,
            accepted: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct FullPass {
    phase: &'static str,
    phase_pass: usize,
    target_passes: usize,
    continuation_active: bool,
    source: f64,
    moved_before: f64,
    moved_after: f64,
    merit_before: f64,
    merit_after: f64,
    residual_exact_before: bool,
    residual_exact_after: bool,
    stamped_merit: f64,
    reference: f64,
    before_at: usize,
    before_available: bool,
    before_value: f64,
    newton_at: usize,
    newton_value: f64,
    newton_delta: f64,
    after_value: f64,
    step_norm: f64,
    reduced: bool,
    boundary: usize,
    searched: bool,
    search_entered: bool,
    limiter_handoff: bool,
    limiter_hold: bool,
    unsettled_devices: usize,
    unsettled_devices_after: usize,
    device_count: usize,
    cycle_rejected: bool,
    settled: bool,
    stuck: bool,
    best_lambda: f64,
    best_merit: f64,
    accepted_lambda: f64,
    accepted_merit: f64,
    fallback_lambda: f64,
    trials: [Trial; TRIAL_CAPACITY],
    trial_count: usize,
    work: SolverControlProfile,
    cumulative: SolverControlProfile,
}

pub(super) struct FullTrace {
    relative_sample: u64,
    selected_solve: u64,
    block_size: usize,
    active: bool,
    complete: bool,
    overflow: bool,
    records: Vec<FullPass>,
    len: usize,
    current: FullPass,
    before: SolverControlProfile,
    total: SolverControlProfile,
    pass_before: SolverControlProfile,
    input: f64,
    last_input: f64,
    earlier_input: f64,
    predictor_available: bool,
    predictor_suppressed: bool,
    predictor_reason: &'static str,
    predictor_scale: f64,
    jacobian_init: Option<super::jacobian_init::Attempt>,
    source_scaled: bool,
    curvature_gate: bool,
    raw_ratio: f64,
    prior: Vec<f64>,
    earlier: Vec<f64>,
    predicted: Vec<f64>,
    residual: Vec<f64>,
    dominant: usize,
    phase: &'static str,
    phase_pass: usize,
    target_passes: usize,
    continuation_active: bool,
    source: f64,
    final_moved: f64,
}

fn selected_solve(base: u64, relative: u64) -> Option<u64> {
    base.checked_add(relative)?.checked_add(1)
}

impl Simulation {
    /// `base` is the number of power solves already performed by warm-up.
    /// Relative zero is the next solve; this matches SolverControlSample output.
    pub fn configure_full_power_trace(&mut self, relative: u64, block_size: usize) {
        assert!(
            self.late_continuation,
            "full trace requires Twin power policy"
        );
        assert!(block_size > 0);
        self.test_full_trace = Some(Box::new(FullTrace {
            relative_sample: relative,
            selected_solve: selected_solve(self.solves, relative).expect("trace sample overflow"),
            block_size,
            active: false,
            complete: false,
            overflow: false,
            records: vec![FullPass::default(); PASS_CAPACITY],
            len: 0,
            current: FullPass::default(),
            before: SolverControlProfile::default(),
            total: SolverControlProfile::default(),
            pass_before: SolverControlProfile::default(),
            input: 0.0,
            last_input: 0.0,
            earlier_input: 0.0,
            predictor_available: false,
            predictor_suppressed: false,
            predictor_reason: "unavailable",
            predictor_scale: 0.0,
            jacobian_init: None,
            source_scaled: false,
            curvature_gate: false,
            raw_ratio: f64::NAN,
            prior: vec![0.0; self.n],
            earlier: vec![0.0; self.n],
            predicted: vec![0.0; self.n],
            residual: vec![0.0; self.n],
            dominant: 0,
            phase: "normal",
            phase_pass: 0,
            target_passes: 0,
            continuation_active: false,
            source: 0.0,
            final_moved: f64::NAN,
        }));
        self.test_profile_solver_control_tail = true;
        if let Some(partition) = &mut self.nonlinear_partition {
            partition.set_test_pivot_profile(true);
        }
    }

    pub(super) fn full_trace_active(&self) -> bool {
        self.test_full_trace
            .as_ref()
            .is_some_and(|trace| trace.active)
    }

    pub(super) fn full_trace_begin(&mut self, input: f64) {
        if !self
            .test_full_trace
            .as_ref()
            .is_some_and(|t| t.selected_solve == self.solves)
        {
            return;
        }
        let before = self.solver_control_profile();
        let trace = self.test_full_trace.as_mut().unwrap();
        trace.active = true;
        trace.before = before;
        trace.input = input;
        trace.last_input = self.last_input;
        trace.earlier_input = self.earlier_input;
    }

    pub(super) fn full_trace_predictor(&mut self, scale: f64, suppressed: bool) {
        if !self.full_trace_active() {
            return;
        }
        let source_scaled = self.source_scaled_predictor_enabled();
        let gate = self.input_curvature_predictor_gate_enabled();
        let trace = self.test_full_trace.as_mut().unwrap();
        trace.predictor_available = self.predictable && !self.last_was_unsettled;
        trace.predictor_scale = scale; // The actual solver's computed scale.
        trace.predictor_suppressed = suppressed;
        trace.source_scaled = source_scaled;
        trace.curvature_gate = gate;
        let step = trace.input - trace.last_input;
        let previous = trace.last_input - trace.earlier_input;
        trace.raw_ratio = step / previous; // Diagnostic geometry only.
        trace.predictor_reason = if !self.predictable {
            "switching_device"
        } else if self.last_was_unsettled {
            "previous_sample_unsettled"
        } else if scale != 0.0 {
            "used"
        } else if !step.is_finite() || !previous.is_finite() {
            "nonfinite_source"
        } else if previous.abs() <= 1e-12 {
            "edge_from_flat_source"
        } else if source_scaled && trace.raw_ratio.abs() > 2.0 {
            "source_ratio_exceeds_two"
        } else if !source_scaled && gate {
            "curvature_gate"
        } else {
            "zero_source_scale"
        };
        trace.prior.copy_from_slice(&self.voltage);
        trace.earlier.copy_from_slice(&self.earlier);
    }

    pub(super) fn full_trace_predicted(&mut self) {
        if let Some(t) = self.test_full_trace.as_mut().filter(|t| t.active) {
            t.predicted.copy_from_slice(&self.voltage);
            t.jacobian_init = self.test_jacobian_init.as_ref().map(|init| init.last);
        }
    }

    pub(super) fn full_trace_source(&mut self, input: f64) {
        if let Some(t) = self.test_full_trace.as_mut().filter(|t| t.active) {
            t.source = input;
        }
    }

    pub(super) fn full_trace_phase(
        &mut self,
        phase: &'static str,
        target: usize,
        local: usize,
        continuation: bool,
    ) {
        if let Some(t) = self.test_full_trace.as_mut().filter(|t| t.active) {
            t.phase = phase;
            t.target_passes = target;
            t.phase_pass = local;
            t.continuation_active = continuation;
        }
    }

    /// Measure the same reduced physical residual as a search trial, using
    /// private scratch. Do not consume/replace the solver's evaluation cache.
    fn full_trace_residual(&mut self) -> (f64, bool) {
        let Some(partition) = &self.nonlinear_partition else {
            return (f64::NAN, false);
        };
        let t = self.test_full_trace.as_mut().unwrap();
        let Some(map) = partition.begin_residual(&self.fixed_rhs, &self.voltage, &mut t.residual)
        else {
            return (f64::NAN, false);
        };
        let mut stamp = ResidualStamper {
            residual: &mut t.residual,
            map,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        for device in &mut self.devices {
            let saved = device.checkpoint();
            device.trial_residual(&mut stamp, &self.voltage, false);
            device.restore_checkpoint(saved);
        }
        let exact = !stamp.mapping_failed && !stamp.junction_held;
        let merit = t.residual[..partition.boundary_len()]
            .iter()
            .map(|v| v * v)
            .sum();
        (merit, exact)
    }

    pub(super) fn full_trace_pass_begin(&mut self) {
        let (merit, exact) = self.full_trace_residual();
        let profile = self.solver_control_profile();
        let t = self.test_full_trace.as_mut().unwrap();
        t.pass_before = profile;
        t.current = FullPass {
            phase: t.phase,
            phase_pass: t.phase_pass,
            target_passes: t.target_passes,
            continuation_active: t.continuation_active,
            source: t.source,
            moved_before: self.moved,
            merit_before: merit,
            residual_exact_before: exact,
            before_at: t.dominant,
            before_available: t.len > 0 && self.moved.is_finite(),
            before_value: self.voltage[t.dominant],
            stamped_merit: f64::NAN,
            reference: f64::NAN,
            step_norm: f64::NAN,
            newton_delta: f64::NAN,
            accepted_lambda: f64::NAN,
            accepted_merit: f64::NAN,
            fallback_lambda: f64::NAN,
            best_lambda: f64::NAN,
            best_merit: f64::INFINITY,
            ..FullPass::default()
        };
    }

    pub(super) fn full_trace_step(
        &mut self,
        reduced: bool,
        searched: bool,
        here: f64,
        reference: f64,
        handoff: bool,
    ) {
        if !self.full_trace_active() {
            return;
        }
        let mut at = 0;
        let mut norm = 0.0;
        for (i, (&delta, &voltage)) in self.scratch.iter().zip(&self.voltage).enumerate() {
            let candidate = delta.abs() / (TOLERANCE + RELATIVE * voltage.abs());
            if candidate > norm {
                at = i;
                norm = candidate;
            }
        }
        let held = self.devices.iter().any(|d| d.limiter_held());
        let record = &mut self.test_full_trace.as_mut().unwrap().current;
        record.newton_at = at;
        record.newton_value = self.voltage[at];
        record.newton_delta = self.scratch[at];
        record.step_norm = self.moved;
        record.reduced = reduced;
        record.boundary = self
            .nonlinear_partition
            .as_ref()
            .map_or(0, |p| p.boundary_len());
        record.searched = searched;
        record.stamped_merit = if searched { here } else { f64::NAN };
        record.reference = if searched { reference } else { f64::NAN };
        record.limiter_handoff = handoff;
        record.limiter_hold = held || !self.exact;
        record.device_count = self.devices.len();
        record.unsettled_devices = self
            .devices
            .iter()
            .filter(|d| !d.settled(TOLERANCE))
            .count();
    }

    pub(super) fn full_trace_trial(&mut self, kind: &'static str, lambda: f64) {
        if let Some(t) = self.test_full_trace.as_mut().filter(|t| t.active) {
            let record = &mut t.current;
            record.search_entered = true;
            if record.trial_count == TRIAL_CAPACITY {
                t.overflow = true;
                return;
            }
            record.trials[record.trial_count] = Trial {
                kind,
                lambda,
                ..Trial::default()
            };
            record.trial_count += 1;
        }
    }

    pub(super) fn full_trace_trial_result(&mut self, merit: f64, exact: bool, accepted: bool) {
        if let Some(t) = self
            .test_full_trace
            .as_mut()
            .filter(|t| t.active && t.current.trial_count > 0)
        {
            let r = &mut t.current;
            let trial = &mut r.trials[r.trial_count - 1];
            trial.merit = merit;
            trial.exact = exact;
            trial.accepted = accepted;
            if exact && merit.is_finite() && merit < r.best_merit {
                r.best_merit = merit;
                r.best_lambda = trial.lambda;
            }
            if accepted {
                r.accepted_lambda = trial.lambda;
                r.accepted_merit = merit;
            }
        }
    }

    pub(super) fn full_trace_pass_end(&mut self, result: Pass) {
        let (merit, exact) = self.full_trace_residual();
        let work = self.solver_control_profile();
        let t = self.test_full_trace.as_mut().unwrap();
        let r = &mut t.current;
        r.moved_after = self.moved;
        r.merit_after = merit;
        r.residual_exact_after = exact;
        r.after_value = self.voltage[r.newton_at];
        r.unsettled_devices_after = self
            .devices
            .iter()
            .filter(|d| !d.settled(TOLERANCE))
            .count();
        r.cycle_rejected = self.last_search_cycle_rejected;
        r.settled = matches!(result, Pass::Settled);
        r.stuck = matches!(result, Pass::Stuck);
        r.work = work.saturating_delta(t.pass_before);
        r.cumulative = work.saturating_delta(t.before);
        if !r.search_entered && !r.stuck {
            r.accepted_lambda = 1.0;
            r.accepted_merit = merit;
        }
        t.dominant = r.newton_at;
        if t.len == PASS_CAPACITY {
            t.overflow = true;
            return;
        }
        t.records[t.len] = *r;
        t.len += 1;
    }

    pub(super) fn full_trace_fallback(&mut self, lambda: f64, merit: f64) {
        if let Some(t) = self.test_full_trace.as_mut().filter(|t| t.active) {
            t.current.fallback_lambda = lambda;
            t.current.best_lambda = lambda;
            t.current.best_merit = merit;
        }
    }

    pub(super) fn full_trace_end(&mut self) {
        if !self.full_trace_active() {
            return;
        }
        let profile = self.solver_control_profile();
        let t = self.test_full_trace.as_mut().unwrap();
        t.total = profile.saturating_delta(t.before);
        t.final_moved = self.moved;
        t.active = false;
        t.complete = true;
    }

    /// Called by the harness outside all measured/heap-guarded callbacks.
    pub fn print_full_power_trace(&self) {
        let Some(t) = &self.test_full_trace else {
            return;
        };
        assert!(t.complete, "selected power solve was not reached");
        assert!(
            !t.overflow,
            "full trace storage exhausted; trajectory is incomplete"
        );
        assert_eq!(t.len as u64, t.total.newton_passes, "missing Newton passes");
        let relative = t.relative_sample;
        let block = relative / t.block_size as u64;
        let frame = relative % t.block_size as u64;
        let step = t.input - t.last_input;
        let previous = t.last_input - t.earlier_input;
        println!("twin_power_full_trace_begin,relative_sample={relative},block={block},frame={frame},solve={},numbering=zero_based_after_warmup,input={:.17e},last_input={:.17e},earlier_input={:.17e},source_step={step:.17e},previous_source_step={previous:.17e},source_curvature={:.17e},predictor_available={},predictor_used={},predictor_suppressed={},predictor_suppression_reason={},source_scaled={},curvature_gate={},raw_source_secant_scale={:.17e},bounded_predictor_scale={:.17e},timing_valid=false",
            t.selected_solve,t.input,t.last_input,t.earlier_input,step-previous,
            t.predictor_available,t.predictor_available && t.predictor_scale != 0.0,t.predictor_suppressed,t.predictor_reason,t.source_scaled,t.curvature_gate,t.raw_ratio,t.predictor_scale);
        if let Some(init) = t.jacobian_init {
            println!("twin_power_full_initializer,relative_sample={relative},kind=previous_jacobian,gate_passed={},attempted={},accepted={},fused_first_newton={},fused_fallback={},source_step_abs={:.17e},source_ratio_abs={:.17e},min_source_step={:.17e},min_source_ratio={:.17e},baseline_merit={:.17e},candidate_merit={:.17e},step_scale={:.17e}", init.gate_passed,init.attempted,init.accepted,init.fused_first_newton,init.fused_fallback,init.source_step_abs,init.source_ratio_abs,init.min_source_step,init.min_source_ratio,init.baseline_merit,init.candidate_merit,init.scale);
        }
        for (at, ((&before, &earlier), &after)) in
            t.prior.iter().zip(&t.earlier).zip(&t.predicted).enumerate()
        {
            println!("twin_power_full_predictor_state,relative_sample={relative},unknown={at},name={},before={before:.17e},earlier={earlier:.17e},unscaled_secant_delta={:.17e},raw_source_secant_delta={:.17e},applied_delta={:.17e},after={after:.17e}", self.solver_unknown_name(at),before-earlier,t.raw_ratio*(before-earlier),after-before);
        }
        for (index, r) in t.records[..t.len].iter().enumerate() {
            let pass = index + 1;
            let w = r.work;
            println!("twin_power_full_pass,relative_sample={relative},block={block},frame={frame},solve={},pass={pass},phase={},phase_pass={},target_passes={},input={:.17e},last_input={:.17e},earlier_input={:.17e},source_step={step:.17e},previous_source_step={previous:.17e},source_curvature={:.17e},solve_source={:.17e},moved_before={:.17e},normalized_correction_before={:.17e},residual_merit_before={:.17e},residual_exact_before={},dominant_before_available={},dominant_unknown_before={},dominant_unknown_name_before={},dominant_unknown_value_before={:.17e},newton_step_norm={:.17e},newton_dominant_unknown={},newton_dominant_unknown_name={},newton_dominant_value={:.17e},newton_dominant_delta={:.17e},reduced={},fixed13={},pivot_replays={},pivot_learns={},pivot_invalidations={},plain_or_searched={},search_entered={},stamped_merit_before={:.17e},search_reference_merit={:.17e},best_trial_lambda={:.17e},best_trial_merit={:.17e},accepted_lambda={:.17e},accepted_merit={:.17e},fallback_used={},fallback_lambda={:.17e},backtracks={},cycle_rejected={},device_settled_count={},unsettled_device_count={},unsettled_device_count_after={},limiter_handoff={},limiter_hold={},continuation_active={},continuation_attempt={},continuation_midpoint={},continuation_success={},restart_active={},restart_pass={},moved_after={:.17e},normalized_correction_after={:.17e},residual_merit_after={:.17e},residual_exact_after={},dominant_unknown_after={},dominant_unknown_name_after={},dominant_unknown_value_after={:.17e},settled={},stuck={}",
                t.selected_solve,r.phase,r.phase_pass,r.target_passes,t.input,t.last_input,t.earlier_input,step-previous,r.source,r.moved_before,r.moved_before,r.merit_before,r.residual_exact_before,r.before_available,r.before_at,self.solver_unknown_name(r.before_at),r.before_value,r.step_norm,r.newton_at,self.solver_unknown_name(r.newton_at),r.newton_value,r.newton_delta,r.reduced,r.reduced&&r.boundary==13,w.reduced_pivot_replays,w.reduced_pivot_learns,w.reduced_pivot_invalidations,if r.searched {"searched"} else {"plain"},r.search_entered,r.stamped_merit,r.reference,r.best_lambda,r.best_merit,r.accepted_lambda,r.accepted_merit,w.fallbacks>0,r.fallback_lambda,w.backtracks,r.cycle_rejected,r.device_count-r.unsettled_devices,r.unsettled_devices,r.unsettled_devices_after,r.limiter_handoff,r.limiter_hold,r.continuation_active,r.cumulative.continuation_attempts,r.phase=="continuation_midpoint",r.cumulative.continuation_successes,r.phase.contains("restart"),if r.phase.contains("restart") {r.phase_pass} else {0},r.moved_after,r.moved_after,r.merit_after,r.residual_exact_after,r.newton_at,self.solver_unknown_name(r.newton_at),r.after_value,r.settled,r.stuck);
            for (trial, p) in r.trials[..r.trial_count].iter().enumerate() {
                println!("twin_power_full_trial,relative_sample={relative},pass={pass},trial={},kind={},lambda={:.17e},trial_merit={:.17e},finite={},exact={},accepted={}",trial+1,p.kind,p.lambda,p.merit,p.merit.is_finite(),p.exact,p.accepted);
            }
        }
        let w = t.total;
        println!("twin_power_full_trace_end,relative_sample={relative},solve={},passes={},newton_passes={},searched_passes={},search_trials={},backtracks={},fallbacks={},cont_attempts={},cont_midpoints={},cont_successes={},restart_attempts={},restart_passes={},final_moved={:.17e},complete={},overflow={},unsettled={}",t.selected_solve,t.len,w.newton_passes,w.searched_passes,w.search_trial_evaluations,w.backtracks,w.fallbacks,w.continuation_attempts,w.continuation_midpoint_successes,w.continuation_successes,w.recovery_restart_attempts,w.recovery_restart_passes,t.final_moved,t.complete,t.overflow,w.unsettled);
    }
}

#[test]
fn relative_sample_mapping_matches_tail_profiler() {
    assert_eq!(selected_solve(1024, 0), Some(1025));
    assert_eq!(selected_solve(1024, 59930), Some(60955));
    assert_eq!((59930 / 64, 59930 % 64), (936, 26));
    assert_eq!(selected_solve(u64::MAX, 0), None);
}

#[test]
fn full_trace_preserves_output_device_cache_and_health_at_supported_rates() {
    use crate::voice::{self, Gain};
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let circuit = voice::build_power(Gain::Twin).unwrap().unwrap();
        let build = || {
            let mut sim = Simulation::new(circuit.clone(), rate);
            sim.test_ceres_lm13 = false;
            sim.test_disable_nlsolve_dogleg_trust_region = true;
            sim.set_late_continuation(true);
            sim.set_backtracks(4);
            assert!(sim.find_operating_point());
            sim
        };
        let mut baseline = build();
        let mut traced = build();
        for _ in 0..32 {
            baseline.process(1.0);
            traced.process(1.0);
        }
        traced.configure_full_power_trace(3, 64);
        for i in 0..64 {
            let input = if i < 3 {
                12.0 + i as f64
            } else {
                -20.0 * (i as f64 * 0.1).cos()
            };
            assert_eq!(
                baseline.process(input).to_bits(),
                traced.process(input).to_bits()
            );
            assert_eq!(baseline.statistics(), traced.statistics());
            assert_eq!(baseline.health(), traced.health());
            for (a, b) in baseline.devices.iter().zip(&traced.devices) {
                assert_eq!(a.checkpoint(), b.checkpoint());
            }
        }
        let trace = traced.test_full_trace.as_ref().unwrap();
        assert!(trace.complete && !trace.overflow);
        assert_eq!(trace.selected_solve, 36);
        assert_eq!(trace.len as u64, trace.total.newton_passes);
        assert!(trace.len > 1);
        assert_eq!(
            trace.records[..trace.len]
                .iter()
                .map(|r| r.trial_count as u64)
                .sum::<u64>(),
            trace.total.search_trial_evaluations
        );
    }
}
