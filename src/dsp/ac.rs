//! The frequency response of a linear network, solved directly.
//!
//! This is the tool the first version did not have, and its absence explains
//! most of what went wrong with it. The engine already assembles a modified
//! nodal analysis matrix to run audio; a linear network's response is that same
//! matrix stamped with complex admittances -- `jwC` for a capacitor,
//! `1/(jwL)` for an inductor -- and solved once per frequency.
//!
//! What that buys is not speed for its own sake. It is that checking a filter
//! stops being an experiment. Running a tone through a circuit and taking a
//! spectrum has to settle first, has to land on a DFT bin, has to keep its
//! phase continuous across the boundary between settling and measuring, and
//! goes through the oversampler on the way -- and getting any of those wrong
//! reports a number that looks like behaviour. Every one of those mistakes was
//! made at least once. Here there is no tone, no settling and no window: the
//! answer is the solution of a linear system, exact to the arithmetic.
//!
//! It only applies to linear parts. A valve or a diode has no single frequency
//! response, and asking for one would be a category error -- so those parts
//! are simply not stampable here, and a network containing them has to be
//! measured the slow way, around a chosen operating point.

use super::complex::C;
use super::netlist::{Circuit, Part, GROUND};

/// A network's response at one frequency.
#[derive(Clone, Copy, Debug)]
pub struct Response {
    pub hz: f64,
    pub gain: C,
}

impl Response {
    pub fn db(&self) -> f64 {
        self.gain.db()
    }

    pub fn phase(&self) -> f64 {
        self.gain.phase()
    }
}

/// Solves a circuit's response across a list of frequencies.
///
/// `controls` are the pot positions, in the order the netlist declared them.
pub fn sweep(circuit: &Circuit, controls: &[f64], frequencies: &[f64]) -> Vec<Response> {
    frequencies
        .iter()
        .map(|&hz| Response {
            hz,
            gain: solve(circuit, controls, hz),
        })
        .collect()
}

/// The transfer function at one frequency: output volts per volt in.
pub fn solve(circuit: &Circuit, controls: &[f64], hz: f64) -> C {
    assert!(
        circuit.is_linear(),
        "'{}' contains a device, and a device has no single frequency response. \
         Ask the time domain solver instead, around a chosen operating point.",
        circuit.name
    );
    let n = circuit.nodes;
    if n == 0 {
        return C::ZERO;
    }
    let w = std::f64::consts::TAU * hz;

    // Admittance matrix and the current the source injects.
    let mut y = vec![C::ZERO; n * n];
    let mut i = vec![C::ZERO; n];

    let stamp = |y: &mut Vec<C>, a: usize, b: usize, adm: C| {
        if a != GROUND {
            y[a * n + a] = y[a * n + a] + adm;
        }
        if b != GROUND {
            y[b * n + b] = y[b * n + b] + adm;
        }
        if a != GROUND && b != GROUND {
            y[a * n + b] = y[a * n + b] - adm;
            y[b * n + a] = y[b * n + a] - adm;
        }
    };

    for part in &circuit.parts {
        match *part {
            Part::Resistor { a, b, ohms } => stamp(&mut y, a, b, C::real(1.0 / ohms)),
            Part::Capacitor { a, b, farads } => stamp(&mut y, a, b, C::new(0.0, w * farads)),
            Part::Inductor { a, b, henry } => {
                // 1 / (jwL), and at DC an inductor is a short: a very large
                // real admittance stands in, because the alternative is a
                // division by zero at the first frequency anyone asks for.
                let adm = if w * henry > 1e-12 {
                    C::new(0.0, -1.0 / (w * henry))
                } else {
                    C::real(1e12)
                };
                stamp(&mut y, a, b, adm);
            }
            Part::Pot { a, wiper, b, ohms, taper, control } => {
                let position = controls.get(control).copied().unwrap_or(0.5);
                // Clamped off both ends: a pot wound hard against a stop is a
                // zero ohm link, and an infinite conductance is a singular row.
                let f = taper.fraction(position).clamp(1e-4, 1.0 - 1e-4);
                stamp(&mut y, wiper, b, C::real(1.0 / (ohms * f)));
                stamp(&mut y, a, wiper, C::real(1.0 / (ohms * (1.0 - f))));
            }
            Part::Input { node, series } => {
                // A Norton source: one volt behind the source impedance.
                let g = 1.0 / series;
                stamp(&mut y, node, GROUND, C::real(g));
                if node != GROUND {
                    i[node] = i[node] + C::real(g);
                }
            }
            // A rail is a short to ground for signal: its voltage is constant,
            // so it contributes no current here, but its series resistance
            // still loads the node it feeds.
            Part::Supply { node, series, .. } => {
                stamp(&mut y, node, GROUND, C::real(1.0 / series));
            }
            Part::Diode { .. } | Part::Triode { .. } => unreachable!("checked above"),
        }
    }

    match gaussian(&mut y, &mut i, n) {
        Some(()) => i[circuit.output],
        None => C::ZERO,
    }
}

/// Gaussian elimination with partial pivoting, in place.
///
/// Pivoting is not optional. The matrix has zeros on the diagonal wherever a
/// node's own admittance cancels, and the first version learned the same thing
/// in the time domain: Cholesky cannot factor this, and an unpivoted
/// elimination divides by whatever happens to be there.
fn gaussian(y: &mut [C], i: &mut [C], n: usize) -> Option<()> {
    for col in 0..n {
        let mut best = col;
        let mut best_magnitude = y[col * n + col].magnitude();
        for row in (col + 1)..n {
            let m = y[row * n + col].magnitude();
            if m > best_magnitude {
                best = row;
                best_magnitude = m;
            }
        }
        if best_magnitude < 1e-30 {
            return None;
        }
        if best != col {
            for k in 0..n {
                y.swap(col * n + k, best * n + k);
            }
            i.swap(col, best);
        }
        let pivot = y[col * n + col];
        for row in (col + 1)..n {
            let factor = y[row * n + col] / pivot;
            if factor == C::ZERO {
                continue;
            }
            for k in col..n {
                y[row * n + k] = y[row * n + k] - factor * y[col * n + k];
            }
            i[row] = i[row] - factor * i[col];
        }
    }
    for col in (0..n).rev() {
        let mut sum = i[col];
        for k in (col + 1)..n {
            sum = sum - y[col * n + k] * i[k];
        }
        i[col] = sum / y[col * n + col];
    }
    Some(())
}
