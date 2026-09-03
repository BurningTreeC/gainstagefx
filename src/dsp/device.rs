//! The parts that bend.
//!
//! Each one is linearised at the working point every Newton iteration: a
//! conductance for the slope of its curve there, and a current source for the
//! offset between that tangent and the curve itself. Iterate and the guess
//! walks onto the real answer.
//!
//! The slopes are taken by finite difference rather than by hand. An analytic
//! Jacobian is faster and is what the previous version used, but it is also
//! one more expression to get wrong silently -- a derivative that disagrees
//! with its own function converges to something, just not to the right thing.
//! Three evaluations of a closed-form curve is not the expensive part of a
//! circuit solve.

use super::netlist::{DiodeSpec, TriodeSpec, GROUND};

/// Thermal voltage at room temperature.
const VT: f64 = 0.025_852;

/// Where a linearised device puts its conductances and currents.
pub struct Stamper<'a> {
    pub matrix: &'a mut [f64],
    pub rhs: &'a mut [f64],
    pub n: usize,
}

impl Stamper<'_> {
    pub fn conductance(&mut self, a: usize, b: usize, g: f64) {
        let n = self.n;
        if a != GROUND {
            self.matrix[a * n + a] += g;
        }
        if b != GROUND {
            self.matrix[b * n + b] += g;
        }
        if a != GROUND && b != GROUND {
            self.matrix[a * n + b] -= g;
            self.matrix[b * n + a] -= g;
        }
    }

    /// A transconductance: current between `a` and `b` controlled by the
    /// voltage between `c` and `d`. Asymmetric, which is why the matrix cannot
    /// be factored as if it were symmetric.
    pub fn transconductance(&mut self, a: usize, b: usize, c: usize, d: usize, gm: f64) {
        let n = self.n;
        for (row, sign) in [(a, 1.0), (b, -1.0)] {
            if row == GROUND {
                continue;
            }
            if c != GROUND {
                self.matrix[row * n + c] += sign * gm;
            }
            if d != GROUND {
                self.matrix[row * n + d] -= sign * gm;
            }
        }
    }

    /// A current flowing out of `a` and into `b`.
    pub fn current(&mut self, a: usize, b: usize, amps: f64) {
        if a != GROUND {
            self.rhs[a] -= amps;
        }
        if b != GROUND {
            self.rhs[b] += amps;
        }
    }
}

pub trait Device: Send {
    /// Linearise at the present guess and stamp it.
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]);
    /// How far the last stamp's terminal voltages moved, so the solver can
    /// tell whether it has stopped moving.
    fn moved(&self) -> f64;
    /// Called once the sample is settled.
    fn advance(&mut self) {}
}

fn across(v: &[f64], a: usize, b: usize) -> f64 {
    let va = if a == GROUND { 0.0 } else { v[a] };
    let vb = if b == GROUND { 0.0 } else { v[b] };
    va - vb
}

/// Keeps a junction voltage from running away between iterations.
///
/// An exponential has no patience: a guess a volt too high asks for a current
/// of about ten to the seventeenth, the next iteration overcorrects, and the
/// solve oscillates or overflows. Limiting the step is what makes Newton
/// usable on a diode at all.
fn limit(new: f64, old: f64, scale: f64) -> f64 {
    let step = new - old;
    let most = 2.0 * scale;
    if step > most {
        old + most
    } else if step < -most {
        old - most
    } else {
        new
    }
}

pub struct Diode {
    a: usize,
    k: usize,
    spec: DiodeSpec,
    voltage: f64,
    delta: f64,
}

impl Diode {
    pub fn new(a: usize, k: usize, spec: DiodeSpec) -> Self {
        Self { a, k, spec, voltage: 0.0, delta: 0.0 }
    }

    fn current(&self, v: f64) -> f64 {
        let scale = self.spec.emission * VT;
        // Held below the point where the exponential overflows; past it the
        // device is a short anyway and the limiter above is what walks the
        // solve back down.
        let x = (v / scale).min(60.0);
        self.spec.saturation * (x.exp() - 1.0)
    }
}

