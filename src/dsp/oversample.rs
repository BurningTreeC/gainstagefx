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

/// One 2x up/down conversion stage.
struct Stage {
    up_fir: OddPhase,
    up_delay: Delay,
    down_fir: OddPhase,
    down_delay: Delay,
}

impl Stage {
    fn new(m: usize, beta: f64) -> Self {
        let odd = halfband_odd_taps(m, beta);
        // The interpolator carries a factor of two to make up for the energy
        // lost to zero stuffing.
        let up_taps: Vec<f64> = odd.iter().map(|c| c * 2.0).collect();
        Self {
            up_fir: OddPhase::new(up_taps),
            up_delay: Delay::new(m / 2),
            down_fir: OddPhase::new(odd),
            down_delay: Delay::new(m / 2),
        }
    }

    /// One input sample in, an even/odd pair at twice the rate out.
    #[inline]
    fn up(&mut self, x: f64) -> (f64, f64) {
        (self.up_delay.process(x), self.up_fir.push_dot(x))
    }

    /// An even/odd pair at twice the rate in, one sample out.
    #[inline]
    fn down(&mut self, even: f64, odd: f64) -> f64 {
        // The odd branch needs `o[n - 1 - k]`, so read the history before the
        // new sample goes in.
        let y = 0.5 * self.down_delay.process(even) + self.down_fir.dot();
        self.down_fir.push(odd);
        y
    }

    fn reset(&mut self) {
        self.up_fir.reset();
        self.up_delay.reset();
        self.down_fir.reset();
        self.down_delay.reset();
    }
}

/// A cascade of 2x stages giving an oversampling factor of 1, 2, 4 or 8.
///
/// Every stage is built up front and the factor selects how many of them are
/// used, so changing the setting while playing cannot allocate.
pub struct Oversampler {
    stages: Vec<Stage>,
    active: usize,
}

impl Oversampler {
    /// `factor` is rounded down to the nearest supported power of two.
    pub fn new(factor: usize) -> Self {
        let mut oversampler = Self {
            stages: STAGES
                .iter()
                .map(|&(m, beta)| Stage::new(m, beta))
                .collect(),
            active: 0,
        };
        oversampler.set_factor(factor);
        oversampler
    }

    pub fn set_factor(&mut self, factor: usize) {
        let active = match factor {
            0..=1 => 0,
            2..=3 => 1,
            4..=7 => 2,
            _ => 3,
        };
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
        STAGES
            .iter()
            .take(self.active)
            .enumerate()
            .map(|(i, &(m, _))| (m >> i) as u32)
            .sum()
    }

    /// Run `f` at the oversampled rate for one host rate sample.
    #[inline]
    pub fn process<F>(&mut self, x: f64, f: &mut F) -> f64
    where
        F: FnMut(f64) -> f64,
    {
        Self::run(&mut self.stages[..self.active], x, f)
    }

    #[inline]
    fn run<F>(stages: &mut [Stage], x: f64, f: &mut F) -> f64
    where
        F: FnMut(f64) -> f64,
    {
        match stages.split_first_mut() {
            None => f(x),
            Some((stage, rest)) => {
                let (even, odd) = stage.up(x);
                let even = Self::run(rest, even, f);
                let odd = Self::run(rest, odd, f);
                stage.down(even, odd)
            }
        }
    }

    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(Stage::reset);
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