//! Baumgarte stabilisation and the convex QP, on the paper's own test circuit.
//!
//! Zea and Rivera, DAFx25 Ancona, "Towards Efficient Emulation of Nonlinear
//! Analog Circuits for Audio Using Constraint Stabilization and Convex
//! Quadratic Programming" -- `DAFx25_paper_45.pdf` in the repository root.
//!
//! The idea, and why it is worth the trouble: a Newton solve iterates an
//! unknown number of times, and that number is what the worst-case column of
//! `examples/deadline.rs` is made of. This method does not iterate at all. It
//! replaces the algebraic constraint `eta(q) = 0` with a stabilised ODE whose
//! solution decays onto the constraint, discretises the nonlinearity at
//! `q[k-1]` rather than `q[k]`, and what is left is a convex quadratic
//! programme -- which, with no cost term and no bounds, is a feasibility
//! problem, which is **one linear solve per sample**. Deterministic, bounded,
//! no iteration variance.
//!
//! ## The circuit
//!
//! The paper's Figure 1: a 1 k resistor into 47 nF, with two 1N4148 in series
//! across the capacitor. `x = V_C`, `u = V_in`, `y = V_C`.
//!
//! ```text
//!        R = 1k        node 1
//!  Vin --/\/\/---+--------+
//!                |        |
//!               ---      D1 (1N4148)
//!               --- C     |  node 2
//!                |       D2 (1N4148)
//!                |        |
//!               gnd      gnd
//! ```
//!
//! Its equations, derived here rather than copied, because the render of the
//! paper does not resolve every subscript:
//!
//! ```text
//!  KVL loop 1:   V_D1 + V_D2 - V_C = 0
//!  KCL node 1:   (Vin - V_C)/R - I_D1 - C dV_C/dt = 0
//!  KCL node 2:   I_D1 - I_D2 = 0
//! ```
//!
//! `cargo run --release --example baumgarte`

use gainstagefx::dsp::netlist::{DiodeSpec, Netlist};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 44_100.0;
const R: f64 = 1_000.0;
const C: f64 = 47e-9;

/// The paper's 1N4148: saturation current and emission coefficient off the
/// data sheet's curves. It prints the emission as `1.75e-3`, which cannot be
/// an emission coefficient -- a diode's is near one to two and ours is 1.752
/// for the same part -- so it is read as 1.75 and noted.
const IS: f64 = 2.9e-9;
const N: f64 = 1.75;
const VT: f64 = 25.85e-3;
/// `n * V_T`, the only combination that appears.
const NVT: f64 = N * VT;
/// What determines the voltage across a diode that is not conducting. Far
/// above anything the junction does; see the note in `process`.
const LEAK: f64 = 10_000_000.0;

/// The state is one capacitor voltage; the nonlinear vector is
/// `q = [V_D1, V_D2, I_D1, I_D2]`.
const NX: usize = 1;
const NQ: usize = 4;
const NZ: usize = NX + NQ;
/// One row for the state equation, two for the linear constraints, two for
/// the stabilised nonlinear one.
const ROWS: usize = NZ;

/// `eta(q) = 0` is the pair of diode equations, written so that it is zero on
/// the constraint: the current the device would pass at its own voltage,
/// against the current the circuit says is flowing.
fn eta(q: &[f64; NQ]) -> [f64; 2] {
    [
        q[2] - IS * ((q[0] / NVT).min(80.0).exp() - 1.0),
        q[3] - IS * ((q[1] / NVT).min(80.0).exp() - 1.0),
    ]
}

/// Its Jacobian, 2 x 4. Analytic, because the exponential is already there.
fn deta(q: &[f64; NQ]) -> [[f64; NQ]; 2] {
    let g = |v: f64| -(IS / NVT) * (v / NVT).min(80.0).exp();
    [[g(q[0]), 0.0, 1.0, 0.0], [0.0, g(q[1]), 0.0, 1.0]]
}

