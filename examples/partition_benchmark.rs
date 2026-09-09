//! Benchmark full versus condensed Newton Jacobian solves on amplifier stages.

use gainstagefx::dsp::partition::condense;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Amplifier, Diode, Gain};

const RATE: f64 = 48_000.0;
const REPEATS: usize = 2_000;

fn solve(matrix: &[f64], rhs: &[f64]) -> Option<Vec<f64>> {
    let n = rhs.len();
    let mut matrix = matrix.to_vec();
    let mut rhs = rhs.to_vec();
    for column in 0..n {
        let pivot = (column..n).max_by(|&a, &b| {
            matrix[a * n + column]
                .abs()
                .total_cmp(&matrix[b * n + column].abs())
        })?;
        if matrix[pivot * n + column].abs() < 1e-30 {
            return None;
        }
        for index in 0..n {
            matrix.swap(column * n + index, pivot * n + index);
        }
        rhs.swap(column, pivot);
        for row in (column + 1)..n {
            let factor = matrix[row * n + column] / matrix[column * n + column];
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
    Some(result)
}

fn measure(label: &str, sim: &mut Simulation, input: f64) {
    let Some((matrix, rhs, output)) = sim.jacobian_snapshot(input) else {
        println!("  {label:<12} no nonlinear Jacobian");
        return;
    };
    let n = rhs.len();
    let internal: Vec<usize> = (0..n).filter(|&index| index != output).collect();
    let started = std::time::Instant::now();
    let Some((reduced, reduced_rhs, recovery)) = condense(&matrix, &rhs, &internal, &[output])
    else {
        println!("  {label:<12} {} unknowns, partition singular", n);
        return;
    };
    let setup = started.elapsed();
    let full = solve(&matrix, &rhs).expect("full Jacobian solves");
    let boundary = solve(&reduced, &reduced_rhs).expect("reduced Jacobian solves");
    let recovered = recovery.recover(&boundary).expect("recovery succeeds");
    let error = full
        .iter()
        .zip(recovered.iter())
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f64::max);

    let started = std::time::Instant::now();
    let mut full_sink = 0.0;
    for _ in 0..REPEATS {
        full_sink += std::hint::black_box(solve(&matrix, &rhs).unwrap()[output]);
    }
    let full_time = started.elapsed();
    let started = std::time::Instant::now();
    let mut reduced_sink = 0.0;
    for _ in 0..REPEATS {
        let reduced_rhs = recovery.reduce_rhs(&rhs).unwrap();
        let boundary = solve(&reduced, &reduced_rhs).unwrap();
        reduced_sink +=
            std::hint::black_box(recovery.recover_with_rhs(&boundary, &rhs).unwrap()[output]);
    }
    let reduced_time = started.elapsed();
    let sink_error = (full_sink - reduced_sink).abs();
    println!(
        "  {label:<12} {n:>2} -> {:>2} boundary, setup {:>8.2?}, full {:>8.2?}, reduced {:>8.2?}, error {:.2e}, sink {:.2e}",
        reduced_rhs.len(), setup, full_time, reduced_time, error, sink_error
    );
}

fn main() {
    println!("Raw Jacobian partition benchmark at {RATE:.0} Hz, {REPEATS} solves each.");
    for gain in [Gain::Peavey, Gain::Boogie, Gain::Twin] {
        println!("{}:", gain.name());
        let netlist = voice::build_voice(gain, Diode::Silicon, Amplifier::Valve).expect("builds");
        let mut sim = Simulation::new(netlist, RATE);
        sim.set_control(gain.drive_control(), 0.8);
        sim.find_operating_point();
        measure("gain", &mut sim, 0.5);
        if let Some(Ok(netlist)) = voice::build_power(gain) {
            let mut sim = Simulation::new(netlist, RATE);
            sim.find_operating_point();
            measure("power", &mut sim, 1.0);
        }
    }
}
