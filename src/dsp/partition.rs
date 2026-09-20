//! Exact condensation of internal circuit unknowns onto boundary unknowns.

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ReducedSolveProfile {
    pub dense_solve_ns: u64,
    pub recovery_ns: u64,
    pub calls: u64,
}

#[cfg(test)]
impl ReducedSolveProfile {
    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            dense_solve_ns: self.dense_solve_ns.saturating_sub(before.dense_solve_ns),
            recovery_ns: self.recovery_ns.saturating_sub(before.recovery_ns),
            calls: self.calls.saturating_sub(before.calls),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ReducedPivotProfile {
    pub replays: u64,
    pub learns: u64,
    pub invalidations: u64,
}

#[cfg(all(test, target_os = "linux"))]
#[inline(always)]
fn test_thread_cpu_time_ns() -> u64 {
    #[repr(C)]
    struct Timespec {
        tv_sec: std::os::raw::c_long,
        tv_nsec: std::os::raw::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock_id: std::os::raw::c_int, tp: *mut Timespec) -> std::os::raw::c_int;
    }
    const CLOCK_THREAD_CPUTIME_ID: std::os::raw::c_int = 3;
    let mut ts = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let result = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    if result == 0 {
        (ts.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(ts.tv_nsec as u64)
    } else {
        0
    }
}

#[cfg(all(test, not(target_os = "linux")))]
#[inline(always)]
fn test_thread_cpu_time_ns() -> u64 {
    0
}

#[derive(Clone, Debug)]
pub struct Condensed {
    boundary: Vec<usize>,
    internal: Vec<usize>,
    inverse_internal: Vec<f64>,
    boundary_internal: Vec<f64>,
    internal_to_boundary: Vec<f64>,
    full_rhs: Vec<f64>,
}

/// A linear circuit reduced to one boundary voltage.
///
/// This is the Norton form seen by the rest of the signal path: one cached
/// boundary admittance, with all internal nodes recovered after the boundary
/// voltage is solved.
#[derive(Clone, Debug)]
pub struct ReducedLinear {
    condensed: Condensed,
    admittance: f64,
    /// Persistent Gauss-Jordan workspace used by `refresh`.
    ///
    /// `Simulation::rebuild` may run on the audio thread when a linear tone or
    /// cabinet control moves. Keeping this here lets the same condensed
    /// topology be refactored numerically without allocating new selection,
    /// inverse, or multiplication vectors on every rebuild.
    refresh_work: Vec<f64>,
    /// Cached `inv(A_ii) * A_ib` for the one output boundary.
    internal_boundary_response: Vec<f64>,
    /// Packed response columns for only fixed-RHS nodes that can vary at audio
    /// rate. This turns per-sample linear solves into scalar combinations.
    rhs_active_nodes: Vec<usize>,
    rhs_active_inverse: Vec<f64>,
    rhs_active_boundary: Vec<f64>,
    valid: bool,
}

impl ReducedLinear {
    /// Install an already prepared reduction for the same topology/settings.
    /// A dormant channel has the same controls but may not have refreshed its
    /// numeric caches yet. Copying those caches avoids a rebuild on wake-up.
    pub(crate) fn copy_runtime_state_from(&mut self, source: &Self) {
        self.condensed.copy_runtime_state_from(&source.condensed);
        self.admittance = source.admittance;
        self.internal_boundary_response
            .copy_from_slice(&source.internal_boundary_response);
        self.rhs_active_inverse
            .copy_from_slice(&source.rhs_active_inverse);
        self.rhs_active_boundary
            .copy_from_slice(&source.rhs_active_boundary);
        self.valid = source.valid;
        // refresh_work is scratch, fully overwritten.
    }

    pub fn new(matrix: &[f64], rhs: &[f64], output: usize) -> Option<Self> {
        let active: Vec<usize> = (0..rhs.len()).collect();
        Self::new_with_rhs_active(matrix, rhs, output, &active)
    }

    pub fn new_with_rhs_active(
        matrix: &[f64],
        rhs: &[f64],
        output: usize,
        rhs_active: &[usize],
    ) -> Option<Self> {
        let n = rhs.len();
        if output >= n {
            return None;
        }
        let internal: Vec<usize> = (0..n).filter(|&index| index != output).collect();
        let (reduced, _, condensed) = condense(matrix, rhs, &internal, &[output])?;
        let admittance = *reduced.first()?;
        if admittance.abs() < 1e-30 {
            return None;
        }
        let m = condensed.internal.len();
        let mut active_mask = vec![false; n];
        for &node in rhs_active {
            if node < n {
                active_mask[node] = true;
            }
        }
        let rhs_active_nodes: Vec<usize> = condensed
            .internal
            .iter()
            .copied()
            .filter(|&node| active_mask[node])
            .collect();
        let a = rhs_active_nodes.len();
        let mut result = Self {
            condensed,
            admittance,
            refresh_work: vec![0.0; 2 * m * m],
            internal_boundary_response: vec![0.0; m],
            rhs_active_nodes,
            rhs_active_inverse: vec![0.0; m * a],
            rhs_active_boundary: vec![0.0; a],
            valid: true,
        };
        result.update_linear_response_bases();
        Some(result)
    }

    /// Recompute the numeric reduction for the same one-output topology using
    /// only storage allocated by `new`.
    ///
    /// The node partition is wiring, not a parameter. Pot positions and sample
    /// rate can change matrix values, but they do not change which node is the
    /// output or which nodes are internal. Reusing that fixed structure keeps
    /// `Simulation::rebuild` allocation-free for linear circuits while doing
    /// the same Gauss-Jordan inverse and Schur complement as `new`/`condense`.
    pub fn refresh(&mut self, matrix: &[f64], rhs: &[f64], output: usize) -> bool {
        let n = rhs.len();
        if matrix.len() != n * n
            || self.condensed.boundary.len() != 1
            || self.condensed.boundary[0] != output
            || self.condensed.internal.len() + 1 != n
            || self.condensed.full_rhs.len() != n
        {
            self.valid = false;
            return false;
        }

        let m = self.condensed.internal.len();
        let a = self.rhs_active_nodes.len();
        if self.condensed.inverse_internal.len() != m * m
            || self.condensed.boundary_internal.len() != m
            || self.condensed.internal_to_boundary.len() != m
            || self.refresh_work.len() != 2 * m * m
            || self.internal_boundary_response.len() != m
            || self.rhs_active_inverse.len() != m * a
            || self.rhs_active_boundary.len() != a
        {
            self.valid = false;
            return false;
        }

        // Invert A_ii with the same Gauss-Jordan operation order as `inverse`,
        // but in the persistent augmented workspace.
        if m > 0 {
            let stride = 2 * m;
            for (row, &row_node) in self.condensed.internal.iter().enumerate() {
                for (column, &column_node) in self.condensed.internal.iter().enumerate() {
                    self.refresh_work[row * stride + column] = matrix[row_node * n + column_node];
                    self.refresh_work[row * stride + m + column] =
                        if row == column { 1.0 } else { 0.0 };
                }
            }

            for column in 0..m {
                // Use the same `max_by` pivot choice as `inverse`, including
                // its tie behaviour, so a refresh walks the identical numeric
                // path a freshly constructed reduction would.
                let pivot = (column..m)
                    .max_by(|&a, &b| {
                        self.refresh_work[a * stride + column]
                            .abs()
                            .total_cmp(&self.refresh_work[b * stride + column].abs())
                    })
                    .unwrap_or(column);
                if self.refresh_work[pivot * stride + column].abs() < 1e-30 {
                    self.valid = false;
                    return false;
                }
                if pivot != column {
                    for index in 0..stride {
                        self.refresh_work
                            .swap(column * stride + index, pivot * stride + index);
                    }
                }
                let scale = self.refresh_work[column * stride + column];
                for index in 0..stride {
                    self.refresh_work[column * stride + index] /= scale;
                }
                for row in 0..m {
                    if row == column {
                        continue;
                    }
                    let factor = self.refresh_work[row * stride + column];
                    for index in 0..stride {
                        let pivot_value = self.refresh_work[column * stride + index];
                        let target = row * stride + index;
                        self.refresh_work[target] -= factor * pivot_value;
                    }
                }
            }

            for row in 0..m {
                for column in 0..m {
                    self.condensed.inverse_internal[row * m + column] =
                        self.refresh_work[row * stride + m + column];
                }
            }
        }

        // A_ib: internal rows to the one boundary/output column.
        for (row, &internal) in self.condensed.internal.iter().enumerate() {
            self.condensed.internal_to_boundary[row] = matrix[internal * n + output];
        }

        // A_bi * inv(A_ii), in the same row/column/k order used by `multiply`.
        for column in 0..m {
            let mut value = 0.0;
            for k in 0..m {
                value += matrix[output * n + self.condensed.internal[k]]
                    * self.condensed.inverse_internal[k * m + column];
            }
            self.condensed.boundary_internal[column] = value;
        }

        // A_bb - A_bi * inv(A_ii) * A_ib. Accumulate the product first and
        // subtract once, matching `multiply` followed by `subtract` in the
        // construction path so refresh and fresh construction have the same
        // floating-point operation order.
        let mut coupling = 0.0;
        for k in 0..m {
            coupling +=
                self.condensed.boundary_internal[k] * self.condensed.internal_to_boundary[k];
        }
        let admittance = matrix[output * n + output] - coupling;

        self.condensed.full_rhs.copy_from_slice(rhs);
        self.admittance = admittance;
        self.update_linear_response_bases();
        self.valid = admittance.abs() >= 1e-30;
        self.valid
    }

    pub fn solve_into(
        &self,
        rhs: &[f64],
        full: &mut [f64],
        reduced: &mut [f64],
        _scratch: &mut [f64],
    ) -> bool {
        if !self.valid
            || rhs.len() != self.condensed.boundary.len() + self.condensed.internal.len()
            || reduced.is_empty()
            || full.len() < rhs.len()
        {
            return false;
        }
        let m = self.condensed.internal.len();
        let a = self.rhs_active_nodes.len();
        if self.rhs_active_inverse.len() != m * a || self.rhs_active_boundary.len() != a {
            return false;
        }

        let mut boundary_rhs = rhs[self.condensed.boundary[0]];
        // `self` is immutable here, so accumulate the internal base directly
        // into `full` at internal nodes rather than a mutable cache field.
        for &node in &self.condensed.internal {
            full[node] = 0.0;
        }
        for active in 0..a {
            let scalar = rhs[self.rhs_active_nodes[active]];
            if scalar == 0.0 {
                continue;
            }
            boundary_rhs -= self.rhs_active_boundary[active] * scalar;
            for row in 0..m {
                let node = self.condensed.internal[row];
                full[node] += self.rhs_active_inverse[row * a + active] * scalar;
            }
        }
        let boundary = boundary_rhs / self.admittance;
        reduced[0] = boundary;
        full[self.condensed.boundary[0]] = boundary;
        for (row, &node) in self.condensed.internal.iter().enumerate() {
            full[node] -= self.internal_boundary_response[row] * boundary;
        }
        true
    }

