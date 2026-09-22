//! Opt-in, one-proposal initialization experiment. No modified-Newton loop:
//! the previous settled sample's Jacobian only proposes a starting voltage.

use super::*;
use crate::dsp::{device::DeviceCheckpoint, partition::JacobianFactors13};

pub(super) struct JacobianInit {
    pub factors: Option<JacobianFactors13>,
    pub settled_rhs: Option<[f64; 13]>,
    baseline: Vec<f64>,
    candidate: Vec<f64>,
    checkpoints: Vec<DeviceCheckpoint>,
    gate: SourceGate,
    fuse_first_newton: bool,
    max_merit_ratio: f64,
    prebuilt_first_pass: bool,
    tangent_predictor: bool,
    tangent_max_norm: f64,
    pub tangent_attempts: u64,
    pub tangent_accepts: u64,
    pub last: Attempt,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Attempt {
    pub gate_passed: bool,
    pub attempted: bool,
    pub accepted: bool,
    pub source_step_abs: f64,
    pub source_ratio_abs: f64,
    pub min_source_step: f64,
    pub min_source_ratio: f64,
    pub baseline_merit: f64,
    pub candidate_merit: f64,
    pub scale: f64,
    pub fused_first_newton: bool,
    pub fused_fallback: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SourceGate {
    min_source_ratio: f64,
    min_source_step: f64,
}

impl SourceGate {
    const QUIET: f64 = 1.0e-12;

    fn from_env() -> Self {
        Self {
            min_source_ratio: Self::env_threshold(
                "GAINSTAGEFX_TEST_JACOBIAN_INIT13_MIN_SOURCE_RATIO",
            ),
            min_source_step: Self::env_threshold(
                "GAINSTAGEFX_TEST_JACOBIAN_INIT13_MIN_SOURCE_STEP",
            ),
        }
    }

    fn env_threshold(name: &str) -> f64 {
        let Ok(value) = std::env::var(name) else {
            return 0.0;
        };
        let parsed = value
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("{name} must be a finite nonnegative number"));
        assert!(
            parsed.is_finite() && parsed >= 0.0,
            "{name} must be a finite nonnegative number"
        );
        parsed
    }

    fn geometry(current: f64, last: f64, earlier: f64) -> (f64, f64) {
        if !current.is_finite() || !last.is_finite() || !earlier.is_finite() {
            return (f64::NAN, f64::NAN);
        }
        let current_step = (current - last).abs();
        let previous_step = (last - earlier).abs();
        let ratio = if previous_step <= Self::QUIET {
            if current_step <= Self::QUIET {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            current_step / previous_step
        };
        (current_step, ratio)
    }

    fn allows(self, current: f64, last: f64, earlier: f64) -> bool {
        // With both thresholds absent/zero, preserve the original broad
        // experiment exactly for A/B reference runs.
        if self.min_source_ratio == 0.0 && self.min_source_step == 0.0 {
            return true;
        }
        let (step, ratio) = Self::geometry(current, last, earlier);
        step.is_finite()
            && step >= self.min_source_step
            && (self.min_source_ratio == 0.0 || ratio >= self.min_source_ratio)
    }
}

impl JacobianInit {
    pub fn new(unknowns: usize, devices: usize) -> Self {
        Self::new_with_gate(unknowns, devices, SourceGate::default())
    }

    pub fn from_env(unknowns: usize, devices: usize) -> Self {
        let mut init = Self::new_with_gate(unknowns, devices, SourceGate::from_env());
        init.fuse_first_newton =
            std::env::var_os("GAINSTAGEFX_TEST_JACOBIAN_INIT13_FUSE_FIRST_NEWTON").is_some();
        init.max_merit_ratio = Self::merit_ratio_from_env();
        init.tangent_predictor = std::env::var_os("GAINSTAGEFX_TEST_JACOBIAN_TANGENT13").is_some();
        init.tangent_max_norm = Self::tangent_max_norm_from_env();
        init
    }

    fn new_with_gate(unknowns: usize, devices: usize, gate: SourceGate) -> Self {
        Self {
            factors: None,
            settled_rhs: None,
            baseline: vec![0.0; unknowns],
            candidate: vec![0.0; unknowns],
            checkpoints: vec![DeviceCheckpoint::default(); devices],
            gate,
            fuse_first_newton: false,
            max_merit_ratio: 0.8,
            prebuilt_first_pass: false,
            tangent_predictor: false,
            tangent_max_norm: 1.0,
            tangent_attempts: 0,
            tangent_accepts: 0,
            last: Attempt::default(),
        }
    }

    fn tangent_max_norm_from_env() -> f64 {
        let name = "GAINSTAGEFX_TEST_JACOBIAN_TANGENT13_MAX_NORM";
        let Ok(value) = std::env::var(name) else {
            return 1.0;
        };
        let parsed = value
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("{name} must be a finite positive number"));
        assert!(
            parsed.is_finite() && parsed > 0.0,
            "{name} must be a finite positive number"
        );
        parsed
    }

    fn merit_ratio_from_env() -> f64 {
        let name = "GAINSTAGEFX_TEST_JACOBIAN_INIT13_MAX_MERIT_RATIO";
        let Ok(value) = std::env::var(name) else {
            return 0.8;
        };
        let parsed = value
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("{name} must be a finite number in (0, 1]"));
        assert!(
            parsed.is_finite() && parsed > 0.0 && parsed <= 1.0,
            "{name} must be a finite number in (0, 1]"
        );
        parsed
    }

