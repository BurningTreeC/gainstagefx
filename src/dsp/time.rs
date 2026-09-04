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

use super::device::{Bipolar, Core, Device, Diode, Jfet, OpAmp, Stamper, Triode};
use super::netlist::{Circuit, Part, GROUND};

/// How still the solve has to get before it is called settled, in volts.
const TOLERANCE: f64 = 1e-6;
/// How many passes a sample may take before the last good answer is used
/// instead. A circuit driven somewhere absurd should go quiet, not explode.
const MAX_ITERATIONS: usize = 32;
/// The operating point is hunted once and can afford to be patient.
const DC_ITERATIONS: usize = 500;

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
    /// The same, as the circuit stands at DC: capacitors open and inductors
    /// shorted, rather than carrying the companion conductances they have when
    /// time is running.
    base_dc: Vec<f64>,
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
    /// What the rails inject, which does not depend on the signal.
    bias: Vec<f64>,
    devices: Vec<Box<dyn Device>>,
    /// Scratch for a Newton pass.
    work: Vec<f64>,
    guess: Vec<f64>,
    /// Scratch for the substitution, and the last settled answer.
    ///
    /// Held rather than allocated where they are used. Both of these were
    /// heap allocations on the audio thread, once per Newton pass and once per
    /// sample -- about one and a half million a second in stereo at four times
    /// oversampling. Allocating there is not merely slow: `malloc` can take a
    /// lock, and a lock on the audio thread is a dropout.
    scratch: Vec<f64>,
    previous: Vec<f64>,
    /// The answer before that, so the next sample can be started from where
    /// the last two were heading rather than from where the last one was.
    earlier: Vec<f64>,
    /// Whether the next solve may be started from an extrapolation. False if
    /// anything in the circuit switches rather than bends.
    predictable: bool,
    dirty: bool,
    /// How much work the solver is actually doing, which is the only way to
    /// tell a circuit that costs what it should from one iterating itself to
    /// a standstill.
    solves: u64,
    newton_passes: u64,
    unsettled: u64,
    rebuilds: u64,
}

impl Simulation {
    pub fn new(circuit: Circuit, rate: f64) -> Self {
        let n = circuit.unknowns();
        let controls = vec![0.5; circuit.controls.max(1)];
        let mut sim = Self {
            circuit,
            rate,
            n,
            base: vec![0.0; n * n],
            base_dc: vec![0.0; n * n],
            matrix: vec![0.0; n * n],
            pivots: vec![0; n],
            rhs: vec![0.0; n],
            voltage: vec![0.0; n],
            capacitors: Vec::new(),
            inductors: Vec::new(),
            controls,
            source: vec![0.0; n],
            bias: vec![0.0; n],
            devices: Vec::new(),
            work: vec![0.0; n * n],
            guess: vec![0.0; n],
            scratch: vec![0.0; n],
            previous: vec![0.0; n],
            earlier: vec![0.0; n],
            predictable: false,
            dirty: true,
            solves: 0,
            newton_passes: 0,
            unsettled: 0,
            rebuilds: 0,
        };
        sim.rebuild();
        sim.find_operating_point();
        sim
    }

    /// Solves, Newton passes, solves that never settled, and matrix rebuilds.
    pub fn statistics(&self) -> (u64, u64, u64, u64) {
        (self.solves, self.newton_passes, self.unsettled, self.rebuilds)
    }

    pub fn is_linear(&self) -> bool {
        self.devices.is_empty()
    }

