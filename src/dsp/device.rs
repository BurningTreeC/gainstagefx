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

use super::netlist::{CoreSpec, DiodeSpec, JfetSpec, TriodeSpec, GROUND};

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

    /// Ties a node to a branch's current: the part sources `sign` amps of it
    /// into that node.
    pub fn branch_current(&mut self, node: usize, branch: usize, sign: f64) {
        if node != GROUND {
            self.matrix[node * self.n + branch] += sign;
        }
    }

    /// One term of a branch's own row, which is the condition the part is
    /// imposing on the voltages.
    pub fn branch_constraint(&mut self, branch: usize, node: usize, sign: f64) {
        if node != GROUND {
            self.matrix[branch * self.n + node] += sign;
        }
    }

    /// What that condition equals.
    pub fn branch_value(&mut self, branch: usize, volts: f64) {
        self.rhs[branch] += volts;
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
    /// Whether this device is done moving.
    ///
    /// A solve whose node voltages have stopped moving is converged -- unless
    /// a device was not allowed to go where it wanted, in which case the
    /// stillness is the limiter's rather than the circuit's.
    ///
    /// The default compares how far the device moved, which is what a device
    /// with a state rather than a limiter needs. For the ones whose step *is*
    /// limited, asking the limiter directly is both more precise and much
    /// cheaper: their `moved` is the distance from the *previous pass*, so it
    /// is stale by one, and testing it forced every sample to take a third
    /// pass purely to notice that the second one had settled.
    fn settled(&self, tolerance: f64) -> bool {
        self.moved() < tolerance
    }
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
fn limit(new: f64, old: f64, scale: f64) -> (f64, bool) {
    let step = new - old;
    let most = 2.0 * scale;
    if step > most {
        (old + most, true)
    } else if step < -most {
        (old - most, true)
    } else {
        (new, false)
    }
}

/// Where the exponential is held, short of overflowing.
const CLAMP: f64 = 60.0;

pub struct Diode {
    a: usize,
    k: usize,
    spec: DiodeSpec,
    voltage: f64,
    delta: f64,
    /// Whether the limiter held the last step back.
    clamped: bool,
}

impl Diode {
    pub fn new(a: usize, k: usize, spec: DiodeSpec) -> Self {
        Self { a, k, spec, voltage: 0.0, delta: 0.0, clamped: false }
    }

    /// The Shockley current, exposed so tests can check the slope the stamp
    /// uses against the curve it claims to be the slope of.
    pub fn current(&self, v: f64) -> f64 {
        let scale = self.spec.emission * VT;
        let x = (v / scale).min(CLAMP);
        self.spec.saturation * (x.exp() - 1.0)
    }
}

impl Device for Diode {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let scale = self.spec.emission * VT;
        let (guess, clamped) = limit(across(v, self.a, self.k), self.voltage, scale);
        self.clamped = clamped;
        self.delta = (guess - self.voltage).abs();
        self.voltage = guess;

        // One exponential, not three. The slope of `Is (e^x - 1)` is
        // `Is e^x / scale`, which is the same exponential the current already
        // needed -- where a central difference asked for two more.
        let x = guess / scale;
        let (i, g) = if x >= CLAMP {
            // Held below the point where the exponential overflows. Past it
            // the current no longer moves with the voltage, so neither does
            // the slope, and the limiter above is what walks the solve back.
            (self.spec.saturation * (CLAMP.exp() - 1.0), 1e-12)
        } else {
            let e = x.exp();
            (
                self.spec.saturation * (e - 1.0),
                (self.spec.saturation * e / scale).max(1e-12),
            )
        };
        s.conductance(self.a, self.k, g);
        s.current(self.a, self.k, i - g * guess);
    }

    fn moved(&self) -> f64 {
        self.delta
    }

    fn settled(&self, _tolerance: f64) -> bool {
        !self.clamped
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
    /// Whether the limiter held the last step back.
    clamped: bool,
}

impl Triode {
    pub fn new(p: usize, g: usize, k: usize, spec: TriodeSpec) -> Self {
        Self { p, g, k, spec, vpk: 0.0, vgk: -1.0, delta: 0.0, clamped: false }
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

impl Triode {
    /// Plate current and both its slopes, from one evaluation.
    ///
    /// The slopes used to come from finite differences, which meant five calls
    /// to `plate` per stamp, each with a `sqrt`, an `exp`, a `ln_1p` and a
    /// `powf` in it. For a three stage preamplifier at four times oversampling
    /// that came to about forty-five million transcendentals a second, and it
    /// was the single largest cost in the plugin.
    ///
    /// Koren's equations differentiate in closed form, and every piece of the
    /// derivative is something the current itself already computed: the slope
    /// of `ln(1 + e^x)` is `e^x / (1 + e^x)`, from the same exponential, and
    /// `E1^(ex-1)` is `E1^ex / E1`. So one evaluation now yields all three,
    /// with one `powf` where there were five.
    fn plate_with_slopes(&self, vpk: f64, vgk: f64) -> (f64, f64, f64) {
        let s = &self.spec;
        if vpk <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let root = (s.kvb + vpk * vpk).sqrt();
        let inner = s.kp * (1.0 / s.mu + vgk / root);
        let (soft, sigma) = if inner > 30.0 {
            (inner, 1.0)
        } else {
            let e = inner.exp();
            (e.ln_1p(), e / (1.0 + e))
        };
        let e1 = vpk / s.kp * soft;
        if e1 <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let powered = e1.powf(s.ex);
        let ip = 2.0 * powered / s.kg1;

        let d_ip = 2.0 * s.ex * powered / (s.kg1 * e1);
        let d_e1_vpk = soft / s.kp - vpk * vpk * vgk * sigma / (root * root * root);
        let d_e1_vgk = vpk * sigma / root;
        (ip, d_ip * d_e1_vpk, d_ip * d_e1_vgk)
    }
}

impl Device for Triode {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let vpk = across(v, self.p, self.k).max(0.0);
        let (vgk, clamped) = limit(across(v, self.g, self.k), self.vgk, 0.5);
        self.clamped = clamped;
        self.delta = (vpk - self.vpk).abs().max((vgk - self.vgk).abs());
        self.vpk = vpk;
        self.vgk = vgk;

        let (ip, slope_p, gm) = self.plate_with_slopes(vpk, vgk);
        let rp = slope_p.max(1e-12);

        s.conductance(self.p, self.k, rp);
        s.transconductance(self.p, self.k, self.g, self.k, gm);
        s.current(self.p, self.k, ip - rp * vpk - gm * vgk);

        // The grid is a straight line once it conducts, so its slope is the
        // line's and needs no difference at all.
        let ig = self.grid(vgk);
        let gg = if vgk < 0.0 { 1e-12 } else { 1.0 / 1_500.0 };
        s.conductance(self.g, self.k, gg);
        s.current(self.g, self.k, ig - gg * vgk);
    }

    fn moved(&self) -> f64 {
        self.delta
    }

    fn settled(&self, _tolerance: f64) -> bool {
        !self.clamped
    }
}

/// An operational amplifier, as two linear states.
///
/// Not a huge gain fed through a `tanh`. That is the obvious model and it does
/// not work: an open loop gain of a hundred thousand gives a linear region
/// about thirty microvolts wide, so every sample fails to converge and the
/// stage falls silent. Limiting the output instead lets it report convergence
/// while sitting pinned sixteen volts the wrong side of a fifteen volt rail.
///
/// So it has two states and picks between them. In the linear one it holds its
/// inputs together and its output carries whatever current that takes. Once
/// the current it would need puts the output past a rail, it stops trying and
/// becomes a voltage source to that rail instead. The switch is what a real
/// one does; the two states are exactly describable and each is linear.
pub struct OpAmp {
    out: usize,
    plus: usize,
    minus: usize,
    branch: usize,
    rail: f64,
    /// Which rail it is against, if any.
    clamped: f64,
    delta: f64,
}

impl OpAmp {
    /// What an op-amp on a nine volt pedal supply can swing either way.
    pub const NINE_VOLT: f64 = 3.5;
    /// And on a studio rail.
    pub const STUDIO: f64 = 15.0;

    pub fn new(out: usize, plus: usize, minus: usize, branch: usize, rail: f64) -> Self {
        Self { out, plus, minus, branch, rail, clamped: 0.0, delta: 0.0 }
    }
}

impl Device for OpAmp {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let output = if self.out == GROUND { 0.0 } else { v[self.out] };
        let error = across(v, self.plus, self.minus);

        // Decide which state to be in from where the last solve put the
        // output. Hysteresis is not needed: the two states agree at the rail.
        let was = self.clamped;
        self.clamped = if output > self.rail {
            self.rail
        } else if output < -self.rail {
            -self.rail
        } else {
            0.0
        };
        self.delta = (self.clamped - was).abs().max(if self.clamped == 0.0 {
            error.abs()
        } else {
            0.0
        });

        // The output carries the branch current either way.
        s.branch_current(self.out, self.branch, 1.0);
        if self.clamped == 0.0 {
            // Linear: hold the inputs together.
            s.branch_constraint(self.branch, self.plus, 1.0);
            s.branch_constraint(self.branch, self.minus, -1.0);
        } else {
            // Against a rail: the output is that rail.
            s.branch_constraint(self.branch, self.out, 1.0);
            s.branch_value(self.branch, self.clamped);
        }
    }

    fn moved(&self) -> f64 {
        self.delta
    }
}

/// A junction FET, by the square law.
///
/// Between a valve and a bipolar transistor in how its harmonics arrive. A
/// triode follows a three-halves power, a bipolar an exponential, and a JFET a
/// square -- and a square law has *only* a second-order term, so a JFET stage
/// run in its saturated region makes second harmonic and almost nothing else
/// until it starts clipping against its own supply. That is the whole reason
/// discrete studio equipment is built from them.
///
/// Three regions, and all three matter here. Cut off below pinch-off; the
/// square law above it while the drain has room; and the ohmic region when it
/// does not, which is what sets how the stage clips at the bottom of its
/// swing.
pub struct Jfet {
    d: usize,
    g: usize,
    s: usize,
    spec: JfetSpec,
    vgs: f64,
    vds: f64,
    delta: f64,
    /// Whether the limiter held the last step back.
    clamped: bool,
}

impl Jfet {
    pub fn new(d: usize, g: usize, s: usize, spec: JfetSpec) -> Self {
        Self { d, g, s, spec, vgs: 0.0, vds: 0.0, delta: 0.0, clamped: false }
    }

    /// Drain current for a pair of terminal voltages.
    pub fn drain(&self, vgs: f64, vds: f64) -> f64 {
        let vp = self.spec.pinch_off;
        // Below pinch-off the channel is shut.
        if vgs <= vp || vds <= 0.0 {
            return 0.0;
        }
        let over = 1.0 - vgs / vp;
        let knee = vgs - vp;
        if vds >= knee {
            // Saturated: the square law, and the only place the stage is
            // usable as an amplifier.
            self.spec.idss * over * over
        } else {
            // Ohmic: the drain has run out of room and the device is closer to
            // a resistor than an amplifier.
            let x = vds / -vp;
            self.spec.idss * (2.0 * over * x - x * x)
        }
    }
}

impl Device for Jfet {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // A square law is gentle enough not to need the limiter a junction
        // does, but the gate can still be walked a long way in one iteration
        // by whatever is in front of it.
        let (vgs, held_g) = limit(across(v, self.g, self.s), self.vgs, 0.5);
        let (vds, held_d) = limit(across(v, self.d, self.s), self.vds, 2.0);
        self.clamped = held_g || held_d;
        self.delta = (vgs - self.vgs).abs().max((vds - self.vds).abs());
        self.vgs = vgs;
        self.vds = vds;

        let id = self.drain(vgs, vds);
        let step = 1e-4;
        let gm = (self.drain(vgs + step, vds) - self.drain(vgs - step, vds)) / (2.0 * step);
        let gds = ((self.drain(vgs, vds + step) - self.drain(vgs, vds - step)) / (2.0 * step))
            .max(1e-9);

        // Drain to source conductance, and the gate's control of it.
        s.conductance(self.d, self.s, gds);
        s.transconductance(self.d, self.s, self.g, self.s, gm);
        s.current(self.d, self.s, id - gds * vds - gm * vgs);
    }

    fn moved(&self) -> f64 {
        self.delta
    }

    fn settled(&self, _tolerance: f64) -> bool {
        !self.clamped
    }
}

