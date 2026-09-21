//! The parts that bend.
//!
//! Each one is linearised at the working point every Newton iteration: a
//! conductance for the slope of its curve there, and a current source for the
//! offset between that tangent and the curve itself. Iterate and the guess
//! walks onto the real answer.
//!
//! Analytic slopes share intermediate values with the current equations, so
//! Newton does not need repeated curve evaluations to estimate derivatives.

use super::netlist::{BipolarSpec, CoreSpec, DiodeSpec, JfetSpec, PentodeSpec, TriodeSpec, GROUND};

/// Thermal voltage at room temperature.
const VT: f64 = 0.025_852;

/// Where a linearised device puts its conductances and currents.
pub struct Stamper<'a> {
    pub matrix: &'a mut [f64],
    pub rhs: &'a mut [f64],
    pub n: usize,
    /// Optional global-unknown to compact-boundary map. Full-MNA stamps leave
    /// this as `None`; Schur-reduced stamps use it so the exact same device
    /// code writes directly into the reduced boundary Jacobian/RHS.
    pub map: Option<&'a [usize]>,
    /// Set when a supposedly-boundary-only nonlinear device attempts to write
    /// an internal unknown. The caller then abandons the reduced pass and uses
    /// the full-MNA fallback rather than silently dropping a coefficient.
    pub mapping_failed: bool,
    /// Whether the devices should hold their steps back.
    ///
    /// The step limiters are damping: they stop a device being carried
    /// somewhere absurd by one over-eager Newton correction. That is worth
    /// having when nothing else is watching the step, and it is exactly wrong
    /// once something is. A limiter moves the point a device linearises at,
    /// so the stamped system no longer describes the point it was asked
    /// about -- and a residual measured against it is the residual of
    /// somewhere else. The Newton direction taken from it then need not
    /// reduce the residual at all, which is what a line search assumes.
    ///
    /// So on the passes where the line search runs, the limiters stand down
    /// and it does their job instead: it is the better damping of the two,
    /// because it decides by asking the circuit rather than by a fixed number
    /// of volts. Overflow is not their responsibility and never was -- the
    /// exponentials guard themselves, at `CLAMP` and in `plate_with_slopes`
    /// -- and a trial that comes back non-finite anyway is refused.
    /// Note this is the *step* limiters only -- the ones holding a device to
    /// so many volts a pass, which is damping and nothing else. The junction
    /// limiting on the diode and the transistor is a different thing wearing a
    /// similar name: logarithmic, guarding `exp(v/vt)` from overflowing, and
    /// never optional. Standing it down alongside these sent every one of the
    /// 73P's and the Big Muff's 48000 solves a second to a non-finite
    /// correction.
    pub limiting: bool,

    /// Whether any device had to hold a *junction* back on this stamp.
    ///
    /// Set by the diode and the transistor, and by nothing else. It says
    /// whether the residual of this system is the residual of the point it was
    /// asked about: junction limiting moves the linearisation and cannot be
    /// switched off, so where it engages the stamp describes somewhere other
    /// than the point being judged, and a line search must not decide anything
    /// on it. Where it does not engage -- most samples, on most circuits --
    /// the stamp sits exactly at the point and the residual can be trusted.
    ///
    /// Asking per stamp rather than per circuit is what lets the search reach
    /// the pedals at all. Refusing every circuit that merely *contains* a
    /// junction shut it out of the Big Muff, the TS808, the 73P and every
    /// diode topology -- which is most of the catalogue.
    pub junction_held: bool,
}

impl Stamper<'_> {
    #[inline]
    fn local(&mut self, node: usize) -> Option<usize> {
        if node == GROUND {
            return None;
        }
        if let Some(map) = self.map {
            let local = map.get(node).copied().unwrap_or(usize::MAX);
            if local == usize::MAX {
                self.mapping_failed = true;
                None
            } else {
                Some(local)
            }
        } else {
            Some(node)
        }
    }

    pub fn conductance(&mut self, a: usize, b: usize, g: f64) {
        let a = self.local(a);
        let b = self.local(b);
        let n = self.n;
        if let Some(a) = a {
            self.matrix[a * n + a] += g;
        }
        if let Some(b) = b {
            self.matrix[b * n + b] += g;
        }
        if let (Some(a), Some(b)) = (a, b) {
            self.matrix[a * n + b] -= g;
            self.matrix[b * n + a] -= g;
        }
    }

    /// A transconductance: current between `a` and `b` controlled by the
    /// voltage between `c` and `d`. Asymmetric, which is why the matrix cannot
    /// be factored as if it were symmetric.
    pub fn transconductance(&mut self, a: usize, b: usize, c: usize, d: usize, gm: f64) {
        let a = self.local(a);
        let b = self.local(b);
        let c = self.local(c);
        let d = self.local(d);
        let n = self.n;
        for (row, sign) in [(a, 1.0), (b, -1.0)] {
            let Some(row) = row else { continue };
            if let Some(c) = c {
                self.matrix[row * n + c] += sign * gm;
            }
            if let Some(d) = d {
                self.matrix[row * n + d] -= sign * gm;
            }
        }
    }

    /// Ties a node to a branch's current: the part sources `sign` amps of it
    /// into that node.
    pub fn branch_current(&mut self, node: usize, branch: usize, sign: f64) {
        let node = self.local(node);
        let branch = self.local(branch);
        if let (Some(node), Some(branch)) = (node, branch) {
            self.matrix[node * self.n + branch] += sign;
        }
    }

    /// One term of a branch's own row, which is the condition the part is
    /// imposing on the voltages.
    pub fn branch_constraint(&mut self, branch: usize, node: usize, sign: f64) {
        let branch = self.local(branch);
        let node = self.local(node);
        if let (Some(branch), Some(node)) = (branch, node) {
            self.matrix[branch * self.n + node] += sign;
        }
    }

    /// What that condition equals.
    pub fn branch_value(&mut self, branch: usize, volts: f64) {
        if let Some(branch) = self.local(branch) {
            self.rhs[branch] += volts;
        }
    }

    /// A current flowing out of `a` and into `b`.
    pub fn current(&mut self, a: usize, b: usize, amps: f64) {
        let a = self.local(a);
        let b = self.local(b);
        if let Some(a) = a {
            self.rhs[a] -= amps;
        }
        if let Some(b) = b {
            self.rhs[b] += amps;
        }
    }
}

/// Residual-only counterpart to [`Stamper`] for line-search trial points.
///
/// A rejected backtracking trial is never solved, so it does not need a
/// Jacobian. It only needs the nonlinear contribution to `F(x)`. Keeping that
/// path separate avoids calculating derivative terms and writing a compact
/// matrix that will immediately be thrown away.
pub struct ResidualStamper<'a> {
    pub residual: &'a mut [f64],
    pub map: &'a [usize],
    pub mapping_failed: bool,
    pub limiting: bool,
    pub junction_held: bool,
}

impl ResidualStamper<'_> {
    #[inline]
    fn local(&mut self, node: usize) -> Option<usize> {
        if node == GROUND {
            return None;
        }
        let local = self.map.get(node).copied().unwrap_or(usize::MAX);
        if local == usize::MAX || local >= self.residual.len() {
            self.mapping_failed = true;
            None
        } else {
            Some(local)
        }
    }

    #[inline]
    pub fn row(&mut self, node: usize, value: f64) {
        if let Some(row) = self.local(node) {
            self.residual[row] += value;
        }
    }

    /// Add a physical current flowing out of `a` and into `b` to `F(x)`.
    #[inline]
    pub fn current(&mut self, a: usize, b: usize, amps: f64) {
        if let Some(a) = self.local(a) {
            self.residual[a] += amps;
        }
        if let Some(b) = self.local(b) {
            self.residual[b] -= amps;
        }
    }
}

/// Which matrix positions a device can write to, in any of its states.
///
/// The solver needs the circuit's *structure* -- which entries can be nonzero
/// -- and it needs it before it has any values to look at. It used to get it
/// by scanning the assembled matrix for zeros on every factorisation, a
/// hundred thousand times a second, for a pattern that never changes: the
/// positions a part stamps are fixed by the nodes it is wired between, and
/// nothing about the audio moves a wire.
///
/// So each device declares its footprint instead. The methods here mirror
/// `Stamper`'s one for one and do the same ground-skipping arithmetic, which
/// is what keeps the two in step; `tests/devices.rs` runs every voice hard
/// with a stamper that rejects any write landing outside the declaration, so
/// the correspondence is checked rather than assumed.
///
/// A device with more than one state must declare the **union** over all of
/// them. `OpAmp` is the one that matters: linear it constrains its inputs
/// together, against a rail it constrains its output, and those are different
/// entries. A footprint that described only the state it happens to be in
/// would be right until the first time it clipped.
pub struct Mark<'a> {
    pub pattern: &'a mut [bool],
    pub n: usize,
}

impl Mark<'_> {
    fn set(&mut self, row: usize, col: usize) {
        let n = self.n;
        self.pattern[row * n + col] = true;
    }

    pub fn conductance(&mut self, a: usize, b: usize) {
        if a != GROUND {
            self.set(a, a);
        }
        if b != GROUND {
            self.set(b, b);
        }
        if a != GROUND && b != GROUND {
            self.set(a, b);
            self.set(b, a);
        }
    }

    pub fn transconductance(&mut self, a: usize, b: usize, c: usize, d: usize) {
        for row in [a, b] {
            if row == GROUND {
                continue;
            }
            if c != GROUND {
                self.set(row, c);
            }
            if d != GROUND {
                self.set(row, d);
            }
        }
    }

    pub fn branch_current(&mut self, node: usize, branch: usize) {
        if node != GROUND {
            self.set(node, branch);
        }
    }

    pub fn branch_constraint(&mut self, branch: usize, node: usize) {
        if node != GROUND {
            self.set(branch, node);
        }
    }
}

/// Where a device is linearised, saved so a mutating trial step can undo it.
///
/// Full-MNA/watch-mode backtracking still evaluates trial points by stamping,
/// and stamping is not free of consequence: every device records the terminal
/// voltages it was linearised at, and that record is the reference its own step
/// limiter measures the *next* step against. The op-amp goes further and keeps
/// which rail it is against, deliberately, so that it stops chattering between
/// states. The production Schur residual probe is read-only and therefore does
/// not need this save/restore path for rejected candidates.
///
/// If a rejected trial were allowed to leave those behind, the limiter's idea
/// of "where we were" would become the last place the line search happened to
/// look rather than the last answer anything accepted, and the op-amp's
/// hysteresis would be decided by arithmetic that was discarded. So the search
/// saves them before it starts and puts them back after every rejection.
///
/// Four slots and a flag covers every device here. What each one keeps where
/// is its own business; nothing but that device ever reads them back.
///
/// This is only the linearisation. The physical state of the timestep --
/// capacitor charge, inductor current, transformer flux history -- is advanced
/// outside the Newton loop entirely and a trial step never touches it.
#[derive(Clone, Copy, Default)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Linearisation {
    at: [f64; 4],
    clamped: bool,
}

pub trait Device: Send {
    /// Linearise at the present guess and stamp it.
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]);
    /// Every matrix position `stamp` can write to, in any state. See `Mark`.
    fn footprint(&self, m: &mut Mark);
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
    /// Where this device is currently linearised.
    fn linearisation(&self) -> Linearisation {
        Linearisation::default()
    }

    /// Puts it back, undoing a trial step the line search rejected.
    fn relinearise(&mut self, _saved: Linearisation) {}

    /// Called once the sample is settled.
    fn advance(&mut self) {}

    /// Whether this device jumps between states rather than bending smoothly.
    ///
    /// An op-amp is either following its input or against a rail, and which
    /// one it is changes in a step. That matters to whoever is choosing where
    /// to start the next solve: a straight line through the last two answers
    /// is a good guess for something that bends and a bad one for something
    /// that switches, because it can carry the device into the wrong state --
    /// where, being stable, it reports itself settled and stays.
    fn switches(&self) -> bool {
        false
    }
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