    fn update_linear_response_bases(&mut self) {
        let m = self.condensed.internal.len();
        let a = self.rhs_active_nodes.len();
        if m == 0 {
            return;
        }
        for row in 0..m {
            let mut value = 0.0;
            for k in 0..m {
                value += self.condensed.inverse_internal[row * m + k]
                    * self.condensed.internal_to_boundary[k];
            }
            self.internal_boundary_response[row] = value;
        }
        for (active, &global) in self.rhs_active_nodes.iter().enumerate() {
            let Some(local) = self
                .condensed
                .internal
                .iter()
                .position(|&node| node == global)
            else {
                continue;
            };
            self.rhs_active_boundary[active] = self.condensed.boundary_internal[local];
            for row in 0..m {
                self.rhs_active_inverse[row * a + active] =
                    self.condensed.inverse_internal[row * m + local];
            }
        }
    }
}

/// Exact Schur reduction for a nonlinear circuit whose changing device stamps
/// are confined to a fixed set of boundary unknowns.
///
/// The passive/internal block is factorised only when the circuit is rebuilt
/// (sample-rate or control change). During a Newton pass the nonlinear devices
/// may change `A_bb` and the boundary right-hand side, but `A_ii`, `A_ib` and
/// `A_bi` are unchanged. The full system
///
/// ```text
/// [ A_bb  A_bi ] [ x_b ] = [ b_b ]
/// [ A_ib  A_ii ] [ x_i ]   [ b_i ]
/// ```
///
/// is therefore solved exactly through
///
/// `S x_b = b_b - A_bi A_ii^-1 b_i`,
/// `S = A_bb - A_bi A_ii^-1 A_ib`,
///
/// followed by recovery of every internal unknown. No component or nonlinear
/// equation is removed; this only eliminates linear unknowns algebraically.
/// All working storage is owned by the partition so a Newton pass allocates
/// nothing.
#[derive(Clone, Debug)]
pub struct ReducedNonlinear {
    condensed: Condensed,
    /// The static Schur term `A_bi * inv(A_ii) * A_ib`.
    coupling: Vec<f64>,
    /// Linear A_bb block cached at rebuild time. Normal Newton passes stamp
    /// nonlinear devices directly into this compact boundary matrix instead of
    /// rebuilding/stamping a full n x n MNA matrix and copying A_bb back out.
    boundary_base: Vec<f64>,
    /// Merit-only linear Schur matrix `A_bb - A_bi inv(A_ii) A_ib`. Newton
    /// keeps its original arithmetic order; only rejected line-search trials
    /// use this precomputed invariant matrix.
    merit_linear: Vec<f64>,
    /// Per-sample constant term of the reduced residual:
    /// `-b_b + A_bi inv(A_ii) b_i`. It is identical for every backtracking
    /// trial in the sample, so compute it once beside the other Schur RHS
    /// caches rather than once per row per trial.
    merit_rhs_base: Vec<f64>,
    /// Global unknown -> compact boundary index. `usize::MAX` means internal.
    node_to_boundary: Vec<usize>,
    /// Full-MNA slots for the boundary-boundary block, in the same row-major
    /// order as `coupling` and `reduced_matrix`. The boundary topology never
    /// changes, so a Newton pass should not recompute `row_node * n + col_node`
    /// for every coefficient it copies into the reduced system.
    boundary_matrix_slots: Vec<usize>,
    /// Newton-pass boundary matrix. Rebuilt from the current `A_bb` minus the
    /// cached static coupling, then factorised in place.
    reduced_matrix: Vec<f64>,
    /// Reduced RHS and, after factorisation, the boundary solution.
    reduced_rhs: Vec<f64>,
    /// Per-sample Schur RHS correction `A_bi * inv(A_ii) * b_i`. The internal
    /// RHS cannot be touched by nonlinear devices, so recomputing this dot
    /// product on every Newton pass is identical repeated work.
    rhs_internal_correction: Vec<f64>,
    /// Per-sample base internal solution `inv(A_ii) * b_i`. The internal RHS
    /// is fixed for an entire Newton solve, so paying this O(i^2) multiply on
    /// every pass is identical repeated work. `prepare_rhs` computes it once.
    internal_rhs_base: Vec<f64>,
    /// Rebuild-time internal response `inv(A_ii) * A_ib`, row-major i x b.
    /// Kept for the test-only legacy recovery path and for direct equivalence
    /// checks against the previous implementation.
    internal_boundary_response: Vec<f64>,
    /// The same response, transposed to boundary-major b x i. Recovery walks
    /// one boundary voltage at a time and updates every independent internal
    /// node contiguously. Each internal node still sees boundary columns in
    /// exactly the same order as the row-major dot product, but the hot inner
    /// loop no longer carries a floating-point dependency between iterations.
    internal_boundary_response_by_boundary: Vec<f64>,
    /// Persistent contiguous recovery workspace. This is scratch only and is
    /// fully overwritten from `internal_rhs_base` on every Newton pass.
    internal_recovery_work: Vec<f64>,
    /// Fixed-RHS contributors that land in the internal block, represented as
    /// precomputed response columns. `prepare_rhs` therefore combines only the
    /// source/reactive histories that can actually be nonzero instead of doing
    /// an i x i inverse mat-vec every audio sample.
    active_rhs_nodes: Vec<usize>,
    active_internal_local: Vec<usize>,
    internal_rhs_responses: Vec<f64>,
    boundary_rhs_responses: Vec<f64>,
    rhs_prepared: bool,
    /// Pivot swap sequence learned on the first reduced solve after rebuild and
    /// replayed thereafter. The reduced topology is fixed and its dominant
    /// passive pivots do not move with audio.
    pivot_plan: Vec<usize>,
    pivot_planned: bool,
    /// Persistent Gauss-Jordan workspace used only when a rebuild refreshes the
    /// passive/internal block numerically.
    refresh_work: Vec<f64>,
    valid: bool,
    #[cfg(test)]
    test_disable_reciprocal_pivots: bool,
    #[cfg(test)]
    test_disable_boundary_major_recovery: bool,
    #[cfg(test)]
    test_disable_fixed_13_dense_solve: bool,
    #[cfg(test)]
    test_disable_precondensed_13_stamp_base: bool,
    #[cfg(test)]
    test_disable_fixed_13_stamped_merit: bool,
    #[cfg(test)]
    test_disable_fixed_13_trial_residual: bool,
    #[cfg(test)]
    test_profile_enabled: bool,
    #[cfg(test)]
    test_profile: ReducedSolveProfile,
    #[cfg(test)]
    test_pivot_profile_enabled: bool,
    #[cfg(test)]
    test_pivot_profile: ReducedPivotProfile,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct TrustRegionModel13 {
    residual: [f64; 13],
    gradient: [f64; 13],
    j_gradient: [f64; 13],
    cauchy: [f64; 13],
    j_cauchy: [f64; 13],
    scale: [f64; 13],
    gradient_norm: f64,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct CeresLmModel13 {
    jacobian: [f64; 169],
    normal: [f64; 169],
    negative_gradient: [f64; 13],
    residual: [f64; 13],
    damping_diagonal: [f64; 13],
    pub(crate) current_merit: f64,
}

#[cfg(test)]
#[inline(always)]
fn trust_weighted_norm_13(vector: &[f64; 13], scale: &[f64; 13]) -> f64 {
    let mut total = 0.0;
    for index in 0..13 {
        let value = vector[index] / scale[index];
        total += value * value;
    }
    total.sqrt()
}

#[inline(always)]
fn recover_boundary_major_in_place(
    work: &mut [f64],
    base: &[f64],
    response_by_boundary: &[f64],
    boundary_solution: &[f64],
) {
    let i = base.len();
    let b = boundary_solution.len();
    debug_assert_eq!(work.len(), i);
    debug_assert_eq!(response_by_boundary.len(), i * b);
    work.copy_from_slice(base);
    for (column, &boundary_voltage) in boundary_solution.iter().enumerate() {
        let response = &response_by_boundary[column * i..(column + 1) * i];
        for (value, &coefficient) in work.iter_mut().zip(response) {
            *value -= coefficient * boundary_voltage;
        }
    }
}

#[inline(always)]
#[allow(
    clippy::needless_range_loop,
    reason = "keep the measured fixed-size kernel and reference operation order"
)]
fn merit_stamped_fixed_13(matrix: &[f64; 169], rhs: &[f64; 13], voltage: &[f64; 13]) -> f64 {
    let mut total = 0.0;
    for row in 0..13 {
        let base = row * 13;
        let mut residual = -rhs[row];
        residual += matrix[base] * voltage[0];
        residual += matrix[base + 1] * voltage[1];
        residual += matrix[base + 2] * voltage[2];
        residual += matrix[base + 3] * voltage[3];
        residual += matrix[base + 4] * voltage[4];
        residual += matrix[base + 5] * voltage[5];
        residual += matrix[base + 6] * voltage[6];
        residual += matrix[base + 7] * voltage[7];
        residual += matrix[base + 8] * voltage[8];
        residual += matrix[base + 9] * voltage[9];
        residual += matrix[base + 10] * voltage[10];
        residual += matrix[base + 11] * voltage[11];
        residual += matrix[base + 12] * voltage[12];
        total += residual * residual;
    }
    total
}

impl ReducedNonlinear {
    /// Copy prepared numeric caches without changing the Schur equations or
    /// allocating/recomputing the reduction. Solve/refresh scratch is fully
    /// overwritten before it is read and does not need synchronization.
    pub(crate) fn copy_runtime_state_from(&mut self, source: &Self) {
        self.condensed.copy_runtime_state_from(&source.condensed);
        self.coupling.copy_from_slice(&source.coupling);
        self.boundary_base.copy_from_slice(&source.boundary_base);
        self.merit_linear.copy_from_slice(&source.merit_linear);
        self.internal_boundary_response
            .copy_from_slice(&source.internal_boundary_response);
        self.internal_boundary_response_by_boundary
            .copy_from_slice(&source.internal_boundary_response_by_boundary);
        self.internal_rhs_responses
            .copy_from_slice(&source.internal_rhs_responses);
        self.boundary_rhs_responses
            .copy_from_slice(&source.boundary_rhs_responses);
        self.pivot_plan.copy_from_slice(&source.pivot_plan);
        self.pivot_planned = source.pivot_planned;
        // This cache belongs to the current sample's RHS and is deliberately
        // rebuilt by `prepare_rhs` before the destination solves audio.
        self.rhs_prepared = false;
        self.valid = source.valid;
    }

    /// Build a reduction for a fixed boundary set. Construction may allocate;
    /// callers should do it while the simulation itself is being constructed,
    /// never from a Newton pass.
    pub fn new(matrix: &[f64], rhs: &[f64], boundary: &[usize]) -> Option<Self> {
        let active_rhs: Vec<usize> = (0..rhs.len()).collect();
        Self::new_with_active_rhs(matrix, rhs, boundary, &active_rhs)
    }

    pub fn new_with_active_rhs(
        matrix: &[f64],
        rhs: &[f64],
        boundary: &[usize],
        active_rhs: &[usize],
    ) -> Option<Self> {
        let n = rhs.len();
        if matrix.len() != n * n || boundary.is_empty() || boundary.len() >= n {
            return None;
        }

        let mut selected = vec![false; n];
        for &node in boundary {
            if node >= n || selected[node] {
                return None;
            }
            selected[node] = true;
        }
        let internal: Vec<usize> = (0..n).filter(|&node| !selected[node]).collect();
        if internal.is_empty() {
            return None;
        }

        let (_, _, condensed) = condense(matrix, rhs, &internal, boundary)?;
        let b = boundary.len();
        let i = internal.len();
        let mut node_to_boundary = vec![usize::MAX; n];
        for (local, &global) in boundary.iter().enumerate() {
            node_to_boundary[global] = local;
        }
        let mut internal_local_of_global = vec![usize::MAX; n];
        for (local, &global) in internal.iter().enumerate() {
            internal_local_of_global[global] = local;
        }
        let mut active_rhs_nodes = Vec::new();
        let mut active_internal_local = Vec::new();
        for &global in active_rhs {
            if global < n {
                let local = internal_local_of_global[global];
                if local != usize::MAX && !active_rhs_nodes.contains(&global) {
                    active_rhs_nodes.push(global);
                    active_internal_local.push(local);
                }
            }
        }
        let active = active_rhs_nodes.len();
        let mut boundary_matrix_slots = Vec::with_capacity(b * b);
        for &row_node in boundary {
            for &column_node in boundary {
                boundary_matrix_slots.push(row_node * n + column_node);
            }
        }
        let mut result = Self {
            condensed,
            coupling: vec![0.0; b * b],
            boundary_base: vec![0.0; b * b],
            merit_linear: vec![0.0; b * b],
            merit_rhs_base: vec![0.0; b],
            node_to_boundary,
            boundary_matrix_slots,
            reduced_matrix: vec![0.0; b * b],
            reduced_rhs: vec![0.0; b],
            rhs_internal_correction: vec![0.0; b],
            internal_rhs_base: vec![0.0; i],
            internal_boundary_response: vec![0.0; i * b],
            internal_boundary_response_by_boundary: vec![0.0; b * i],
            internal_recovery_work: vec![0.0; i],
            active_rhs_nodes,
            active_internal_local,
            internal_rhs_responses: vec![0.0; active * i],
            boundary_rhs_responses: vec![0.0; active * b],
            rhs_prepared: false,
            pivot_plan: (0..b).collect(),
            pivot_planned: false,
            refresh_work: vec![0.0; 2 * i * i],
            valid: true,
            #[cfg(test)]
            test_disable_reciprocal_pivots: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_REDUCED_LU_RECIPROCAL",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_boundary_major_recovery: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_BOUNDARY_MAJOR_RECOVERY",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_fixed_13_dense_solve: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_FIXED_13_DENSE_SOLVE",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_precondensed_13_stamp_base: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_PRECONDENSED_13_STAMP_BASE",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_fixed_13_stamped_merit: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_FIXED_13_STAMPED_MERIT",
            )
            .is_some(),
            #[cfg(test)]
            test_disable_fixed_13_trial_residual: std::env::var_os(
                "GAINSTAGEFX_TEST_DISABLE_FIXED_13_TRIAL_RESIDUAL",
            )
            .is_some(),
            #[cfg(test)]
            test_profile_enabled: false,
            #[cfg(test)]
            test_profile: ReducedSolveProfile::default(),
            #[cfg(test)]
            test_pivot_profile_enabled: false,
            #[cfg(test)]
            test_pivot_profile: ReducedPivotProfile::default(),
        };
        result.update_internal_boundary_response();
        result.update_coupling();
        result.update_boundary_base(matrix);
        result.update_merit_linear();
        result.update_rhs_responses();
        Some(result)
    }

    pub fn boundary_len(&self) -> usize {
        self.condensed.boundary.len()
    }

    #[inline]
    pub fn boundary_nodes(&self) -> &[usize] {
        &self.condensed.boundary
    }

    pub fn internal_len(&self) -> usize {
        self.condensed.internal.len()
    }

