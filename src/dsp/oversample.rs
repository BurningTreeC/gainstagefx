//! Linear phase halfband oversampling.
//!
//! The structure is Comp76Fx's by way of the first attempt at this plugin:
//! a cascade of Kaiser windowed halfband FIR stages, `M` odd-phase taps each,
//! so a 2x stage costs `M` multiplies per sample in each direction and the
//! even output samples are a plain delay of the input.
//!
//! The *lengths* are not the ones it arrived with, and the reason they moved
//! is that they were specified for a plugin with a different job. The first
//! stage was 160 taps at beta 14, which by Kaiser's rule is about 136 dB of
//! stopband attenuation and a passband flat to within a tenth of a decibel
//! right up against the input Nyquist. That is the right figure for a
//! mastering chain. It is not the right figure for a guitar amplifier, whose
//! speaker stops somewhere past four kilohertz and whose cabinet model
//! already says so -- and the cost of the extra attenuation was 180 samples
//! of round-trip latency, or 3.75 ms at 48 kHz, which is three times what
//! the plugin's latency budget allows.
//!
//! The lengths here are 56, 16 and 8 taps at betas 9, 8 and 6. That is
//! roughly 90, 80 and 63 dB of stopband, with the first stage's passband
//! flat to 20.1 kHz at 44.1 kHz and its stopband opening from 23.9 kHz.
//! Everything a speaker cone passes goes through untouched; what is above
//! that is attenuated more than any harmonic a clipper makes already is, and
//! the round trip is 66 samples at 8x, or 1.38 ms at 48 kHz.
//!
//! `M` is even for every stage, which keeps the round trip latency an integer
//! number of samples at the host rate (`M` samples for the first stage,
//! `M / 2` for the second, `M / 4` for the third).

/// Taps and Kaiser beta per stage. See the module comment for why these are
/// what they are.
const STAGES: [(usize, f64); 3] = [(56, 9.0), (16, 8.0), (8, 6.0)];

/// A delay line of a fixed whole number of samples.
struct Delay {
    buf: Vec<f64>,
    pos: usize,
}

impl Delay {
    fn copy_runtime_state_from(&mut self, source: &Self) {
        self.buf.copy_from_slice(&source.buf);
        self.pos = source.pos;
    }

    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            pos: 0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let out = self.buf[self.pos];
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        out
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
}

/// The odd phase of a halfband filter: `y[n] = sum_k c[k] * x[n - k]`.
struct OddPhase {
    coefs: Vec<f64>,
    /// Twice the history so the taps are always contiguous.
    buf: Vec<f64>,
    pos: usize,
}

impl OddPhase {
    fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.coefs, source.coefs);
        self.buf.copy_from_slice(&source.buf);
        self.pos = source.pos;
    }

    fn new(coefs: Vec<f64>) -> Self {
        let m = coefs.len();
        Self {
            coefs,
            buf: vec![0.0; 2 * m],
            pos: 0,
        }
    }

    /// Dot the taps with the history *without* including `x[n]` yet.
    #[inline]
    fn dot(&self) -> f64 {
        let m = self.coefs.len();
        let window = &self.buf[self.pos..self.pos + m];
        let mut acc = 0.0;
        for (c, x) in self.coefs.iter().zip(window.iter()) {
            acc += c * x;
        }
        acc
    }

    #[inline]
    fn push(&mut self, x: f64) {
        let m = self.coefs.len();
        self.pos = if self.pos == 0 { m - 1 } else { self.pos - 1 };
        self.buf[self.pos] = x;
        self.buf[self.pos + m] = x;
    }

    #[inline]
    fn push_dot(&mut self, x: f64) -> f64 {
        self.push(x);
        self.dot()
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
}

/// The interpolating half of one 2x stage.
struct UpStage {
    fir: OddPhase,
    delay: Delay,
}

/// The decimating half of one 2x stage.
///
/// Up and down are separate state machines: the interpolator's history is its
/// inputs and the decimator's is the processed samples coming back. Keeping
/// them in separate vectors lets `Oversampler::split` hand them to two threads
/// (see `StageWorker`) without either touching the other's state.
struct DownStage {
    fir: OddPhase,
    delay: Delay,
}