/// A volt below where a triode grid starts conducting.
///
/// Below this the grid draws nothing worth solving for -- the stamp itself
/// says so, putting a picosiemens across it until `vgk` reaches zero -- and
/// Koren's plate curve is smooth. The volt of margin is there because the
/// interesting behaviour starts before zero, not at it, and the limiter should
/// already be careful by the time it arrives.
const GRID_CONDUCTS: f64 = -1.0;

/// The step limiter for a triode grid.
///
/// A grid is the one place on a triode where the solve can run away, and the
/// limiter is what damps it. But `limit` was applied at a scale of 0.5
/// everywhere, holding the grid to one volt a pass wherever it happened to be
/// -- including the tens of volts below cutoff that it crosses on every
/// transient, where the stamp has already made it an open circuit and there is
/// nothing to run away from.
///
/// That mattered because of how convergence is judged. `Device::settled`
/// reports `!clamped`: a solve is finished only on a pass where nothing was
/// held back. So the pass count was, in part, the distance the grid had to
/// travel measured in volts. The 5150, six triodes and the hardest grid drive
/// of any voice here, spent 7.69 passes a sample and failed to converge at all
/// on 3304 solves a second.
///
/// Four volts a pass below cutoff and the original one volt from there up:
/// 7.69 passes to 4.88, 3304 unsettled solves to 1062, and 64.1 % of realtime
/// to 41.0 % on one channel. It nulls against the previous build at -236 dB,
/// which is round-off -- the limiter changes the path Newton walks, not the
/// answer it arrives at. `docs/experiments/crackle-root-cause.md` has the
/// working, including why removing the limit entirely does not work: an
/// unbounded step below cutoff sends every solve to the iteration cap.
fn limit_grid(new: f64, old: f64) -> (f64, bool) {
    let scale = if new < GRID_CONDUCTS && old < GRID_CONDUCTS {
        2.0
    } else {
        0.75
    };
    limit(new, old, scale)
}

/// Where the exponential is held, short of overflowing.
const CLAMP: f64 = 60.0;

#[inline]
fn powi_small(mut base: f64, mut exponent: u8) -> f64 {
    let mut result = 1.0;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result *= base;
        }
        exponent >>= 1;
        if exponent != 0 {
            base *= base;
        }
    }
    result
}

pub struct Diode {
    a: usize,
    k: usize,
    spec: DiodeSpec,
    scale: f64,
    inv_scale: f64,
    critical: f64,
    voltage: f64,
    delta: f64,
    /// Whether the limiter held the last step back.
    clamped: bool,
}

impl Diode {
    pub fn new(a: usize, k: usize, spec: DiodeSpec) -> Self {
        let scale = spec.emission * VT;
        let critical = scale * (scale / (std::f64::consts::SQRT_2 * spec.saturation)).ln();
        Self {
            a,
            k,
            spec,
            scale,
            inv_scale: 1.0 / scale,
            critical,
            voltage: 0.0,
            delta: 0.0,
            clamped: false,
        }
    }

    /// Keeps a junction voltage from running away, in the way a junction
    /// wants rather than by a flat cap.
    ///
    /// A fixed cap of a couple of thermal voltages is fine while the answer is
    /// near, and hopeless when it is not: an LED conducts at about 1.8 V and
    /// the cap is 0.1, so walking there takes twenty-odd iterations. Measured,
    /// the LED Wall preset was taking ten Newton passes a sample against two
    /// or three for everything else, and cost four times as much because of
    /// it.
    ///
    /// This is the standard junction limiter instead. Above the critical
    /// voltage -- where the exponential starts to run away -- the step is
    /// taken in the logarithm, so a long way is covered in one iteration
    /// without overshooting into an overflow.
    fn limit_junction(&mut self, wanted: f64) -> f64 {
        let old = self.voltage;
        let scale = self.scale;
        let critical = self.critical;
        if wanted > critical && (wanted - old).abs() > 2.0 * scale {
            self.clamped = true;
            if old > 0.0 {
                let arg = 1.0 + (wanted - old) * self.inv_scale;
                if arg > 0.0 {
                    old + scale * arg.ln()
                } else {
                    critical
                }
            } else {
                scale * (wanted * self.inv_scale).ln()
            }
        } else {
            self.clamped = false;
            wanted
        }
    }

    /// The Shockley current, exposed so tests can check the slope the stamp
    /// uses against the curve it claims to be the slope of.
    pub fn current(&self, v: f64) -> f64 {
        let x = (v * self.inv_scale).min(CLAMP);
        self.spec.saturation * (x.exp() - 1.0)
    }
}

impl Device for Diode {
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.a, self.k);
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.voltage, self.delta, 0.0, 0.0],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.voltage = saved.at[0];
        self.delta = saved.at[1];
        self.clamped = saved.clamped;
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let _scale = self.scale;
        let guess = self.limit_junction(across(v, self.a, self.k));
        s.junction_held |= self.clamped;
        self.delta = (guess - self.voltage).abs();
        self.voltage = guess;

        // One exponential, not three. The slope of `Is (e^x - 1)` is
        // `Is e^x / scale`, which is the same exponential the current already
        // needed -- where a central difference asked for two more.
        let x = guess * self.inv_scale;
        let (i, g) = if x >= CLAMP {
            (self.spec.saturation * (CLAMP.exp() - 1.0), 1e-12)
        } else {
            let e = x.exp();
            (
                self.spec.saturation * (e - 1.0),
                (self.spec.saturation * e * self.inv_scale).max(1e-12),
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

/// A rectifier valve, by Child's law.
///
/// `i = k v^1.5` when the plate is positive of the cathode, and nothing when it
/// is not -- a valve cannot conduct backwards at all, which is the other half of
/// what makes it different from a silicon bridge.
///
/// The two-thirds power is the whole point. A silicon diode's drop barely moves
/// with current, so the supply behind it is stiff; this one's drop climbs as the
/// amplifier draws, so the rail falls under a chord and comes back with the
/// reservoir's time constant. That is sag, and it is a property of the part
/// rather than a number anybody tunes.
pub struct Rectifier {
    a: usize,
    k: usize,
    perveance: f64,
    voltage: f64,
    delta: f64,
    clamped: bool,
}

impl Rectifier {
    /// Below this the valve is treated as off, with a tiny conductance so the
    /// node it feeds is never left floating.
    const OFF: f64 = 1e-9;

    pub fn new(a: usize, k: usize, spec: crate::dsp::netlist::RectifierSpec) -> Self {
        Self {
            a,
            k,
            perveance: spec.perveance(),
            voltage: 0.0,
            delta: 0.0,
            clamped: false,
        }
    }
}

impl Device for Rectifier {
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.a, self.k);
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.voltage, self.delta, 0.0, 0.0],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.voltage = saved.at[0];
        self.delta = saved.at[1];
        self.clamped = saved.clamped;
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // The step limiter a power supply needs is a plain one: the law is a
        // three-halves power rather than an exponential, so it cannot run away
        // the way a junction does, but a solve that starts far from the answer
        // can still ask for a plate volt that is negative on one pass and a
        // hundred on the next.
        let wanted = across(v, self.a, self.k);
        let (guess, clamped) = limit(wanted, self.voltage, 25.0);
        self.clamped = clamped;
        self.delta = (guess - self.voltage).abs();
        self.voltage = guess;

        let (i, g) = if guess > 0.0 {
            let root = guess.sqrt();
            (
                self.perveance * guess * root,
                (1.5 * self.perveance * root).max(Self::OFF),
            )
        } else {
            (0.0, Self::OFF)
        };
        s.conductance(self.a, self.k, g);
        s.current(self.a, self.k, i - g * guess);
    }

    fn moved(&self) -> f64 {
        self.delta
    }

    fn settled(&self, tolerance: f64) -> bool {
        !self.clamped && self.delta < tolerance
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
    inv_mu: f64,
    inv_kp: f64,
    inv_kg1: f64,
    vpk: f64,
    vgk: f64,
    delta: f64,
    /// Whether the limiter held the last step back.
    clamped: bool,
    /// Current-path intermediates from the most recent reduced line-search
    /// candidate. A winning candidate becomes the next Newton linearisation
    /// point, so the following stamp can derive its analytic slopes without
    /// repeating sqrt/exp/ln1p/powf. Rejected trials do no derivative work.
    trial_eval: Option<TriodeEval>,
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
struct TriodeEval {
    vpk: f64,
    vgk: f64,
    ip: f64,
    root: f64,
    soft: f64,
    exp_inner: f64,
    e1: f64,
    powered: f64,
    soft_is_linear: bool,
    active: bool,
}

impl TriodeEval {
    #[inline]
    fn matches(self, vpk: f64, vgk: f64) -> bool {
        self.vpk.to_bits() == vpk.to_bits() && self.vgk.to_bits() == vgk.to_bits()
    }
}

impl Triode {
    pub fn new(p: usize, g: usize, k: usize, spec: TriodeSpec) -> Self {
        Self {
            p,
            g,
            k,
            spec,
            inv_mu: 1.0 / spec.mu,
            inv_kp: 1.0 / spec.kp,
            inv_kg1: 1.0 / spec.kg1,
            vpk: 0.0,
            vgk: -1.0,
            delta: 0.0,
            clamped: false,
            trial_eval: None,
        }
    }

    /// Plate current for a pair of terminal voltages.
    pub fn plate(&self, vpk: f64, vgk: f64) -> f64 {
        let s = &self.spec;
        if vpk <= 0.0 {
            return 0.0;
        }
        let inner = s.kp * (self.inv_mu + vgk / (s.kvb + vpk * vpk).sqrt());
        // ln(1 + e^x) without overflowing for large x.
        let soft = if inner > 30.0 {
            inner
        } else {
            inner.exp().ln_1p()
        };
        let e1 = vpk * self.inv_kp * soft;
        if e1 <= 0.0 {
            0.0
        } else {
            2.0 * e1.powf(s.ex) * self.inv_kg1
        }
    }

    /// Current-only evaluation that also retains the intermediates already
    /// paid for by a reduced line-search residual. No derivative arithmetic is
    /// done here: rejected trials therefore keep the original transcendental
    /// workload, while an accepted trial can form the next Newton slopes
    /// without repeating sqrt/exp/ln1p/powf.
    #[inline]
    fn plate_trial_eval(&self, vpk: f64, vgk: f64) -> TriodeEval {
        let s = &self.spec;
        if vpk <= 0.0 {
            return TriodeEval {
                vpk,
                vgk,
                ip: 0.0,
                root: 0.0,
                soft: 0.0,
                exp_inner: 0.0,
                e1: 0.0,
                powered: 0.0,
                soft_is_linear: false,
                active: false,
            };
        }
        let root = (s.kvb + vpk * vpk).sqrt();
        let inner = s.kp * (self.inv_mu + vgk / root);
        let (soft, exp_inner, soft_is_linear) = if inner > 30.0 {
            (inner, 0.0, true)
        } else {
            let e = inner.exp();
            (e.ln_1p(), e, false)
        };
        let e1 = vpk * self.inv_kp * soft;
        if e1 <= 0.0 {
            return TriodeEval {
                vpk,
                vgk,
                ip: 0.0,
                root,
                soft,
                exp_inner,
                e1,
                powered: 0.0,
                soft_is_linear,
                active: false,
            };
        }
        let powered = e1.powf(s.ex);
        let ip = 2.0 * powered * self.inv_kg1;
        TriodeEval {
            vpk,
            vgk,
            ip,
            root,
            soft,
            exp_inner,
            e1,
            powered,
            soft_is_linear,
            active: true,
        }
    }

    #[inline]
    fn plate_slopes_from_trial(&self, evaluation: TriodeEval) -> (f64, f64, f64) {
        if !evaluation.active {
            return (evaluation.ip, 0.0, 0.0);
        }
        let sigma = if evaluation.soft_is_linear {
            1.0
        } else {
            evaluation.exp_inner / (1.0 + evaluation.exp_inner)
        };
        let d_ip = 2.0 * self.spec.ex * evaluation.powered * self.inv_kg1 / evaluation.e1;
        let d_e1_vpk = evaluation.soft * self.inv_kp
            - evaluation.vpk * evaluation.vpk * evaluation.vgk * sigma
                / (evaluation.root * evaluation.root * evaluation.root);
        let d_e1_vgk = evaluation.vpk * sigma / evaluation.root;
        (evaluation.ip, d_ip * d_e1_vpk, d_ip * d_e1_vgk)
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
        let inner = s.kp * (self.inv_mu + vgk / root);
        let (soft, sigma) = if inner > 30.0 {
            (inner, 1.0)
        } else {
            let e = inner.exp();
            (e.ln_1p(), e / (1.0 + e))
        };
        let e1 = vpk * self.inv_kp * soft;
        if e1 <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let powered = e1.powf(s.ex);
        let ip = 2.0 * powered * self.inv_kg1;

        let d_ip = 2.0 * s.ex * powered * self.inv_kg1 / e1;
        let d_e1_vpk = soft * self.inv_kp - vpk * vpk * vgk * sigma / (root * root * root);
        let d_e1_vgk = vpk * sigma / root;
        (ip, d_ip * d_e1_vpk, d_ip * d_e1_vgk)
    }
}

impl Device for Triode {
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.p, self.k);
        m.transconductance(self.p, self.k, self.g, self.k);
        m.conductance(self.g, self.k);
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.vpk, self.vgk, self.delta, 0.0],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.vpk = saved.at[0];
        self.vgk = saved.at[1];
        self.delta = saved.at[2];
        self.clamped = saved.clamped;
        self.trial_eval = None;
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let vpk = across(v, self.p, self.k).max(0.0);
        let raw = across(v, self.g, self.k);
        let (vgk, clamped) = if s.limiting {
            limit_grid(raw, self.vgk)
        } else {
            (raw, false)
        };
        self.clamped = clamped;
        self.delta = (vpk - self.vpk).abs().max((vgk - self.vgk).abs());
        self.vpk = vpk;
        self.vgk = vgk;

        let (ip, slope_p, gm) = match self.trial_eval.take() {
            Some(cached) if cached.matches(vpk, vgk) => self.plate_slopes_from_trial(cached),
            _ => self.plate_with_slopes(vpk, vgk),
        };
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