    #[inline]
    fn use_reciprocal_pivots(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_reciprocal_pivots
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline]
    fn use_boundary_major_recovery(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_boundary_major_recovery
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[inline]
    fn use_fixed_13_dense_solve(&self) -> bool {
        #[cfg(test)]
        {
            !self.test_disable_fixed_13_dense_solve
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    /// The Twin/American-6L6 nonlinear boundary is exactly 13 unknowns. For
    /// that hot path the fixed Schur subtraction can be moved to rebuild/sample
    /// preparation time: `merit_linear` already contains
    /// `A_bb - A_bi inv(A_ii) A_ib`. Nonlinear devices then stamp the same
    /// equations directly on top. The RHS deliberately keeps the legacy
    /// post-stamp correction order. This changes only matrix floating-point
    /// evaluation order relative to the legacy post-stamp subtraction.
    #[inline]
    fn use_precondensed_13_stamp_base(&self) -> bool {
        if self.condensed.boundary.len() != 13 {
            return false;
        }
        #[cfg(test)]
        {
            !self.test_disable_precondensed_13_stamp_base
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    /// The Twin/American-6L6 line-search current-point merit is evaluated
    /// hundreds of thousands of times per stress trace. The hot boundary is
    /// always exactly 13 unknowns, so avoid repeated indirect boundary lookups
    /// and dynamic-length bounds checks while preserving the generic method's
    /// floating-point operation order exactly.
    #[inline]
    fn use_fixed_13_stamped_merit(&self) -> bool {
        if self.condensed.boundary.len() != 13 {
            return false;
        }
        #[cfg(test)]
        {
            !self.test_disable_fixed_13_stamped_merit
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    #[cfg(test)]
    pub fn set_test_phase_profile(&mut self, enabled: bool) {
        self.test_profile_enabled = enabled && cfg!(target_os = "linux");
        self.test_profile = ReducedSolveProfile::default();
    }

    #[cfg(test)]
    pub fn test_phase_profile(&self) -> ReducedSolveProfile {
        self.test_profile
    }

    #[cfg(test)]
    pub fn set_test_pivot_profile(&mut self, enabled: bool) {
        self.test_pivot_profile_enabled = enabled;
        self.test_pivot_profile = ReducedPivotProfile::default();
    }

    #[cfg(test)]
    pub fn test_pivot_profile(&self) -> ReducedPivotProfile {
        self.test_pivot_profile
    }

    /// Recompute the passive/internal reduction for the same topology without
    /// allocating. This is needed when a pot position or sample rate changes a
    /// linear coefficient in `A_ii`, `A_ib` or `A_bi`.
    pub fn refresh(&mut self, matrix: &[f64], rhs: &[f64]) -> bool {
        let n = rhs.len();
        let b = self.condensed.boundary.len();
        let i = self.condensed.internal.len();
        if matrix.len() != n * n
            || b + i != n
            || self.condensed.inverse_internal.len() != i * i
            || self.condensed.boundary_internal.len() != b * i
            || self.condensed.internal_to_boundary.len() != i * b
            || self.condensed.full_rhs.len() != n
            || self.coupling.len() != b * b
            || self.boundary_base.len() != b * b
            || self.merit_linear.len() != b * b
            || self.merit_rhs_base.len() != b
            || self.node_to_boundary.len() != n
            || self.boundary_matrix_slots.len() != b * b
            || self.reduced_matrix.len() != b * b
            || self.reduced_rhs.len() != b
            || self.rhs_internal_correction.len() != b
            || self.internal_rhs_base.len() != i
            || self.internal_boundary_response.len() != i * b
            || self.internal_boundary_response_by_boundary.len() != b * i
            || self.internal_recovery_work.len() != i
            || self.internal_rhs_responses.len() != self.active_rhs_nodes.len() * i
            || self.boundary_rhs_responses.len() != self.active_rhs_nodes.len() * b
            || self.pivot_plan.len() != b
            || self.refresh_work.len() != 2 * i * i
        {
            self.valid = false;
            return false;
        }

        // Recompute inv(A_ii) in the persistent augmented workspace. The
        // internal block contains only fixed linear parts, so this work happens
        // on rebuild, never once per Newton iteration.
        if i > 0 {
            let stride = 2 * i;
            for (row, &row_node) in self.condensed.internal.iter().enumerate() {
                for (column, &column_node) in self.condensed.internal.iter().enumerate() {
                    self.refresh_work[row * stride + column] = matrix[row_node * n + column_node];
                    self.refresh_work[row * stride + i + column] =
                        if row == column { 1.0 } else { 0.0 };
                }
            }

            for column in 0..i {
                let pivot = (column..i)
                    .max_by(|&a, &b_row| {
                        self.refresh_work[a * stride + column]
                            .abs()
                            .total_cmp(&self.refresh_work[b_row * stride + column].abs())
                    })
                    .unwrap_or(column);
                if self.refresh_work[pivot * stride + column].abs() < 1e-30 {
                    self.valid = false;
                    return false;
                }
                if pivot != column {
                    for index in 0..stride {
                        self.refresh_work
                            .swap(column * stride + index, pivot * stride + index);
                    }
                }
                let scale = self.refresh_work[column * stride + column];
                for index in 0..stride {
                    self.refresh_work[column * stride + index] /= scale;
                }
                for row in 0..i {
                    if row == column {
                        continue;
                    }
                    let factor = self.refresh_work[row * stride + column];
                    for index in 0..stride {
                        let pivot_value = self.refresh_work[column * stride + index];
                        self.refresh_work[row * stride + index] -= factor * pivot_value;
                    }
                }
            }

            for row in 0..i {
                for column in 0..i {
                    self.condensed.inverse_internal[row * i + column] =
                        self.refresh_work[row * stride + i + column];
                }
            }
        }

        // A_ib: internal rows, boundary columns.
        for (row, &internal) in self.condensed.internal.iter().enumerate() {
            for (column, &boundary) in self.condensed.boundary.iter().enumerate() {
                self.condensed.internal_to_boundary[row * b + column] =
                    matrix[internal * n + boundary];
            }
        }

        // A_bi * inv(A_ii).
        for (row, &boundary) in self.condensed.boundary.iter().enumerate() {
            for column in 0..i {
                let mut value = 0.0;
                for k in 0..i {
                    value += matrix[boundary * n + self.condensed.internal[k]]
                        * self.condensed.inverse_internal[k * i + column];
                }
                self.condensed.boundary_internal[row * i + column] = value;
            }
        }

        self.condensed.full_rhs.copy_from_slice(rhs);
        self.update_internal_boundary_response();
        self.update_coupling();
        self.update_boundary_base(matrix);
        self.update_merit_linear();
        self.update_rhs_responses();
        self.rhs_prepared = false;
        self.pivot_planned = false;
        self.valid = true;
        true
    }

    /// Cache the passive/internal contribution to the Schur RHS once per sample.
    /// Only structurally-active fixed RHS positions are visited; each has a
    /// rebuild-time response column for both internal recovery and boundary RHS.
    pub fn prepare_rhs(&mut self, rhs: &[f64]) -> bool {
        if !self.valid || rhs.len() != self.node_to_boundary.len() {
            self.rhs_prepared = false;
            return false;
        }
        self.internal_rhs_base.fill(0.0);
        self.rhs_internal_correction.fill(0.0);
        let i = self.condensed.internal.len();
        let b = self.condensed.boundary.len();
        for (column, &global) in self.active_rhs_nodes.iter().enumerate() {
            let scalar = rhs[global];
            if scalar == 0.0 {
                continue;
            }
            let internal = &self.internal_rhs_responses[column * i..(column + 1) * i];
            for (target, &response) in self.internal_rhs_base.iter_mut().zip(internal) {
                *target += response * scalar;
            }
            let boundary = &self.boundary_rhs_responses[column * b..(column + 1) * b];
            for (target, &response) in self.rhs_internal_correction.iter_mut().zip(boundary) {
                *target += response * scalar;
            }
        }
        for ((value, &global), &correction) in self
            .merit_rhs_base
            .iter_mut()
            .zip(&self.condensed.boundary)
            .zip(&self.rhs_internal_correction)
        {
            *value = -rhs[global] + correction;
        }
        self.rhs_prepared = true;
        true
    }

    /// Reset compact boundary storage to the immutable linear A_bb and this
    /// sample's fixed boundary RHS. Nonlinear devices may stamp directly into
    /// the returned buffers using `node_to_boundary`.
    pub fn begin_stamp<'a>(
        &'a mut self,
        fixed_rhs: &[f64],
    ) -> Option<(&'a mut [f64], &'a mut [f64], &'a [usize])> {
        if !self.valid || !self.rhs_prepared || fixed_rhs.len() != self.node_to_boundary.len() {
            return None;
        }
        if self.use_precondensed_13_stamp_base() {
            // `merit_linear` is the exact fixed Schur matrix cached at rebuild
            // time. Starting here removes the otherwise repeated 13x13 coupling
            // subtraction after every nonlinear stamp. Keep RHS preparation in
            // its legacy order so this optimization is matrix-only.
            self.reduced_matrix.copy_from_slice(&self.merit_linear);
        } else {
            self.reduced_matrix.copy_from_slice(&self.boundary_base);
        }
        for (value, &global) in self.reduced_rhs.iter_mut().zip(&self.condensed.boundary) {
            *value = fixed_rhs[global];
        }
        let Self {
            reduced_matrix,
            reduced_rhs,
            node_to_boundary,
            ..
        } = self;
        Some((reduced_matrix, reduced_rhs, node_to_boundary))
    }

    /// Build the linear part of the exact Schur residual at a trial point.
    /// Nonlinear devices then add their physical current/constraint equations
    /// directly to this compact vector. No Jacobian is needed because the
    /// line-search candidate is judged but never solved.
    #[inline]
    #[allow(
        clippy::needless_range_loop,
        reason = "keep the measured residual kernel and reference operation order"
    )]
    pub fn begin_residual<'a>(
        &'a self,
        fixed_rhs: &[f64],
        full: &[f64],
        residual: &mut [f64],
    ) -> Option<&'a [usize]> {
        if !self.valid
            || !self.rhs_prepared
            || fixed_rhs.len() != self.node_to_boundary.len()
            || full.len() != self.node_to_boundary.len()
            || residual.len() < self.condensed.boundary.len()
        {
            return None;
        }
        let b = self.condensed.boundary.len();
        let fixed_trial_residual = {
            #[cfg(test)]
            {
                !self.test_disable_fixed_13_trial_residual
            }
            #[cfg(not(test))]
            {
                true
            }
        };
        if b == 13 && fixed_trial_residual {
            // Each difficult block can evaluate hundreds of trial residuals.
            // Gather once instead of chasing the boundary map in every row.
            // Keep the generic row/column sum order: even a reassociation here
            // can change line-search acceptance near a conduction boundary.
            let voltage: [f64; 13] =
                std::array::from_fn(|column| full[self.condensed.boundary[column]]);
            let matrix: &[f64; 169] = self.merit_linear.as_slice().try_into().ok()?;
            let base: &[f64; 13] = self.merit_rhs_base.as_slice().try_into().ok()?;
            let result: &mut [f64; 13] = (&mut residual[..13]).try_into().ok()?;
            for row in 0..13 {
                let mut value = base[row];
                for column in 0..13 {
                    value += matrix[row * 13 + column] * voltage[column];
                }
                result[row] = value;
            }
            return Some(&self.node_to_boundary);
        }
        for row in 0..b {
            let mut value = self.merit_rhs_base[row];
            let coefficients = &self.merit_linear[row * b..(row + 1) * b];
            for (&coefficient, &global_column) in coefficients.iter().zip(&self.condensed.boundary)
            {
                value += coefficient * full[global_column];
            }
            residual[row] = value;
        }
        Some(&self.node_to_boundary)
    }

    #[inline]
    pub fn residual_merit(&self, residual: &[f64]) -> Option<f64> {
        let b = self.condensed.boundary.len();
        if residual.len() < b {
            return None;
        }
        let mut total = 0.0;
        for &value in &residual[..b] {
            total += value * value;
        }
        Some(total)
    }

    /// Complete the Schur system after direct nonlinear device stamping.
    pub fn finish_stamp(&mut self) {
        if !self.use_precondensed_13_stamp_base() {
            for (value, &coupling) in self.reduced_matrix.iter_mut().zip(&self.coupling) {
                *value -= coupling;
            }
        }
        // Deliberately retain the legacy RHS arithmetic order even on the
        // precondensed matrix path.
        for (value, &correction) in self
            .reduced_rhs
            .iter_mut()
            .zip(&self.rhs_internal_correction)
        {
            *value -= correction;
        }
    }

    /// Exact reduced residual before `finish_stamp` materialises the fixed
    /// Schur coupling into the trial matrix. Line-search trials never solve
    /// this matrix, so writing every `A_bb - A_bi inv(A_ii) A_ib` coefficient
    /// only to read it once is avoidable hot-path work.
    ///
    /// The subtraction order is deliberately identical to `finish_stamp`
    /// followed by `merit_stamped`: one f64 subtraction for each coefficient
    /// and RHS entry, then the same row/column accumulation order.
    #[inline]
    pub fn merit_unfinished_stamp(&self, full: &[f64]) -> f64 {
        if self.use_precondensed_13_stamp_base() {
            let b = self.condensed.boundary.len();
            let mut total = 0.0;
            for row in 0..b {
                let reduced_rhs = self.reduced_rhs[row] - self.rhs_internal_correction[row];
                let mut residual = -reduced_rhs;
                for (column, &global) in self.condensed.boundary.iter().enumerate() {
                    residual += self.reduced_matrix[row * b + column] * full[global];
                }
                total += residual * residual;
            }
            return total;
        }
        let b = self.condensed.boundary.len();
        let mut total = 0.0;
        for row in 0..b {
            let reduced_rhs = self.reduced_rhs[row] - self.rhs_internal_correction[row];
            let mut residual = -reduced_rhs;
            for (column, &global) in self.condensed.boundary.iter().enumerate() {
                let slot = row * b + column;
                let coefficient = self.reduced_matrix[slot] - self.coupling[slot];
                residual += coefficient * full[global];
            }
            total += residual * residual;
        }
        total
    }

    /// Exact reduced residual at a full-vector candidate. Internal linear rows
    /// are identically zero on the segment between recovered Newton iterates, so
    /// this is the same merit used by the full MNA line search without touching
    /// the passive/internal matrix.
    #[inline]
    pub fn merit_stamped(&self, full: &[f64]) -> f64 {
        let b = self.condensed.boundary.len();
        if b == 13 && self.use_fixed_13_stamped_merit() {
            let boundary: &[usize; 13] = self
                .condensed
                .boundary
                .as_slice()
                .try_into()
                .expect("13-node boundary length checked above");
            let matrix: &[f64; 169] = self
                .reduced_matrix
                .as_slice()
                .try_into()
                .expect("13x13 reduced matrix");
            let rhs: &[f64; 13] = self
                .reduced_rhs
                .as_slice()
                .try_into()
                .expect("13-entry reduced RHS");
            let boundary_voltage = [
                full[boundary[0]],
                full[boundary[1]],
                full[boundary[2]],
                full[boundary[3]],
                full[boundary[4]],
                full[boundary[5]],
                full[boundary[6]],
                full[boundary[7]],
                full[boundary[8]],
                full[boundary[9]],
                full[boundary[10]],
                full[boundary[11]],
                full[boundary[12]],
            ];
            return merit_stamped_fixed_13(matrix, rhs, &boundary_voltage);
        }

        let mut total = 0.0;
        for row in 0..b {
            let mut residual = -self.reduced_rhs[row];
            for (column, &global) in self.condensed.boundary.iter().enumerate() {
                residual += self.reduced_matrix[row * b + column] * full[global];
            }
            total += residual * residual;
        }
        total
    }

    /// Test-only Ceres-style Levenberg-Marquardt model for the fixed 13-node
    /// Twin boundary. The unfactorised reduced Jacobian is captured once, then
    /// J^T J, -J^T F, and the squared-column-norm damping diagonal are cached so
    /// rejected LM steps only need another tiny fixed-size linear solve.
    #[cfg(test)]
    pub(crate) fn ceres_lm_model_13(&self, current: &[f64]) -> Option<CeresLmModel13> {
        const MIN_DIAGONAL: f64 = 1.0e-6;
        const MAX_DIAGONAL: f64 = 1.0e32;

        if !self.valid || !self.rhs_prepared || self.condensed.boundary.len() != 13 {
            return None;
        }
        let boundary: &[usize; 13] = self.condensed.boundary.as_slice().try_into().ok()?;
        if current.len() < self.node_to_boundary.len() {
            return None;
        }
        let jacobian: &[f64; 169] = self.reduced_matrix.as_slice().try_into().ok()?;
        let rhs: &[f64; 13] = self.reduced_rhs.as_slice().try_into().ok()?;

        let mut x = [0.0; 13];
        for index in 0..13 {
            x[index] = current[boundary[index]];
            if !x[index].is_finite() {
                return None;
            }
        }

        let mut residual = [0.0; 13];
        let mut current_merit = 0.0;
        for row in 0..13 {
            let base = row * 13;
            let mut value = -rhs[row];
            for column in 0..13 {
                value += jacobian[base + column] * x[column];
            }
            if !value.is_finite() {
                return None;
            }
            residual[row] = value;
            current_merit += value * value;
        }
        if !current_merit.is_finite() {
            return None;
        }

        let mut normal = [0.0; 169];
        let mut negative_gradient = [0.0; 13];
        let mut damping_diagonal = [0.0; 13];
        for column in 0..13 {
            let mut gradient = 0.0;
            let mut column_norm_sq = 0.0;
            for row in 0..13 {
                let j = jacobian[row * 13 + column];
                gradient += j * residual[row];
                column_norm_sq += j * j;
            }
            if !gradient.is_finite() || !column_norm_sq.is_finite() {
                return None;
            }
            negative_gradient[column] = -gradient;
            damping_diagonal[column] = column_norm_sq.clamp(MIN_DIAGONAL, MAX_DIAGONAL);
        }

        for row in 0..13 {
            for column in row..13 {
                let mut value = 0.0;
                for k in 0..13 {
                    value += jacobian[k * 13 + row] * jacobian[k * 13 + column];
                }
                if !value.is_finite() {
                    return None;
                }
                normal[row * 13 + column] = value;
                normal[column * 13 + row] = value;
            }
        }

        Some(CeresLmModel13 {
            jacobian: *jacobian,
            normal,
            negative_gradient,
            residual,
            damping_diagonal,
            current_merit,
        })
    }

    /// Construct one Ceres-style LM candidate while leaving the captured local
    /// model untouched for a possible rejected-step retry. `radius` follows
    /// Ceres's convention: the effective diagonal is D^2 / radius. Returns the
    /// local model merit ||F + Jp||^2 used for the actual/predicted reduction
    /// ratio. No heap allocation and no nonlinear evaluation occur here.
    #[cfg(test)]
    pub(crate) fn ceres_lm_trial_point_13(
        &self,
        model: &CeresLmModel13,
        current: &[f64],
        radius: f64,
        point: &mut [f64],
    ) -> Option<f64> {
        if self.condensed.boundary.len() != 13
            || current.len() < self.node_to_boundary.len()
            || point.len() < self.node_to_boundary.len()
            || !radius.is_finite()
            || radius <= 0.0
        {
            return None;
        }
        let boundary: &[usize; 13] = self.condensed.boundary.as_slice().try_into().ok()?;

        let mut matrix = model.normal;
        let mut step = model.negative_gradient;
        for index in 0..13 {
            matrix[index * 13 + index] += model.damping_diagonal[index] / radius;
            if !matrix[index * 13 + index].is_finite() {
                return None;
            }
        }
        let mut pivot_plan = [0usize; 13];
        let mut pivot_planned = false;
        if !solve_dense_planned_fixed::<13>(
            &mut matrix,
            &mut step,
            &mut pivot_plan,
            &mut pivot_planned,
            true,
        ) {
            return None;
        }

        let mut predicted_merit = 0.0;
        for row in 0..13 {
            let base = row * 13;
            let mut predicted = model.residual[row];
            for (column, &delta) in step.iter().enumerate() {
                predicted += model.jacobian[base + column] * delta;
            }
            if !predicted.is_finite() {
                return None;
            }
            predicted_merit += predicted * predicted;
        }
        if !predicted_merit.is_finite() {
            return None;
        }

        for index in 0..13 {
            let value = current[boundary[index]] + step[index];
            if !value.is_finite() {
                return None;
            }
            point[boundary[index]] = value;
        }
        Some(predicted_merit)
    }

    /// Test-only NLsolve-style dogleg model for the fixed 13-node Twin boundary.
    ///
    /// This is captured before the reduced matrix is factorised. `residual` is
    /// the exact linear-model residual J*x-b at the current iterate. The
    /// scaled steepest-descent/Cauchy direction follows NLsolve's dogleg
    /// formulation, using the same per-unknown absolute+relative scale as the
    /// production convergence test. No circuit equation or Jacobian entry is
    /// changed; this only prepares an alternate globalization direction.
    #[cfg(test)]
    pub(crate) fn trust_region_model_13(
        &self,
        current: &[f64],
        tolerance: f64,
        relative: f64,
    ) -> Option<TrustRegionModel13> {
        if !self.valid || !self.rhs_prepared || self.condensed.boundary.len() != 13 {
            return None;
        }
        let boundary: &[usize; 13] = self.condensed.boundary.as_slice().try_into().ok()?;
        if current.len() < self.node_to_boundary.len() {
            return None;
        }
        let matrix: &[f64; 169] = self.reduced_matrix.as_slice().try_into().ok()?;
        let rhs: &[f64; 13] = self.reduced_rhs.as_slice().try_into().ok()?;

        let mut x = [0.0; 13];
        let mut scale = [0.0; 13];
        for index in 0..13 {
            let value = current[boundary[index]];
            x[index] = value;
            let local_scale = tolerance + relative * value.abs();
            if !value.is_finite() || !local_scale.is_finite() || local_scale <= 0.0 {
                return None;
            }
            scale[index] = local_scale;
        }

        let mut residual = [0.0; 13];
        for row in 0..13 {
            let base = row * 13;
            let mut value = -rhs[row];
            for column in 0..13 {
                value += matrix[base + column] * x[column];
            }
            if !value.is_finite() {
                return None;
            }
            residual[row] = value;
        }

        // NLsolve's scaled dogleg computes g = J' r ./ d^2 while measuring
        // steps with ||d .* p||. Here d = 1/(VNTOL + RELTOL*|x|), matching
        // the solver's existing notion of a normalized node correction.
        let mut gradient = [0.0; 13];
        for column in 0..13 {
            let mut value = 0.0;
            for row in 0..13 {
                value += matrix[row * 13 + column] * residual[row];
            }
            value *= scale[column] * scale[column];
            if !value.is_finite() {
                return None;
            }
            gradient[column] = value;
        }
        let gradient_norm = trust_weighted_norm_13(&gradient, &scale);
        if !gradient_norm.is_finite() || gradient_norm <= f64::MIN_POSITIVE {
            return None;
        }

        let mut j_gradient = [0.0; 13];
        let mut j_gradient_norm_sq = 0.0;
        for (row, entry) in j_gradient.iter_mut().enumerate() {
            let base = row * 13;
            let mut value = 0.0;
            for column in 0..13 {
                value += matrix[base + column] * gradient[column];
            }
            if !value.is_finite() {
                return None;
            }
            *entry = value;
            j_gradient_norm_sq += value * value;
        }
        if !j_gradient_norm_sq.is_finite() || j_gradient_norm_sq <= f64::MIN_POSITIVE {
            return None;
        }

        let alpha = (gradient_norm * gradient_norm) / j_gradient_norm_sq;
        if !alpha.is_finite() || alpha <= 0.0 {
            return None;
        }
        let mut cauchy = [0.0; 13];
        let mut j_cauchy = [0.0; 13];
        for index in 0..13 {
            cauchy[index] = -alpha * gradient[index];
            j_cauchy[index] = -alpha * j_gradient[index];
        }

        Some(TrustRegionModel13 {
            residual,
            gradient,
            j_gradient,
            cauchy,
            j_cauchy,
            scale,
            gradient_norm,
        })
    }

    /// Build one NLsolve-style dogleg candidate into `point`'s boundary
    /// entries. The full Newton correction is supplied by the ordinary exact
    /// reduced solve; the model captured above supplies the Cauchy leg.
    /// Returns `(step_norm / newton_norm, predicted_merit)`.
    #[cfg(test)]
    pub(crate) fn trust_region_trial_point_13(
        &self,
        model: &TrustRegionModel13,
        current: &[f64],
        newton_delta: &[f64],
        radius: f64,
        point: &mut [f64],
    ) -> Option<(f64, f64)> {
        if self.condensed.boundary.len() != 13
            || current.len() < self.node_to_boundary.len()
            || newton_delta.len() < self.node_to_boundary.len()
            || point.len() < self.node_to_boundary.len()
            || !radius.is_finite()
            || radius <= 0.0
        {
            return None;
        }
        let boundary: &[usize; 13] = self.condensed.boundary.as_slice().try_into().ok()?;
        let mut newton = [0.0; 13];
        for index in 0..13 {
            newton[index] = newton_delta[boundary[index]];
            if !newton[index].is_finite() {
                return None;
            }
        }
        let newton_norm = trust_weighted_norm_13(&newton, &model.scale);
        if !newton_norm.is_finite() || newton_norm <= f64::MIN_POSITIVE {
            return None;
        }
        let radius = radius.min(newton_norm);

        let mut step = [0.0; 13];
        let mut j_step = [0.0; 13];
        if newton_norm <= radius * (1.0 + 8.0 * f64::EPSILON) {
            step = newton;
            for (index, entry) in j_step.iter_mut().enumerate() {
                // For the Newton correction J*p = -F in the captured linear
                // model. Using that identity avoids retaining/copying 169
                // Jacobian coefficients after factorisation.
                *entry = -model.residual[index];
            }
        } else {
            let cauchy_norm = trust_weighted_norm_13(&model.cauchy, &model.scale);
            if !cauchy_norm.is_finite() {
                return None;
            }
            if cauchy_norm >= radius {
                let factor = radius / model.gradient_norm;
                for index in 0..13 {
                    step[index] = -factor * model.gradient[index];
                    j_step[index] = -factor * model.j_gradient[index];
                }
            } else {
                let mut diff = [0.0; 13];
                let mut a = 0.0;
                let mut b = 0.0;
                for index in 0..13 {
                    diff[index] = newton[index] - model.cauchy[index];
                    let scaled_diff = diff[index] / model.scale[index];
                    let scaled_cauchy = model.cauchy[index] / model.scale[index];
                    a += scaled_diff * scaled_diff;
                    b += 2.0 * scaled_cauchy * scaled_diff;
                }
                let c = cauchy_norm * cauchy_norm - radius * radius;
                let discriminant = b * b - 4.0 * a * c;
                if !a.is_finite()
                    || a <= f64::MIN_POSITIVE
                    || !discriminant.is_finite()
                    || discriminant < 0.0
                {
                    return None;
                }
                let tau = (-b + discriminant.sqrt()) / (2.0 * a);
                if !tau.is_finite() || !(0.0..=1.0 + 8.0 * f64::EPSILON).contains(&tau) {
                    return None;
                }
                let tau = tau.clamp(0.0, 1.0);
                for index in 0..13 {
                    step[index] = model.cauchy[index] + tau * diff[index];
                    let j_diff = -model.residual[index] - model.j_cauchy[index];
                    j_step[index] = model.j_cauchy[index] + tau * j_diff;
                }
            }
        }

        let step_norm = trust_weighted_norm_13(&step, &model.scale);
        if !step_norm.is_finite() {
            return None;
        }
        let mut predicted_merit = 0.0;
        for index in 0..13 {
            let predicted = model.residual[index] + j_step[index];
            if !predicted.is_finite() {
                return None;
            }
            predicted_merit += predicted * predicted;
            let value = current[boundary[index]] + step[index];
            if !value.is_finite() {
                return None;
            }
            point[boundary[index]] = value;
        }
        if !predicted_merit.is_finite() {
            return None;
        }
        Some(((step_norm / newton_norm).clamp(0.0, 1.0), predicted_merit))
    }

    #[cfg(test)]
    pub(crate) fn trust_region_newton_norm_13(
        &self,
        model: &TrustRegionModel13,
        newton_delta: &[f64],
    ) -> Option<f64> {
        if self.condensed.boundary.len() != 13 || newton_delta.len() < self.node_to_boundary.len() {
            return None;
        }
        let boundary: &[usize; 13] = self.condensed.boundary.as_slice().try_into().ok()?;
        let mut newton = [0.0; 13];
        for index in 0..13 {
            newton[index] = newton_delta[boundary[index]];
        }
        let norm = trust_weighted_norm_13(&newton, &model.scale);
        norm.is_finite().then_some(norm)
    }

    /// Recover eliminated internal nodes for an arbitrary accepted boundary
    /// point. Ordinary line search never needs this because its trial remains
    /// on the full recovered Newton ray; dogleg and LM change the boundary
    /// direction and therefore must reapply the exact cached Schur recovery once.
    #[cfg(test)]
    pub(crate) fn trust_region_recover_internal_13(&mut self, full: &mut [f64]) -> bool {
        if !self.valid
            || !self.rhs_prepared
            || self.condensed.boundary.len() != 13
            || full.len() < self.node_to_boundary.len()
        {
            return false;
        }
        let boundary: &[usize; 13] = match self.condensed.boundary.as_slice().try_into() {
            Ok(boundary) => boundary,
            Err(_) => return false,
        };
        let mut boundary_solution = [0.0; 13];
        for index in 0..13 {
            boundary_solution[index] = full[boundary[index]];
            if !boundary_solution[index].is_finite() {
                return false;
            }
        }
        let boundary_major = self.use_boundary_major_recovery();
        if boundary_major {
            recover_boundary_major_in_place(
                &mut self.internal_recovery_work,
                &self.internal_rhs_base,
                &self.internal_boundary_response_by_boundary,
                &boundary_solution,
            );
            for (row, &node) in self.condensed.internal.iter().enumerate() {
                let value = self.internal_recovery_work[row];
                if !value.is_finite() {
                    return false;
                }
                full[node] = value;
            }
        } else {
            let b = 13;
            for (row, &node) in self.condensed.internal.iter().enumerate() {
                let response = &self.internal_boundary_response[row * b..(row + 1) * b];
                let mut value = self.internal_rhs_base[row];
                for (&coefficient, &boundary_voltage) in response.iter().zip(&boundary_solution) {
                    value -= coefficient * boundary_voltage;
                }
                if !value.is_finite() {
                    return false;
                }
                full[node] = value;
            }
        }
        true
    }

    /// Solve a directly-stamped Schur boundary and recover every full unknown,
    /// fusing recovery with Newton delta/convergence measurement so the full
    /// vector is not walked a second time immediately afterwards.
    pub fn solve_stamped_with_delta(
        &mut self,
        full: &mut [f64],
        current: &[f64],
        delta: &mut [f64],
        tolerance: f64,
        relative: f64,
    ) -> Option<f64> {
        if !self.valid || !self.rhs_prepared || full.len() < self.node_to_boundary.len() {
            return None;
        }
        let b = self.condensed.boundary.len();
        #[cfg(test)]
        let solve_started = self.test_profile_enabled.then(test_thread_cpu_time_ns);
        let reciprocal_pivots = self.use_reciprocal_pivots();
        #[cfg(test)]
        let pivot_was_planned = self.pivot_planned;
        let solved = if b == 13 && self.use_fixed_13_dense_solve() {
            solve_dense_planned_fixed::<13>(
                &mut self.reduced_matrix,
                &mut self.reduced_rhs,
                &mut self.pivot_plan,
                &mut self.pivot_planned,
                reciprocal_pivots,
            )
        } else {
            solve_dense_planned(
                &mut self.reduced_matrix,
                &mut self.reduced_rhs,
                b,
                &mut self.pivot_plan,
                &mut self.pivot_planned,
                reciprocal_pivots,
            )
        };
        #[cfg(test)]
        if let Some(started) = solve_started {
            self.test_profile.dense_solve_ns = self
                .test_profile
                .dense_solve_ns
                .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
            self.test_profile.calls = self.test_profile.calls.saturating_add(1);
        }
        #[cfg(test)]
        if self.test_pivot_profile_enabled {
            if pivot_was_planned {
                self.test_pivot_profile.replays = self.test_pivot_profile.replays.saturating_add(1);
                if !self.pivot_planned {
                    self.test_pivot_profile.invalidations =
                        self.test_pivot_profile.invalidations.saturating_add(1);
                }
            } else if self.pivot_planned {
                self.test_pivot_profile.learns = self.test_pivot_profile.learns.saturating_add(1);
            }
        }
        if !solved {
            return None;
        }
        #[cfg(test)]
        let recovery_started = self.test_profile_enabled.then(test_thread_cpu_time_ns);
        let mut moved = 0.0f64;
        for (index, &node) in self.condensed.boundary.iter().enumerate() {
            let value = self.reduced_rhs[index];
            if !value.is_finite() {
                return None;
            }
            full[node] = value;
            let d = value - current[node];
            delta[node] = d;
            moved = moved.max(d.abs() / (tolerance + relative * current[node].abs()));
        }
        if self.use_boundary_major_recovery() {
            recover_boundary_major_in_place(
                &mut self.internal_recovery_work,
                &self.internal_rhs_base,
                &self.internal_boundary_response_by_boundary,
                &self.reduced_rhs,
            );
            for (row, &node) in self.condensed.internal.iter().enumerate() {
                let value = self.internal_recovery_work[row];
                if !value.is_finite() {
                    return None;
                }
                full[node] = value;
                let d = value - current[node];
                delta[node] = d;
                moved = moved.max(d.abs() / (tolerance + relative * current[node].abs()));
            }
        } else {
            for (row, &node) in self.condensed.internal.iter().enumerate() {
                let response = &self.internal_boundary_response[row * b..(row + 1) * b];
                let mut value = self.internal_rhs_base[row];
                for (&coefficient, &boundary_voltage) in response.iter().zip(&self.reduced_rhs) {
                    value -= coefficient * boundary_voltage;
                }
                if !value.is_finite() {
                    return None;
                }
                full[node] = value;
                let d = value - current[node];
                delta[node] = d;
                moved = moved.max(d.abs() / (tolerance + relative * current[node].abs()));
            }
        }
        #[cfg(test)]
        if let Some(started) = recovery_started {
            self.test_profile.recovery_ns = self
                .test_profile
                .recovery_ns
                .saturating_add(test_thread_cpu_time_ns().saturating_sub(started));
        }
        Some(moved)
    }

    /// Solve the current nonlinear Jacobian into the full unknown vector.
    ///
    /// Safety of the reduction depends on the caller's boundary proof: device
    /// stamps may alter only boundary rows/columns. `Simulation` constructs the
    /// boundary from every nonlinear part terminal plus every MNA branch row,
    /// so the remaining blocks are purely linear and fixed between rebuilds.
    pub fn solve_into(&mut self, matrix: &[f64], rhs: &[f64], full: &mut [f64]) -> bool {
        if !self.valid || !self.rhs_prepared {
            return false;
        }
        let n = rhs.len();
        let b = self.condensed.boundary.len();
        if matrix.len() != n * n || full.len() < n || b == 0 {
            return false;
        }

        // Only A_bb changes with the Newton linearisation. The expensive Schur
        // contribution from the internal network is cached at rebuild time.
        // Its full-MNA slots are topology-only, so walk the precomputed slot
        // list instead of rebuilding row/column addresses every Newton pass.
        for ((value, &coupling), &slot) in self
            .reduced_matrix
            .iter_mut()
            .zip(&self.coupling)
            .zip(&self.boundary_matrix_slots)
        {
            *value = matrix[slot] - coupling;
        }
        for ((value, &boundary), &correction) in self
            .reduced_rhs
            .iter_mut()
            .zip(&self.condensed.boundary)
            .zip(&self.rhs_internal_correction)
        {
            *value = rhs[boundary] - correction;
        }
        let reciprocal_pivots = self.use_reciprocal_pivots();
        if !solve_dense_planned(
            &mut self.reduced_matrix,
            &mut self.reduced_rhs,
            b,
            &mut self.pivot_plan,
            &mut self.pivot_planned,
            reciprocal_pivots,
        ) {
            return false;
        }
        // Fill the complete unknown vector because the existing convergence
        // test and device evaluation deliberately still see every node on every
        // pass. Only the algebra used to recover passive internals changed:
        // the O(i^2) inverse/RHS multiply was hoisted to `prepare_rhs`, leaving
        // this O(i*b) boundary response in the Newton hot loop.
        for (index, &node) in self.condensed.boundary.iter().enumerate() {
            full[node] = self.reduced_rhs[index];
        }
        if self.use_boundary_major_recovery() {
            recover_boundary_major_in_place(
                &mut self.internal_recovery_work,
                &self.internal_rhs_base,
                &self.internal_boundary_response_by_boundary,
                &self.reduced_rhs,
            );
            for (row, &node) in self.condensed.internal.iter().enumerate() {
                full[node] = self.internal_recovery_work[row];
            }
        } else {
            for (row, &node) in self.condensed.internal.iter().enumerate() {
                let response = &self.internal_boundary_response[row * b..(row + 1) * b];
                let mut value = self.internal_rhs_base[row];
                for (&coefficient, &boundary_voltage) in response.iter().zip(&self.reduced_rhs) {
                    value -= coefficient * boundary_voltage;
                }
                full[node] = value;
            }
        }
        true
    }

    fn update_boundary_base(&mut self, matrix: &[f64]) {
        for (value, &slot) in self
            .boundary_base
            .iter_mut()
            .zip(&self.boundary_matrix_slots)
        {
            *value = matrix[slot];
        }
    }

    fn update_merit_linear(&mut self) {
        for ((target, &base), &coupling) in self
            .merit_linear
            .iter_mut()
            .zip(&self.boundary_base)
            .zip(&self.coupling)
        {
            *target = base - coupling;
        }
    }

    fn update_rhs_responses(&mut self) {
        let i = self.condensed.internal.len();
        let b = self.condensed.boundary.len();
        for (column, &local) in self.active_internal_local.iter().enumerate() {
            for row in 0..i {
                self.internal_rhs_responses[column * i + row] =
                    self.condensed.inverse_internal[row * i + local];
            }
            for row in 0..b {
                self.boundary_rhs_responses[column * b + row] =
                    self.condensed.boundary_internal[row * i + local];
            }
        }
    }

    /// Cache `inv(A_ii) * A_ib` after construction/rebuild. This matrix is
    /// independent of Newton's changing boundary stamps and of the per-sample
    /// reactive/source RHS, so there is no reason to form it in the audio hot
    /// loop.
    fn update_internal_boundary_response(&mut self) {
        let b = self.condensed.boundary.len();
        let i = self.condensed.internal.len();
        for row in 0..i {
            for column in 0..b {
                let mut value = 0.0;
                for k in 0..i {
                    value += self.condensed.inverse_internal[row * i + k]
                        * self.condensed.internal_to_boundary[k * b + column];
                }
                self.internal_boundary_response[row * b + column] = value;
                self.internal_boundary_response_by_boundary[column * i + row] = value;
            }
        }
    }

    fn update_coupling(&mut self) {
        let b = self.condensed.boundary.len();
        let i = self.condensed.internal.len();
        for row in 0..b {
            for column in 0..b {
                let mut value = 0.0;
                for k in 0..i {
                    value += self.condensed.boundary_internal[row * i + k]
                        * self.condensed.internal_to_boundary[k * b + column];
                }
                self.coupling[row * b + column] = value;
            }
        }
    }
}

/// Fixed-size specialization of the same reduced dense solve. Keeping the
/// boundary dimension in the type lets LLVM see the Twin's exact 13x13 trip
/// counts and remove dynamic loop/bounds machinery without changing pivoting,
/// arithmetic order, or the solved equations. The generic solver remains as a
/// test-switch fallback and for all other boundary sizes.
#[inline]
#[allow(
    clippy::chunks_exact_to_as_chunks,
    reason = "keep the measured LU kernel unchanged during solver-policy experiments"
)]
fn solve_dense_planned_fixed<const N: usize>(
    matrix: &mut [f64],
    rhs: &mut [f64],
    plan: &mut [usize],
    planned: &mut bool,
    reciprocal_pivots: bool,
) -> bool {
    if matrix.len() != N * N || rhs.len() < N || plan.len() < N {
        return false;
    }
    let replay = *planned;
    let mut searching = !replay;
    let mut sound = true;
    for column in 0..N {
        let mut pivot = if searching {
            let mut best = column;
            let mut largest = matrix[column * N + column].abs();
            for row in (column + 1)..N {
                let candidate = matrix[row * N + column].abs();
                if candidate > largest {
                    largest = candidate;
                    best = row;
                }
            }
            plan[column] = best;
            best
        } else {
            plan[column]
        };
        if pivot < column || pivot >= N {
            searching = true;
            sound = false;
            pivot = column;
            let mut largest = matrix[column * N + column].abs();
            for row in (column + 1)..N {
                let candidate = matrix[row * N + column].abs();
                if candidate > largest {
                    largest = candidate;
                    pivot = row;
                }
            }
            plan[column] = pivot;
        }
        if pivot != column {
            let a = column * N;
            let b = pivot * N;
            for offset in column..N {
                matrix.swap(a + offset, b + offset);
            }
            rhs.swap(column, pivot);
        }

        let mut diagonal = matrix[column * N + column];
        if !searching && (diagonal.abs() < 1e-30 || !diagonal.is_finite()) {
            searching = true;
            sound = false;
            let mut best = column;
            let mut largest = diagonal.abs();
            for row in (column + 1)..N {
                let candidate = matrix[row * N + column].abs();
                if candidate > largest {
                    largest = candidate;
                    best = row;
                }
            }
            if best != column {
                let a = column * N;
                let b = best * N;
                for offset in column..N {
                    matrix.swap(a + offset, b + offset);
                }
                rhs.swap(column, best);
            }
            plan[column] = best;
            diagonal = matrix[column * N + column];
        }
        if diagonal.abs() < 1e-30 || !diagonal.is_finite() {
            *planned = false;
            return false;
        }

        let pivot_scale = if reciprocal_pivots {
            let inverse = 1.0 / diagonal;
            matrix[column * N + column] = inverse;
            inverse
        } else {
            diagonal
        };

        let ceiling = diagonal.abs();
        let (above, below) = matrix.split_at_mut((column + 1) * N);
        let pivot_row = &above[column * N..];
        let (rhs_above, rhs_below) = rhs[..N].split_at_mut(column + 1);
        let pivot_rhs = rhs_above[column];
        for (row, value) in below.chunks_exact_mut(N).zip(rhs_below) {
            let entry = row[column];
            if entry == 0.0 {
                continue;
            }
            if !searching && entry.abs() > ceiling * 16.0 {
                sound = false;
            }
            let factor = if reciprocal_pivots {
                entry * pivot_scale
            } else {
                entry / pivot_scale
            };
            for (target, &source) in row[column + 1..].iter_mut().zip(&pivot_row[column + 1..]) {
                *target -= factor * source;
            }
            *value -= factor * pivot_rhs;
        }
    }
    for row in (0..N).rev() {
        let mut value = rhs[row];
        for (&coefficient, &known) in matrix[row * N + row + 1..(row + 1) * N]
            .iter()
            .zip(&rhs[row + 1..N])
        {
            value -= coefficient * known;
        }
        let diagonal = matrix[row * N + row];
        if !diagonal.is_finite()
            || (!reciprocal_pivots && diagonal.abs() < 1e-30)
            || (reciprocal_pivots && diagonal == 0.0)
        {
            *planned = false;
            return false;
        }
        rhs[row] = if reciprocal_pivots {
            value * diagonal
        } else {
            value / diagonal
        };
        if !rhs[row].is_finite() {
            *planned = false;
            return false;
        }
    }
    // Match the generic solver's learned-plan lifecycle exactly. A replay can
    // remain numerically solvable while still being a poor pivot sequence
    // (for example when a below-pivot entry grows far beyond the replayed
    // diagonal). In that case `sound` is cleared and the next solve must learn
    // a fresh plan. Keeping the stale plan was the fixed-13 production bug.
    *planned = if replay { sound } else { true };
    true
}

