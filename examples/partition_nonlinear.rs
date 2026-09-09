//! Prototype a condensed Newton correction for a coupled nonlinear block.

use gainstagefx::dsp::partition::condense;

fn residual(state: [f64; 2]) -> [f64; 2] {
    let boundary = state[0];
    let internal = state[1];
    [boundary + internal - 1.0, internal - boundary.exp()]
}

fn jacobian(state: [f64; 2]) -> [f64; 4] {
    [1.0, 1.0, -state[0].exp(), 1.0]
}

fn solve_full(matrix: &[f64], rhs: &[f64]) -> [f64; 2] {
    let determinant = matrix[0] * matrix[3] - matrix[1] * matrix[2];
    [
        (rhs[0] * matrix[3] - matrix[1] * rhs[1]) / determinant,
        (matrix[0] * rhs[1] - rhs[0] * matrix[2]) / determinant,
    ]
}

fn solve_boundary(matrix: &[f64], rhs: &[f64]) -> Vec<f64> {
    assert_eq!(matrix.len(), 1);
    vec![rhs[0] / matrix[0]]
}

fn main() {
    let mut full = [0.3, 0.3];
    let mut condensed = full;
    let internal = [1usize];
    let boundary = [0usize];

    println!("iteration  full residual  condensed residual  correction error");
    for iteration in 0..8 {
        let full_residual = residual(full);
        let _condensed_residual = residual(condensed);
        let full_jacobian = jacobian(full);
        let full_rhs = [-full_residual[0], -full_residual[1]];
        let full_step = solve_full(&full_jacobian, &full_rhs);

        let (reduced, reduced_rhs, recovery) =
            condense(&full_jacobian, &full_rhs, &internal, &boundary)
                .expect("nonlinear Jacobian partition is nonsingular");
        let boundary_step = solve_boundary(&reduced, &reduced_rhs);
        let condensed_step = recovery
            .recover_with_rhs(&boundary_step, &full_rhs)
            .expect("condensed correction recovers");
        let error = condensed_step
            .iter()
            .zip(full_step)
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0, f64::max);

        for index in 0..2 {
            full[index] += full_step[index];
            condensed[index] += condensed_step[index];
        }
        let full_norm = residual(full)
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let condensed_norm = residual(condensed)
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        println!("{iteration:>9}  {full_norm:>13.3e}  {condensed_norm:>18.3e}  {error:>16.3e}");
    }
    println!("full solution:      [{:.9}, {:.9}]", full[0], full[1]);
    println!(
        "condensed solution: [{:.9}, {:.9}]",
        condensed[0], condensed[1]
    );
}