/// A beam tetrode or pentode, by Koren's equations.
///
/// The difference from a triode is where the cathode current comes from. In a
/// triode the plate pulls the electrons through and the plate voltage
/// therefore sets the current; in a pentode the *screen* does that job and the
/// plate mostly collects what arrives. So the controlling term is the screen
/// voltage rather than the plate's, and the plate's own influence survives
/// only as the `arctan` knee that lifts the current out of zero at low plate
/// voltages and then flattens.
///
/// That flat shelf is the whole character of a power stage. It is why a power
/// tube behaves as a current source into the transformer's primary, why the
/// two halves of a push-pull pair hand over the way they do, and why the
/// clipping is hard at the top and asymmetric once the grids draw.
///
/// The screen is a terminal here rather than an assumed voltage, so the drop
/// across the screen resistor -- and whatever the supply behind it is doing at
/// the time -- falls out of the solve. That is what screen sag *is*, and it is
/// most of what a hundred and twenty watt amplifier does when it is asked for
/// a hundred and thirty.
pub struct Pentode {
    p: usize,
    g: usize,
    k: usize,
    s: usize,
    /// How many tubes in parallel this part stands for.
    count: f64,
    spec: PentodeSpec,
    inv_mu: f64,
    inv_kp: f64,
    inv_kvb: f64,
    inv_kg1: f64,
    inv_kg2: f64,
    vpk: f64,
    vgk: f64,
    vsk: f64,
    delta: f64,
    clamped: bool,
    /// Current-path intermediates from the most recent reduced line-search
    /// candidate. The residual probe already pays for the softplus, power law
    /// and plate knee; if that point wins, the next Newton stamp derives the
    /// analytic slopes from those exact intermediates.
    trial_eval: Option<PentodeEval>,
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
struct PentodeEval {
    vpk: f64,
    vgk: f64,
    vsk: f64,
    ip: f64,
    ig2: f64,
    soft: f64,
    exp_inner: f64,
    e1: f64,
    plate_base: f64,
    screen_base: f64,
    knee: f64,
    soft_is_linear: bool,
    active: bool,
}

impl PentodeEval {
    #[inline]
    fn matches(self, vpk: f64, vgk: f64, vsk: f64) -> bool {
        self.vpk.to_bits() == vpk.to_bits()
            && self.vgk.to_bits() == vgk.to_bits()
            && self.vsk.to_bits() == vsk.to_bits()
    }
}

impl Pentode {
    pub fn new(p: usize, g: usize, k: usize, s: usize, count: f64, spec: PentodeSpec) -> Self {
        Self {
            p,
            g,
            k,
            s,
            count,
            spec,
            inv_mu: 1.0 / spec.mu,
            inv_kp: 1.0 / spec.kp,
            inv_kvb: 1.0 / spec.kvb,
            inv_kg1: 1.0 / spec.kg1,
            inv_kg2: 1.0 / spec.kg2,
            vpk: 0.0,
            vgk: -30.0,
            vsk: 0.0,
            delta: 0.0,
            clamped: false,
            trial_eval: None,
        }
    }

    /// Plate and screen current and their slopes, from one evaluation.
    ///
    /// Koren gives the two electrodes separate formulae over a shared `E1`:
    /// the plate takes `E1^ex / Kg1` through a knee that lifts it out of zero
    /// at low plate voltages and then flattens, and the screen takes
    /// `E1^ex / Kg2` with no knee at all, because what the screen collects
    /// does not depend on what the plate is doing.
    ///
    /// Same trick as the triode: every piece of the derivative is something
    /// the currents already computed, so this costs one `powf`, one `exp` and
    /// one `atan` rather than four evaluations of the whole thing.
    ///
    /// Returns `(ip, ig2, dip/dvpk, dip/dvgk, dip/dvsk, dig2/dvgk, dig2/dvsk)`.
    #[allow(clippy::type_complexity)]
    fn split_with_slopes(
        &self,
        vpk: f64,
        vgk: f64,
        vsk: f64,
    ) -> (f64, f64, f64, f64, f64, f64, f64) {
        let c = &self.spec;
        if vsk <= 0.0 || vpk <= 0.0 {
            return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
        let inner = c.kp * (self.inv_mu + vgk / vsk);
        let (soft, sigma) = if inner > 30.0 {
            (inner, 1.0)
        } else {
            let e = inner.exp();
            (e.ln_1p(), e / (1.0 + e))
        };
        let e1 = vsk * self.inv_kp * soft;
        if e1 <= 0.0 {
            return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
        let powered = e1.powf(c.ex);

        // The knee. Below a few tens of volts the plate cannot collect what
        // the screen has launched and the current falls away; above it the
        // curve is the long flat shelf that makes a power tube a current
        // source into its transformer.
        let knee = (vpk * self.inv_kvb).atan();
        let vpk_ratio = vpk * self.inv_kvb;
        let d_knee = self.inv_kvb / (1.0 + vpk_ratio * vpk_ratio);

        let plate_base = powered * self.inv_kg1;
        let screen_base = powered * self.inv_kg2;
        let ip = self.count * plate_base * knee;
        let ig2 = self.count * screen_base;

        // dE1 with respect to each terminal, and d(E1^ex)/dE1 = ex * E1^ex / E1.
        let d_e1_vgk = sigma;
        let d_e1_vsk = soft * self.inv_kp - vgk * sigma / vsk;
        let d_plate = c.ex * plate_base / e1;
        let d_screen = c.ex * screen_base / e1;

        (
            ip,
            ig2,
            self.count * plate_base * d_knee,
            self.count * d_plate * d_e1_vgk * knee,
            self.count * d_plate * d_e1_vsk * knee,
            self.count * d_screen * d_e1_vgk,
            self.count * d_screen * d_e1_vsk,
        )
    }

    /// Plate and screen current at a set of terminal voltages, for checking
    /// the model against a data sheet without going through a solve.
    pub fn currents(&self, vpk: f64, vgk: f64, vsk: f64) -> (f64, f64) {
        let (ip, ig2, ..) = self.split_with_slopes(vpk, vgk, vsk);
        (ip, ig2)
    }

    /// Current-only pentode evaluation for reduced line-search trials. It
    /// retains only intermediates the original current formula already had to
    /// compute; analytic slopes are derived later only if this exact point is
    /// used as the next Newton linearisation.
    #[inline]
    fn split_trial_eval(&self, vpk: f64, vgk: f64, vsk: f64) -> PentodeEval {
        if vsk <= 0.0 || vpk <= 0.0 {
            return PentodeEval {
                vpk,
                vgk,
                vsk,
                ip: 0.0,
                ig2: 0.0,
                soft: 0.0,
                exp_inner: 0.0,
                e1: 0.0,
                plate_base: 0.0,
                screen_base: 0.0,
                knee: 0.0,
                soft_is_linear: false,
                active: false,
            };
        }
        let c = &self.spec;
        let inner = c.kp * (self.inv_mu + vgk / vsk);
        let (soft, exp_inner, soft_is_linear) = if inner > 30.0 {
            (inner, 0.0, true)
        } else {
            let e = inner.exp();
            (e.ln_1p(), e, false)
        };
        let e1 = vsk * self.inv_kp * soft;
        if e1 <= 0.0 {
            return PentodeEval {
                vpk,
                vgk,
                vsk,
                ip: 0.0,
                ig2: 0.0,
                soft,
                exp_inner,
                e1,
                plate_base: 0.0,
                screen_base: 0.0,
                knee: 0.0,
                soft_is_linear,
                active: false,
            };
        }
        let powered = e1.powf(c.ex);
        let plate_base = powered * self.inv_kg1;
        let screen_base = powered * self.inv_kg2;
        let knee = (vpk * self.inv_kvb).atan();
        PentodeEval {
            vpk,
            vgk,
            vsk,
            ip: self.count * plate_base * knee,
            ig2: self.count * screen_base,
            soft,
            exp_inner,
            e1,
            plate_base,
            screen_base,
            knee,
            soft_is_linear,
            active: true,
        }
    }

    #[inline]
    fn split_slopes_from_trial(
        &self,
        evaluation: PentodeEval,
    ) -> (f64, f64, f64, f64, f64, f64, f64) {
        if !evaluation.active {
            return (evaluation.ip, evaluation.ig2, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
        let sigma = if evaluation.soft_is_linear {
            1.0
        } else {
            evaluation.exp_inner / (1.0 + evaluation.exp_inner)
        };
        let vpk_ratio = evaluation.vpk * self.inv_kvb;
        let d_knee = self.inv_kvb / (1.0 + vpk_ratio * vpk_ratio);
        let d_e1_vgk = sigma;
        let d_e1_vsk = evaluation.soft * self.inv_kp - evaluation.vgk * sigma / evaluation.vsk;
        let d_plate = self.spec.ex * evaluation.plate_base / evaluation.e1;
        let d_screen = self.spec.ex * evaluation.screen_base / evaluation.e1;
        (
            evaluation.ip,
            evaluation.ig2,
            self.count * evaluation.plate_base * d_knee,
            self.count * d_plate * d_e1_vgk * evaluation.knee,
            self.count * d_plate * d_e1_vsk * evaluation.knee,
            self.count * d_screen * d_e1_vgk,
            self.count * d_screen * d_e1_vsk,
        )
    }

    /// Grid current. Nothing until the grid goes positive, then it conducts
    /// like the junction it is. A power tube's grid draws far harder than a
    /// preamp triode's, and what it does to the coupling capacitor in front of
    /// it is the blocking distortion a driven amplifier is known for.
    fn grid(&self, vgk: f64) -> f64 {
        if vgk < 0.0 {
            0.0
        } else {
            self.count * vgk / 600.0
        }
    }
}

impl Device for Pentode {
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.p, self.k);
        m.transconductance(self.p, self.k, self.g, self.k);
        m.transconductance(self.p, self.k, self.s, self.k);
        m.conductance(self.s, self.k);
        m.transconductance(self.s, self.k, self.g, self.k);
        m.conductance(self.g, self.k);
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.vpk, self.vgk, self.vsk, self.delta],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.vpk = saved.at[0];
        self.vgk = saved.at[1];
        self.vsk = saved.at[2];
        self.delta = saved.at[3];
        self.clamped = saved.clamped;
        self.trial_eval = None;
    }

    fn stamp(&mut self, st: &mut Stamper, v: &[f64]) {
        let vpk = across(v, self.p, self.k).max(0.0);
        let vsk = across(v, self.s, self.k).max(0.0);
        let raw = across(v, self.g, self.k);
        let (vgk, clamped) = if st.limiting {
            limit(raw, self.vgk, 4.0)
        } else {
            (raw, false)
        };
        self.clamped = clamped;
        self.delta = (vpk - self.vpk)
            .abs()
            .max((vgk - self.vgk).abs())
            .max((vsk - self.vsk).abs());
        self.vpk = vpk;
        self.vgk = vgk;
        self.vsk = vsk;

        let (ip, ig2, gp, gm, gs, gm2, gs2) = match self.trial_eval.take() {
            Some(cached) if cached.matches(vpk, vgk, vsk) => self.split_slopes_from_trial(cached),
            _ => self.split_with_slopes(vpk, vgk, vsk),
        };

        // Plate branch: its own conductance, plus the two transconductances
        // that say how the grid and the screen move it.
        let rp = gp.max(1e-12);
        st.conductance(self.p, self.k, rp);
        st.transconductance(self.p, self.k, self.g, self.k, gm);
        st.transconductance(self.p, self.k, self.s, self.k, gs);
        st.current(self.p, self.k, ip - rp * vpk - gm * vgk - gs * vsk);

        // Screen branch. No knee, so nothing here depends on the plate.
        let rs = gs2.max(1e-12);
        st.conductance(self.s, self.k, rs);
        st.transconductance(self.s, self.k, self.g, self.k, gm2);
        st.current(self.s, self.k, ig2 - rs * vsk - gm2 * vgk);

        let ig = self.grid(vgk);
        let gg = if vgk < 0.0 { 1e-12 } else { self.count / 600.0 };
        st.conductance(self.g, self.k, gg);
        st.current(self.g, self.k, ig - gg * vgk);
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
    /// What the rails are measured from. Ground on a split supply; the bias
    /// point on a pedal running off one battery, where the whole circuit --
    /// op-amp output included -- sits at half the supply and clips about that.
    reference: usize,
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

    pub fn new(
        out: usize,
        plus: usize,
        minus: usize,
        reference: usize,
        branch: usize,
        rail: f64,
    ) -> Self {
        Self {
            out,
            plus,
            minus,
            reference,
            branch,
            rail,
            clamped: 0.0,
            delta: 0.0,
        }
    }
}

impl Device for OpAmp {
    fn footprint(&self, m: &mut Mark) {
        m.branch_current(self.out, self.branch);
        m.branch_constraint(self.branch, self.plus);
        m.branch_constraint(self.branch, self.minus);
        m.branch_constraint(self.branch, self.out);
        m.branch_constraint(self.branch, self.reference);
    }

    /// The rail state travels with the linearisation, because it *is* one:
    /// `stamp` decides which state to be in partly from the state it was
    /// already in, and that hysteresis is what stops it chattering.
    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.clamped, self.delta, 0.0, 0.0],
            clamped: false,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.clamped = saved.at[0];
        self.delta = saved.at[1];
    }