/// Dense partial-pivoting solve for a reduced boundary system with a learned
/// swap sequence. The first solve searches exactly as before and records each
/// pivot row; subsequent solves replay those swaps and only validate the chosen
/// diagonal. The production path reuses one reciprocal per pivot across every
/// row elimination and back substitution, avoiding dozens of repeated FP
/// divisions on the Twin's 13x13 boundary. A test-only switch retains the old
/// division sequence for same-build A/B. A failed replay returns false so the
/// caller can take full MNA.
#[inline]
fn solve_dense_planned(
    matrix: &mut [f64],
    rhs: &mut [f64],
    n: usize,
    plan: &mut [usize],
    planned: &mut bool,
    reciprocal_pivots: bool,
) -> bool {
    if matrix.len() != n * n || rhs.len() < n || plan.len() < n {
        return false;
    }
    let replay = *planned;
    let mut searching = !replay;
    let mut sound = true;
    for column in 0..n {
        let mut pivot = if searching {
            let mut best = column;
            let mut largest = matrix[column * n + column].abs();
            for row in (column + 1)..n {
                let candidate = matrix[row * n + column].abs();
                if candidate > largest {
                    largest = candidate;
                    best = row;
                }
            }
            plan[column] = best;
            best
        } else {
            plan[column]
        };
        if pivot < column || pivot >= n {
            searching = true;
            sound = false;
            pivot = column;
            let mut largest = matrix[column * n + column].abs();
            for row in (column + 1)..n {
                let candidate = matrix[row * n + column].abs();
                if candidate > largest {
                    largest = candidate;
                    pivot = row;
                }
            }
            plan[column] = pivot;
        }
        if pivot != column {
            let a = column * n;
            let b = pivot * n;
            for offset in column..n {
                matrix.swap(a + offset, b + offset);
            }
            rhs.swap(column, pivot);
        }

        let mut diagonal = matrix[column * n + column];
        if !searching && (diagonal.abs() < 1e-30 || !diagonal.is_finite()) {
            // Replayed row no longer contains a usable pivot. Search from this
            // column onward; completed columns remain a valid LU prefix.
            searching = true;
            sound = false;
            let mut best = column;
            let mut largest = diagonal.abs();
            for row in (column + 1)..n {
                let candidate = matrix[row * n + column].abs();
                if candidate > largest {
                    largest = candidate;
                    best = row;
                }
            }
            if best != column {
                let a = column * n;
                let b = best * n;
                for offset in column..n {
                    matrix.swap(a + offset, b + offset);
                }
                rhs.swap(column, best);
            }
            plan[column] = best;
            diagonal = matrix[column * n + column];
        }
        if diagonal.abs() < 1e-30 || !diagonal.is_finite() {
            *planned = false;
            return false;
        }

        // Every elimination below this column divides by the same pivot. On
        // the Twin's exact 13x13 Schur boundary that is up to 12+11+...+1 =
        // 78 serial FP divisions per Newton pass, plus 13 more during back
        // substitution. Compute one reciprocal per pivot instead and reuse it
        // for both elimination and the later diagonal solve. This changes only
        // floating-point evaluation order, never the LU equations or pivot
        // sequence; the test switch keeps the previous division path available
        // for same-build A/B and the full-reference null test guards drift.
        let pivot_scale = if reciprocal_pivots {
            let inverse = 1.0 / diagonal;
            matrix[column * n + column] = inverse;
            inverse
        } else {
            diagonal
        };

        let ceiling = diagonal.abs();
        let (above, below) = matrix.split_at_mut((column + 1) * n);
        let pivot_row = &above[column * n..];
        let (rhs_above, rhs_below) = rhs[..n].split_at_mut(column + 1);
        let pivot_rhs = rhs_above[column];
        for (row, value) in below.chunks_exact_mut(n).zip(rhs_below) {
            let entry = row[column];
            if entry == 0.0 {
                continue;
            }
            if !searching && entry.abs() > ceiling * 16.0 {
                // Still an exact LU; just relearn the pivot sequence next pass.
                sound = false;
            }
            let factor = if reciprocal_pivots {
                entry * pivot_scale
            } else {
                entry / pivot_scale
            };
            for (target, &source) in row[column + 1..].iter_mut().zip(&pivot_row[column + 1..]) {
                *target -= factor * source;
            }
            *value -= factor * pivot_rhs;
        }
    }
    for row in (0..n).rev() {
        let mut value = rhs[row];
        for (&coefficient, &known) in matrix[row * n + row + 1..(row + 1) * n]
            .iter()
            .zip(&rhs[row + 1..n])
        {
            value -= coefficient * known;
        }
        let diagonal = matrix[row * n + row];
        if !diagonal.is_finite()
            || (!reciprocal_pivots && diagonal.abs() < 1e-30)
            || (reciprocal_pivots && diagonal == 0.0)
        {
            *planned = false;
            return false;
        }
        rhs[row] = if reciprocal_pivots {
            value * diagonal
        } else {
            value / diagonal
        };
        if !rhs[row].is_finite() {
            *planned = false;
            return false;
        }
    }
    *planned = if replay { sound } else { true };
    true
}