impl UpStage {
    fn new(odd: &[f64], m: usize) -> Self {
        // The interpolator carries a factor of two to make up for the energy
        // lost to zero stuffing.
        Self {
            fir: OddPhase::new(odd.iter().map(|c| c * 2.0).collect()),
            delay: Delay::new(m / 2),
        }
    }

    /// One input sample in, an even/odd pair at twice the rate out.
    #[inline]
    fn up(&mut self, x: f64) -> (f64, f64) {
        (self.delay.process(x), self.fir.push_dot(x))
    }

    fn reset(&mut self) {
        self.fir.reset();
        self.delay.reset();
    }
}

impl DownStage {
    fn new(odd: Vec<f64>, m: usize) -> Self {
        Self {
            fir: OddPhase::new(odd),
            delay: Delay::new(m / 2),
        }
    }

    /// An even/odd pair at twice the rate in, one sample out.
    #[inline]
    fn down(&mut self, even: f64, odd: f64) -> f64 {
        // The odd branch needs `o[n - 1 - k]`, so read the history before the
        // new sample goes in.
        let y = 0.5 * self.delay.process(even) + self.fir.dot();
        self.fir.push(odd);
        y
    }

    fn reset(&mut self) {
        self.fir.reset();
        self.delay.reset();
    }
}

/// A cascade of 2x stages giving an oversampling factor of 1, 2, 4 or 8.
///
/// Every stage is built up front and the factor selects how many of them are
/// used, so changing the setting while playing cannot allocate.
pub struct Oversampler {
    ups: Vec<UpStage>,
    downs: Vec<DownStage>,
    active: usize,
}

/// The interpolating half of an `Oversampler`, borrowed on its own.
pub struct Upsampler<'a> {
    stages: &'a mut [UpStage],
}

/// The decimating half of an `Oversampler`, borrowed on its own.
pub struct Downsampler<'a> {
    stages: &'a mut [DownStage],
}

impl Upsampler<'_> {
    /// Oversampled samples per host sample.
    pub fn factor(&self) -> usize {
        1 << self.stages.len()
    }

    /// The oversampled rate's samples for one host sample, in the order
    /// `Oversampler::process` would hand them to its closure. Returns how
    /// many were written (the factor).
    #[inline]
    pub fn push(&mut self, x: f64, out: &mut [f64]) -> usize {
        let mut at = 0;
        Self::run(self.stages, x, out, &mut at);
        at
    }

    fn run(stages: &mut [UpStage], x: f64, out: &mut [f64], at: &mut usize) {
        match stages.split_first_mut() {
            None => {
                out[*at] = x;
                *at += 1;
            }
            Some((stage, rest)) => {
                let (even, odd) = stage.up(x);
                Self::run(rest, even, out, at);
                Self::run(rest, odd, out, at);
            }
        }
    }
}

impl Downsampler<'_> {
    /// One host sample from the processed oversampled ones `Upsampler::push`
    /// produced, taken in the same order.
    #[inline]
    pub fn pull(&mut self, processed: &[f64]) -> f64 {
        let mut at = 0;
        Self::run(self.stages, processed, &mut at)
    }

    fn run(stages: &mut [DownStage], processed: &[f64], at: &mut usize) -> f64 {
        match stages.split_first_mut() {
            None => {
                let v = processed[*at];
                *at += 1;
                v
            }
            Some((stage, rest)) => {
                let even = Self::run(rest, processed, at);
                let odd = Self::run(rest, processed, at);
                stage.down(even, odd)
            }
        }
    }
}