    fn switches(&self) -> bool {
        true
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // Measured from the reference, so a circuit biased at half its supply
        // clips about that rather than about ground.
        let output = across(v, self.out, self.reference);
        let reference = if self.reference == GROUND {
            0.0
        } else {
            v[self.reference]
        };
        let error = across(v, self.plus, self.minus);

        // Which state to be in, from where the last solve put the output --
        // and, once against a rail, from whether the input still asks for
        // more.
        //
        // That second half is not a refinement. Deciding purely on the output
        // chatters: solved linear the output wants five volts, which is past
        // the rail, so the next pass pins it at three and a half -- where it
        // is no longer *past* the rail, so the pass after that goes linear
        // again and asks for five. Measured at high drive, more than half of
        // all samples never converged at all and the solver simply gave up on
        // them after thirty-two passes. A saturated amplifier stays saturated
        // while its input goes on demanding more, which is both what the
        // hardware does and what breaks the loop.
        let was = self.clamped;
        self.clamped = if was > 0.0 {
            if error >= 0.0 {
                self.rail
            } else {
                0.0
            }
        } else if was < 0.0 {
            if error <= 0.0 {
                -self.rail
            } else {
                0.0
            }
        } else if output > self.rail {
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
            if self.reference != GROUND {
                s.branch_constraint(self.branch, self.reference, -1.0);
            }
            s.branch_value(self.branch, self.clamped);
            let _ = reference;
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
        Self {
            d,
            g,
            s,
            spec,
            vgs: 0.0,
            vds: 0.0,
            delta: 0.0,
            clamped: false,
        }
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
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.d, self.s);
        m.transconductance(self.d, self.s, self.g, self.s);
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.vgs, self.vds, self.delta, 0.0],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.vgs = saved.at[0];
        self.vds = saved.at[1];
        self.delta = saved.at[2];
        self.clamped = saved.clamped;
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // A square law is gentle enough not to need the limiter a junction
        // does, but the gate can still be walked a long way in one iteration
        // by whatever is in front of it.
        let (vgs, held_g) = if s.limiting {
            limit(across(v, self.g, self.s), self.vgs, 0.5)
        } else {
            (across(v, self.g, self.s), false)
        };
        let (vds, held_d) = if s.limiting {
            limit(across(v, self.d, self.s), self.vds, 2.0)
        } else {
            (across(v, self.d, self.s), false)
        };
        self.clamped = held_g || held_d;
        self.delta = (vgs - self.vgs).abs().max((vds - self.vds).abs());
        self.vgs = vgs;
        self.vds = vds;

        let id = self.drain(vgs, vds);
        let step = 1e-4;
        let gm = (self.drain(vgs + step, vds) - self.drain(vgs - step, vds)) / (2.0 * step);
        let gds =
            ((self.drain(vgs, vds + step) - self.drain(vgs, vds - step)) / (2.0 * step)).max(1e-9);

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
    inv_henry: f64,
    inv_knee: f64,
    sharpness_integer: u8,
    /// Antialias only the nonlinear excess magnetising current. The linear
    /// inductance remains exact, so the small-signal transformer response does
    /// not acquire the half-sample low-pass of naive ADAA.
    antialias: bool,
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
    /// Nonlinear magnetising evaluation from the last reduced line-search
    /// candidate.  `antialiased_magnetising()` already computes the slope and
    /// discards it; retaining it lets an accepted candidate feed the next
    /// Newton stamp without evaluating the ADAA/core curve twice.
    trial_eval: Option<CoreEval>,
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
struct CoreEval {
    flux: f64,
    current: f64,
    slope: f64,
}

impl CoreEval {
    #[inline]
    fn matches(self, flux: f64) -> bool {
        self.flux.to_bits() == flux.to_bits()
    }
}

impl Core {
    pub fn new(a: usize, b: usize, spec: CoreSpec, rate: f64) -> Self {
        Self::with_antialias(a, b, spec, rate, false)
    }

    pub fn new_antialiased(a: usize, b: usize, spec: CoreSpec, rate: f64) -> Self {
        Self::with_antialias(a, b, spec, rate, true)
    }

    fn with_antialias(a: usize, b: usize, spec: CoreSpec, rate: f64, antialias: bool) -> Self {
        let rounded = spec.sharpness.round();
        let sharpness_integer =
            if (spec.sharpness - rounded).abs() < 1e-12 && (1.0..=15.0).contains(&rounded) {
                rounded as u8
            } else {
                0
            };
        Self {
            a,
            b,
            spec,
            inv_henry: 1.0 / spec.henry,
            inv_knee: 1.0 / spec.knee,
            sharpness_integer,
            antialias,
            flux: 0.0,
            last_flux: 0.0,
            last_volts: 0.0,
            volts: 0.0,
            delta: 0.0,
            clamped: false,
            half_step: 0.5 / rate,
            trial_eval: None,
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.half_step = 0.5 / rate;
        self.trial_eval = None;
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
        let knee = self.spec.knee;
        let sharp = self.spec.sharpness;
        let over = flux * self.inv_knee;
        let magnitude = over.abs();
        if magnitude < 1e-30 {
            return (flux * self.inv_henry, self.inv_henry);
        }
        let powered = match self.sharpness_integer {
            3 => {
                let square = magnitude * magnitude;
                square * magnitude
            }
            6 => {
                let cube = magnitude * magnitude * magnitude;
                cube * cube
            }
            7 => {
                let cube = magnitude * magnitude * magnitude;
                cube * cube * magnitude
            }
            9 => {
                let cube = magnitude * magnitude * magnitude;
                cube * cube * cube
            }
            exponent if exponent != 0 => powi_small(magnitude, exponent),
            _ => magnitude.powf(sharp),
        };
        let current = flux * self.inv_henry + powered * over.signum() * knee * self.inv_henry;
        let slope = self.inv_henry + sharp * (powered / magnitude) * self.inv_henry;
        (current, slope)
    }

    /// Nonlinear saturation current only, excluding the linear magnetising
    /// inductance, and its derivative with respect to flux.
    #[inline]
    fn nonlinear_excess_with_slope(&self, flux: f64) -> (f64, f64) {
        let over = flux * self.inv_knee;
        let magnitude = over.abs();
        if magnitude < 1e-30 {
            return (0.0, 0.0);
        }
        let powered = match self.sharpness_integer {
            exponent if exponent != 0 => powi_small(magnitude, exponent),
            _ => magnitude.powf(self.spec.sharpness),
        };
        let current = powered * over.signum() * self.spec.knee * self.inv_henry;
        let slope = self.spec.sharpness * (powered / magnitude) * self.inv_henry;
        (current, slope)
    }

    /// Antiderivative of the nonlinear saturation current. This is the cheap
    /// primitive needed for first-order antiderivative antialiasing.
    #[inline]
    fn nonlinear_excess_antiderivative(&self, flux: f64) -> f64 {
        let magnitude = (flux * self.inv_knee).abs();
        if magnitude < 1e-30 {
            return 0.0;
        }
        let exponent = self.spec.sharpness + 1.0;
        let powered = if self.sharpness_integer != 0 {
            powi_small(magnitude, self.sharpness_integer.saturating_add(1))
        } else {
            magnitude.powf(exponent)
        };
        self.spec.knee * self.spec.knee * self.inv_henry * powered / exponent
    }

    /// Average only the nonlinear excess current over the flux interval from
    /// the previous settled sample to this candidate sample. This suppresses
    /// the high-order fold-back from a hard core knee without doubling the
    /// entire Newton solve. The linear magnetising current remains evaluated
    /// at the current flux, preserving the transformer's low-level response.
    #[inline]
    fn antialiased_magnetising_with_slope(&self, flux: f64) -> (f64, f64) {
        if !self.antialias {
            return self.magnetising_with_slope(flux);
        }

        let previous = self.last_flux;
        let delta = flux - previous;
        let threshold = (self.spec.knee.abs() * 1e-8).max(1e-15);

        let (nonlinear, nonlinear_slope) = if delta.abs() <= threshold {
            // The divided difference tends to f at the interval midpoint; its
            // derivative with respect to the new endpoint tends to half f'.
            let midpoint = 0.5 * (flux + previous);
            let (value, slope) = self.nonlinear_excess_with_slope(midpoint);
            (value, 0.5 * slope)
        } else {
            let primitive_now = self.nonlinear_excess_antiderivative(flux);
            let primitive_previous = self.nonlinear_excess_antiderivative(previous);
            let average = (primitive_now - primitive_previous) / delta;
            let (instantaneous, _) = self.nonlinear_excess_with_slope(flux);
            let slope = ((instantaneous - average) / delta).max(0.0);
            (average, slope)
        };

        (
            flux * self.inv_henry + nonlinear,
            self.inv_henry + nonlinear_slope,
        )
    }

    #[inline]
    fn antialiased_magnetising(&self, flux: f64) -> f64 {
        self.antialiased_magnetising_with_slope(flux).0
    }

    pub fn flux(&self) -> f64 {
        self.flux
    }
}

impl Device for Core {
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.a, self.b);
    }

