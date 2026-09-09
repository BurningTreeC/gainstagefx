//! Apply exact internal-node condensation to a real linear tone network.

use gainstagefx::dsp::partition::condense;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::Tone;

fn solve(matrix: &[f64], rhs: &[f64]) -> Vec<f64> {
    let n = rhs.len();
    let mut matrix = matrix.to_vec();
    let mut rhs = rhs.to_vec();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|&a, &b| {
                matrix[a * n + column]
                    .abs()
                    .total_cmp(&matrix[b * n + column].abs())
            })
            .unwrap();
        for index in 0..n {
            matrix.swap(column * n + index, pivot * n + index);
        }
        rhs.swap(column, pivot);
        let pivot_value = matrix[column * n + column];
        for row in (column + 1)..n {
            let factor = matrix[row * n + column] / pivot_value;
            for index in column..n {
                matrix[row * n + index] -= factor * matrix[column * n + index];
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    let mut result = vec![0.0; n];
    for row in (0..n).rev() {
        result[row] = (rhs[row]
            - ((row + 1)..n)
                .map(|column| matrix[row * n + column] * result[column])
                .sum::<f64>())
            / matrix[row * n + row];
    }
    result
}

fn main() {
    let circuit = Tone::Wide
        .build()
        .expect("tone network exists")
        .expect("tone network builds");
    let simulation = Simulation::new(circuit, 48_000.0);
    let (matrix, source, output) = simulation.linear_system().expect("tone is linear");
    let internal: Vec<usize> = (0..source.len()).filter(|&index| index != output).collect();
    let (reduced, reduced_rhs, condensed) =
        condense(&matrix, &source, &internal, &[output]).expect("partition is nonsingular");
    let full = solve(&matrix, &source);
    let boundary = solve(&reduced, &reduced_rhs);
    let recovered = condensed.recover(&boundary).expect("recovery succeeds");
    let error = recovered
        .iter()
        .zip(full.iter())
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f64::max);

    println!(
        "Tone Wide: {} unknowns -> {} boundary unknown, max error {:.3e}, full output {:.6}, condensed output {:.6}",
        source.len(),
        reduced_rhs.len(),
        error,
        full[output],
        recovered[output]
    );
}