    fn residual(
        partition: &ReducedNonlinear,
        rhs: &[f64],
        point: &[f64],
        devices: &mut [AnyDevice],
        cache: bool,
    ) -> Option<([f64; 13], f64)> {
        let mut values = [0.0; 13];
        let map = partition.begin_residual(rhs, point, &mut values)?;
        let mut stamp = ResidualStamper {
            residual: &mut values,
            map,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        for device in devices {
            device.trial_residual(&mut stamp, point, cache);
            if stamp.mapping_failed || stamp.junction_held {
                return None;
            }
        }
        let merit = partition.residual_merit(&values)?;
        merit.is_finite().then_some((values, merit))
    }

    fn record_fused_outcome(
        &mut self,
        accepted: bool,
        evaluations: u64,
        profile: Option<&mut SolverControlProfile>,
        phase_profile: Option<&mut TwinPowerPhaseProfile>,
    ) {
        self.last.accepted = accepted;
        if let Some(phase) = phase_profile {
            if accepted {
                phase.jacobian_init_accepts = phase.jacobian_init_accepts.saturating_add(1);
            } else {
                phase.jacobian_init_rejects = phase.jacobian_init_rejects.saturating_add(1);
            }
        }
        if let Some(profile) = profile {
            profile.jacobian_init_attempts = profile.jacobian_init_attempts.saturating_add(1);
            profile.jacobian_init_evaluations = profile
                .jacobian_init_evaluations
                .saturating_add(evaluations);
            if accepted {
                profile.jacobian_init_accepts = profile.jacobian_init_accepts.saturating_add(1);
            } else {
                profile.jacobian_init_rejects = profile.jacobian_init_rejects.saturating_add(1);
            }
        }
    }

    /// Prepare the previous-J proposal but deliberately do not judge its exact
    /// residual yet. The caller may let the first ordinary Newton stamp judge
    /// and consume the candidate in one pass when no device limiter changes
    /// the point. This is test-only A/B plumbing; the original two-residual
    /// initializer remains the default.
    fn prepare_fused_candidate(
        &mut self,
        partition: &mut ReducedNonlinear,
        rhs: &[f64],
        voltage: &mut [f64],
        devices: &mut [AnyDevice],
        profile: Option<&mut SolverControlProfile>,
        phase_profile: Option<&mut TwinPowerPhaseProfile>,
    ) -> bool {
        let Some(factors) = self.factors else {
            if let Some(profile) = profile {
                profile.jacobian_init_unavailable =
                    profile.jacobian_init_unavailable.saturating_add(1);
            }
            return false;
        };
        self.prebuilt_first_pass = false;
        self.last.attempted = true;
        self.last.baseline_merit = f64::NAN;
        self.last.candidate_merit = f64::NAN;
        self.last.fused_first_newton = false;
        self.last.fused_fallback = false;
        self.baseline.copy_from_slice(voltage);
        for (saved, device) in self.checkpoints.iter_mut().zip(devices.iter()) {
            *saved = device.checkpoint();
        }

        let mut phase_profile = phase_profile;
        let prepared = (|| {
            let baseline_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let baseline = Self::residual(partition, rhs, voltage, devices, false);
            if let (Some(started), Some(phase)) = (baseline_started, phase_profile.as_deref_mut()) {
                phase.jacobian_init_baseline_residual_ns = phase
                    .jacobian_init_baseline_residual_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_baseline_residual_calls = phase
                    .jacobian_init_baseline_residual_calls
                    .saturating_add(1);
            }
            let (residual, here) = baseline?;
            self.last.baseline_merit = here;
            if here == 0.0 {
                return None;
            }

            let solve_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let delta = factors.solve(residual.map(|value| -value));
            if let (Some(started), Some(phase)) = (solve_started, phase_profile.as_deref_mut()) {
                phase.jacobian_init_previous_j_solve_ns = phase
                    .jacobian_init_previous_j_solve_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_previous_j_solve_calls =
                    phase.jacobian_init_previous_j_solve_calls.saturating_add(1);
            }
            let delta = delta?;

            let mut norm = 1.0f64;
            for (&node, &step) in partition.boundary_nodes().iter().zip(&delta) {
                norm = norm.max(step.abs() / (1.0 + voltage[node].abs()));
            }
            let scale = 1.0 / norm;
            self.last.scale = scale;
            self.candidate.copy_from_slice(voltage);
            for (&node, &step) in partition.boundary_nodes().iter().zip(&delta) {
                self.candidate[node] = voltage[node] + scale * step;
            }

            let recovery_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let recovered = partition.trust_region_recover_internal_13(&mut self.candidate);
            if let (Some(started), Some(phase)) = (recovery_started, phase_profile.as_deref_mut()) {
                phase.jacobian_init_candidate_recovery_ns = phase
                    .jacobian_init_candidate_recovery_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_candidate_recovery_calls = phase
                    .jacobian_init_candidate_recovery_calls
                    .saturating_add(1);
            }
            recovered.then_some(())
        })()
        .is_some();

        // Baseline residual evaluation is not allowed to leak any trial cache
        // into the tentative first Newton stamp.
        for (device, &saved) in devices.iter_mut().zip(&self.checkpoints) {
            device.restore_checkpoint(saved);
        }

        if prepared {
            voltage.copy_from_slice(&self.candidate);
            true
        } else {
            voltage.copy_from_slice(&self.baseline);
            self.record_fused_outcome(false, 1, profile, phase_profile);
            false
        }
    }

    /// Finish a fused attempt using the residual represented by the already
    /// built, unheld first-Newton stamp. On acceptance that stamp remains live
    /// and the next Newton pass consumes it without rebuilding anything.
    fn finish_fused_stamp(
        &mut self,
        merit: f64,
        devices: &mut [AnyDevice],
        voltage: &mut [f64],
        profile: Option<&mut SolverControlProfile>,
        phase_profile: Option<&mut TwinPowerPhaseProfile>,
    ) {
        self.last.fused_first_newton = true;
        self.last.candidate_merit = merit;
        let accepted =
            merit.is_finite() && merit <= self.max_merit_ratio * self.last.baseline_merit;
        self.prebuilt_first_pass = accepted;
        if !accepted {
            for (device, &saved) in devices.iter_mut().zip(&self.checkpoints) {
                device.restore_checkpoint(saved);
            }
            voltage.copy_from_slice(&self.baseline);
        }
        self.record_fused_outcome(accepted, 1, profile, phase_profile);
    }

    /// If the normal first stamp had to limit a device, its stamped residual
    /// is not the exact physical residual used by the initializer oracle. Fall
    /// back to the original exact candidate probe and preserve its cache/state
    /// semantics. This path changes no acceptance rule.
    fn finish_fused_fallback(
        &mut self,
        partition: &ReducedNonlinear,
        rhs: &[f64],
        devices: &mut [AnyDevice],
        voltage: &mut [f64],
        profile: Option<&mut SolverControlProfile>,
        phase_profile: Option<&mut TwinPowerPhaseProfile>,
    ) {
        self.last.fused_fallback = true;
        self.prebuilt_first_pass = false;
        for (device, &saved) in devices.iter_mut().zip(&self.checkpoints) {
            device.restore_checkpoint(saved);
        }

        let mut phase_profile = phase_profile;
        let candidate_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
        let candidate = Self::residual(partition, rhs, &self.candidate, devices, true);
        if let (Some(started), Some(phase)) = (candidate_started, phase_profile.as_deref_mut()) {
            phase.jacobian_init_candidate_residual_ns = phase
                .jacobian_init_candidate_residual_ns
                .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
            phase.jacobian_init_candidate_residual_calls = phase
                .jacobian_init_candidate_residual_calls
                .saturating_add(1);
        }
        let merit = candidate.map(|(_, merit)| merit).unwrap_or(f64::NAN);
        self.last.candidate_merit = merit;
        let accepted =
            merit.is_finite() && merit <= self.max_merit_ratio * self.last.baseline_merit;
        for (device, &saved) in devices.iter_mut().zip(&self.checkpoints) {
            if accepted {
                device.restore_coordinates_keep_trial_cache(saved);
            } else {
                device.restore_checkpoint(saved);
            }
        }
        if !accepted {
            voltage.copy_from_slice(&self.baseline);
        }
        self.record_fused_outcome(accepted, 2, profile, phase_profile);
    }

    pub(super) fn take_prebuilt_first_pass(&mut self) -> bool {
        std::mem::take(&mut self.prebuilt_first_pass)
    }

    /// Two physical residual evaluations at most, one triangular solve, no
    /// stamp/factorization, and no device-history commit on either outcome.
    /// The candidate includes this sample's eliminated-node companion RHS.
    fn propose(
        &mut self,
        partition: &mut ReducedNonlinear,
        rhs: &[f64],
        voltage: &mut [f64],
        devices: &mut [AnyDevice],
        profile: Option<&mut SolverControlProfile>,
        phase_profile: Option<&mut TwinPowerPhaseProfile>,
    ) {
        let Some(factors) = self.factors else {
            if let Some(profile) = profile {
                profile.jacobian_init_unavailable += 1;
            }
            return;
        };
        self.prebuilt_first_pass = false;
        self.last.attempted = true;
        self.last.baseline_merit = f64::NAN;
        self.last.candidate_merit = f64::NAN;
        self.last.fused_first_newton = false;
        self.last.fused_fallback = false;
        for (saved, device) in self.checkpoints.iter_mut().zip(devices.iter()) {
            *saved = device.checkpoint();
        }
        let mut evaluations = 0;
        let mut phase_profile = phase_profile;
        let accepted = (|| {
            evaluations += 1;
            let baseline_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let baseline = Self::residual(partition, rhs, voltage, devices, false);
            if let (Some(started), Some(phase)) = (baseline_started, phase_profile.as_deref_mut()) {
                phase.jacobian_init_baseline_residual_ns = phase
                    .jacobian_init_baseline_residual_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_baseline_residual_calls = phase
                    .jacobian_init_baseline_residual_calls
                    .saturating_add(1);
            }
            let (residual, here) = baseline?;
            self.last.baseline_merit = here;
            if here == 0.0 {
                return None;
            }
            let solve_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let delta = factors.solve(residual.map(|value| -value));
            if let (Some(started), Some(phase)) = (solve_started, phase_profile.as_deref_mut()) {
                phase.jacobian_init_previous_j_solve_ns = phase
                    .jacobian_init_previous_j_solve_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_previous_j_solve_calls =
                    phase.jacobian_init_previous_j_solve_calls.saturating_add(1);
            }
            let delta = delta?;
            // Bound only the proposed initialization along its original
            // direction. One native unit is the scale floor for near-zero unknowns;
            // large plate voltages retain their own scale. This is neither a
            // clamp of a circuit solution nor a changed convergence tolerance.
            let mut norm = 1.0f64;
            for (&node, &step) in partition.boundary_nodes().iter().zip(&delta) {
                norm = norm.max(step.abs() / (1.0 + voltage[node].abs()));
            }
            let scale = 1.0 / norm;
            self.last.scale = scale;
            self.candidate.copy_from_slice(voltage);
            for (&node, &step) in partition.boundary_nodes().iter().zip(&delta) {
                self.candidate[node] = voltage[node] + scale * step;
            }
            let recovery_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let recovered = partition.trust_region_recover_internal_13(&mut self.candidate);
            if let (Some(started), Some(phase)) = (recovery_started, phase_profile.as_deref_mut()) {
                phase.jacobian_init_candidate_recovery_ns = phase
                    .jacobian_init_candidate_recovery_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_candidate_recovery_calls = phase
                    .jacobian_init_candidate_recovery_calls
                    .saturating_add(1);
            }
            if !recovered {
                return None;
            }
            evaluations += 1;
            let candidate_started = phase_profile.as_ref().map(|_| test_thread_cpu_time_ns());
            let candidate = Self::residual(partition, rhs, &self.candidate, devices, true);
            if let (Some(started), Some(phase)) = (candidate_started, phase_profile.as_deref_mut())
            {
                phase.jacobian_init_candidate_residual_ns = phase
                    .jacobian_init_candidate_residual_ns
                    .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
                phase.jacobian_init_candidate_residual_calls = phase
                    .jacobian_init_candidate_residual_calls
                    .saturating_add(1);
            }
            let (_, there) = candidate?;
            self.last.candidate_merit = there;
            // Demand material progress in the exact root residual. This only
            // chooses an initializer; it never declares the sample settled.
            (there <= self.max_merit_ratio * here).then_some(())
        })()
        .is_some();
        // Even residual-only evaluation clears nonlinear caches. Restore the
        // complete checkpoint on rejection. Acceptance retains only the exact
        // candidate evaluation; limiter/integration coordinates remain those of
        // the previous sample, just as with ordinary voltage prediction.
        for (device, &saved) in devices.iter_mut().zip(&self.checkpoints) {
            if accepted {
                device.restore_coordinates_keep_trial_cache(saved);
            } else {
                device.restore_checkpoint(saved);
            }
        }
        self.last.accepted = accepted;
        if let Some(phase) = phase_profile {
            if accepted {
                phase.jacobian_init_accepts = phase.jacobian_init_accepts.saturating_add(1);
            } else {
                phase.jacobian_init_rejects = phase.jacobian_init_rejects.saturating_add(1);
            }
        }
        if accepted {
            voltage.copy_from_slice(&self.candidate);
        }
        if let Some(profile) = profile {
            profile.jacobian_init_attempts += 1;
            profile.jacobian_init_evaluations += evaluations;
            if accepted {
                profile.jacobian_init_accepts += 1;
            } else {
                profile.jacobian_init_rejects += 1;
            }
        }
    }
    /// First-order new-sample predictor from the previous settled Jacobian.
    ///
    /// The only forcing change used here is the exact reduced fixed RHS delta
    /// between samples.  No nonlinear residual is evaluated and no device state
    /// is touched.  If the correction is implausibly large, the proposal is
    /// rejected and the caller keeps the ordinary state/secant predictor.
    fn tangent_predict(
        &mut self,
        partition: &mut ReducedNonlinear,
        rhs: &[f64],
        voltage: &mut [f64],
    ) -> bool {
        if !self.tangent_predictor {
            return false;
        }
        self.tangent_attempts = self.tangent_attempts.saturating_add(1);
        let Some(factors) = self.factors else {
            return false;
        };
        let Some(previous_rhs) = self.settled_rhs else {
            return false;
        };
        let Some(current_rhs) = partition.fixed_reduced_rhs_13(rhs) else {
            return false;
        };
        let delta_rhs = std::array::from_fn(|index| current_rhs[index] - previous_rhs[index]);
        let Some(step) = factors.solve(delta_rhs) else {
            return false;
        };

        let mut norm = 0.0f64;
        for (&node, &delta) in partition.boundary_nodes().iter().zip(&step) {
            if !delta.is_finite() {
                return false;
            }
            norm = norm.max(delta.abs() / (1.0 + voltage[node].abs()));
        }
        if !norm.is_finite() || norm > self.tangent_max_norm {
            return false;
        }

        self.candidate.copy_from_slice(voltage);
        for (&node, &delta) in partition.boundary_nodes().iter().zip(&step) {
            self.candidate[node] = voltage[node] + delta;
        }
        if !partition.trust_region_recover_internal_13(&mut self.candidate) {
            return false;
        }
        voltage.copy_from_slice(&self.candidate);
        self.tangent_accepts = self.tangent_accepts.saturating_add(1);
        true
    }
}

impl Simulation {
    pub(super) fn invalidate_jacobian_init(&mut self) {
        if let Some(init) = &mut self.test_jacobian_init {
            init.factors = None;
            init.settled_rhs = None;
            init.prebuilt_first_pass = false;
        }
    }

