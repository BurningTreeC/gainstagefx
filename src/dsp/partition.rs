//! Exact condensation of internal circuit unknowns onto boundary unknowns.

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
        if !self.valid || rhs.len() != self.condensed.boundary.len() + self.condensed.internal.len()
            || reduced.is_empty() || full.len() < rhs.len()
        {
            return false;
        }
        let m = self.condensed.internal.len();
        let a = self.rhs_active_nodes.len();
        if self.rhs_active_inverse.len() != m * a
            || self.rhs_active_boundary.len() != a
        {
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
            let Some(local) = self.condensed.internal.iter().position(|&node| node == global) else {
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
    /// Recovery is therefore only `x_i = base_i - response_i * x_b` on each
    /// Newton pass: O(i*b) instead of another O(i^2) inverse mat-vec.
    internal_boundary_response: Vec<f64>,
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
            node_to_boundary,
            boundary_matrix_slots,
            reduced_matrix: vec![0.0; b * b],
            reduced_rhs: vec![0.0; b],
            rhs_internal_correction: vec![0.0; b],
            internal_rhs_base: vec![0.0; i],
            internal_boundary_response: vec![0.0; i * b],
            active_rhs_nodes,
            active_internal_local,
            internal_rhs_responses: vec![0.0; active * i],
            boundary_rhs_responses: vec![0.0; active * b],
            rhs_prepared: false,
            pivot_plan: (0..b).collect(),
            pivot_planned: false,
            refresh_work: vec![0.0; 2 * i * i],
            valid: true,
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

    pub fn internal_len(&self) -> usize {
        self.condensed.internal.len()
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
            || self.node_to_boundary.len() != n
            || self.boundary_matrix_slots.len() != b * b
            || self.reduced_matrix.len() != b * b
            || self.reduced_rhs.len() != b
            || self.rhs_internal_correction.len() != b
            || self.internal_rhs_base.len() != i
            || self.internal_boundary_response.len() != i * b
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
        self.reduced_matrix.copy_from_slice(&self.boundary_base);
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
        for row in 0..b {
            let global_row = self.condensed.boundary[row];
            let mut value = -fixed_rhs[global_row] + self.rhs_internal_correction[row];
            let coefficients = &self.merit_linear[row * b..(row + 1) * b];
            for (&coefficient, &global_column) in
                coefficients.iter().zip(&self.condensed.boundary)
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
        for (value, &coupling) in self.reduced_matrix.iter_mut().zip(&self.coupling) {
            *value -= coupling;
        }
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
        if !solve_dense_planned(
            &mut self.reduced_matrix,
            &mut self.reduced_rhs,
            b,
            &mut self.pivot_plan,
            &mut self.pivot_planned,
        ) {
            return None;
        }
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
        if !solve_dense_planned(
            &mut self.reduced_matrix,
            &mut self.reduced_rhs,
            b,
            &mut self.pivot_plan,
            &mut self.pivot_planned,
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
        for (row, &node) in self.condensed.internal.iter().enumerate() {
            let response = &self.internal_boundary_response[row * b..(row + 1) * b];
            let mut value = self.internal_rhs_base[row];
            for (&coefficient, &boundary_voltage) in response.iter().zip(&self.reduced_rhs) {
                value -= coefficient * boundary_voltage;
            }
            full[node] = value;
        }
        true
    }

    fn update_boundary_base(&mut self, matrix: &[f64]) {
        for (value, &slot) in self.boundary_base.iter_mut().zip(&self.boundary_matrix_slots) {
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

/// Dense partial-pivoting solve for a reduced boundary system with a learned
/// swap sequence. The first solve searches exactly as before and records each
/// pivot row; subsequent solves replay those swaps and only validate the chosen
/// diagonal. A failed replay returns false so the caller can take full MNA.
#[inline]
fn solve_dense_planned(
    matrix: &mut [f64],
    rhs: &mut [f64],
    n: usize,
    plan: &mut [usize],
    planned: &mut bool,
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
            let factor = entry / diagonal;
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
        if diagonal.abs() < 1e-30 || !diagonal.is_finite() {
            *planned = false;
            return false;
        }
        rhs[row] = value / diagonal;
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
            for (&coefficient, &voltage) in self.internal_to_boundary[row * width..(row + 1) * width]
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
    use super::{condense, ReducedLinear, ReducedNonlinear};

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
    fn unfinished_trial_merit_matches_finished_stamp_bit_for_bit() {
        let base = [
            10.0, 1.0, 0.5, 2.0, 0.0,
            1.0, 8.0, 1.0, 0.5, 0.2,
            0.5, 1.0, 7.0, 1.5, 0.4,
            2.0, 0.5, 1.5, 9.0, 1.0,
            0.0, 0.2, 0.4, 1.0, 6.0,
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
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5,
            0.4, 2.0, 0.5, 1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
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
            10.0, 1.0, 0.5, 2.0, 0.0, 1.0, 8.0, 1.0, 0.5, 0.2, 0.5, 1.0, 7.0, 1.5,
            0.4, 2.0, 0.5, 1.5, 9.0, 1.0, 0.0, 0.2, 0.4, 1.0, 6.0,
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
            9.0, 1.0, 0.4, 0.2,
            1.0, 7.0, 0.5, 0.3,
            0.4, 0.5, 6.0, 0.8,
            0.2, 0.3, 0.8, 5.0,
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
            10.0, 1.0, 0.5, 2.0, 0.0,
            1.0, 8.0, 1.0, 0.5, 0.2,
            0.5, 1.0, 7.0, 1.5, 0.4,
            2.0, 0.5, 1.5, 9.0, 1.0,
            0.0, 0.2, 0.4, 1.0, 6.0,
        ];
        let zero = [0.0; 5];
        let fixed_rhs = [1.0, -1.25, 0.8, 0.5, -0.3];
        // Internal fixed RHS positions 1, 2 and 4 are the only changing basis
        // contributors. Boundary 0 and 3 remain direct RHS entries.
        let mut reduced = ReducedNonlinear::new_with_active_rhs(
            &base,
            &zero,
            &[0, 3],
            &[1, 2, 4],
        )
        .unwrap();

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
            let moved = reduced.solve_stamped_with_delta(
                &mut actual,
                &current,
                &mut delta,
                1e-6,
                1e-6,
            );
            assert!(moved.is_some());

            let expected = solve(&full_matrix, &full_rhs);
            for ((actual, expected), (delta, current)) in actual
                .iter()
                .zip(&expected)
                .zip(delta.iter().zip(&current))
            {
                assert!((*actual - *expected).abs() < 1e-11);
                assert!((*delta - (*expected - *current)).abs() < 1e-11);
            }
        }
    }

}