    /// What a node is sitting at. Worth having: an operating point is the
    /// first thing to check when a stage sounds wrong, and it is a number the
    /// data sheet also has.
    pub fn voltage_at(&self, node: usize) -> f64 {
        self.voltage.get(node).copied().unwrap_or(0.0)
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
        self.rebuilds += 1;
        let n = self.n;
        self.base.iter_mut().for_each(|x| *x = 0.0);
        self.base_dc.iter_mut().for_each(|x| *x = 0.0);
        self.source.iter_mut().for_each(|x| *x = 0.0);
        self.bias.iter_mut().for_each(|x| *x = 0.0);
        self.capacitors.clear();
        self.inductors.clear();
        self.devices.clear();
        let step = 1.0 / self.rate;

        let mut base = std::mem::take(&mut self.base);
        let mut base_dc = std::mem::take(&mut self.base_dc);
        let mut source = std::mem::take(&mut self.source);
        let mut bias = std::mem::take(&mut self.bias);
        {
            // `into` says which of the two matrices a part belongs in. A
            // capacitor's companion conductance is 2C/T, which for a 22 uF
            // bypass at 96 kHz is four siemens -- a quarter of an ohm. Leaving
            // that in the matrix while hunting the operating point shorts the
            // cathode to ground and the stage comes up with no bias at all.

            for (index, part) in self.circuit.parts.iter().enumerate() {
                match *part {
                    Part::Resistor { a, b, ohms } => {
                        stamp_both(&mut base, &mut base_dc, n, a, b, 1.0 / ohms)
                    }
                    Part::Pot { a, wiper, b, ohms, taper, control } => {
                        let position = self.controls.get(control).copied().unwrap_or(0.5);
                        let f = taper.fraction(position).clamp(1e-4, 1.0 - 1e-4);
                        stamp_both(&mut base, &mut base_dc, n, wiper, b, 1.0 / (ohms * f));
                        stamp_both(&mut base, &mut base_dc, n, a, wiper, 1.0 / (ohms * (1.0 - f)));
                    }
                    Part::Input { node, series } => {
                        let g = 1.0 / series;
                        stamp_both(&mut base, &mut base_dc, n, node, GROUND, g);
                        if node != GROUND {
                            source[node] += g;
                        }
                    }
                    Part::Capacitor { a, b, farads } => {
                        // Trapezoidal: 2C/T, with the history current carried
                        // between steps. Only into the running matrix -- at DC
                        // a capacitor is an open circuit.
                        let conductance = 2.0 * farads / step;
                        stamp_matrix(&mut base, n, a, b, conductance);
                        self.capacitors.push(Capacitor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            voltage: 0.0,
                        });
                    }
                    Part::Inductor { a, b, henry } => {
                        // And at DC an inductor is a piece of wire.
                        let conductance = step / (2.0 * henry);
                        stamp_matrix(&mut base, n, a, b, conductance);
                        stamp_matrix(&mut base_dc, n, a, b, 1e6);
                        self.inductors.push(Inductor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            current: 0.0,
                        });
                    }
                    Part::Supply { node, series, volts } => {
                        let g = 1.0 / series;
                        stamp_both(&mut base, &mut base_dc, n, node, GROUND, g);
                        if node != GROUND {
                            bias[node] += g * volts;
                        }
                    }
                    Part::Diode { a, k, spec } => {
                        self.devices.push(Box::new(Diode::new(a, k, spec)));
                    }
                    Part::Triode { p, g, k, spec } => {
                        self.devices.push(Box::new(Triode::new(p, g, k, spec)));
                    }
                    Part::Jfet { d, g, s: source_pin, spec } => {
                        self.devices
                            .push(Box::new(Jfet::new(d, g, source_pin, spec)));
                    }
                    Part::Bipolar { c, b, e, spec } => {
                        self.devices.push(Box::new(Bipolar::new(c, b, e, spec)));
                    }
                    Part::Core { a, b, spec } => {
                        // The only device that has to be told the rate: it
                        // integrates the voltage across it, so its answer
                        // depends on how long a sample lasts.
                        self.devices.push(Box::new(Core::new(a, b, spec, self.rate)));
                    }
                    Part::OpAmp { out, plus, minus, rail } => {
                        let branch = self.circuit.branch_of(index);
                        self.devices
                            .push(Box::new(OpAmp::new(out, plus, minus, branch, rail)));
                    }
                    Part::Transformer { p1, p2, s1, s2, ratio } => {
                        let branch = self.circuit.branch_of(index);
                        stamp_branch(&mut base, n, branch, p1, p2, s1, s2, ratio);
                        stamp_branch(&mut base_dc, n, branch, p1, p2, s1, s2, ratio);
                    }
                }
            }
        }
        self.predictable = !self.devices.iter().any(|d| d.switches());
        self.base = base;
        self.base_dc = base_dc;
        self.source = source;
        self.bias = bias;

        // A linear circuit is factorised once and reused. A circuit with a
        // device in it has to be refactorised every Newton pass, because the
        // device's own conductance is part of the matrix and moves with the
        // answer.
        self.matrix.copy_from_slice(&self.base);
        factorise(&mut self.matrix, &mut self.pivots, n);
        self.dirty = false;
    }

    /// Where the circuit sits with no signal on it.
    ///
    /// Hunted before any audio arrives, with the reactances held at their
    /// steady state, so the first sample starts from the operating point
    /// rather than charging up through it. Without this a valve stage spends
    /// its first tenth of a second climbing to its own bias, and whatever is
    /// listening hears that as a thump.
    pub fn find_operating_point(&mut self) -> bool {
        // The matrix has to be the one the controls currently describe. Asking
        // for an operating point after moving a control, without rebuilding
        // first, solves the circuit as it used to be -- and the answer looks
        // perfectly converged, because it is: it is the converged answer to
        // the wrong question. What gives it away is Kirchhoff's law, which the
        // returned voltages then quietly fail.
        if self.dirty {
            self.rebuild();
        }
        if self.devices.is_empty() {
            return true;
        }
        let mut settled = false;
        // At rest a capacitor carries no current and an inductor no voltage,
        // which is what the zeroed histories already say.
        for _ in 0..DC_ITERATIONS {
            if self.iterate(0.0, true) {
                settled = true;
                break;
            }
        }
        // The reactances take up whatever the operating point implies, so they
        // do not then charge through it.
        for c in &mut self.capacitors {
            let v = across(&self.voltage, c.a, c.b);
            c.voltage = v;
            c.history = c.conductance * v;
        }
        for l in &mut self.inductors {
            l.history = -l.current;
        }
        settled
    }

    /// One Newton pass. Returns whether it has stopped moving.
    fn iterate(&mut self, input: f64, dc: bool) -> bool {
        let n = self.n;
        self.work
            .copy_from_slice(if dc { &self.base_dc } else { &self.base });
        for k in 0..n {
            self.rhs[k] = self.source[k] * input + self.bias[k];
        }
        if !dc {
            for c in &self.capacitors {
                inject(&mut self.rhs, c.a, c.b, c.history);
            }
            for l in &self.inductors {
                inject(&mut self.rhs, l.a, l.b, l.history);
            }
        }

        {
            let mut stamper = Stamper { matrix: &mut self.work, rhs: &mut self.rhs, n };
            for device in &mut self.devices {
                device.stamp(&mut stamper, &self.voltage);
            }
        }

        self.guess.copy_from_slice(&self.rhs);
        factorise(&mut self.work, &mut self.pivots, n);
        substitute(&self.work, &self.pivots, &mut self.guess, n, &mut self.scratch);

        let mut moved: f64 = 0.0;
        for k in 0..n {
            moved = moved.max((self.guess[k] - self.voltage[k]).abs());
        }
        self.voltage.copy_from_slice(&self.guess);
        // Every device has to agree it is done, and each says so in the way
        // that suits it -- see `Device::settled`.
        moved < TOLERANCE && self.devices.iter().all(|d| d.settled(TOLERANCE))
    }

    /// One sample in, one out.
    pub fn process(&mut self, input: f64) -> f64 {
        self.solves += 1;
        if self.dirty {
            self.rebuild();
            self.find_operating_point();
        }
        let n = self.n;
        if self.devices.is_empty() {
            // Nothing bends, so the matrix from `rebuild` still stands and one
            // substitution is the whole solve.
            for k in 0..n {
                self.rhs[k] = self.source[k] * input + self.bias[k];
            }
            for c in &self.capacitors {
                inject(&mut self.rhs, c.a, c.b, c.history);
            }
            for l in &self.inductors {
                inject(&mut self.rhs, l.a, l.b, l.history);
            }
            substitute(&self.matrix, &self.pivots, &mut self.rhs, n, &mut self.scratch);
            self.voltage.copy_from_slice(&self.rhs);
        } else {
            // Start from where the last two samples were heading, not from
            // where the last one was. Audio is smooth over a sample, so a
            // straight line through the last two lands much closer to the
            // answer than the last one does, and Newton then has less to do:
            // measured, the passes drop from 2.9 a sample to the floor of 2.
            //
            // Only where nothing in the circuit switches. Carried through an
            // op-amp deciding which rail it is against, the prediction put its
            // output at -90 V against a 3.5 V rail and left it there: two
            // passes is enough for a device that bends and not for one that
            // steps, and a settled wrong state still reports itself settled.
            if self.predictable {
                for k in 0..n {
                    let predicted = 2.0 * self.voltage[k] - self.earlier[k];
                    self.earlier[k] = self.voltage[k];
                    self.voltage[k] = predicted;
                }
            } else {
                self.earlier.copy_from_slice(&self.voltage);
            }
            self.previous.copy_from_slice(&self.earlier);
            let mut settled = false;
            for _ in 0..MAX_ITERATIONS {
                self.newton_passes += 1;
                if self.iterate(input, false) {
                    settled = true;
                    break;
                }
            }
            if !settled {
                self.unsettled += 1;
            }
            if !settled && !self.voltage.iter().all(|v| v.is_finite()) {
                // Driven somewhere it cannot follow. Hold the last answer
                // rather than let a runaway out into the audio.
                self.voltage.copy_from_slice(&self.previous);
            }
        }

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
        // And what each device remembers. Most of them remember nothing -- a
        // diode's current depends on its voltage now and on nothing else -- so
        // this went uncalled for a long time without any of them noticing. A
        // saturating core is the first device here with a state, and it
        // integrates the voltage across it: left unadvanced its flux never
        // accumulated past a single sample, so it drew a hundred-thousandth of
        // the current it should have and iron in the signal path did nothing
        // whatsoever.
        for device in &mut self.devices {
            device.advance();
        }

        self.voltage[self.circuit.output]
    }

    /// Empties every reactance, so the next sample starts from rest.
    /// Empties every reactance and puts the circuit back at its operating
    /// point, so the next sample starts from rest rather than from flat.
    ///
    /// Those are not the same thing, and the difference is audible. A valve
    /// plate sits a couple of hundred volts above ground, and what keeps that
    /// out of the output is the charge standing on the coupling capacitor.
    /// Zero the capacitors without re-establishing the operating point and the
    /// circuit charges up through them from nothing over the next few
    /// milliseconds -- which arrives at the output as a loud pop, once, at the
    /// moment the host starts the transport and calls this.
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
        for device in &mut self.devices {
            device.advance();
        }
        if self.dirty {
            self.rebuild();
        }
        self.find_operating_point();
    }
}