    pub(super) fn jacobian_tangent_predict(&mut self, eligible: bool) -> bool {
        if !eligible || !self.late_continuation || self.last_was_unsettled || self.watching {
            return false;
        }
        let Some(mut init) = self.test_jacobian_init.take() else {
            return false;
        };
        if !init.tangent_predictor {
            self.test_jacobian_init = Some(init);
            return false;
        }

        // Only the experimental lane pays this copy.  It preserves the previous
        // settled full state so predictor history can advance exactly as before
        // after `voltage` is replaced by the tangent candidate.
        self.predicted.copy_from_slice(&self.voltage);
        let used = match self.nonlinear_partition.as_mut() {
            Some(partition) if partition.boundary_len() == 13 => {
                init.tangent_predict(partition, &self.fixed_rhs, &mut self.voltage)
            }
            _ => false,
        };
        self.test_jacobian_init = Some(init);
        used
    }

    pub(super) fn jacobian_assisted_init(&mut self, suppressed: bool, input: f64) {
        let profile_enabled = self.solver_control_tail_profile_enabled();
        let last_input = self.last_input;
        let earlier_input = self.earlier_input;
        let Some(mut init) = self.test_jacobian_init.take() else {
            return;
        };
        init.prebuilt_first_pass = false;
        let (source_step_abs, source_ratio_abs) =
            SourceGate::geometry(input, last_input, earlier_input);
        let gate_passed = init.gate.allows(input, last_input, earlier_input);
        init.last = Attempt {
            gate_passed,
            source_step_abs,
            source_ratio_abs,
            min_source_step: init.gate.min_source_step,
            min_source_ratio: init.gate.min_source_ratio,
            ..Attempt::default()
        };
        if !suppressed
            || !self.late_continuation
            || self.last_was_unsettled
            || self.watching
            || !gate_passed
        {
            self.test_jacobian_init = Some(init);
            return;
        }
        let boundary_is_13 = self
            .nonlinear_partition
            .as_ref()
            .is_some_and(|partition| partition.boundary_len() == 13);
        if !boundary_is_13 {
            self.test_jacobian_init = Some(init);
            return;
        }

        if !init.fuse_first_newton {
            init.propose(
                self.nonlinear_partition.as_mut().unwrap(),
                &self.fixed_rhs,
                &mut self.voltage,
                &mut self.devices,
                profile_enabled.then_some(&mut self.test_control_profile),
                self.test_profile_twin_power_phases
                    .then_some(&mut self.test_phase_profile),
            );
            self.test_jacobian_init = Some(init);
            return;
        }

        let exact_before = self.exact;
        let prepared = init.prepare_fused_candidate(
            self.nonlinear_partition.as_mut().unwrap(),
            &self.fixed_rhs,
            &mut self.voltage,
            &mut self.devices,
            profile_enabled.then_some(&mut self.test_control_profile),
            self.test_profile_twin_power_phases
                .then_some(&mut self.test_phase_profile),
        );
        if prepared {
            // Build exactly the stamp the first ordinary Newton pass would
            // build. It can also serve as the initializer's candidate oracle
            // only when no limiter changed the requested candidate point.
            let built = self.build_current_reduced(false, true);
            let limiter_held = self.devices.iter().any(AnyDevice::limiter_held);
            if built && self.exact && !limiter_held {
                let merit = self.current_reduced_merit(false).unwrap_or(f64::NAN);
                init.finish_fused_stamp(
                    merit,
                    &mut self.devices,
                    &mut self.voltage,
                    profile_enabled.then_some(&mut self.test_control_profile),
                    self.test_profile_twin_power_phases
                        .then_some(&mut self.test_phase_profile),
                );
                if !init.prebuilt_first_pass {
                    self.exact = exact_before;
                }
            } else {
                // A limited stamp is not the exact residual oracle. Undo its
                // observable solver state and use the original candidate probe.
                self.exact = exact_before;
                init.finish_fused_fallback(
                    self.nonlinear_partition.as_ref().unwrap(),
                    &self.fixed_rhs,
                    &mut self.devices,
                    &mut self.voltage,
                    profile_enabled.then_some(&mut self.test_control_profile),
                    self.test_profile_twin_power_phases
                        .then_some(&mut self.test_phase_profile),
                );
            }
        }
        self.test_jacobian_init = Some(init);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{self, Gain};

    fn build(rate: f64) -> Simulation {
        let mut sim = Simulation::new(voice::build_power(Gain::Twin).unwrap().unwrap(), rate);
        sim.test_jacobian_init = Some(Box::new(JacobianInit::new(sim.n, sim.devices.len())));
        sim.test_ceres_lm13 = false;
        sim.test_disable_nlsolve_dogleg_trust_region = true;
        sim.test_profile_solver_control_tail = true;
        sim.set_backtracks(4);
        sim.set_late_continuation(true);
        sim.set_pass_ceiling(64);
        assert!(sim.find_operating_point());
        sim
    }

    #[test]
    fn jacobian_init_source_gate_is_optional_and_targets_large_source_events() {
        let broad = SourceGate::default();
        assert!(broad.allows(f64::NAN, 1.0, 0.0));

        let selective = SourceGate {
            min_source_ratio: 4.0,
            min_source_step: 8.0,
        };
        // The measured Standard Twin pathological sample: ~38.41 V after
        // ~2.55 V, an absolute source-step ratio of ~15.06.
        assert!(selective.allows(
            -20.245_530_571_170_86,
            18.166_187_516_433_837,
            20.717_380_708_906_887,
        ));
        // Ratio large enough but absolute jump too small.
        assert!(!selective.allows(5.0, 0.0, -1.0));
        // Jump large enough but ratio too mild.
        assert!(!selective.allows(10.0, 0.0, -3.0));
        // First large edge out of a flat source has effectively infinite ratio.
        assert!(selective.allows(8.0, 0.0, 0.0));
        assert!(!selective.allows(f64::INFINITY, 0.0, 0.0));

        let ratio_only = SourceGate {
            min_source_ratio: 12.0,
            min_source_step: 0.0,
        };
        assert!(ratio_only.allows(15.0, 0.0, -1.0));
        assert!(!ratio_only.allows(8.0, 0.0, -1.0));

        let step_only = SourceGate {
            min_source_ratio: 0.0,
            min_source_step: 16.0,
        };
        assert!(step_only.allows(16.0, 0.0, -100.0));
        assert!(!step_only.allows(15.99, 0.0, -100.0));
    }

    #[test]
    fn jacobian_init_source_gate_skips_before_residual_evaluation() {
        let mut sim = build(48_000.0);
        sim.process(0.1);
        assert!(sim.test_jacobian_init.as_ref().unwrap().factors.is_some());
        sim.test_jacobian_init.as_mut().unwrap().gate = SourceGate {
            min_source_ratio: 4.0,
            min_source_step: 8.0,
        };

        sim.last_input = 0.0;
        sim.earlier_input = -1.0;
        sim.prepare_rhs(5.0, false);
        let voltage = sim.voltage.clone();
        let devices: Vec<_> = sim.devices.iter().map(AnyDevice::checkpoint).collect();
        let before = sim.solver_control_profile();
        sim.jacobian_assisted_init(true, 5.0);
        let attempt = sim.test_jacobian_init.as_ref().unwrap().last;
        assert!(!attempt.gate_passed);
        assert!(!attempt.attempted);
        assert_eq!(sim.voltage, voltage);
        assert_eq!(
            sim.devices
                .iter()
                .map(AnyDevice::checkpoint)
                .collect::<Vec<_>>(),
            devices
        );
        let work = sim.solver_control_profile().saturating_delta(before);
        assert_eq!(work.jacobian_init_attempts, 0);
        assert_eq!(work.jacobian_init_evaluations, 0);

        sim.prepare_rhs(20.0, false);
        let before = sim.solver_control_profile();
        sim.jacobian_assisted_init(true, 20.0);
        let attempt = sim.test_jacobian_init.as_ref().unwrap().last;
        assert!(attempt.gate_passed);
        assert!(attempt.attempted);
        let work = sim.solver_control_profile().saturating_delta(before);
        assert_eq!(work.jacobian_init_attempts, 1);
        assert!((1..=2).contains(&work.jacobian_init_evaluations));
    }

    #[test]
    fn jacobian_init_phase_profile_counts_each_initializer_stage() {
        let mut sim = build(48_000.0);
        sim.process(0.1);
        assert!(sim.test_jacobian_init.as_ref().unwrap().factors.is_some());

        sim.test_profile_twin_power_phases = true;
        sim.test_phase_profile = TwinPowerPhaseProfile::default();
        sim.prepare_rhs(20.0, false);
        let before = sim.solver_control_profile();
        sim.jacobian_assisted_init(true, 20.0);

        let attempt = sim.test_jacobian_init.as_ref().unwrap().last;
        assert!(attempt.attempted);
        let work = sim.solver_control_profile().saturating_delta(before);
        let phase = sim.test_phase_profile;
        assert_eq!(phase.jacobian_init_baseline_residual_calls, 1);
        assert_eq!(phase.jacobian_init_previous_j_solve_calls, 1);
        assert_eq!(phase.jacobian_init_candidate_recovery_calls, 1);
        assert_eq!(
            phase.jacobian_init_candidate_residual_calls,
            if work.jacobian_init_evaluations == 2 {
                1
            } else {
                0
            }
        );
        assert_eq!(phase.jacobian_init_accepts + phase.jacobian_init_rejects, 1);
        assert_eq!(
            phase.jacobian_init_accepts,
            if attempt.accepted { 1 } else { 0 }
        );
        assert_eq!(
            phase.jacobian_init_rejects,
            if attempt.accepted { 0 } else { 1 }
        );
    }

    #[test]
    fn jacobian_init_restores_device_state_on_acceptance_and_rejection() {
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let mut sim = build(rate);
            sim.process(0.1);
            let original = sim.voltage.clone();
            let factors = sim.test_jacobian_init.as_ref().unwrap().factors;
            assert!(factors.is_some());
            let mut accepted = 0;
            let mut rejected = 0;
            for input in [
                0.0,
                0.01,
                -0.01,
                1.0,
                -1.0,
                10.0,
                -10.0,
                50.0,
                -50.0,
                100.0,
                -100.0,
                1e6,
                f64::MAX,
            ] {
                sim.voltage.copy_from_slice(&original);
                sim.prepare_rhs(input, false);
                // Seed a real pending evaluation so rollback tests the cache,
                // not just an empty checkpoint.
                sim.point.copy_from_slice(&sim.voltage);
                let _ = sim.reduced_trial_merit(false, true);
                let devices: Vec<_> = sim.devices.iter().map(AnyDevice::checkpoint).collect();
                let exact = sim.exact;
                let before = sim.solver_control_profile();
                sim.jacobian_assisted_init(true, input);
                let attempt = sim.test_jacobian_init.as_ref().unwrap().last;
                assert!(attempt.attempted);
                if attempt.accepted {
                    for (device, &before) in sim.devices.iter_mut().zip(&devices) {
                        let after = device.linearisation();
                        device.restore_checkpoint(before);
                        assert_eq!(device.linearisation(), after);
                    }
                } else {
                    assert_eq!(
                        sim.devices
                            .iter()
                            .map(AnyDevice::checkpoint)
                            .collect::<Vec<_>>(),
                        devices
                    );
                }
                assert_eq!(sim.exact, exact);
                assert_eq!(sim.test_jacobian_init.as_ref().unwrap().factors, factors);
                let work = sim.solver_control_profile().saturating_delta(before);
                assert_eq!(work.jacobian_init_attempts, 1);
                assert!((1..=2).contains(&work.jacobian_init_evaluations));
                assert_eq!(work.newton_passes, 0);
                assert_eq!(work.search_trial_evaluations, 0);
                if attempt.accepted {
                    accepted += 1;
                    assert!(attempt.candidate_merit <= 0.8 * attempt.baseline_merit);
                    assert!(sim.voltage.iter().all(|v| v.is_finite()));
                } else {
                    rejected += 1;
                    assert_eq!(sim.voltage, original);
                }
            }
            assert!(
                accepted > 0 && rejected > 0,
                "rate={rate}: accepts={accepted} rejects={rejected}"
            );
        }
    }