    /// `flux` and `volts` only. `last_flux` and `last_volts` are the timestep
    /// speaking, not the linearisation, and `advance` owns those.
    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.volts, self.flux, self.delta, 0.0],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.volts = saved.at[0];
        self.flux = saved.at[1];
        self.delta = saved.at[2];
        self.clamped = saved.clamped;
        self.trial_eval = None;
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // Limit the change in flux, which is what makes this curve steep.
        // A fixed eight-volt limit forced a power transformer to walk a
        // normal plate swing across many Newton passes even when its flux
        // barely moved. Bound each correction to knee/sharpness in flux;
        // d(flux)/d(volts) is half_step. This limits Newton, not the signal
        // or the magnetising curve, and scales with the actual timestep.
        let raw = across(v, self.a, self.b);
        let (volts, clamped) = if s.limiting {
            let scale = self.spec.knee / (2.0 * self.half_step * self.spec.sharpness);
            limit(raw, self.volts, scale)
        } else {
            (raw, false)
        };
        self.clamped = clamped;
        self.delta = (volts - self.volts).abs();
        self.volts = volts;

        // Trapezoidal: the flux at this instant is what it was, plus the area
        // under the voltage since.
        let history = self.last_flux + self.half_step * self.last_volts;
        let flux = history + self.half_step * volts;
        self.flux = flux;

        let evaluation = match self.trial_eval.take() {
            Some(cached) if cached.matches(flux) => cached,
            _ => {
                let (current, slope) = self.antialiased_magnetising_with_slope(flux);
                CoreEval {
                    flux,
                    current,
                    slope,
                }
            }
        };
        let CoreEval {
            current: i, slope, ..
        } = evaluation;
        // d(current)/d(volts) is d(current)/d(flux) times the half step, which
        // is what the integration contributes. Use the exact derivative of
        // this same current curve: differencing nearby flux values both loses
        // precision and evaluates the power curve three times per stamp.
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
        self.trial_eval = None;
    }
}

/// A bipolar transistor, by the Ebers-Moll transport model.
///
/// Both junctions, not just the forward one. A transistor in a pedal spends a
/// good deal of its time driven into saturation -- collector pulled down to
/// near the emitter -- and what it does there is a large part of how the
/// circuit sounds. Modelling only the base-emitter junction gives a device
/// that amplifies forever and never runs out of collector.
///
/// The Early voltage gives the collector its finite output resistance, which
/// is what stops a common-emitter stage having infinite gain.
pub struct Bipolar {
    c: usize,
    b: usize,
    e: usize,
    spec: BipolarSpec,
    /// A PNP part: the same equations on mirrored junctions and currents.
    pnp: bool,
    critical: f64,
    inv_early: f64,
    inv_forward_beta: f64,
    inv_reverse_beta: f64,
    vbe: f64,
    vbc: f64,
    delta: f64,
    clamped: bool,
}

impl Bipolar {
    pub fn new(c: usize, b: usize, e: usize, spec: BipolarSpec, pnp: bool) -> Self {
        Self {
            c,
            b,
            e,
            spec,
            pnp,
            critical: VT * (VT / (std::f64::consts::SQRT_2 * spec.saturation)).ln(),
            inv_early: 1.0 / spec.early,
            inv_forward_beta: 1.0 / spec.forward_beta,
            inv_reverse_beta: 1.0 / spec.reverse_beta,
            vbe: 0.0,
            vbc: 0.0,
            delta: 0.0,
            clamped: false,
        }
    }

    /// The junction limiter, as for a diode: a flat cap on the step is
    /// hopeless when the answer is a long way off, and an exponential
    /// overflows if it is let run.
    fn limit_junction(&self, wanted: f64, old: f64) -> (f64, bool) {
        let critical = self.critical;
        if wanted > critical && (wanted - old).abs() > 2.0 * VT {
            if old > 0.0 {
                let arg = 1.0 + (wanted - old) / VT;
                (
                    if arg > 0.0 {
                        old + VT * arg.ln()
                    } else {
                        critical
                    },
                    true,
                )
            } else {
                (VT * (wanted / VT).ln().max(-40.0), true)
            }
        } else {
            (wanted, false)
        }
    }
}

impl Device for Bipolar {
    fn footprint(&self, m: &mut Mark) {
        let (c, b, e) = (self.c, self.b, self.e);
        if self.pnp {
            m.transconductance(e, c, e, b);
            m.transconductance(e, c, c, b);
            m.transconductance(e, b, e, b);
            m.transconductance(e, b, c, b);
        } else {
            m.transconductance(c, e, b, e);
            m.transconductance(c, e, b, c);
            m.transconductance(b, e, b, e);
            m.transconductance(b, e, b, c);
        }
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.vbe, self.vbc, self.delta, 0.0],
            clamped: self.clamped,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.vbe = saved.at[0];
        self.vbc = saved.at[1];
        self.delta = saved.at[2];
        self.clamped = saved.clamped;
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        // A PNP part is the NPN equations on mirrored junctions: emitter-base
        // and collector-base, with every current reversed. `vbe` and `vbc`
        // below are then the mirrored junction voltages.
        let (vbe_now, vbc_now) = if self.pnp {
            (across(v, self.e, self.b), across(v, self.c, self.b))
        } else {
            (across(v, self.b, self.e), across(v, self.b, self.c))
        };
        let (vbe, held_e) = self.limit_junction(vbe_now, self.vbe);
        let (vbc, held_c) = self.limit_junction(vbc_now, self.vbc);
        s.junction_held |= held_e || held_c;
        self.clamped = held_e || held_c;
        self.delta = (vbe - self.vbe).abs().max((vbc - self.vbc).abs());
        self.vbe = vbe;
        self.vbc = vbc;

        // Both junctions, each with its own slope -- and the slope of
        // `Is (e^x - 1)` is `(i + Is) / VT`, so it comes from the same
        // exponential rather than a second one.
        let cap = 60.0;
        let ef = (vbe / VT).min(cap).exp();
        let er = (vbc / VT).min(cap).exp();
        let forward = self.spec.saturation * (ef - 1.0);
        let reverse = self.spec.saturation * (er - 1.0);
        let gf = (self.spec.saturation * ef / VT).max(1e-12);
        let gr = (self.spec.saturation * er / VT).max(1e-12);

        // The Early effect: the collector current rises slightly with the
        // voltage across it, which is the stage's finite output resistance.
        let vce = vbe - vbc;
        let early = 1.0 + (vce * self.inv_early).max(-0.9);

        let ic = (forward - reverse) * early - reverse * self.inv_reverse_beta;
        let ib = forward * self.inv_forward_beta + reverse * self.inv_reverse_beta;

        let dic_dvbe = gf * early;
        let dic_dvbc = -gr * (early + self.inv_reverse_beta);
        let dib_dvbe = gf * self.inv_forward_beta;
        let dib_dvbc = gr * self.inv_reverse_beta;

        let (c, b, e) = (self.c, self.b, self.e);
        if self.pnp {
            // Emitter to collector and emitter to base, controlled by the
            // emitter-base and collector-base junctions.
            s.transconductance(e, c, e, b, dic_dvbe);
            s.transconductance(e, c, c, b, dic_dvbc);
            s.current(e, c, ic - dic_dvbe * vbe - dic_dvbc * vbc);
            s.transconductance(e, b, e, b, dib_dvbe);
            s.transconductance(e, b, c, b, dib_dvbc);
            s.current(e, b, ib - dib_dvbe * vbe - dib_dvbc * vbc);
        } else {
            // Collector current, controlled by both junctions.
            s.transconductance(c, e, b, e, dic_dvbe);
            s.transconductance(c, e, b, c, dic_dvbc);
            s.current(c, e, ic - dic_dvbe * vbe - dic_dvbc * vbc);

            // And the base current it takes to get it.
            s.transconductance(b, e, b, e, dib_dvbe);
            s.transconductance(b, e, b, c, dib_dvbc);
            s.current(b, e, ib - dib_dvbe * vbe - dib_dvbc * vbc);
        }
    }

    fn moved(&self) -> f64 {
        self.delta
    }

    fn settled(&self, _tolerance: f64) -> bool {
        !self.clamped
    }
}

/// A transconductance that runs out of current.
///
/// The slow half of a compensated op-amp. An input pair drives a current into
/// the compensation capacitor, and that current is limited by the pair's tail:
/// small differences give `gm` times the difference, which on the capacitor is
/// an integrator and so a gain-bandwidth of `gm / 2 pi C`; large ones give the
/// whole tail current, which is a slew rate of `limit / C`. `tanh` joins the
/// two smoothly, as a long-tailed pair's transfer curve does.
pub struct Transconductor {
    plus: usize,
    minus: usize,
    out: usize,
    reference: usize,
    gm: f64,
    limit: f64,
    difference: f64,
    delta: f64,
}

impl Transconductor {
    pub fn new(
        plus: usize,
        minus: usize,
        out: usize,
        reference: usize,
        gm: f64,
        limit: f64,
    ) -> Self {
        Self {
            plus,
            minus,
            out,
            reference,
            gm,
            limit,
            difference: 0.0,
            delta: 0.0,
        }
    }

    /// The output current for an input difference.
    pub fn current(&self, difference: f64) -> f64 {
        self.limit * (self.gm * difference / self.limit).tanh()
    }
}

impl Device for Transconductor {
    fn footprint(&self, m: &mut Mark) {
        m.transconductance(self.reference, self.out, self.plus, self.minus);
    }

    fn linearisation(&self) -> Linearisation {
        Linearisation {
            at: [self.difference, self.delta, 0.0, 0.0],
            clamped: false,
        }
    }

    fn relinearise(&mut self, saved: Linearisation) {
        self.difference = saved.at[0];
        self.delta = saved.at[1];
    }

    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        let difference = across(v, self.plus, self.minus);
        self.delta = (difference - self.difference).abs();
        self.difference = difference;
        let x = self.gm * difference / self.limit;
        let t = x.tanh();
        let i = self.limit * t;
        // Tangent in the linear region, chord through the origin once it
        // saturates. The tangent of a saturated tanh is flat, so Newton reads
        // a saturated stage as one that no input can move, asks for volts to
        // move it, and lands saturated the other way -- and a stage inside a
        // fast feedback loop (an op-amp at unity gain) did exactly that on
        // every pass until the solve gave up. The chord is never flatter than
        // `limit / difference`, so the linear solve cannot carry the input past
        // the point where the current changes sign.
        let g = if x.abs() < 1.0 {
            self.gm * (1.0 - t * t)
        } else {
            i / difference
        };
        // Out of `reference` and into `out`.
        s.transconductance(self.reference, self.out, self.plus, self.minus, g);
        s.current(self.reference, self.out, i - g * difference);
    }

    fn moved(&self) -> f64 {
        self.delta
    }
}

// ---------------------------------------------------------------------------
// Devices, stored inline
// ---------------------------------------------------------------------------

