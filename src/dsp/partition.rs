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
}

impl ReducedLinear {
    pub fn new(matrix: &[f64], rhs: &[f64], output: usize) -> Option<Self> {
        let n = rhs.len();
        if output >= n {
            return None;
        }
        let internal: Vec<usize> = (0..n).filter(|&index| index != output).collect();
        let (reduced, _, condensed) = condense(matrix, rhs, &internal, &[output])?;
        let admittance = *reduced.first()?;
        (admittance.abs() >= 1e-30).then_some(Self {
            condensed,
            admittance,
        })
    }

    pub fn solve_into(
        &self,
        rhs: &[f64],
        full: &mut [f64],
        reduced: &mut [f64],
        scratch: &mut [f64],
    ) -> bool {
        if !self.condensed.reduce_rhs_into(rhs, reduced) || reduced.is_empty() {
            return false;
        }
        let boundary = [reduced[0] / self.admittance];
        self.condensed.recover_into(&boundary, rhs, full, scratch)
    }
}

impl Condensed {
    pub fn reduce_rhs_into(&self, rhs: &[f64], reduced: &mut [f64]) -> bool {
        if rhs.len() != self.boundary.len() + self.internal.len()
            || reduced.len() < self.boundary.len()
        {
            return false;
        }
        for (row, &boundary) in self.boundary.iter().enumerate() {
            let mut value = rhs[boundary];
            for (column, &internal) in self.internal.iter().enumerate() {
                value -= self.boundary_internal[row * self.internal.len() + column] * rhs[internal];
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
            for (column, _) in self.boundary.iter().enumerate() {
                value -= self.internal_to_boundary[row * self.boundary.len() + column]
                    * boundary_solution[column];
            }
            scratch[row] = value;
        }
        for (row, &internal) in self.internal.iter().enumerate() {
            let mut value = 0.0;
            for column in 0..self.internal.len() {
                value +=
                    self.inverse_internal[row * self.internal.len() + column] * scratch[column];
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
    use super::{condense, ReducedLinear};

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
}