    #[test]
    fn jacobian_init_fused_first_newton_reuses_unlimited_stamp() {
        let mut found = false;
        for input in [0.2, -0.2, 0.5, -0.5, 1.0, -1.0, 5.0, -5.0, 20.0] {
            let mut sim = build(48_000.0);
            sim.process(0.1);
            assert!(sim.test_jacobian_init.as_ref().unwrap().factors.is_some());
            sim.test_jacobian_init.as_mut().unwrap().fuse_first_newton = true;
            sim.test_profile_twin_power_phases = true;
            sim.test_phase_profile = TwinPowerPhaseProfile::default();
            sim.prepare_rhs(input, false);
            let before = sim.solver_control_profile();
            sim.jacobian_assisted_init(true, input);
            let attempt = sim.test_jacobian_init.as_ref().unwrap().last;
            if !(attempt.fused_first_newton && attempt.accepted) {
                continue;
            }

            let work = sim.solver_control_profile().saturating_delta(before);
            assert_eq!(work.jacobian_init_attempts, 1);
            assert_eq!(work.jacobian_init_evaluations, 1);
            assert_eq!(
                sim.test_phase_profile.jacobian_init_baseline_residual_calls,
                1
            );
            assert_eq!(
                sim.test_phase_profile
                    .jacobian_init_candidate_residual_calls,
                0
            );
            assert_eq!(sim.test_phase_profile.reduced_stamp_calls, 1);
            let stamps = sim.test_phase_profile.reduced_stamp_calls;
            let _ = sim.iterate(false, false);
            assert_eq!(sim.test_phase_profile.reduced_stamp_calls, stamps);
            assert!(!sim.test_jacobian_init.as_ref().unwrap().prebuilt_first_pass);
            found = true;
            break;
        }
        assert!(
            found,
            "expected at least one unlimited accepted fused candidate"
        );
    }