/// A resistor whose value may change at audio/control rate without rebuilding
/// the circuit topology.  It is linear at every instant, but it is stamped with
/// the nonlinear devices because its conductance is not part of the immutable
/// base matrix.  The AB763 optocoupler uses this for the photocell: the lamp/LDR
/// dynamics choose the resistance once per sample and Newton then sees the real
/// loading of the V4-B plate hand-off, rather than a post-circuit gain multiply.
pub struct VariableResistor {
    a: usize,
    b: usize,
    slot: usize,
    ohms: f64,
}

impl VariableResistor {
    pub fn new(a: usize, b: usize, slot: usize, ohms: f64) -> Self {
        Self { a, b, slot, ohms }
    }

    #[inline]
    pub fn set(&mut self, slot: usize, ohms: f64) -> bool {
        if self.slot == slot && ohms.is_finite() && ohms > 0.0 {
            self.ohms = ohms;
            true
        } else {
            false
        }
    }
}

impl Device for VariableResistor {
    #[inline]
    fn footprint(&self, m: &mut Mark) {
        m.conductance(self.a, self.b);
    }

    #[inline]
    fn stamp(&mut self, s: &mut Stamper, _v: &[f64]) {
        s.conductance(self.a, self.b, 1.0 / self.ohms);
    }

    #[inline]
    fn moved(&self) -> f64 {
        0.0
    }
}

/// Device evaluation used only by Schur-reduced line-search merit probes.
///
/// A rejected reduced trial must not perturb physical device state. These
/// probes may update only an ephemeral evaluation cache; limiter/history/
/// linearisation state remains untouched. That keeps rejected probes free of
/// the old save/restore walk while letting an accepted point reuse the exact
/// same nonlinear evaluation in the immediately following Newton stamp. The
/// accepted trial still commits physical state separately.
trait TrialResidual {
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], cache_eval: bool);
}

impl TrialResidual for Diode {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        let wanted = across(v, self.a, self.k);
        let old = self.voltage;
        let scale = self.scale;
        let (guess, held) = if wanted > self.critical && (wanted - old).abs() > 2.0 * scale {
            let guess = if old > 0.0 {
                let arg = 1.0 + (wanted - old) * self.inv_scale;
                if arg > 0.0 {
                    old + scale * arg.ln()
                } else {
                    self.critical
                }
            } else {
                scale * (wanted * self.inv_scale).ln()
            };
            (guess, true)
        } else {
            (wanted, false)
        };
        r.junction_held |= held;
        if held {
            return;
        }
        let x = guess * self.inv_scale;
        let i = if x >= CLAMP {
            self.spec.saturation * (CLAMP.exp() - 1.0)
        } else {
            self.spec.saturation * (x.exp() - 1.0)
        };
        r.current(self.a, self.k, i);
    }
}

impl TrialResidual for Rectifier {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        let wanted = across(v, self.a, self.k);
        let (guess, _) = limit(wanted, self.voltage, 25.0);

        // Rectifier limiting is intentionally unconditional in the production
        // stamp. When it engages, reproduce the old tangent residual at the
        // requested point rather than substituting the current at `guess`.
        let (i, g) = if guess > 0.0 {
            let root = guess.sqrt();
            (
                self.perveance * guess * root,
                (1.5 * self.perveance * root).max(Self::OFF),
            )
        } else {
            (0.0, Self::OFF)
        };
        r.current(self.a, self.k, i + g * (wanted - guess));
    }
}

impl TrialResidual for Triode {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], cache_eval: bool) {
        let vpk = across(v, self.p, self.k).max(0.0);
        let raw = across(v, self.g, self.k);
        let vgk = if r.limiting {
            limit_grid(raw, self.vgk).0
        } else {
            raw
        };
        let ip = if cache_eval {
            let evaluation = self.plate_trial_eval(vpk, vgk);
            let ip = evaluation.ip;
            self.trial_eval = Some(evaluation);
            ip
        } else {
            self.trial_eval = None;
            self.plate(vpk, vgk)
        };
        r.current(self.p, self.k, ip);
        r.current(self.g, self.k, self.grid(vgk));
    }
}

impl TrialResidual for Pentode {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], cache_eval: bool) {
        let vpk = across(v, self.p, self.k).max(0.0);
        let vsk = across(v, self.s, self.k).max(0.0);
        let raw = across(v, self.g, self.k);
        let vgk = if r.limiting {
            limit(raw, self.vgk, 4.0).0
        } else {
            raw
        };

        let (ip, ig2) = if cache_eval {
            let evaluation = self.split_trial_eval(vpk, vgk, vsk);
            let currents = (evaluation.ip, evaluation.ig2);
            self.trial_eval = Some(evaluation);
            currents
        } else {
            self.trial_eval = None;
            // This is deliberately the pre-cache residual path. The test-only
            // A/B switch must restore the old line-search workload exactly.
            if vsk <= 0.0 || vpk <= 0.0 {
                (0.0, 0.0)
            } else {
                let c = &self.spec;
                let inner = c.kp * (self.inv_mu + vgk / vsk);
                let soft = if inner > 30.0 {
                    inner
                } else {
                    inner.exp().ln_1p()
                };
                let e1 = vsk * self.inv_kp * soft;
                if e1 <= 0.0 {
                    (0.0, 0.0)
                } else {
                    let powered = e1.powf(c.ex);
                    let plate_base = powered * self.inv_kg1;
                    let screen_base = powered * self.inv_kg2;
                    (
                        self.count * plate_base * (vpk * self.inv_kvb).atan(),
                        self.count * screen_base,
                    )
                }
            }
        };
        r.current(self.p, self.k, ip);
        r.current(self.s, self.k, ig2);
        r.current(self.g, self.k, self.grid(vgk));
    }
}

impl TrialResidual for Jfet {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        let raw_vgs = across(v, self.g, self.s);
        let raw_vds = across(v, self.d, self.s);
        let vgs = if r.limiting {
            limit(raw_vgs, self.vgs, 0.5).0
        } else {
            raw_vgs
        };
        let vds = if r.limiting {
            limit(raw_vds, self.vds, 2.0).0
        } else {
            raw_vds
        };
        r.current(self.d, self.s, self.drain(vgs, vds));
    }
}

impl TrialResidual for Bipolar {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        let (vbe_now, vbc_now) = if self.pnp {
            (across(v, self.e, self.b), across(v, self.c, self.b))
        } else {
            (across(v, self.b, self.e), across(v, self.b, self.c))
        };
        let (vbe, held_e) = self.limit_junction(vbe_now, self.vbe);
        let (vbc, held_c) = self.limit_junction(vbc_now, self.vbc);
        r.junction_held |= held_e || held_c;
        if held_e || held_c {
            return;
        }

        let ef = (vbe / VT).min(60.0).exp();
        let er = (vbc / VT).min(60.0).exp();
        let forward = self.spec.saturation * (ef - 1.0);
        let reverse = self.spec.saturation * (er - 1.0);
        let vce = vbe - vbc;
        let early = 1.0 + (vce * self.inv_early).max(-0.9);
        let ic = (forward - reverse) * early - reverse * self.inv_reverse_beta;
        let ib = forward * self.inv_forward_beta + reverse * self.inv_reverse_beta;
        if self.pnp {
            r.current(self.e, self.c, ic);
            r.current(self.e, self.b, ib);
        } else {
            r.current(self.c, self.e, ic);
            r.current(self.b, self.e, ib);
        }
    }
}

impl TrialResidual for OpAmp {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        let output = across(v, self.out, self.reference);
        let error = across(v, self.plus, self.minus);
        let was = self.clamped;
        let clamped = if was > 0.0 {
            if error >= 0.0 {
                self.rail
            } else {
                0.0
            }
        } else if was < 0.0 {
            if error <= 0.0 {
                -self.rail
            } else {
                0.0
            }
        } else if output > self.rail {
            self.rail
        } else if output < -self.rail {
            -self.rail
        } else {
            0.0
        };

        let branch_current = if self.branch == GROUND {
            0.0
        } else {
            v[self.branch]
        };
        r.row(self.out, branch_current);
        if clamped == 0.0 {
            r.row(self.branch, error);
        } else {
            r.row(self.branch, output - clamped);
        }
    }
}

impl TrialResidual for Core {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], cache_eval: bool) {
        let raw = across(v, self.a, self.b);
        let volts = if r.limiting {
            let scale = self.spec.knee / (2.0 * self.half_step * self.spec.sharpness);
            limit(raw, self.volts, scale).0
        } else {
            raw
        };
        let history = self.last_flux + self.half_step * self.last_volts;
        let flux = history + self.half_step * volts;
        let current = if cache_eval {
            let (current, slope) = self.antialiased_magnetising_with_slope(flux);
            self.trial_eval = Some(CoreEval {
                flux,
                current,
                slope,
            });
            current
        } else {
            self.trial_eval = None;
            self.antialiased_magnetising(flux)
        };
        r.current(self.a, self.b, current);
    }
}

impl TrialResidual for Transconductor {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        let difference = across(v, self.plus, self.minus);
        r.current(self.reference, self.out, self.current(difference));
    }
}

impl TrialResidual for VariableResistor {
    #[inline]
    fn trial_residual(&mut self, r: &mut ResidualStamper<'_>, v: &[f64], _cache_eval: bool) {
        r.current(self.a, self.b, across(v, self.a, self.b) / self.ohms);
    }
}

/// A device, as one type rather than one box.
///
/// `Simulation::devices` used to be `Vec<Box<dyn Device>>`. Every Newton pass
/// dispatched through a vtable eight times per device and every write a device
/// made to its own fields went through an `&mut dyn Device` the compiler could
/// not see into. `Triode::stamp` computes an `exp`, a `ln_1p`, a `powf` and a
/// `sqrt`, and the intermediate results were being spilled to memory between
/// the operations that produced them because the compiler could not prove
/// nobody else held a reference to `self`.
///
/// An enum removes both: the match is monomorphic at each arm, so the concrete
/// `stamp` inlines into the pass loop, and the device's fields live in a
/// caller-frame local rather than behind a pointer. The vector is contiguous,
/// which also improves cache behaviour on the foot and linearisation walks
/// that visit every device in turn.
///
/// The trait stays, and `AnyDevice` implements it by forwarding, so nothing
/// that calls `device.stamp(...)` or `device.settled(...)` needs to know which
/// type it is holding.
pub enum AnyDevice {
    Diode(Diode),
    Rectifier(Rectifier),
    Triode(Triode),
    Pentode(Pentode),
    Jfet(Jfet),
    Bipolar(Bipolar),
    OpAmp(OpAmp),
    Core(Core),
    Transconductor(Transconductor),
    VariableResistor(VariableResistor),
}

/// A rejected rescue must restore both limiter history and the evaluation
/// waiting for the next stamp. `relinearise` alone intentionally clears caches.
/// Integration history is not included: neither stamps nor trials advance it.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct DeviceCheckpoint {
    linearisation: Linearisation,
    cache: TrialCache,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum TrialCache {
    #[default]
    None,
    Triode(Option<TriodeEval>),
    Pentode(Option<PentodeEval>),
    Core(Option<CoreEval>),
}

impl AnyDevice {
    #[cfg(test)]
    pub(crate) fn limiter_held(&self) -> bool {
        self.linearisation().clamped
    }

    #[cfg(test)]
    pub(super) fn checkpoint(&self) -> DeviceCheckpoint {
        DeviceCheckpoint {
            linearisation: self.linearisation(),
            cache: match self {
                Self::Triode(device) => TrialCache::Triode(device.trial_eval),
                Self::Pentode(device) => TrialCache::Pentode(device.trial_eval),
                Self::Core(device) => TrialCache::Core(device.trial_eval),
                _ => TrialCache::None,
            },
        }
    }

    #[cfg(test)]
    pub(super) fn restore_checkpoint(&mut self, saved: DeviceCheckpoint) {
        self.relinearise(saved.linearisation);
        match (self, saved.cache) {
            (Self::Triode(device), TrialCache::Triode(cache)) => device.trial_eval = cache,
            (Self::Pentode(device), TrialCache::Pentode(cache)) => device.trial_eval = cache,
            (Self::Core(device), TrialCache::Core(cache)) => device.trial_eval = cache,
            _ => {}
        }
    }