/// Dense Gaussian elimination with partial pivoting on a small square system.
fn solve(a: &mut [[f64; NZ]; ROWS], b: &mut [f64; ROWS]) -> Option<[f64; NZ]> {
    for col in 0..NZ {
        let mut best = col;
        for row in (col + 1)..ROWS {
            if a[row][col].abs() > a[best][col].abs() {
                best = row;
            }
        }
        if a[best][col].abs() < 1e-30 {
            return None;
        }
        a.swap(col, best);
        b.swap(col, best);
        for row in (col + 1)..ROWS {
            let f = a[row][col] / a[col][col];
            if f == 0.0 {
                continue;
            }
            for k in col..NZ {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; NZ];
    for row in (0..NZ).rev() {
        let mut sum = b[row];
        for k in (row + 1)..NZ {
            sum -= a[row][k] * x[k];
        }
        x[row] = sum / a[row][row];
    }
    Some(x)
}

/// One sample of the paper's method. No iteration: assemble and solve once.
struct Baumgarte {
    x: f64,
    q: [f64; NQ],
    dt: f64,
    /// The Hurwitz matrix of (10). The paper's common choice is a negative
    /// multiple of the identity; `-1/dt` makes the constraint decay by a
    /// factor of e every sample.
    k: f64,
    /// How many times the stabilised constraint is applied per sample.
    ///
    /// One is the paper's method: the nonlinearity is evaluated at `q[k-1]`
    /// and never revisited, which is what makes the cost fixed. More than one
    /// re-linearises at the answer just found, which is Newton wearing a
    /// different hat -- but a *bounded* Newton, with no convergence test and
    /// no variance, which is still the property we are after.
    steps: usize,
    failures: usize,
}

impl Baumgarte {
    fn new(rate: f64) -> Self {
        let dt = 1.0 / rate;
        Self {
            x: 0.0,
            q: [0.0; NQ],
            dt,
            k: -1.0 / dt,
            steps: 1,
            failures: 0,
        }
    }

    fn process(&mut self, u: f64) -> f64 {
        for _ in 0..self.steps {
            self.step(u);
        }
        self.x
    }

    fn step(&mut self, u: f64) {
        let dt = self.dt;
        // The state equation, backward Euler: (I - A dt) x - (G dt) q = ...
        //   A = -1/(RC), B = 1/(RC), G = [0, 0, -1/C, 0]
        let mut a = [[0.0f64; NZ]; ROWS];
        let mut b = [0.0f64; ROWS];

        a[0][0] = 1.0 + dt / (R * C);
        a[0][1 + 2] = -dt * (-1.0 / C);
        b[0] = self.x + dt / (R * C) * u;

        // The linear constraints, H q = D x + E u:
        //   V_D1 + V_D2 = V_C      and      I_D1 - I_D2 = 0
        a[1][1] = 1.0;
        a[1][2] = 1.0;
        a[1][0] = -1.0;
        b[1] = 0.0;

        // KCL at node 2, with the same 10 M to ground the Newton reference
        // gets. Without it this row is `I_D1 - I_D2 = 0`, and when both diodes
        // are off `deta/dq` is `-(Is/nVT) e^-55`, about 1e-32: the nonlinear
        // rows then pin the two currents and say nothing whatever about how
        // V_C divides between two series diodes. One equation, two unknowns.
        // The solve does not fail -- it returns a finite answer to a singular
        // system -- and the output went to the full 5 V input.
        //
        // This is the paper's feasibility reduction meeting a circuit it does
        // not cover. Its own escape is the one not used here: a QP has a cost
        // term, and minimising it regularises exactly this degeneracy. A shunt
        // conductance is the circuit's way of saying the same thing, and it is
        // what our own solver has always had.
        a[2][3] = 1.0;
        a[2][4] = -1.0;
        a[2][2] = -1.0 / LEAK;
        b[2] = 0.0;

        // The stabilised nonlinear constraint, evaluated at q[k-1]:
        //   (deta/dq)(q_k - q_{k-1}) = (K dt) eta(q_{k-1})
        let j = deta(&self.q);
        let e = eta(&self.q);
        for r in 0..2 {
            let mut carried = 0.0;
            for c in 0..NQ {
                a[3 + r][1 + c] = j[r][c];
                carried += j[r][c] * self.q[c];
            }
            b[3 + r] = self.k * dt * e[r] + carried;
        }

        match solve(&mut a, &mut b) {
            Some(z) => {
                if z.iter().all(|v| v.is_finite()) {
                    self.x = z[0];
                    self.q = [z[1], z[2], z[3], z[4]];
                } else {
                    self.failures += 1;
                }
            }
            None => self.failures += 1,
        }
    }
}

/// The same circuit through the solver this plugin already has: modified
/// nodal analysis with companion models and a damped Newton iteration.
fn reference(rate: f64) -> Simulation {
    let spec = DiodeSpec {
        saturation: IS,
        emission: N,
    };
    let mut net = Netlist::new("paper clipper");
    net.input("in", 1.0)
        .resistor("in", "n1", R)
        .capacitor("n1", "gnd", C)
        .diode("n1", "n2", spec)
        .diode("n2", "gnd", spec)
        // The junction of the two diodes needs a direct path to ground or its
        // row is empty; ten megohms is far above anything the diodes do.
        .resistor("n2", "gnd", 10_000_000.0);
    Simulation::new(net.build("n1").expect("builds"), rate)
}

thread_local! {
    static STEPS: std::cell::RefCell<usize> = const { std::cell::RefCell::new(1) };
}

fn main() {
    println!("Zea & Rivera's diode clipper, {RATE} Hz, 5 V 10 Hz sine.\n");

    let seconds = 0.4;
    let samples = (RATE * seconds) as usize;
    let input = |k: usize| 5.0 * (std::f64::consts::TAU * 10.0 * k as f64 / RATE).sin();

    let mut qp = Baumgarte::new(RATE);
    let mut newton = reference(RATE);
    newton.find_operating_point();

    let mut a = Vec::with_capacity(samples);
    let mut n = Vec::with_capacity(samples);
    for k in 0..samples {
        a.push(qp.process(input(k)));
        n.push(newton.process(input(k)));
    }

    let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
    let diff: Vec<f64> = a.iter().zip(n.iter()).map(|(x, y)| x - y).collect();
    let (peak_a, peak_n) = (
        a.iter().fold(0.0f64, |m, v| m.max(v.abs())),
        n.iter().fold(0.0f64, |m, v| m.max(v.abs())),
    );

    println!("  {:<22}{:>12}{:>12}", "", "QP", "Newton");
    println!("  {:<22}{:>12.4}{:>12.4}", "peak out, V", peak_a, peak_n);
    println!("  {:<22}{:>12.4}{:>12.4}", "rms out, V", rms(&a), rms(&n));
    println!(
        "  {:<22}{:>12}{:>12.2}",
        "solves a sample",
        1,
        newton.statistics().1 as f64 / samples as f64
    );
    println!(
        "  {:<22}{:>12}{:>12}",
        "non-finite / failed", qp.failures, 0
    );
    println!(
        "\n  difference: {:.4} V rms, {:.1} dB below the Newton solve",
        rms(&diff),
        20.0 * (rms(&diff) / rms(&n)).log10()
    );

    // Where it stops working. The paper tests one signal at one rate; ours
    // are stiffer circuits at higher frequencies, and an explicit evaluation
    // of the nonlinearity is exactly what stiffness punishes.
    for steps in [1usize, 2, 3] {
        STEPS.with(|s| *s.borrow_mut() = steps);
        println!("\n=== {steps} stabilised step(s) a sample, against the Newton solve ===");
        println!(
            "  {:<10}{:>9}{:>9}{:>9}{:>9}{:>9}",
            "rate", "10 Hz", "100 Hz", "1 kHz", "5 kHz", "10 kHz"
        );
        for rate in [8_000.0f64, 22_050.0, 44_100.0, 96_000.0, 192_000.0] {
            print!("  {:<10}", format!("{:.1}k", rate / 1000.0));
            for hz in [10.0f64, 100.0, 1000.0, 5000.0, 10_000.0] {
                if hz * 2.0 >= rate {
                    print!("{:>9}", "--");
                    continue;
                }
                let count = (rate * 0.2) as usize;
                let drive = |k: usize| 5.0 * (std::f64::consts::TAU * hz * k as f64 / rate).sin();
                let mut q = Baumgarte::new(rate);
                q.steps = STEPS.with(|s| *s.borrow());
                let mut nn = reference(rate);
                nn.find_operating_point();
                let (mut qa, mut na) = (Vec::new(), Vec::new());
                for k in 0..count {
                    qa.push(q.process(drive(k)));
                    na.push(nn.process(drive(k)));
                }
                let r = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
                let d: Vec<f64> = qa.iter().zip(na.iter()).map(|(x, y)| x - y).collect();
                let db = if r(&na) > 0.0 && r(&d) > 0.0 {
                    20.0 * (r(&d) / r(&na)).log10()
                } else {
                    -999.0
                };
                print!("{:>8.0} dB", db);
            }
            println!();
        }
        println!("  (difference from the Newton solve. More negative is closer.)");
    }
    STEPS.with(|s| *s.borrow_mut() = 1);

    let max_pos = |v: &[f64]| v.iter().cloned().fold(f64::MIN, f64::max);
    let min_neg = |v: &[f64]| v.iter().cloned().fold(f64::MAX, f64::min);
    println!(
        "\n  clipped at: QP {:+.3} V / {:+.3} V, Newton {:+.3} V / {:+.3} V",
        max_pos(&a),
        min_neg(&a),
        max_pos(&n),
        min_neg(&n)
    );
    // Through the first rise into conduction, where one step from q[k-1] has
    // the most to do.
    println!(
        "\n  {:>8}{:>10}{:>10}{:>10}",
        "sample", "in", "QP", "Newton"
    );
    for k in (0..1400).step_by(100) {
        println!("  {k:>8}{:>10.3}{:>10.3}{:>10.3}", input(k), a[k], n[k]);
    }
}
