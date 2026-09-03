//! Running a circuit sample by sample.
//!
//! The same modified nodal analysis the AC solver uses, in real arithmetic and
//! stepped through time: reactances become companion models -- a conductance
//! and a current source standing in for what the part remembers -- and the
//! matrix is refactorised whenever something about it changes.
//!
//! Having both solvers is worth more than either alone. A linear network has
//! one right answer, and the two arrive at it by completely different routes:
//! one solves a complex system once per frequency, the other integrates
//! through time and has its spectrum taken. When they agree, both are working.
//! `the_two_solvers_agree` is the test that says so, and it is the closest
//! thing to an independent check either of them can have.

use super::netlist::{Circuit, Part, GROUND};

/// A capacitor's companion, by the trapezoidal rule.
#[derive(Clone, Copy)]
struct Capacitor {
    a: usize,
    b: usize,
    conductance: f64,
    /// Current the companion source carries into the next step.
    history: f64,
    voltage: f64,
}

/// An inductor's, likewise.
#[derive(Clone, Copy)]
struct Inductor {
    a: usize,
    b: usize,
    conductance: f64,
    history: f64,
    current: f64,
}

pub struct Simulation {
    circuit: Circuit,
    rate: f64,
    n: usize,
    /// Everything that does not change from sample to sample.
    base: Vec<f64>,
    /// The working matrix, and its factorisation.
    matrix: Vec<f64>,
    pivots: Vec<usize>,
    rhs: Vec<f64>,
    voltage: Vec<f64>,
    capacitors: Vec<Capacitor>,
    inductors: Vec<Inductor>,
    controls: Vec<f64>,
    /// The drive the input source injects for one volt in.
    source: Vec<f64>,
    dirty: bool,
}

impl Simulation {
    pub fn new(circuit: Circuit, rate: f64) -> Self {
        let n = circuit.nodes;
        let controls = vec![0.5; circuit.controls.max(1)];
        let mut sim = Self {
            circuit,
            rate,
            n,
            base: vec![0.0; n * n],
            matrix: vec![0.0; n * n],
            pivots: vec![0; n],
            rhs: vec![0.0; n],
            voltage: vec![0.0; n],
            capacitors: Vec::new(),
            inductors: Vec::new(),
            controls,
            source: vec![0.0; n],
            dirty: true,
        };
        sim.rebuild();
        sim
    }

    pub fn controls(&self) -> usize {
        self.circuit.controls
    }

    pub fn set_control(&mut self, which: usize, position: f64) {
        if which < self.controls.len() && (self.controls[which] - position).abs() > 1e-12 {
            self.controls[which] = position.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        if (self.rate - rate).abs() > 1e-9 {
            self.rate = rate;
            self.dirty = true;
        }
    }

    /// Stamps everything that does not depend on the solution, and factorises
    /// it once. A pot moving or the rate changing is what invalidates this.
    fn rebuild(&mut self) {
        let n = self.n;
        self.base.iter_mut().for_each(|x| *x = 0.0);
        self.source.iter_mut().for_each(|x| *x = 0.0);
        self.capacitors.clear();
        self.inductors.clear();
        let step = 1.0 / self.rate;

        let mut base = std::mem::take(&mut self.base);
        let mut source = std::mem::take(&mut self.source);
        {
            let mut stamp = |a: usize, b: usize, g: f64| {
                if a != GROUND {
                    base[a * n + a] += g;
                }
                if b != GROUND {
                    base[b * n + b] += g;
                }
                if a != GROUND && b != GROUND {
                    base[a * n + b] -= g;
                    base[b * n + a] -= g;
                }
            };

            for part in &self.circuit.parts {
                match *part {
                    Part::Resistor { a, b, ohms } => stamp(a, b, 1.0 / ohms),
                    Part::Pot { a, wiper, b, ohms, taper, control } => {
                        let position = self.controls.get(control).copied().unwrap_or(0.5);
                        let f = taper.fraction(position).clamp(1e-4, 1.0 - 1e-4);
                        stamp(wiper, b, 1.0 / (ohms * f));
                        stamp(a, wiper, 1.0 / (ohms * (1.0 - f)));
                    }
                    Part::Input { node, series } => {
                        let g = 1.0 / series;
                        stamp(node, GROUND, g);
                        if node != GROUND {
                            source[node] += g;
                        }
                    }
                    Part::Capacitor { a, b, farads } => {
                        // Trapezoidal: 2C/T, with the history current carried
                        // between steps.
                        let conductance = 2.0 * farads / step;
                        stamp(a, b, conductance);
                        self.capacitors.push(Capacitor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            voltage: 0.0,
                        });
                    }
                    Part::Inductor { a, b, henry } => {
                        let conductance = step / (2.0 * henry);
                        stamp(a, b, conductance);
                        self.inductors.push(Inductor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            current: 0.0,
                        });
                    }
                }
            }
        }
        self.base = base;
        self.source = source;