    /// Copy an identical device's evolving state without reconstructing it.
    /// All variants keep their component specifications and terminal indices.
    /// A core additionally owns committed integration history, which is not
    /// part of the trial Newton linearisation.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        match (self, source) {
            (Self::Diode(dst), Self::Diode(src)) => dst.relinearise(src.linearisation()),
            (Self::Rectifier(dst), Self::Rectifier(src)) => dst.relinearise(src.linearisation()),
            (Self::Triode(dst), Self::Triode(src)) => dst.relinearise(src.linearisation()),
            (Self::Pentode(dst), Self::Pentode(src)) => dst.relinearise(src.linearisation()),
            (Self::Jfet(dst), Self::Jfet(src)) => dst.relinearise(src.linearisation()),
            (Self::Bipolar(dst), Self::Bipolar(src)) => dst.relinearise(src.linearisation()),
            (Self::OpAmp(dst), Self::OpAmp(src)) => dst.relinearise(src.linearisation()),
            (Self::Transconductor(dst), Self::Transconductor(src)) => {
                dst.relinearise(src.linearisation())
            }
            (Self::VariableResistor(dst), Self::VariableResistor(src)) => {
                dst.ohms = src.ohms;
            }
            (Self::Core(dst), Self::Core(src)) => {
                dst.relinearise(src.linearisation());
                dst.last_flux = src.last_flux;
                dst.last_volts = src.last_volts;
                // The simulation's requested rate already agrees. A dormant
                // device may not yet have installed that timestep's cache.
                dst.half_step = src.half_step;
            }
            _ => panic!("runtime copy requires identical device variants"),
        }
    }

    /// Update one audio-rate resistor by its netlist slot. Returns true when
    /// this is the device that owns the slot.
    #[inline]
    pub fn set_realtime_resistor(&mut self, slot: usize, ohms: f64) -> bool {
        match self {
            Self::VariableResistor(r) => r.set(slot, ohms),
            _ => false,
        }
    }

    /// Whether this device's ordinary Newton *step limiter* would hold the
    /// present point back.  This deliberately excludes diode/BJT junction
    /// limiting: those limiters alter the semiconductor equation used for the
    /// stamp and retain their existing SPICE-style path.
    ///
    /// The Twin power solver uses this cheap preflight only to choose between
    /// its two existing globalization mechanisms.  If a smooth valve-grid
    /// limiter would have to walk several volts over several Newton passes, a
    /// global line search can instead damp the exact Newton direction now.
    /// Nothing is stamped and no device state is changed here.
    #[inline]
    pub(crate) fn step_limiter_would_hold(&self, v: &[f64]) -> bool {
        match self {
            Self::Triode(t) => {
                let wanted = across(v, t.g, t.k);
                limit_grid(wanted, t.vgk).1
            }
            Self::Pentode(p) => {
                let wanted = across(v, p.g, p.k);
                limit(wanted, p.vgk, 4.0).1
            }
            _ => false,
        }
    }

    /// Evaluate only the nonlinear residual for a line-search trial point.
    ///
    /// Valve/core variants may also retain an ephemeral full evaluation for
    /// the immediately following Newton stamp.  That cache is not physical
    /// device state: rejected trials still leave limiter/history state alone,
    /// and a cached value is consumed only when its terminal point matches
    /// bit-for-bit.
    #[inline]
    pub(crate) fn trial_residual(
        &mut self,
        r: &mut ResidualStamper<'_>,
        v: &[f64],
        cache_eval: bool,
    ) {
        match self {
            Self::Diode(d) => d.trial_residual(r, v, cache_eval),
            Self::Rectifier(x) => x.trial_residual(r, v, cache_eval),
            Self::Triode(t) => t.trial_residual(r, v, cache_eval),
            Self::Pentode(p) => p.trial_residual(r, v, cache_eval),
            Self::Jfet(j) => j.trial_residual(r, v, cache_eval),
            Self::Bipolar(b) => b.trial_residual(r, v, cache_eval),
            Self::OpAmp(o) => o.trial_residual(r, v, cache_eval),
            Self::Core(c) => c.trial_residual(r, v, cache_eval),
            Self::Transconductor(t) => t.trial_residual(r, v, cache_eval),
            Self::VariableResistor(vr) => vr.trial_residual(r, v, cache_eval),
        }
    }

    /// Commit only the nonlinear device state for an accepted reduced
    /// line-search point. Rejected probes are read-only; doing this once for
    /// the winner preserves the exact limiter/hysteresis/convergence state the
    /// old mutating trial stamp would have left behind.
    #[inline]
    pub(crate) fn commit_trial_state(&mut self, v: &[f64]) {
        match self {
            Self::Diode(d) => {
                let guess = d.limit_junction(across(v, d.a, d.k));
                d.delta = (guess - d.voltage).abs();
                d.voltage = guess;
            }
            Self::Rectifier(x) => {
                let wanted = across(v, x.a, x.k);
                let (guess, clamped) = limit(wanted, x.voltage, 25.0);
                x.clamped = clamped;
                x.delta = (guess - x.voltage).abs();
                x.voltage = guess;
            }
            Self::Triode(t) => {
                let vpk = across(v, t.p, t.k).max(0.0);
                let vgk = across(v, t.g, t.k);
                t.clamped = false;
                t.delta = (vpk - t.vpk).abs().max((vgk - t.vgk).abs());
                t.vpk = vpk;
                t.vgk = vgk;
            }
            Self::Pentode(p) => {
                let vpk = across(v, p.p, p.k).max(0.0);
                let vsk = across(v, p.s, p.k).max(0.0);
                let vgk = across(v, p.g, p.k);
                p.clamped = false;
                p.delta = (vpk - p.vpk)
                    .abs()
                    .max((vgk - p.vgk).abs())
                    .max((vsk - p.vsk).abs());
                p.vpk = vpk;
                p.vgk = vgk;
                p.vsk = vsk;
            }
            Self::Jfet(j) => {
                let vgs = across(v, j.g, j.s);
                let vds = across(v, j.d, j.s);
                j.clamped = false;
                j.delta = (vgs - j.vgs).abs().max((vds - j.vds).abs());
                j.vgs = vgs;
                j.vds = vds;
            }
            Self::Bipolar(b) => {
                let (vbe_now, vbc_now) = if b.pnp {
                    (across(v, b.e, b.b), across(v, b.c, b.b))
                } else {
                    (across(v, b.b, b.e), across(v, b.b, b.c))
                };
                let (vbe, held_e) = b.limit_junction(vbe_now, b.vbe);
                let (vbc, held_c) = b.limit_junction(vbc_now, b.vbc);
                b.clamped = held_e || held_c;
                b.delta = (vbe - b.vbe).abs().max((vbc - b.vbc).abs());
                b.vbe = vbe;
                b.vbc = vbc;
            }
            Self::OpAmp(o) => {
                let output = across(v, o.out, o.reference);
                let error = across(v, o.plus, o.minus);
                let was = o.clamped;
                o.clamped = if was > 0.0 {
                    if error >= 0.0 {
                        o.rail
                    } else {
                        0.0
                    }
                } else if was < 0.0 {
                    if error <= 0.0 {
                        -o.rail
                    } else {
                        0.0
                    }
                } else if output > o.rail {
                    o.rail
                } else if output < -o.rail {
                    -o.rail
                } else {
                    0.0
                };
                o.delta =
                    (o.clamped - was)
                        .abs()
                        .max(if o.clamped == 0.0 { error.abs() } else { 0.0 });
            }
            Self::Core(c) => {
                let volts = across(v, c.a, c.b);
                c.clamped = false;
                c.delta = (volts - c.volts).abs();
                c.volts = volts;
                let history = c.last_flux + c.half_step * c.last_volts;
                c.flux = history + c.half_step * volts;
            }
            Self::Transconductor(t) => {
                let difference = across(v, t.plus, t.minus);
                t.delta = (difference - t.difference).abs();
                t.difference = difference;
            }
            Self::VariableResistor(_) => {}
        }
    }
}

impl Device for AnyDevice {
    #[inline]
    fn stamp(&mut self, s: &mut Stamper, v: &[f64]) {
        match self {
            AnyDevice::Diode(d) => d.stamp(s, v),
            AnyDevice::Rectifier(r) => r.stamp(s, v),
            AnyDevice::Triode(t) => t.stamp(s, v),
            AnyDevice::Pentode(p) => p.stamp(s, v),
            AnyDevice::Jfet(j) => j.stamp(s, v),
            AnyDevice::Bipolar(b) => b.stamp(s, v),
            AnyDevice::OpAmp(o) => o.stamp(s, v),
            AnyDevice::Core(c) => c.stamp(s, v),
            AnyDevice::Transconductor(t) => t.stamp(s, v),
            AnyDevice::VariableResistor(r) => r.stamp(s, v),
        }
    }

    #[inline]
    fn footprint(&self, m: &mut Mark) {
        match self {
            AnyDevice::Diode(d) => d.footprint(m),
            AnyDevice::Rectifier(r) => r.footprint(m),
            AnyDevice::Triode(t) => t.footprint(m),
            AnyDevice::Pentode(p) => p.footprint(m),
            AnyDevice::Jfet(j) => j.footprint(m),
            AnyDevice::Bipolar(b) => b.footprint(m),
            AnyDevice::OpAmp(o) => o.footprint(m),
            AnyDevice::Core(c) => c.footprint(m),
            AnyDevice::Transconductor(t) => t.footprint(m),
            AnyDevice::VariableResistor(r) => r.footprint(m),
        }
    }

    #[inline]
    fn moved(&self) -> f64 {
        match self {
            AnyDevice::Diode(d) => d.moved(),
            AnyDevice::Rectifier(r) => r.moved(),
            AnyDevice::Triode(t) => t.moved(),
            AnyDevice::Pentode(p) => p.moved(),
            AnyDevice::Jfet(j) => j.moved(),
            AnyDevice::Bipolar(b) => b.moved(),
            AnyDevice::OpAmp(o) => o.moved(),
            AnyDevice::Core(c) => c.moved(),
            AnyDevice::Transconductor(t) => t.moved(),
            AnyDevice::VariableResistor(r) => r.moved(),
        }
    }

    #[inline]
    fn settled(&self, tolerance: f64) -> bool {
        match self {
            AnyDevice::Diode(d) => d.settled(tolerance),
            AnyDevice::Rectifier(r) => r.settled(tolerance),
            AnyDevice::Triode(t) => t.settled(tolerance),
            AnyDevice::Pentode(p) => p.settled(tolerance),
            AnyDevice::Jfet(j) => j.settled(tolerance),
            AnyDevice::Bipolar(b) => b.settled(tolerance),
            AnyDevice::OpAmp(o) => o.settled(tolerance),
            AnyDevice::Core(c) => c.settled(tolerance),
            AnyDevice::Transconductor(t) => t.settled(tolerance),
            AnyDevice::VariableResistor(r) => r.settled(tolerance),
        }
    }

    #[inline]
    fn linearisation(&self) -> Linearisation {
        match self {
            AnyDevice::Diode(d) => d.linearisation(),
            AnyDevice::Rectifier(r) => r.linearisation(),
            AnyDevice::Triode(t) => t.linearisation(),
            AnyDevice::Pentode(p) => p.linearisation(),
            AnyDevice::Jfet(j) => j.linearisation(),
            AnyDevice::Bipolar(b) => b.linearisation(),
            AnyDevice::OpAmp(o) => o.linearisation(),
            AnyDevice::Core(c) => c.linearisation(),
            AnyDevice::Transconductor(t) => t.linearisation(),
            AnyDevice::VariableResistor(r) => r.linearisation(),
        }
    }