impl Condensed {
    fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.boundary, source.boundary);
        debug_assert_eq!(self.internal, source.internal);
        self.inverse_internal
            .copy_from_slice(&source.inverse_internal);
        self.boundary_internal
            .copy_from_slice(&source.boundary_internal);
        self.internal_to_boundary
            .copy_from_slice(&source.internal_to_boundary);
        self.full_rhs.copy_from_slice(&source.full_rhs);
    }

    pub fn reduce_rhs_into(&self, rhs: &[f64], reduced: &mut [f64]) -> bool {
        if rhs.len() != self.boundary.len() + self.internal.len()
            || reduced.len() < self.boundary.len()
        {
            return false;
        }
        for (row, &boundary) in self.boundary.iter().enumerate() {
            let mut value = rhs[boundary];
            let width = self.internal.len();
            for (&coefficient, &internal) in self.boundary_internal[row * width..(row + 1) * width]
                .iter()
                .zip(&self.internal)
            {
                value -= coefficient * rhs[internal];
            }
            reduced[row] = value;
        }
        true
    }

    pub fn reduce_rhs(&self, rhs: &[f64]) -> Option<Vec<f64>> {
        let mut reduced = vec![0.0; self.boundary.len()];
        if !self.reduce_rhs_into(rhs, &mut reduced) {
            return None;
        }
        Some(reduced)
    }

    pub fn recover(&self, boundary_solution: &[f64]) -> Option<Vec<f64>> {
        self.recover_with_rhs(boundary_solution, &self.full_rhs)
    }

    pub fn recover_with_rhs(&self, boundary_solution: &[f64], rhs: &[f64]) -> Option<Vec<f64>> {
        let mut full = vec![0.0; self.boundary.len() + self.internal.len()];
        let mut scratch = vec![0.0; self.internal.len()];
        if !self.recover_into(boundary_solution, rhs, &mut full, &mut scratch) {
            return None;
        }
        Some(full)
    }

    pub fn recover_into(
        &self,
        boundary_solution: &[f64],
        rhs: &[f64],
        full: &mut [f64],
        scratch: &mut [f64],
    ) -> bool {
        if boundary_solution.len() != self.boundary.len()
            || rhs.len() != self.boundary.len() + self.internal.len()
            || full.len() < rhs.len()
            || scratch.len() < self.internal.len()
        {
            return false;
        }
        for (index, &node) in self.boundary.iter().enumerate() {
            full[node] = boundary_solution[index];
        }
        for (row, &internal) in self.internal.iter().enumerate() {
            let mut value = rhs[internal];
            let width = self.boundary.len();
            for (&coefficient, &voltage) in self.internal_to_boundary
                [row * width..(row + 1) * width]
                .iter()
                .zip(boundary_solution)
            {
                value -= coefficient * voltage;
            }
            scratch[row] = value;
        }
        for (row, &internal) in self.internal.iter().enumerate() {
            let mut value = 0.0;
            let width = self.internal.len();
            for (&coefficient, &current) in self.inverse_internal[row * width..(row + 1) * width]
                .iter()
                .zip(&scratch[..width])
            {
                value += coefficient * current;
            }
            full[internal] = value;
        }
        true
    }
}