        self.matrix.copy_from_slice(&self.base);
        factorise(&mut self.matrix, &mut self.pivots, n);
        self.dirty = false;
    }

    /// One sample in, one out.
    pub fn process(&mut self, input: f64) -> f64 {
        if self.dirty {
            self.rebuild();
        }
        let n = self.n;
        for k in 0..n {
            self.rhs[k] = self.source[k] * input;
        }
        for c in &self.capacitors {
            inject(&mut self.rhs, c.a, c.b, c.history);
        }
        for l in &self.inductors {
            inject(&mut self.rhs, l.a, l.b, l.history);
        }
        substitute(&self.matrix, &self.pivots, &mut self.rhs, n);
        self.voltage.copy_from_slice(&self.rhs);

        // Advance what each reactance remembers.
        //
        // The convention throughout is that a branch's current leaving node
        // `a` is `G * v - history`, so `history` is exactly what gets injected
        // into the right hand side. Both companions are written to that shape
        // so one `inject` serves them, and getting the shape wrong is easy: a
        // capacitor whose recurrence carries the wrong term reads 24 dB down
        // on a network that should be flat, and an inductor's diverges to NaN
        // within a few samples.
        for c in &mut self.capacitors {
            let v = across(&self.voltage, c.a, c.b);
            // Trapezoidal: i = 2C/T (v - v_prev) - i_prev, which rearranges to
            // i = G v - (G v_prev + i_prev).
            let current = c.conductance * v - c.history;
            c.history = c.conductance * v + current;
            c.voltage = v;
        }
        for l in &mut self.inductors {
            let v = across(&self.voltage, l.a, l.b);
            // i = i_prev + T/2L (v + v_prev) = G v + (i_prev + G v_prev), so
            // the injected term is the negative of that bracket.
            let current = l.conductance * v - l.history;
            l.history = -(current + l.conductance * v);
            l.current = current;
        }

        self.voltage[self.circuit.output]
    }

    /// Empties every reactance, so the next sample starts from rest.
    pub fn reset(&mut self) {
        for c in &mut self.capacitors {
            c.history = 0.0;
            c.voltage = 0.0;
        }
        for l in &mut self.inductors {
            l.history = 0.0;
            l.current = 0.0;
        }
        self.voltage.iter_mut().for_each(|v| *v = 0.0);
    }
}

fn across(v: &[f64], a: usize, b: usize) -> f64 {
    let va = if a == GROUND { 0.0 } else { v[a] };
    let vb = if b == GROUND { 0.0 } else { v[b] };
    va - vb
}

fn inject(rhs: &mut [f64], a: usize, b: usize, current: f64) {
    if a != GROUND {
        rhs[a] += current;
    }
    if b != GROUND {
        rhs[b] -= current;
    }
}

/// LU with partial pivoting, in place.
///
/// Pivoting is not optional here. The diagonal carries zeros wherever a node's
/// own admittance cancels, and an unpivoted elimination divides by whatever
/// happens to be sitting there.
fn factorise(m: &mut [f64], pivots: &mut [usize], n: usize) {
    for (k, p) in pivots.iter_mut().enumerate() {
        *p = k;
    }
    for col in 0..n {
        let mut best = col;
        let mut magnitude = m[col * n + col].abs();
        for row in (col + 1)..n {
            let candidate = m[row * n + col].abs();
            if candidate > magnitude {
                best = row;
                magnitude = candidate;
            }
        }
        if best != col {
            for k in 0..n {
                m.swap(col * n + k, best * n + k);
            }
            pivots.swap(col, best);
        }
        let pivot = m[col * n + col];
        if pivot.abs() < 1e-30 {
            continue;
        }
        for row in (col + 1)..n {
            let factor = m[row * n + col] / pivot;
            m[row * n + col] = factor;
            for k in (col + 1)..n {
                m[row * n + k] -= factor * m[col * n + k];
            }
        }
    }
}

fn substitute(m: &[f64], pivots: &[usize], rhs: &mut [f64], n: usize) {
    let mut permuted = vec![0.0; n];
    for (row, &from) in pivots.iter().enumerate() {
        permuted[row] = rhs[from];
    }
    for row in 1..n {
        let mut sum = permuted[row];
        for k in 0..row {
            sum -= m[row * n + k] * permuted[k];
        }
        permuted[row] = sum;
    }
    for row in (0..n).rev() {
        let mut sum = permuted[row];
        for k in (row + 1)..n {
            sum -= m[row * n + k] * permuted[k];
        }
        let pivot = m[row * n + row];
        permuted[row] = if pivot.abs() < 1e-30 { 0.0 } else { sum / pivot };
    }
    rhs.copy_from_slice(&permuted);
}