/// An ideal transformer's rows: the primary is `ratio` times the secondary in
/// volts, and one over that in amps.
#[allow(clippy::too_many_arguments)]
fn stamp_branch(
    m: &mut [f64],
    n: usize,
    branch: usize,
    p1: usize,
    p2: usize,
    s1: usize,
    s2: usize,
    ratio: f64,
) {
    for (node, sign) in [(p1, 1.0), (p2, -1.0), (s1, -ratio), (s2, ratio)] {
        if node != GROUND {
            m[node * n + branch] += sign;
            m[branch * n + node] += sign;
        }
    }
}

/// Adds a conductance between two nodes.
fn stamp_matrix(m: &mut [f64], n: usize, a: usize, b: usize, g: f64) {
    if a != GROUND {
        m[a * n + a] += g;
    }
    if b != GROUND {
        m[b * n + b] += g;
    }
    if a != GROUND && b != GROUND {
        m[a * n + b] -= g;
        m[b * n + a] -= g;
    }
}

/// Into both the running matrix and the one the operating point is solved on.
/// Only the reactances differ between them.
fn stamp_both(base: &mut [f64], dc: &mut [f64], n: usize, a: usize, b: usize, g: f64) {
    stamp_matrix(base, n, a, b, g);
    stamp_matrix(dc, n, a, b, g);
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
            // A circuit matrix is mostly zeros: a part only ever stamps the
            // nodes it is connected to, so most rows have nothing below the
            // pivot at all. Eliminating with a zero multiplier subtracts
            // nothing from every remaining column, which for these sizes is
            // most of the arithmetic in the factorisation.
            if factor == 0.0 {
                continue;
            }
            for k in (col + 1)..n {
                m[row * n + k] -= factor * m[col * n + k];
            }
        }
    }
}

fn substitute(m: &[f64], pivots: &[usize], rhs: &mut [f64], n: usize, permuted: &mut [f64]) {
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
    rhs.copy_from_slice(&permuted[..n]);
}
