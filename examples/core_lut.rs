//! Measure a 1D transformer-core lookup table against the analytic model.

use gainstagefx::dsp::device::Core;
use gainstagefx::dsp::netlist::CoreSpec;

const SAMPLES: usize = 4_096;
const REPEATS: usize = 1_000_000;

struct CoreTable {
    core: Core,
    knee: f64,
    limit: f64,
    values: Vec<(f64, f64)>,
}

impl CoreTable {
    fn new(spec: CoreSpec, rate: f64) -> Self {
        let core = Core::new(0, 1, spec, rate);
        let limit = 8.0 * spec.knee;
        let values = (0..SAMPLES)
            .map(|index| {
                let half = SAMPLES / 2;
                let normalized = if index < half {
                    index as f64 / (half - 1) as f64
                } else {
                    1.0 + (index - (half - 1)) as f64 * 7.0 / (SAMPLES - half) as f64
                };
                let flux = normalized * spec.knee;
                core.magnetising_with_slope(flux)
            })
            .collect();
        Self {
            core,
            knee: spec.knee,
            limit,
            values,
        }
    }

    fn lookup(&self, flux: f64) -> (f64, f64) {
        if flux.abs() >= self.limit {
            return self.core.magnetising_with_slope(flux);
        }
        let sign = flux.signum();
        let normalized = flux.abs() / self.knee;
        let half = SAMPLES / 2;
        let position = if normalized <= 1.0 {
            normalized * (half - 1) as f64
        } else {
            (half - 1) as f64 + (normalized - 1.0) * (SAMPLES - half) as f64 / 7.0
        };
        let lower = position.floor() as usize;
        let fraction = position - lower as f64;
        let (current_a, slope_a) = self.values[lower];
        let (current_b, slope_b) = self.values[lower + 1];
        (
            sign * (current_a + fraction * (current_b - current_a)),
            slope_a + fraction * (slope_b - slope_a),
        )
    }
}

fn main() {
    println!("Core LUT: {SAMPLES} samples, +/-8 knees, {REPEATS} evaluations");
    for (name, spec) in [
        ("Steel", CoreSpec::STEEL),
        ("Nickel", CoreSpec::NICKEL),
        ("Amorphous", CoreSpec::AMORPHOUS),
    ] {
        let table = CoreTable::new(spec, 48_000.0);
        let mut max_current_error: f64 = 0.0;
        let mut max_slope_error: f64 = 0.0;
        for index in 0..(SAMPLES * 4) {
            let flux =
                -table.limit * 1.1 + index as f64 * table.limit * 2.2 / (SAMPLES * 4 - 1) as f64;
            let expected = table.core.magnetising_with_slope(flux);
            let actual = table.lookup(flux);
            max_current_error = max_current_error.max((actual.0 - expected.0).abs());
            max_slope_error = max_slope_error.max((actual.1 - expected.1).abs());
        }

        let started = std::time::Instant::now();
        let mut analytic_sink = 0.0;
        for index in 0..REPEATS {
            let flux = ((index % SAMPLES) as f64 / SAMPLES as f64 * 2.0 - 1.0) * table.limit;
            analytic_sink += std::hint::black_box(table.core.magnetising_with_slope(flux).0);
        }
        let analytic_time = started.elapsed();

        let started = std::time::Instant::now();
        let mut lookup_sink = 0.0;
        for index in 0..REPEATS {
            let flux = ((index % SAMPLES) as f64 / SAMPLES as f64 * 2.0 - 1.0) * table.limit;
            lookup_sink += std::hint::black_box(table.lookup(flux).0);
        }
        let lookup_time = started.elapsed();

        println!(
            "  {name:<10} current error {:>9.3e}, slope error {:>9.3e}, analytic {:>8.2?}, lookup {:>8.2?}, sinks {:>.3e}/{:>.3e}",
            max_current_error,
            max_slope_error,
            analytic_time,
            lookup_time,
            analytic_sink,
            lookup_sink,
        );
    }
}