/// The magnetising branch of a transformer: the part that saturates.
///
/// An ideal transformer is a ratio and nothing else, and a plugin built on one
/// would have iron in the signal path that made no difference whatever. What
/// a transformer actually does to a sound is here.
///
/// The winding has an inductance across it, and the current that flows in it
/// magnetises the core. It is *flux* that saturates, not voltage -- and flux
/// is the integral of voltage, so the same signal makes far more of it at
/// forty hertz than at four thousand. That is why transformer distortion is a
/// bottom-end phenomenon, and why it is not a waveshaper: no curve applied to
/// the samples can be frequency dependent in that way.
///
/// Past the knee the magnetising current rises very steeply as more flux is
/// forced into a core that has no more room, which loads whatever is driving
/// the winding and bends the waveform. The curve continues rather than
/// clamping, so driving it harder goes on doing more.
pub struct Core {
    a: usize,
    b: usize,
    spec: CoreSpec,
    /// Flux linkage, in weber-turns. The state that makes this frequency
    /// dependent.
    flux: f64,
    /// Flux and voltage at the end of the last settled sample.
    last_flux: f64,
    last_volts: f64,
    volts: f64,
    delta: f64,
    /// Half the timestep, which is what trapezoidal integration of the voltage
    /// comes to.
    half_step: f64,
    /// Whether the limiter held the last step back.
    clamped: bool,
}