pub fn condense(
    matrix: &[f64],
    rhs: &[f64],
    internal: &[usize],
    boundary: &[usize],
) -> Option<(Vec<f64>, Vec<f64>, Condensed)> {
    let n = rhs.len();
    if matrix.len() != n * n || internal.len() + boundary.len() != n {
        return None;
    }
    if !is_partition(n, internal, boundary) {
        return None;
    }

    let a_ii = select(matrix, internal, internal, n);
    let a_ib = select(matrix, internal, boundary, n);
    let a_bi = select(matrix, boundary, internal, n);
    let a_bb = select(matrix, boundary, boundary, n);
    let inverse_internal = inverse(&a_ii, internal.len())?;
    let internal_rhs = select_vector(rhs, internal);
    let boundary_rhs = select_vector(rhs, boundary);
    let boundary_internal = multiply(&a_bi, &inverse_internal, boundary.len(), internal.len());
    let reduced_matrix = subtract(
        &a_bb,
        &multiply(&boundary_internal, &a_ib, boundary.len(), internal.len()),
    );
    let reduced_rhs = subtract_vector(
        &boundary_rhs,
        &multiply(
            &boundary_internal,
            &internal_rhs,
            boundary.len(),
            internal.len(),
        ),
    );
    let recovery = Condensed {
        boundary: boundary.to_vec(),
        internal: internal.to_vec(),
        inverse_internal,
        boundary_internal,
        internal_to_boundary: a_ib,
        full_rhs: rhs.to_vec(),
    };
    Some((reduced_matrix, reduced_rhs, recovery))
}

fn is_partition(n: usize, internal: &[usize], boundary: &[usize]) -> bool {
    let mut seen = vec![false; n];
    for &index in internal.iter().chain(boundary) {
        if index >= n || seen[index] {
            return false;
        }
        seen[index] = true;
    }
    seen.into_iter().all(|value| value)
}

fn select(matrix: &[f64], rows: &[usize], columns: &[usize], n: usize) -> Vec<f64> {
    rows.iter()
        .flat_map(|&row| columns.iter().map(move |&column| matrix[row * n + column]))
        .collect()
}

fn select_vector(vector: &[f64], indices: &[usize]) -> Vec<f64> {
    indices.iter().map(|&index| vector[index]).collect()
}

fn multiply(left: &[f64], right: &[f64], rows: usize, shared: usize) -> Vec<f64> {
    let columns = right.len() / shared.max(1);
    let mut result = vec![0.0; rows * columns];
    for row in 0..rows {
        for column in 0..columns {
            for k in 0..shared {
                result[row * columns + column] +=
                    left[row * shared + k] * right[k * columns + column];
            }
        }
    }
    result
}

fn subtract(left: &[f64], right: &[f64]) -> Vec<f64> {
    left.iter().zip(right).map(|(a, b)| a - b).collect()
}

fn subtract_vector(left: &[f64], right: &[f64]) -> Vec<f64> {
    subtract(left, right)
}