    #[inline]
    fn relinearise(&mut self, saved: Linearisation) {
        match self {
            AnyDevice::Diode(d) => d.relinearise(saved),
            AnyDevice::Rectifier(r) => r.relinearise(saved),
            AnyDevice::Triode(t) => t.relinearise(saved),
            AnyDevice::Pentode(p) => p.relinearise(saved),
            AnyDevice::Jfet(j) => j.relinearise(saved),
            AnyDevice::Bipolar(b) => b.relinearise(saved),
            AnyDevice::OpAmp(o) => o.relinearise(saved),
            AnyDevice::Core(c) => c.relinearise(saved),
            AnyDevice::Transconductor(t) => t.relinearise(saved),
            AnyDevice::VariableResistor(r) => r.relinearise(saved),
        }
    }

    #[inline]
    fn advance(&mut self) {
        match self {
            AnyDevice::Diode(d) => d.advance(),
            AnyDevice::Rectifier(r) => r.advance(),
            AnyDevice::Triode(t) => t.advance(),
            AnyDevice::Pentode(p) => p.advance(),
            AnyDevice::Jfet(j) => j.advance(),
            AnyDevice::Bipolar(b) => b.advance(),
            AnyDevice::OpAmp(o) => o.advance(),
            AnyDevice::Core(c) => c.advance(),
            AnyDevice::Transconductor(t) => t.advance(),
            AnyDevice::VariableResistor(r) => r.advance(),
        }
    }

    #[inline]
    fn switches(&self) -> bool {
        match self {
            AnyDevice::Diode(d) => d.switches(),
            AnyDevice::Rectifier(r) => r.switches(),
            AnyDevice::Triode(t) => t.switches(),
            AnyDevice::Pentode(p) => p.switches(),
            AnyDevice::Jfet(j) => j.switches(),
            AnyDevice::Bipolar(b) => b.switches(),
            AnyDevice::OpAmp(o) => o.switches(),
            AnyDevice::Core(c) => c.switches(),
            AnyDevice::Transconductor(t) => t.switches(),
            AnyDevice::VariableResistor(r) => r.switches(),
        }
    }
}

#[cfg(test)]
mod optimization_tests {
    use super::{
        powi_small, AnyDevice, Core, Device, Pentode, ResidualStamper, Stamper, Triode, GROUND,
    };
    use crate::dsp::netlist::{CoreSpec, PentodeSpec, TriodeSpec};

    #[test]
    fn mapped_stamper_writes_compact_boundary() {
        let map = [0usize, usize::MAX, 1usize];
        let mut matrix = [0.0; 4];
        let mut rhs = [0.0; 2];
        let mut stamper = Stamper {
            matrix: &mut matrix,
            rhs: &mut rhs,
            n: 2,
            map: Some(&map),
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        stamper.conductance(0, 2, 3.0);
        stamper.current(0, 2, 0.5);
        assert_eq!(stamper.matrix, &[3.0, -3.0, -3.0, 3.0]);
        assert_eq!(stamper.rhs, &[-0.5, 0.5]);
        assert!(!stamper.mapping_failed);

        // An internal terminal is a construction bug, not something to drop
        // silently. The reduced caller will abandon this pass and use full MNA.
        stamper.current(1, 2, 0.25);
        assert!(stamper.mapping_failed);
    }

    #[test]
    fn valve_grid_limiter_preflight_is_read_only_and_exact() {
        // Put the triode at the same deep-cutoff operating point this test's
        // candidate voltages are measured from. `Triode::new()` deliberately
        // starts vgk at -1 V, so testing -29 V against a fresh device would
        // correctly request limiting (a 28 V step) and would not exercise the
        // intended below-cutoff 4 V/pass branch at all.
        let mut triode_state = Triode::new(0, 1, GROUND, TriodeSpec::ECC83);
        triode_state.vgk = -30.0;
        let triode = AnyDevice::Triode(triode_state);
        let mut triode_v = vec![0.0; 2];
        triode_v[1] = -20.0;
        assert!(triode.step_limiter_would_hold(&triode_v));
        triode_v[1] = -29.0;
        assert!(!triode.step_limiter_would_hold(&triode_v));
        let AnyDevice::Triode(triode_state) = &triode else {
            unreachable!();
        };
        assert_eq!(triode_state.vgk, -30.0);

        let pentode = AnyDevice::Pentode(Pentode::new(0, 1, GROUND, 2, 2.0, PentodeSpec::T6L6GC));
        let mut pentode_v = vec![0.0; 3];
        pentode_v[1] = -20.0;
        assert!(pentode.step_limiter_would_hold(&pentode_v));
        pentode_v[1] = -25.0;
        assert!(!pentode.step_limiter_would_hold(&pentode_v));
        let AnyDevice::Pentode(pentode_state) = &pentode else {
            unreachable!();
        };
        assert_eq!(pentode_state.vgk, -30.0);
    }

    fn stamp_device(device: &mut AnyDevice, voltage: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = voltage.len();
        let mut matrix = vec![0.0; n * n];
        let mut rhs = vec![0.0; n];
        let mut stamper = Stamper {
            matrix: &mut matrix,
            rhs: &mut rhs,
            n,
            map: None,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        device.stamp(&mut stamper, voltage);
        (matrix, rhs)
    }

    fn trial_residual_bits(device: &mut AnyDevice, voltage: &[f64], cache_eval: bool) -> Vec<u64> {
        let n = voltage.len();
        let map: Vec<usize> = (0..n).collect();
        let mut residual = vec![0.0; n];
        let mut stamper = ResidualStamper {
            residual: &mut residual,
            map: &map,
            mapping_failed: false,
            limiting: false,
            junction_held: false,
        };
        device.trial_residual(&mut stamper, voltage, cache_eval);
        assert!(!stamper.mapping_failed);
        assert!(!stamper.junction_held);
        residual.into_iter().map(f64::to_bits).collect()
    }

    fn stamp_bits(device: &mut AnyDevice, voltage: &[f64]) -> (Vec<u64>, Vec<u64>) {
        let (matrix, rhs) = stamp_device(device, voltage);
        (
            matrix.into_iter().map(f64::to_bits).collect(),
            rhs.into_iter().map(f64::to_bits).collect(),
        )
    }

    #[test]
    fn rescue_checkpoint_restores_device_history_and_cached_evaluations() {
        let core_spec = CoreSpec {
            henry: 0.8,
            knee: 0.04,
            sharpness: 6.0,
        };
        let cases = [
            (
                AnyDevice::Triode(Triode::new(0, 1, GROUND, TriodeSpec::ECC81)),
                vec![180.0, -0.7],
            ),
            (
                AnyDevice::Pentode(Pentode::new(0, 1, GROUND, 2, 2.0, PentodeSpec::T6L6GC)),
                vec![310.0, -24.0, 405.0],
            ),
            (
                AnyDevice::Core(Core::new_antialiased(0, GROUND, core_spec, 48_000.0)),
                vec![28.0],
            ),
        ];
        for (mut device, voltage) in cases {
            stamp_device(&mut device, &voltage);
            trial_residual_bits(&mut device, &voltage, true);
            let saved = device.checkpoint();
            let expected = stamp_bits(&mut device, &voltage);
            // A lazy restamp changes moved/settled history and consumes the
            // trial cache; an LM probe then evaluates a different point.
            let rejected: Vec<_> = voltage.iter().map(|v| v * 0.75).collect();
            stamp_device(&mut device, &rejected);
            trial_residual_bits(&mut device, &rejected, false);
            assert_ne!(device.checkpoint(), saved);
            device.restore_checkpoint(saved);
            assert_eq!(device.checkpoint(), saved);
            assert_eq!(stamp_bits(&mut device, &voltage), expected);
        }
    }

    #[test]
    fn accepted_trial_device_cache_is_bit_exact_for_twin_nonlinear_devices() {
        let triode_voltage = [180.0, -0.7];
        let mut triode_cached = AnyDevice::Triode(Triode::new(0, 1, GROUND, TriodeSpec::ECC81));
        let mut triode_fresh = AnyDevice::Triode(Triode::new(0, 1, GROUND, TriodeSpec::ECC81));
        let mut triode_legacy = AnyDevice::Triode(Triode::new(0, 1, GROUND, TriodeSpec::ECC81));
        assert_eq!(
            trial_residual_bits(&mut triode_cached, &triode_voltage, true),
            trial_residual_bits(&mut triode_legacy, &triode_voltage, false),
        );
        assert_eq!(
            stamp_bits(&mut triode_cached, &triode_voltage),
            stamp_bits(&mut triode_fresh, &triode_voltage),
        );

        let pentode_voltage = [310.0, -24.0, 405.0];
        let mut pentode_cached =
            AnyDevice::Pentode(Pentode::new(0, 1, GROUND, 2, 2.0, PentodeSpec::T6L6GC));
        let mut pentode_fresh =
            AnyDevice::Pentode(Pentode::new(0, 1, GROUND, 2, 2.0, PentodeSpec::T6L6GC));
        let mut pentode_legacy =
            AnyDevice::Pentode(Pentode::new(0, 1, GROUND, 2, 2.0, PentodeSpec::T6L6GC));
        assert_eq!(
            trial_residual_bits(&mut pentode_cached, &pentode_voltage, true),
            trial_residual_bits(&mut pentode_legacy, &pentode_voltage, false),
        );
        assert_eq!(
            stamp_bits(&mut pentode_cached, &pentode_voltage),
            stamp_bits(&mut pentode_fresh, &pentode_voltage),
        );

        let core_spec = CoreSpec {
            henry: 0.8,
            knee: 0.04,
            sharpness: 6.0,
        };
        let core_voltage = [28.0];
        let mut core_cached =
            AnyDevice::Core(Core::new_antialiased(0, GROUND, core_spec, 48_000.0));
        let mut core_fresh = AnyDevice::Core(Core::new_antialiased(0, GROUND, core_spec, 48_000.0));
        let mut core_legacy =
            AnyDevice::Core(Core::new_antialiased(0, GROUND, core_spec, 48_000.0));
        assert_eq!(
            trial_residual_bits(&mut core_cached, &core_voltage, true),
            trial_residual_bits(&mut core_legacy, &core_voltage, false),
        );
        assert_eq!(
            stamp_bits(&mut core_cached, &core_voltage),
            stamp_bits(&mut core_fresh, &core_voltage),
        );
    }

    #[test]
    fn antialiased_core_preserves_the_static_magnetising_curve() {
        let spec = CoreSpec {
            henry: 12.0,
            knee: 0.01,
            sharpness: 6.0,
        };
        let mut core = Core::new_antialiased(0, GROUND, spec, 48_000.0);
        for flux in [-0.02, -0.01, -0.003, 0.0, 0.003, 0.01, 0.02] {
            core.last_flux = flux;
            let antialiased = core.antialiased_magnetising(flux);
            let original = core.magnetising(flux);
            assert!(
                (antialiased - original).abs() <= 2e-13 * (1.0 + original.abs()),
                "flux={flux}, antialiased={antialiased}, original={original}"
            );
        }
    }

    #[test]
    fn antialiased_core_does_not_move_the_small_signal_inductance() {
        let spec = CoreSpec {
            henry: 12.0,
            knee: 0.01,
            sharpness: 6.0,
        };
        let core = Core::new_antialiased(0, GROUND, spec, 48_000.0);
        let flux = 1e-6;
        let antialiased = core.antialiased_magnetising(flux);
        let linear = flux / spec.henry;
        assert!(
            (antialiased - linear).abs() < 1e-24,
            "antialiased={antialiased}, linear={linear}"
        );
    }

    #[test]
    fn cached_integer_core_power_matches_powf() {
        for exponent in [3u8, 6, 7, 9] {
            for step in 0..=2000 {
                let x = step as f64 / 200.0;
                let actual = powi_small(x, exponent);
                let expected = x.powf(exponent as f64);
                assert!(
                    (actual - expected).abs() < 2e-13 * (1.0 + expected.abs()),
                    "x={x}, exponent={exponent}, actual={actual}, expected={expected}"
                );
            }
        }
    }
}