impl Oversampler {
    /// Resume the same configured stream, including every FIR/delay history.
    /// All stages already exist; installing the active stage count here must
    /// not call `set_factor`, which would clear the history we are preserving.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.ups.len(), source.ups.len());
        for (dst, src) in self.ups.iter_mut().zip(&source.ups) {
            dst.fir.copy_runtime_state_from(&src.fir);
            dst.delay.copy_runtime_state_from(&src.delay);
        }
        for (dst, src) in self.downs.iter_mut().zip(&source.downs) {
            dst.fir.copy_runtime_state_from(&src.fir);
            dst.delay.copy_runtime_state_from(&src.delay);
        }
        self.active = source.active;
    }

    /// `factor` is rounded down to the nearest supported power of two.
    pub fn new(factor: usize) -> Self {
        let odd: Vec<Vec<f64>> = STAGES
            .iter()
            .map(|&(m, beta)| halfband_odd_taps(m, beta))
            .collect();
        let mut oversampler = Self {
            ups: odd
                .iter()
                .zip(STAGES)
                .map(|(taps, (m, _))| UpStage::new(taps, m))
                .collect(),
            downs: odd
                .into_iter()
                .zip(STAGES)
                .map(|(taps, (m, _))| DownStage::new(taps, m))
                .collect(),
            active: 0,
        };
        oversampler.set_factor(factor);
        oversampler
    }

    pub fn set_factor(&mut self, factor: usize) {
        let active = Self::active_stages(factor);
        if active != self.active {
            self.active = active;
            self.reset();
        }
    }

    pub fn factor(&self) -> usize {
        1 << self.active
    }

    /// Round trip latency in samples at the host rate.
    pub fn latency(&self) -> u32 {
        Self::stages_latency(self.active)
    }

    /// What `latency` will be once `set_factor(factor)` is in effect.
    pub fn latency_of(factor: usize) -> u32 {
        Self::stages_latency(Self::active_stages(factor))
    }

    fn stages_latency(active: usize) -> u32 {
        STAGES
            .iter()
            .take(active)
            .enumerate()
            .map(|(i, &(m, _))| (m >> i) as u32)
            .sum()
    }

    fn active_stages(factor: usize) -> usize {
        match factor {
            0..=1 => 0,
            2..=3 => 1,
            4..=7 => 2,
            _ => 3,
        }
    }

    /// The two halves, borrowed separately. Pushing every host sample through
    /// the upsampler, processing what it writes, and pulling it back through
    /// the downsampler is `process`, operation for operation.
    pub fn split(&mut self) -> (Upsampler<'_>, Downsampler<'_>) {
        (
            Upsampler {
                stages: &mut self.ups[..self.active],
            },
            Downsampler {
                stages: &mut self.downs[..self.active],
            },
        )
    }

    /// Run `f` at the oversampled rate for one host rate sample.
    #[inline]
    pub fn process<F>(&mut self, x: f64, f: &mut F) -> f64
    where
        F: FnMut(f64) -> f64,
    {
        Self::run(
            &mut self.ups[..self.active],
            &mut self.downs[..self.active],
            x,
            f,
        )
    }

    #[inline]
    fn run<F>(ups: &mut [UpStage], downs: &mut [DownStage], x: f64, f: &mut F) -> f64
    where
        F: FnMut(f64) -> f64,
    {
        match (ups.split_first_mut(), downs.split_first_mut()) {
            (Some((up, ups)), Some((down, downs))) => {
                let (even, odd) = up.up(x);
                let even = Self::run(ups, downs, even, f);
                let odd = Self::run(ups, downs, odd, f);
                down.down(even, odd)
            }
            _ => f(x),
        }
    }

    pub fn reset(&mut self) {
        self.ups.iter_mut().for_each(UpStage::reset);
        self.downs.iter_mut().for_each(DownStage::reset);
    }
}

/// Odd indexed taps of a Kaiser windowed halfband lowpass of length `2M + 1`.
fn halfband_odd_taps(m: usize, beta: f64) -> Vec<f64> {
    let n = 2 * m + 1;
    let denom = bessel_i0(beta);
    let mut odd = Vec::with_capacity(m);
    let mut sum = 0.5; // the centre tap, which is not part of the odd phase
    for k in 0..m {
        let j = 2 * k + 1;
        let t = (j as f64 - m as f64) / 2.0;
        // 0.5 * sinc(t), the ideal halfband response.
        let ideal = 0.5 * sinc(t);
        let r = 2.0 * j as f64 / (n - 1) as f64 - 1.0;
        let w = bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / denom;
        let tap = ideal * w;
        sum += tap;
        odd.push(tap);
    }
    // Normalise to exactly unity gain at DC.
    let scale = 0.5 / (sum - 0.5);
    odd.iter_mut().for_each(|c| *c *= scale);
    odd
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        px.sin() / px
    }
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let half = x / 2.0;
    for k in 1..40 {
        term *= (half / k as f64) * (half / k as f64);
        sum += term;
        if term < 1e-18 * sum {
            break;
        }
    }
    sum
}