    #[test]
    fn jacobian_init_fused_first_newton_falls_back_for_limited_candidate() {
        let rate = 44_100.0;
        let drive = 0.2;
        let mut channel = Simulation::new(
            voice::build_voice(Gain::Twin, voice::Diode::Silicon, voice::Amplifier::Valve).unwrap(),
            rate,
        );
        channel.test_jacobian_init = None;
        channel.test_ceres_lm13 = false;
        channel.test_disable_nlsolve_dogleg_trust_region = true;
        channel.set_control(Gain::Twin.drive_control(), drive);
        assert!(channel.find_operating_point());

        let mut fused = build(rate);
        fused.test_jacobian_init.as_mut().unwrap().fuse_first_newton = true;
        fused.test_profile_twin_power_phases = true;
        fused.test_phase_profile = TwinPowerPhaseProfile::default();

        for i in 0..4096 {
            let t = i as f64 / rate;
            let input = 0.969
                * 0.5
                * (t / 0.0005).min(1.0)
                * (-t / 0.080).exp()
                * ((std::f64::consts::TAU * 110.0 * t).sin()
                    + 0.5 * (std::f64::consts::TAU * 330.0 * t).sin());
            let signal = channel.process(input);
            let output = fused.process(signal);
            assert!(output.is_finite());
        }

        let profile = fused.solver_control_profile();
        assert!(profile.jacobian_init_attempts > 0);
        assert!(
            profile.jacobian_init_evaluations > profile.jacobian_init_attempts,
            "expected at least one limiter-triggered fused fallback: attempts={} evaluations={}",
            profile.jacobian_init_attempts,
            profile.jacobian_init_evaluations
        );
        assert_eq!(
            fused
                .test_phase_profile
                .jacobian_init_candidate_residual_calls,
            profile.jacobian_init_evaluations - profile.jacobian_init_attempts
        );
        assert!(
            !fused
                .test_jacobian_init
                .as_ref()
                .unwrap()
                .prebuilt_first_pass
        );
        assert_eq!(fused.unsettled, 0);
        assert_eq!(fused.nonfinite, 0);
    }