impl Device for Diode {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let scale = self.spec.emission * VT;
        let guess = limit(across(v, self.a, self.k), self.voltage, scale);
        self.delta = (guess - self.voltage).abs();
        self.voltage = guess;

        let i = self.current(guess);
        let step = scale * 1e-3;
        let g = ((self.current(guess + step) - self.current(guess - step)) / (2.0 * step))
            .max(1e-12);
        s.conductance(self.a, self.k, g);
        s.current(self.a, self.k, i - g * guess);
    }

    fn moved(&self) -> f64 {
        self.delta
    }
}

/// A triode, by Koren's equations.
///
/// The plate current follows a three-halves power law rather than an
/// exponential, which is why a valve's harmonics fill in gradually from the
/// bottom of its range where a transistor's arrive all at once. The grid draws
/// current once it goes positive with respect to the cathode, and that is what
/// a stage does when it is driven past its bias -- so it is here rather than
/// left out as a detail.
pub struct Triode {
    p: usize,
    g: usize,
    k: usize,
    spec: TriodeSpec,
    vpk: f64,
    vgk: f64,
    delta: f64,
}

impl Triode {
    pub fn new(p: usize, g: usize, k: usize, spec: TriodeSpec) -> Self {
        Self { p, g, k, spec, vpk: 0.0, vgk: -1.0, delta: 0.0 }
    }

    /// Plate current for a pair of terminal voltages.
    pub fn plate(&self, vpk: f64, vgk: f64) -> f64 {
        let s = &self.spec;
        if vpk <= 0.0 {
            return 0.0;
        }
        let inner = s.kp * (1.0 / s.mu + vgk / (s.kvb + vpk * vpk).sqrt());
        // ln(1 + e^x) without overflowing for large x.
        let soft = if inner > 30.0 { inner } else { inner.exp().ln_1p() };
        let e1 = vpk / s.kp * soft;
        if e1 <= 0.0 {
            0.0
        } else {
            2.0 * e1.powf(s.ex) / s.kg1
        }
    }

    /// Grid current. Nothing until the grid goes positive, then it conducts
    /// like the junction it is.
    pub fn grid(&self, vgk: f64) -> f64 {
        if vgk < 0.0 {
            0.0
        } else {
            // A stopper's worth of resistance once it is on.
            vgk / 1_500.0
        }
    }
}

impl Device for Triode {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let vpk = across(v, self.p, self.k).max(0.0);
        let vgk = limit(across(v, self.g, self.k), self.vgk, 0.5);
        self.delta = (vpk - self.vpk).abs().max((vgk - self.vgk).abs());
        self.vpk = vpk;
        self.vgk = vgk;

        let ip = self.plate(vpk, vgk);
        // Slopes by finite difference, on steps small against the curvature.
        let dp = (vpk.abs() * 1e-4).max(1e-3);
        let dg = 1e-4;
        let rp = ((self.plate(vpk + dp, vgk) - self.plate(vpk - dp, vgk)) / (2.0 * dp)).max(1e-12);
        let gm = (self.plate(vpk, vgk + dg) - self.plate(vpk, vgk - dg)) / (2.0 * dg);

        // The plate's own conductance, the transconductance the grid has over
        // it, and whatever is left over as a source.
        s.conductance(self.p, self.k, rp);
        s.transconductance(self.p, self.k, self.g, self.k, gm);
        s.current(self.p, self.k, ip - rp * vpk - gm * vgk);

        let ig = self.grid(vgk);
        let gg = ((self.grid(vgk + dg) - self.grid(vgk - dg)) / (2.0 * dg)).max(1e-12);
        s.conductance(self.g, self.k, gg);
        s.current(self.g, self.k, ig - gg * vgk);
    }

    fn moved(&self) -> f64 {
        self.delta
    }
}