fn inverse(matrix: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut augmented = vec![0.0; n * n * 2];
    for row in 0..n {
        for column in 0..n {
            augmented[row * 2 * n + column] = matrix[row * n + column];
        }
        augmented[row * 2 * n + n + row] = 1.0;
    }
    for column in 0..n {
        let pivot = (column..n).max_by(|&a, &b| {
            augmented[a * 2 * n + column]
                .abs()
                .total_cmp(&augmented[b * 2 * n + column].abs())
        })?;
        if augmented[pivot * 2 * n + column].abs() < 1e-30 {
            return None;
        }
        if pivot != column {
            for index in 0..2 * n {
                augmented.swap(column * 2 * n + index, pivot * 2 * n + index);
            }
        }
        let scale = augmented[column * 2 * n + column];
        for index in 0..2 * n {
            augmented[column * 2 * n + index] /= scale;
        }
        for row in 0..n {
            if row == column {
                continue;
            }
            let factor = augmented[row * 2 * n + column];
            for index in 0..2 * n {
                augmented[row * 2 * n + index] -= factor * augmented[column * 2 * n + index];
            }
        }
    }
    Some(
        augmented
            .chunks_exact(2 * n)
            .flat_map(|row| row[n..].iter().copied())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        condense, merit_stamped_fixed_13, recover_boundary_major_in_place, solve_dense_planned,
        solve_dense_planned_fixed, ReducedLinear, ReducedNonlinear,
    };

    #[test]
    fn boundary_major_recovery_matches_row_major_bit_for_bit() {
        const INTERNAL: usize = 18;
        const BOUNDARY: usize = 13;
        let base: Vec<f64> = (0..INTERNAL)
            .map(|row| ((row * 11 + 7) as f64).sin() * 0.75)
            .collect();
        let boundary: Vec<f64> = (0..BOUNDARY)
            .map(|column| ((column * 5 + 3) as f64).cos() * 1.3)
            .collect();
        let mut row_major = vec![0.0; INTERNAL * BOUNDARY];
        let mut boundary_major = vec![0.0; BOUNDARY * INTERNAL];
        for row in 0..INTERNAL {
            for column in 0..BOUNDARY {
                let coefficient = (((row * 19 + column * 23 + 5) % 31) as f64 - 15.0) * 0.0078125;
                row_major[row * BOUNDARY + column] = coefficient;
                boundary_major[column * INTERNAL + row] = coefficient;
            }
        }

        let mut legacy = vec![0.0; INTERNAL];
        for row in 0..INTERNAL {
            let mut value = base[row];
            for column in 0..BOUNDARY {
                value -= row_major[row * BOUNDARY + column] * boundary[column];
            }
            legacy[row] = value;
        }

        let mut transposed = vec![0.0; INTERNAL];
        recover_boundary_major_in_place(&mut transposed, &base, &boundary_major, &boundary);
        for (&candidate, &reference) in transposed.iter().zip(&legacy) {
            assert_eq!(candidate.to_bits(), reference.to_bits());
        }
    }

    #[test]
    fn fixed_13_dense_solve_matches_generic_bit_for_bit() {
        const N: usize = 13;
        let mut original = vec![0.0; N * N];
        for row in 0..N {
            for column in 0..N {
                let distance = row.abs_diff(column) as f64;
                let signed = (((row * 29 + column * 13 + 7) % 23) as f64 - 11.0) * 0.019;
                original[row * N + column] = if row == column {
                    2.75 + 0.11 * row as f64
                } else {
                    signed / (1.0 + distance)
                };
            }
        }
        original[5 * N] = 4.75;
        original[10 * N + 6] = -5.25;
        let original_rhs: Vec<f64> = (0..N)
            .map(|index| ((index * 9 + 4) as f64).cos() * 1.9)
            .collect();

        for reciprocal in [false, true] {
            let mut generic_plan: Vec<usize> = (0..N).collect();
            let mut fixed_plan = generic_plan.clone();
            let mut generic_planned = false;
            let mut fixed_planned = false;

            let mut generic_matrix = original.clone();
            let mut fixed_matrix = original.clone();
            let mut generic_rhs = original_rhs.clone();
            let mut fixed_rhs = original_rhs.clone();
            assert!(solve_dense_planned(
                &mut generic_matrix,
                &mut generic_rhs,
                N,
                &mut generic_plan,
                &mut generic_planned,
                reciprocal,
            ));
            assert!(solve_dense_planned_fixed::<N>(
                &mut fixed_matrix,
                &mut fixed_rhs,
                &mut fixed_plan,
                &mut fixed_planned,
                reciprocal,
            ));
            assert_eq!(fixed_plan, generic_plan);
            assert_eq!(fixed_planned, generic_planned);
            for (&candidate, &reference) in fixed_matrix.iter().zip(&generic_matrix) {
                assert_eq!(candidate.to_bits(), reference.to_bits());
            }
            for (&candidate, &reference) in fixed_rhs.iter().zip(&generic_rhs) {
                assert_eq!(candidate.to_bits(), reference.to_bits());
            }

            let mut replay_matrix = original.clone();
            for row in 0..N {
                replay_matrix[row * N + row] *= 1.0 + (row as f64 + 1.0) * 1e-5;
            }
            let replay_rhs: Vec<f64> = original_rhs
                .iter()
                .enumerate()
                .map(|(index, &value)| value + (index as f64 - 6.0) * 1e-6)
                .collect();
            let mut generic_matrix = replay_matrix.clone();
            let mut fixed_matrix = replay_matrix;
            let mut generic_rhs = replay_rhs.clone();
            let mut fixed_rhs = replay_rhs;
            assert!(solve_dense_planned(
                &mut generic_matrix,
                &mut generic_rhs,
                N,
                &mut generic_plan,
                &mut generic_planned,
                reciprocal,
            ));
            assert!(solve_dense_planned_fixed::<N>(
                &mut fixed_matrix,
                &mut fixed_rhs,
                &mut fixed_plan,
                &mut fixed_planned,
                reciprocal,
            ));
            assert_eq!(fixed_plan, generic_plan);
            assert_eq!(fixed_planned, generic_planned);
            for (&candidate, &reference) in fixed_matrix.iter().zip(&generic_matrix) {
                assert_eq!(candidate.to_bits(), reference.to_bits());
            }
            for (&candidate, &reference) in fixed_rhs.iter().zip(&generic_rhs) {
                assert_eq!(candidate.to_bits(), reference.to_bits());
            }
        }
    }

    #[test]
    fn fixed_13_dense_solve_invalidates_an_unsound_replayed_plan() {
        const N: usize = 13;
        let mut original = vec![0.0; N * N];
        for row in 0..N {
            original[row * N + row] = 2.0 + row as f64 * 0.1;
            if row + 1 < N {
                original[(row + 1) * N + row] = 0.01;
            }
        }
        let original_rhs: Vec<f64> = (0..N).map(|index| index as f64 * 0.25 - 1.0).collect();

        for reciprocal in [false, true] {
            let mut generic_plan: Vec<usize> = (0..N).collect();
            let mut fixed_plan = generic_plan.clone();
            let mut generic_planned = false;
            let mut fixed_planned = false;

            // First solve learns the same identity pivot plan in both paths.
            let mut generic_matrix = original.clone();
            let mut fixed_matrix = original.clone();
            let mut generic_rhs = original_rhs.clone();
            let mut fixed_rhs = original_rhs.clone();
            assert!(solve_dense_planned(
                &mut generic_matrix,
                &mut generic_rhs,
                N,
                &mut generic_plan,
                &mut generic_planned,
                reciprocal,
            ));
            assert!(solve_dense_planned_fixed::<N>(
                &mut fixed_matrix,
                &mut fixed_rhs,
                &mut fixed_plan,
                &mut fixed_planned,
                reciprocal,
            ));
            assert!(generic_planned);
            assert!(fixed_planned);
            assert_eq!(fixed_plan, generic_plan);

            // Keep the replayed diagonal finite but make the entry below the
            // first pivot >16x larger. The solve remains exact/finite, but the
            // replay is deliberately marked unsound so the *next* pass must
            // relearn pivots. This is the state transition the original
            // fixed-13 unit test did not exercise.
            let mut replay = original.clone();
            replay[N] = replay[0].abs() * 32.0;
            let replay_rhs: Vec<f64> = original_rhs
                .iter()
                .enumerate()
                .map(|(index, &value)| value + index as f64 * 1e-6)
                .collect();
            let mut generic_matrix = replay.clone();
            let mut fixed_matrix = replay;
            let mut generic_rhs = replay_rhs.clone();
            let mut fixed_rhs = replay_rhs;
            assert!(solve_dense_planned(
                &mut generic_matrix,
                &mut generic_rhs,
                N,
                &mut generic_plan,
                &mut generic_planned,
                reciprocal,
            ));
            assert!(solve_dense_planned_fixed::<N>(
                &mut fixed_matrix,
                &mut fixed_rhs,
                &mut fixed_plan,
                &mut fixed_planned,
                reciprocal,
            ));
            assert_eq!(fixed_plan, generic_plan);
            assert_eq!(fixed_planned, generic_planned);
            assert!(
                !generic_planned,
                "unsound replay must invalidate the pivot plan"
            );
            for (&candidate, &reference) in fixed_matrix.iter().zip(&generic_matrix) {
                assert_eq!(candidate.to_bits(), reference.to_bits());
            }
            for (&candidate, &reference) in fixed_rhs.iter().zip(&generic_rhs) {
                assert_eq!(candidate.to_bits(), reference.to_bits());
            }
        }
    }

    #[test]
    fn reciprocal_pivot_dense_solve_matches_division_path() {
        // Twin-sized dense boundary with deliberately nontrivial off-diagonal
        // coupling. The reciprocal path is allowed to round differently, but
        // must solve the same linear system to far below the plugin's Newton
        // tolerances and must learn the same pivot sequence.
        const N: usize = 13;
        let mut original = vec![0.0; N * N];
        for row in 0..N {
            for column in 0..N {
                let distance = row.abs_diff(column) as f64;
                let signed = (((row * 17 + column * 11 + 3) % 19) as f64 - 9.0) * 0.013;
                original[row * N + column] = if row == column {
                    3.0 + 0.17 * row as f64
                } else {
                    signed / (1.0 + distance)
                };
            }
        }
        // Force a couple of row choices away from the diagonal so this also
        // covers learned partial pivoting instead of a diagonal-only toy.
        original[4 * N] = 4.25;
        original[9 * N + 5] = -5.0;
        let original_rhs: Vec<f64> = (0..N)
            .map(|index| ((index * 7 + 5) as f64).sin() * 1.7)
            .collect();

        let mut divided_matrix = original.clone();
        let mut divided_rhs = original_rhs.clone();
        let mut divided_plan: Vec<usize> = (0..N).collect();
        let mut divided_planned = false;
        assert!(solve_dense_planned(
            &mut divided_matrix,
            &mut divided_rhs,
            N,
            &mut divided_plan,
            &mut divided_planned,
            false,
        ));

        let mut reciprocal_matrix = original.clone();
        let mut reciprocal_rhs = original_rhs.clone();
        let mut reciprocal_plan: Vec<usize> = (0..N).collect();
        let mut reciprocal_planned = false;
        assert!(solve_dense_planned(
            &mut reciprocal_matrix,
            &mut reciprocal_rhs,
            N,
            &mut reciprocal_plan,
            &mut reciprocal_planned,
            true,
        ));

        assert_eq!(reciprocal_plan, divided_plan);
        for (&reciprocal, &divided) in reciprocal_rhs.iter().zip(&divided_rhs) {
            let scale = 1.0 + divided.abs();
            assert!((reciprocal - divided).abs() <= 2e-13 * scale);
        }
        for row in 0..N {
            let predicted = original[row * N..(row + 1) * N]
                .iter()
                .zip(&reciprocal_rhs)
                .map(|(&coefficient, &value)| coefficient * value)
                .sum::<f64>();
            assert!((predicted - original_rhs[row]).abs() < 2e-12);
        }
    }

    #[test]
    fn condensation_matches_full_three_node_system() {
        let matrix = [4.0, 1.0, 2.0, 1.0, 3.0, 1.0, 2.0, 1.0, 5.0];
        let rhs = [7.0, 5.0, 11.0];
        let (reduced, reduced_rhs, recovery) = condense(&matrix, &rhs, &[1], &[0, 2]).unwrap();
        assert_eq!(reduced.len(), 4);
        assert_eq!(reduced_rhs.len(), 2);
        let boundary = solve(&reduced, &reduced_rhs);
        let full = recovery.recover(&boundary).unwrap();
        let expected = solve(&matrix, &rhs);
        for (actual, expected) in full.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn invalid_or_singular_partitions_are_rejected() {
        let matrix = [0.0, 0.0, 0.0, 0.0];
        assert!(condense(&matrix, &[1.0, 1.0], &[0], &[1]).is_none());
        assert!(condense(&[0.0], &[1.0], &[0], &[]).is_none());
    }

    #[test]
    fn reduced_linear_block_solves_and_recovers_in_place() {
        let matrix = [4.0, 1.0, 2.0, 1.0, 3.0, 1.0, 2.0, 1.0, 5.0];
        let source = [7.0, 5.0, 11.0];
        let block = ReducedLinear::new(&matrix, &[0.0; 3], 0).unwrap();
        let mut full = [0.0; 3];
        let mut reduced = [0.0; 1];
        let mut scratch = [0.0; 2];
        assert!(block.solve_into(&source, &mut full, &mut reduced, &mut scratch));
        let expected = solve(&matrix, &source);
        for (actual, expected) in full.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    fn solve(matrix: &[f64], rhs: &[f64]) -> Vec<f64> {
        let n = rhs.len();
        let mut a = matrix.to_vec();
        let mut b = rhs.to_vec();
        for column in 0..n {
            let pivot = (column..n)
                .max_by(|&a_row, &b_row| {
                    a[a_row * n + column]
                        .abs()
                        .total_cmp(&a[b_row * n + column].abs())
                })
                .unwrap();
            for index in 0..n {
                a.swap(column * n + index, pivot * n + index);
            }
            b.swap(column, pivot);
            for index in (column + 1)..n {
                let factor = a[index * n + column] / a[column * n + column];
                for k in column..n {
                    a[index * n + k] -= factor * a[column * n + k];
                }
                b[index] -= factor * b[column];
            }
        }
        let mut result = vec![0.0; n];
        for row in (0..n).rev() {
            result[row] = (b[row]
                - ((row + 1)..n)
                    .map(|column| a[row * n + column] * result[column])
                    .sum::<f64>())
                / a[row * n + row];
        }
        result
    }
    #[test]
    fn reduced_linear_refresh_matches_fresh_reduction() {
        let first = [4.0, 1.0, 2.0, 1.0, 3.0, 1.0, 2.0, 1.0, 5.0];
        let second = [5.0, 0.5, 1.5, 0.5, 4.0, 0.75, 1.5, 0.75, 6.0];
        let zero = [0.0; 3];
        let rhs = [7.0, 5.0, 11.0];

        let mut cached = ReducedLinear::new(&first, &zero, 0).unwrap();
        assert!(cached.refresh(&second, &zero, 0));
        let fresh = ReducedLinear::new(&second, &zero, 0).unwrap();

        let mut cached_full = [0.0; 3];
        let mut cached_reduced = [0.0; 3];
        let mut cached_scratch = [0.0; 3];
        assert!(cached.solve_into(
            &rhs,
            &mut cached_full,
            &mut cached_reduced,
            &mut cached_scratch,
        ));

        let mut fresh_full = [0.0; 3];
        let mut fresh_reduced = [0.0; 3];
        let mut fresh_scratch = [0.0; 3];
        assert!(fresh.solve_into(
            &rhs,
            &mut fresh_full,
            &mut fresh_reduced,
            &mut fresh_scratch,
        ));

        for (actual, expected) in cached_full.iter().zip(fresh_full) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn reduced_nonlinear_rejects_a_singular_internal_block() {
        // Boundary 0 is perfectly usable, but the internal [1, 2] block has
        // no pivot. The caller must keep the original full-MNA path instead.
        let matrix = [4.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(ReducedNonlinear::new(&matrix, &[0.0; 3], &[0]).is_none());
    }

    #[test]
    fn precondensed_13_stamp_matches_legacy_schur_system() {
        const B: usize = 13;
        const N: usize = 15;
        let boundary: Vec<usize> = (0..B).collect();
        let mut base = vec![0.0; N * N];
        for row in 0..N {
            base[row * N + row] = 6.0 + row as f64 * 0.17;
            if row + 1 < N {
                base[row * N + row + 1] = -0.07;
                base[(row + 1) * N + row] = 0.05;
            }
        }
        // Give both internal nodes nontrivial connections to every boundary
        // node so the Schur coupling is dense and the test exercises all 169
        // fixed boundary coefficients.
        for column in 0..B {
            let scale = column as f64 + 1.0;
            base[column * N + 13] += 0.013 * scale;
            base[13 * N + column] -= 0.009 * scale;
            base[column * N + 14] -= 0.007 * scale;
            base[14 * N + column] += 0.011 * scale;
        }
        base[13 * N + 14] = 0.12;
        base[14 * N + 13] = -0.08;

        let zero = vec![0.0; N];
        let fixed_rhs: Vec<f64> = (0..N)
            .map(|index| ((index * 7 + 3) as f64).sin() * 0.75)
            .collect();
        let mut precondensed = ReducedNonlinear::new(&base, &zero, &boundary).unwrap();
        let mut legacy = precondensed.clone();
        precondensed.test_disable_precondensed_13_stamp_base = false;
        legacy.test_disable_precondensed_13_stamp_base = true;
        assert!(precondensed.prepare_rhs(&fixed_rhs));
        assert!(legacy.prepare_rhs(&fixed_rhs));

        {
            let (matrix, rhs, _) = precondensed.begin_stamp(&fixed_rhs).unwrap();
            for slot in [0usize, 1, 14, 57, 88, 120, 168] {
                matrix[slot] += (slot as f64 + 1.0) * 0.001_25;
            }
            rhs[0] += 0.125;
            rhs[5] -= 0.031_25;
            rhs[12] += 0.062_5;
        }
        {
            let (matrix, rhs, _) = legacy.begin_stamp(&fixed_rhs).unwrap();
            for slot in [0usize, 1, 14, 57, 88, 120, 168] {
                matrix[slot] += (slot as f64 + 1.0) * 0.001_25;
            }
            rhs[0] += 0.125;
            rhs[5] -= 0.031_25;
            rhs[12] += 0.062_5;
        }
        precondensed.finish_stamp();
        legacy.finish_stamp();

        for (&candidate, &reference) in precondensed
            .reduced_matrix
            .iter()
            .zip(&legacy.reduced_matrix)
        {
            let scale = candidate.abs().max(reference.abs()).max(1.0);
            assert!((candidate - reference).abs() <= scale * 8.0 * f64::EPSILON);
        }
        for (&candidate, &reference) in precondensed.reduced_rhs.iter().zip(&legacy.reduced_rhs) {
            assert_eq!(candidate.to_bits(), reference.to_bits());
        }

        // Both materialised systems must also solve to the same boundary and
        // recovered internal state far below Newton's production tolerance.
        let mut fast_full = vec![0.0; N];
        let mut legacy_full = vec![0.0; N];
        let current = vec![0.0; N];
        let mut fast_delta = vec![0.0; N];
        let mut legacy_delta = vec![0.0; N];
        assert!(precondensed
            .solve_stamped_with_delta(&mut fast_full, &current, &mut fast_delta, 1e-6, 1e-6,)
            .is_some());
        assert!(legacy
            .solve_stamped_with_delta(&mut legacy_full, &current, &mut legacy_delta, 1e-6, 1e-6,)
            .is_some());
        for (&candidate, &reference) in fast_full.iter().zip(&legacy_full) {
            assert!((candidate - reference).abs() <= 1e-12);
        }
    }

    #[test]
    fn trust_region_dogleg_stays_on_radius_and_reduces_linear_model() {
        const N: usize = 14;
        let mut base = vec![0.0; N * N];
        for index in 0..N {
            base[index * N + index] = 1.0;
        }
        let boundary: Vec<usize> = (0..13).collect();
        let mut reduced = ReducedNonlinear::new(&base, &[0.0; N], &boundary).unwrap();
        assert!(reduced.prepare_rhs(&[0.0; N]));

        // Use a diagonal but non-uniform local model so the scaled Cauchy and
        // Newton directions are not trivially identical.
        reduced.reduced_matrix.fill(0.0);
        for index in 0..13 {
            reduced.reduced_matrix[index * 13 + index] = 0.75 + 0.25 * index as f64;
            reduced.reduced_rhs[index] = 0.2 + 0.03 * (index as f64).sin();
        }
        let current = vec![0.0; N];
        let model = reduced
            .trust_region_model_13(&current, 1.0, 0.0)
            .expect("finite trust model");
        let mut newton_delta = vec![0.0; N];
        for index in 0..13 {
            newton_delta[index] =
                reduced.reduced_rhs[index] / reduced.reduced_matrix[index * 13 + index];
        }
        let newton_norm = reduced
            .trust_region_newton_norm_13(&model, &newton_delta)
            .unwrap();
        let radius = 0.4 * newton_norm;
        let mut point = vec![0.0; N];
        let (fraction, predicted_merit) = reduced
            .trust_region_trial_point_13(&model, &current, &newton_delta, radius, &mut point)
            .expect("finite dogleg point");
        assert!((fraction - 0.4).abs() < 1.0e-12);
        let mut step_norm_sq = 0.0;
        for value in &point[..13] {
            step_norm_sq += value * value;
        }
        assert!((step_norm_sq.sqrt() - radius).abs() < 1.0e-12);
        let initial_merit: f64 = model.residual.iter().map(|value| value * value).sum();
        assert!(predicted_merit < initial_merit);
    }

    #[test]
    fn ceres_lm13_reuses_model_and_stronger_damping_shortens_the_step() {
        const N: usize = 14;
        let mut base = vec![0.0; N * N];
        for index in 0..N {
            base[index * N + index] = 1.0;
        }
        let boundary: Vec<usize> = (0..13).collect();
        let mut reduced = ReducedNonlinear::new(&base, &[0.0; N], &boundary).unwrap();
        assert!(reduced.prepare_rhs(&[0.0; N]));

        reduced.reduced_matrix.fill(0.0);
        for row in 0..13 {
            for column in 0..13 {
                reduced.reduced_matrix[row * 13 + column] = if row == column {
                    1.0 + row as f64 * 0.125
                } else {
                    ((row * 7 + column * 11 + 3) as f64).sin() * 0.015
                };
            }
            reduced.reduced_rhs[row] = 0.15 + row as f64 * 0.02;
        }

        let current = vec![0.0; N];
        let model = reduced
            .ceres_lm_model_13(&current)
            .expect("finite LM model");
        assert!(model.current_merit > 0.0);

        let mut loose = vec![0.0; N];
        let loose_predicted = reduced
            .ceres_lm_trial_point_13(&model, &current, 1.0e4, &mut loose)
            .expect("finite loose LM step");
        let mut tight = vec![0.0; N];
        let tight_predicted = reduced
            .ceres_lm_trial_point_13(&model, &current, 1.0e-2, &mut tight)
            .expect("finite tight LM step");

        let loose_norm = loose[..13].iter().map(|v| v * v).sum::<f64>().sqrt();
        let tight_norm = tight[..13].iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(loose_norm > tight_norm);
        assert!(loose_predicted < model.current_merit);
        assert!(tight_predicted < model.current_merit);

        // The model is immutable across retries: reusing the same radius must
        // reproduce the exact same candidate and predicted reduction.
        let mut repeated = vec![0.0; N];
        let repeated_predicted = reduced
            .ceres_lm_trial_point_13(&model, &current, 1.0e4, &mut repeated)
            .expect("finite repeated LM step");
        assert_eq!(loose[..13], repeated[..13]);
        assert_eq!(loose_predicted, repeated_predicted);
    }

    #[test]
    fn fixed_13_stamped_merit_matches_generic_bit_for_bit() {
        let mut matrix = [0.0; 169];
        for (index, value) in matrix.iter_mut().enumerate() {
            let row = index / 13;
            let column = index % 13;
            *value = if row == column {
                7.0 + row as f64 * 0.125
            } else {
                ((row * 17 + column * 11 + 3) as f64) * 0.000_976_562_5 - 0.75
            };
        }
        let mut rhs = [0.0; 13];
        let mut voltage = [0.0; 13];
        for index in 0..13 {
            rhs[index] = (index as f64 - 6.0) * 0.3125;
            voltage[index] = ((index * 7 + 5) as f64) * 0.0625 - 2.0;
        }

        let mut generic = 0.0;
        for row in 0..13 {
            let mut residual = -rhs[row];
            for column in 0..13 {
                residual += matrix[row * 13 + column] * voltage[column];
            }
            generic += residual * residual;
        }

        let fixed = merit_stamped_fixed_13(&matrix, &rhs, &voltage);
        assert_eq!(fixed.to_bits(), generic.to_bits());
    }

    #[test]
    fn fixed_13_merit_and_trial_residual_preserve_mapped_cancellation_cases() {
        // Exercise the public dispatch and noncontiguous boundary gather, not
        // just the arithmetic helper. Mixed scales and nearly cancelling rows
        // expose reassociation that a single dyadic fixture cannot detect.
        const N: usize = 31;
        let mut base = vec![0.0; N * N];
        for row in 0..N {
            base[row * N + row] = 1.0;
        }
        let boundary = [29, 1, 23, 3, 19, 5, 17, 7, 13, 9, 11, 21, 27];
        let mut reduced = ReducedNonlinear::new(&base, &[0.0; N], &boundary).unwrap();
        assert!(reduced.prepare_rhs(&[0.0; N]));
        let mut seed = 0x7a91_12bc_9931_584du64;
        let mut random = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 11) as f64 / ((1u64 << 53) as f64) - 0.5) * 2.0
        };
        for case in 0..256 {
            let mut full = [0.0; N];
            for voltage in &mut full {
                *voltage = random() * 450.0;
            }
            for row in 0..13 {
                let mut sum = 0.0;
                for (column, &node) in boundary.iter().enumerate() {
                    let coefficient = random() * 10f64.powi((column % 9) as i32 - 4);
                    reduced.reduced_matrix[row * 13 + column] = coefficient;
                    sum += coefficient * full[node];
                }
                reduced.reduced_rhs[row] = if case % 2 == 0 {
                    sum * (1.0 + 2.0 * f64::EPSILON)
                } else {
                    random() * 100.0
                };
            }
            reduced.test_disable_fixed_13_stamped_merit = true;
            let generic = reduced.merit_stamped(&full);
            reduced.test_disable_fixed_13_stamped_merit = false;
            let fixed = reduced.merit_stamped(&full);
            assert_eq!(fixed.to_bits(), generic.to_bits(), "case {case}");

            reduced
                .merit_linear
                .copy_from_slice(&reduced.reduced_matrix);
            for row in 0..13 {
                reduced.merit_rhs_base[row] = -reduced.reduced_rhs[row];
            }
            let mut generic_residual = [0.0; 13];
            let mut fixed_residual = [0.0; 13];
            reduced.test_disable_fixed_13_trial_residual = true;
            assert!(reduced
                .begin_residual(&[0.0; N], &full, &mut generic_residual)
                .is_some());
            reduced.test_disable_fixed_13_trial_residual = false;
            assert!(reduced
                .begin_residual(&[0.0; N], &full, &mut fixed_residual)
                .is_some());
            assert_eq!(
                fixed_residual.map(f64::to_bits),
                generic_residual.map(f64::to_bits),
                "case {case}"
            );
        }
    }

    #[test]
    fn unfinished_trial_merit_matches_finished_stamp_bit_for_bit() {
        let base = [
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5, 0.4, 2.0, 0.5,
            1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let zero = [0.0; 5];
        let prepared_rhs = [0.25, -0.5, 0.75, 0.125, -0.3];
        let fixed_rhs = [1.25, -0.75, 0.5, 2.0, -0.125];
        let point = [0.3, -0.2, 0.7, 1.1, -0.4];
        let mut reduced = ReducedNonlinear::new(&base, &zero, &[0, 3]).unwrap();
        assert!(reduced.prepare_rhs(&prepared_rhs));

        {
            let (matrix, rhs, _map) = reduced.begin_stamp(&fixed_rhs).unwrap();
            matrix[0] += 2.3;
            matrix[1] -= 0.4;
            matrix[2] += 0.15;
            matrix[3] += 1.1;
            rhs[0] += 0.375;
            rhs[1] -= 0.625;
        }

        let direct = reduced.merit_unfinished_stamp(&point);
        reduced.finish_stamp();
        let materialised = reduced.merit_stamped(&point);
        assert_eq!(direct.to_bits(), materialised.to_bits());
    }

    #[test]
    fn reduced_nonlinear_matches_full_system_when_boundary_block_moves() {
        let base = [
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5, 0.4, 2.0, 0.5,
            1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let zero = [0.0; 5];
        let mut reduced = ReducedNonlinear::new(&base, &zero, &[0, 3]).unwrap();

        // Stand in for a nonlinear device stamp: only boundary rows/columns
        // change, and they need not be symmetric.
        let mut jacobian = base;
        jacobian[0] += 2.3;
        jacobian[3] -= 0.7;
        jacobian[15] += 0.25;
        jacobian[18] += 1.1;
        let rhs = [3.0, -1.0, 2.5, 0.75, -0.4];

        let mut actual = [0.0; 5];
        assert!(reduced.prepare_rhs(&rhs));
        assert!(reduced.solve_into(&jacobian, &rhs, &mut actual));
        let expected = solve(&jacobian, &rhs);
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-11);
        }
    }

    #[test]
    fn split_internal_recovery_matches_full_system_with_boundary_rhs_changes() {
        let base = [
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5, 0.4, 2.0, 0.5,
            1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let zero = [0.0; 5];
        let mut reduced = ReducedNonlinear::new(&base, &zero, &[0, 3]).unwrap();

        let mut jacobian = base;
        jacobian[0] += 3.1;
        jacobian[3] -= 0.45;
        jacobian[15] += 0.35;
        jacobian[18] += 1.7;

        // The internal RHS is the per-sample passive/source history and is
        // prepared once. Nonlinear equivalent-current sources then change only
        // boundary entries while Newton and its line search run.
        let fixed_rhs = [2.0, -1.25, 0.8, 1.0, -0.3];
        let mut stamped_rhs = fixed_rhs;
        stamped_rhs[0] += 0.7;
        stamped_rhs[3] -= 0.4;

        assert!(reduced.prepare_rhs(&fixed_rhs));
        let mut actual = [0.0; 5];
        assert!(reduced.solve_into(&jacobian, &stamped_rhs, &mut actual));
        let expected = solve(&jacobian, &stamped_rhs);
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-11);
        }
    }

    #[test]
    fn split_internal_recovery_matches_legacy_recovery() {
        let base = [
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5, 0.4, 2.0, 0.5,
            1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let rhs = [3.0, -1.0, 2.5, 0.75, -0.4];
        let mut reduced = ReducedNonlinear::new(&base, &[0.0; 5], &[0, 3]).unwrap();
        assert!(reduced.prepare_rhs(&rhs));

        let mut actual = [0.0; 5];
        assert!(reduced.solve_into(&base, &rhs, &mut actual));
        let legacy = reduced
            .condensed
            .recover_with_rhs(&reduced.reduced_rhs, &rhs)
            .unwrap();
        for (actual, legacy) in actual.iter().zip(legacy) {
            assert!((actual - legacy).abs() < 1e-12);
        }
    }

    #[test]
    fn reduced_nonlinear_refresh_matches_full_system_after_linear_rebuild() {
        let first = [
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5, 0.4, 2.0, 0.5,
            1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let second = [
            11.0, 1.2, 0.35, 2.0, 0.0, 0.9, 9.0, 0.8, 0.6, 0.15, 0.45, 1.1, 7.5, 1.35, 0.5, 2.0,
            0.55, 1.4, 9.5, 0.9, 0.0, 0.25, 0.3, 1.1, 6.5,
        ];
        let zero = [0.0; 5];
        let mut reduced = ReducedNonlinear::new(&first, &zero, &[0, 3]).unwrap();
        assert!(reduced.refresh(&second, &zero));

        let mut jacobian = second;
        jacobian[0] += 4.0;
        jacobian[3] -= 0.5;
        jacobian[15] += 0.9;
        jacobian[18] += 2.0;
        let rhs = [4.0, 1.0, -2.0, 3.0, 0.5];

        let mut actual = [0.0; 5];
        assert!(reduced.prepare_rhs(&rhs));
        assert!(reduced.solve_into(&jacobian, &rhs, &mut actual));
        let expected = solve(&jacobian, &rhs);
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-11);
        }
    }
    #[test]
    fn active_rhs_linear_response_matches_full_system() {
        let matrix = [
            9.0, 1.0, 0.4, 0.2, 1.0, 7.0, 0.5, 0.3, 0.4, 0.5, 6.0, 0.8, 0.2, 0.3, 0.8, 5.0,
        ];
        // Only nodes 1 and 2 can receive audio-rate source/history current;
        // node 3 is structurally inactive and stays zero.
        let block = ReducedLinear::new_with_rhs_active(&matrix, &[0.0; 4], 0, &[1, 2]).unwrap();
        for (one, two) in [(1.2, -0.4), (-3.0, 2.25), (0.0, 0.75)] {
            let rhs = [0.6, one, two, 0.0];
            let mut full = [0.0; 4];
            let mut reduced = [0.0; 1];
            let mut scratch = [0.0; 3];
            assert!(block.solve_into(&rhs, &mut full, &mut reduced, &mut scratch));
            let expected = solve(&matrix, &rhs);
            for (actual, expected) in full.iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn direct_reduced_stamp_and_pivot_replay_match_full_system() {
        let base = [
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5, 0.4, 2.0, 0.5,
            1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let zero = [0.0; 5];
        let fixed_rhs = [1.0, -1.25, 0.8, 0.5, -0.3];
        // Internal fixed RHS positions 1, 2 and 4 are the only changing basis
        // contributors. Boundary 0 and 3 remain direct RHS entries.
        let mut reduced =
            ReducedNonlinear::new_with_active_rhs(&base, &zero, &[0, 3], &[1, 2, 4]).unwrap();

        for pass in 0..2 {
            assert!(reduced.prepare_rhs(&fixed_rhs));
            let mut full_matrix = base;
            let mut full_rhs = fixed_rhs;
            let (d00, d03, d30, d33, r0, r3) = if pass == 0 {
                (2.3, -0.7, 0.25, 1.1, 0.7, -0.4)
            } else {
                // A materially different boundary Jacobian exercises the
                // learned reduced pivot sequence on its second solve.
                (0.6, 1.2, -0.45, 3.4, -0.2, 0.9)
            };
            full_matrix[0] += d00;
            full_matrix[3] += d03;
            full_matrix[15] += d30;
            full_matrix[18] += d33;
            full_rhs[0] += r0;
            full_rhs[3] += r3;

            {
                let (matrix, rhs, _) = reduced.begin_stamp(&fixed_rhs).unwrap();
                matrix[0] += d00;
                matrix[1] += d03;
                matrix[2] += d30;
                matrix[3] += d33;
                rhs[0] += r0;
                rhs[1] += r3;
            }
            reduced.finish_stamp();
            let current = [0.1, -0.2, 0.3, -0.4, 0.5];
            let mut actual = [0.0; 5];
            let mut delta = [0.0; 5];
            let moved =
                reduced.solve_stamped_with_delta(&mut actual, &current, &mut delta, 1e-6, 1e-6);
            assert!(moved.is_some());

            let expected = solve(&full_matrix, &full_rhs);
            for ((actual, expected), (delta, current)) in
                actual.iter().zip(&expected).zip(delta.iter().zip(&current))
            {
                assert!((*actual - *expected).abs() < 1e-11);
                assert!((*delta - (*expected - *current)).abs() < 1e-11);
            }
        }
    }
}