    #[test]
    fn jacobian_init_invalidation_and_runtime_copy() {
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let mut source = build(rate);
            let mut copy = build(rate);
            for i in 0..64 {
                source.process((i as f64 * 0.03).sin());
            }
            assert!(source
                .test_jacobian_init
                .as_ref()
                .unwrap()
                .factors
                .is_some());
            copy.copy_runtime_state_from(&source);
            for i in 0..256 {
                let input = if i % 7 == 0 {
                    -5.0
                } else {
                    (i as f64 * 0.09).sin()
                };
                assert_eq!(
                    source.process(input).to_bits(),
                    copy.process(input).to_bits()
                );
            }
            assert_eq!(source.unsettled, 0);
            source.reset_deferred();
            assert!(source
                .test_jacobian_init
                .as_ref()
                .unwrap()
                .factors
                .is_none());
            assert!(source.find_operating_point());
            source.process(0.0);
            assert!(source
                .test_jacobian_init
                .as_ref()
                .unwrap()
                .factors
                .is_some());
            source.set_rate(rate * 2.0);
            source.rebuild();
            assert!(source
                .test_jacobian_init
                .as_ref()
                .unwrap()
                .factors
                .is_none());
        }
    }

    #[test]
    fn jacobian_init_matches_full_reference_at_supported_rates() {
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            for drive in [0.2, 0.6, 1.0] {
                let mut channel = Simulation::new(
                    voice::build_voice(Gain::Twin, voice::Diode::Silicon, voice::Amplifier::Valve)
                        .unwrap(),
                    rate,
                );
                channel.test_jacobian_init = None;
                channel.test_ceres_lm13 = false;
                channel.test_disable_nlsolve_dogleg_trust_region = true;
                channel.set_control(Gain::Twin.drive_control(), drive);
                assert!(channel.find_operating_point());
                let mut realtime = build(rate);
                let mut fused = build(rate);
                fused.test_jacobian_init.as_mut().unwrap().fuse_first_newton = true;
                let mut reference = build(rate);
                reference.test_jacobian_init = None;
                reference.set_backtracks(6);
                reference.set_late_continuation(false);
                reference.nonlinear_partition = None;
                reference.nonlinear_partition_dc = None;
                let mut error_energy = 0.0;
                let mut reference_energy = 0.0;
                for i in 0..4096 {
                    let t = i as f64 / rate;
                    let input = 0.969
                        * 0.5
                        * (t / 0.0005).min(1.0)
                        * (-t / 0.080).exp()
                        * ((std::f64::consts::TAU * 110.0 * t).sin()
                            + 0.5 * (std::f64::consts::TAU * 330.0 * t).sin());
                    let signal = channel.process(input);
                    let got = realtime.process(signal);
                    let fused_got = fused.process(signal);
                    let want = reference.process(signal);
                    assert!(got.is_finite() && fused_got.is_finite() && want.is_finite());
                    assert_eq!(
                        fused_got.to_bits(),
                        got.to_bits(),
                        "fused initializer changed output: rate={rate}, drive={drive}, sample={i}"
                    );
                    error_energy += (got - want).powi(2);
                    reference_energy += want.powi(2);
                }
                let null_db =
                    10.0 * (error_energy.max(1e-300) / reference_energy.max(1e-300)).log10();
                let profile = realtime.solver_control_profile();
                let fused_profile = fused.solver_control_profile();
                eprintln!("Jacobian init reference: rate={rate}, drive={drive}, null={null_db:.1} dB, attempts={}, accepts={}, fused_evals={}", profile.jacobian_init_attempts, profile.jacobian_init_accepts, fused_profile.jacobian_init_evaluations);
                assert!(null_db < -140.0);
                assert_eq!(
                    fused_profile.jacobian_init_attempts,
                    profile.jacobian_init_attempts
                );
                assert_eq!(
                    fused_profile.jacobian_init_accepts,
                    profile.jacobian_init_accepts
                );
                assert!(
                    fused_profile.jacobian_init_evaluations <= profile.jacobian_init_evaluations
                );
                assert_eq!(realtime.unsettled, 0);
                assert_eq!(fused.unsettled, 0);
                assert_eq!(reference.unsettled, 0);
                assert_eq!(realtime.nonfinite, 0);
                assert_eq!(fused.nonfinite, 0);
                assert_eq!(reference.nonfinite, 0);
            }
        }
    }
}