impl Core {
    pub fn new(a: usize, b: usize, spec: CoreSpec, rate: f64) -> Self {
        Self {
            a,
            b,
            spec,
            flux: 0.0,
            last_flux: 0.0,
            last_volts: 0.0,
            volts: 0.0,
            delta: 0.0,
            clamped: false,
            half_step: 0.5 / rate,
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.half_step = 0.5 / rate;
    }

    /// Magnetising current for a flux linkage.
    ///
    /// Linear through the origin, plus an odd power that only matters past the
    /// knee. Odd so the core behaves the same on both halves of the wave --
    /// which is why a transformer makes odd harmonics and a single-ended valve
    /// stage makes even ones.
    pub fn magnetising(&self, flux: f64) -> f64 {
        self.magnetising_with_slope(flux).0
    }

    /// The magnetising current and its slope, from one evaluation.
    ///
    /// `|x|^(s-1)` is `|x|^s / |x|`, so the slope costs no second power.
    pub fn magnetising_with_slope(&self, flux: f64) -> (f64, f64) {
        let (l, knee, sharp) = (self.spec.henry, self.spec.knee, self.spec.sharpness);
        let over = flux / knee;
        let magnitude = over.abs();
        if magnitude < 1e-30 {
            return (flux / l, 1.0 / l);
        }
        let powered = magnitude.powf(sharp);
        let current = flux / l + powered * over.signum() * knee / l;
        let slope = 1.0 / l + sharp * (powered / magnitude) / l;
        (current, slope)
    }

    pub fn flux(&self) -> f64 {
        self.flux
    }
}

impl Device for Core {
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // The winding voltage is what drives the flux, and a long way in one
        // iteration is how a steep curve makes the solve oscillate.
        let (volts, clamped) = limit(across(v, self.a, self.b), self.volts, 4.0);
        self.clamped = clamped;
        self.delta = (volts - self.volts).abs();
        self.volts = volts;

        // Trapezoidal: the flux at this instant is what it was, plus the area
        // under the voltage since.
        let history = self.last_flux + self.half_step * self.last_volts;
        let flux = history + self.half_step * volts;
        self.flux = flux;

        let i = self.magnetising(flux);
        let step = self.spec.knee * 1e-3;
        // d(current)/d(volts) is d(current)/d(flux) times the half step, which
        // is what the integration contributes.
        let slope = (self.magnetising(flux + step) - self.magnetising(flux - step))
            / (2.0 * step);
        let g = (slope * self.half_step).max(1e-12);

        s.conductance(self.a, self.b, g);
        s.current(self.a, self.b, i - g * volts);
    }

    fn moved(&self) -> f64 {
        self.delta
    }

    fn settled(&self, _tolerance: f64) -> bool {
        !self.clamped
    }

    fn advance(&mut self) {
        self.last_flux = self.flux;
        self.last_volts = self.volts;
    }
}
