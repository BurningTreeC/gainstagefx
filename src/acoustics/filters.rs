//! The small linear building blocks the acoustic stages are made of.
//!
//! Every response here was *fitted* as an analog prototype (see `tools/speaker_fit`),
//! so each digital section is designed to match its prototype's magnitude, not merely
//! warped from it. The poles are placed by impulse invariance, then the numerator is
//! solved so the digital magnitude equals the analog one at DC, at the corner and at
//! Nyquist ("matched" second-order design, after M. Vicanek, *Matched Second Order
//! Digital Filters*, 2016). The bilinear transform instead cramps every low-pass towards
//! Nyquist: a 5.1 kHz speaker roll-off came out 3.8 dB low at 12 kHz at 44.1 kHz, and the
//! acoustic chain then sounded different at every sample rate.

use std::f64::consts::PI;

/// Poles above this fraction of the rate are pulled down to it. Only prototypes that
/// barely act in the audible band (a ribbon's 40 kHz low-pass) are affected, and their
/// magnitude is still matched at three frequencies.
const POLE_LIMIT: f64 = 0.45;
/// Corners below this fraction of the rate use the bilinear transform instead.
const BILINEAR_BELOW: f64 = 0.05;

/// `sin^2(w/2)` terms of the magnitude-squared parameterisation.
fn phis(rate: f64, hz: f64) -> (f64, f64, f64) {
    let s = (PI * hz / rate).sin().powi(2);
    (1.0 - s, s, 4.0 * (1.0 - s) * s)
}

/// A second-order section, transposed direct form II, with its state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Default for Biquad {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Biquad {
    pub const IDENTITY: Biquad = Biquad {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
        z1: 0.0,
        z2: 0.0,
    };

    /// `(n2 s^2 + n1 s + n0) / (s^2 + d1 s + 1)` in normalised `s = j f / fc`, matched
    /// at DC, the corner and Nyquist. State is kept.
    fn set_matched(&mut self, rate: f64, fc: f64, num: [f64; 3], d1: f64) {
        let fc = fc.max(1.0);
        if fc < BILINEAR_BELOW * rate {
            return self.set_bilinear(rate, fc, num, d1);
        }
        let magnitude2 = |hz: f64| {
            let x = hz / fc;
            let n = (num[2] - num[0] * x * x).powi(2) + (num[1] * x).powi(2);
            let d = (1.0 - x * x).powi(2) + (d1 * x).powi(2);
            n / d
        };
        let pole = fc.min(POLE_LIMIT * rate);
        let wt = 2.0 * PI * pole / rate;
        let half = d1 / 2.0;
        let disc = half * half - 1.0;
        let (a1, a2) = if disc < 0.0 {
            let (re, im) = (-half * wt, (-disc).sqrt() * wt);
            (-2.0 * re.exp() * im.cos(), (2.0 * re).exp())
        } else {
            let root = disc.sqrt();
            let (r1, r2) = ((-half + root) * wt, (-half - root) * wt);
            (-(r1.exp() + r2.exp()), (r1 + r2).exp())
        };
        let big_a0 = (1.0 + a1 + a2).powi(2);
        let big_a1 = (1.0 - a1 + a2).powi(2);
        let big_a2 = -4.0 * a2;
        let b0_sq = magnitude2(0.0) * big_a0;
        let b1_sq = magnitude2(rate / 2.0) * big_a1;
        let at = fc.min(0.3 * rate);
        let (p0, p1, p2) = phis(rate, at);
        let target = magnitude2(at) * (big_a0 * p0 + big_a1 * p1 + big_a2 * p2);
        let big_b2 = (target - b0_sq * p0 - b1_sq * p1) / p2;
        let (r0, r1) = (b0_sq.max(0.0).sqrt(), b1_sq.max(0.0).sqrt());
        let w = 0.5 * (r0 + r1);
        let b0 = 0.5 * (w + (w * w + big_b2).max(0.0).sqrt());
        self.b0 = b0;
        self.b1 = 0.5 * (r0 - r1);
        self.b2 = if b0 > 0.0 { -big_b2 / (4.0 * b0) } else { 0.0 };
        self.a1 = a1;
        self.a2 = a2;
    }

    /// Bilinear with prewarp at the corner. Exact for corners far below Nyquist, and
    /// numerically better conditioned there than the matched solve.
    fn set_bilinear(&mut self, rate: f64, fc: f64, num: [f64; 3], d1: f64) {
        let k = 1.0 / (PI * fc / rate).tan();
        let k2 = k * k;
        let (n2, n1, n0) = (num[0] * k2, num[1] * k, num[2]);
        let (e2, e1, e0) = (k2, d1 * k, 1.0);
        let a0 = e2 + e1 + e0;
        self.b0 = (n2 + n1 + n0) / a0;
        self.b1 = 2.0 * (n0 - n2) / a0;
        self.b2 = (n2 - n1 + n0) / a0;
        self.a1 = 2.0 * (e0 - e2) / a0;
        self.a2 = (e2 - e1 + e0) / a0;
    }

    pub fn peaking(rate: f64, fc: f64, gain_db: f64, q: f64) -> Self {
        let mut b = Self::IDENTITY;
        b.set_peaking(rate, fc, gain_db, q);
        b
    }

    pub fn set_peaking(&mut self, rate: f64, fc: f64, gain_db: f64, q: f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let q = q.max(0.05);
        self.set_matched(rate, fc, [1.0, a / q, 1.0], 1.0 / (a * q));
    }

    pub fn lowpass(rate: f64, fc: f64, q: f64) -> Self {
        let mut b = Self::IDENTITY;
        b.set_matched(rate, fc, [0.0, 0.0, 1.0], 1.0 / q.max(0.05));
        b
    }

    pub fn highpass(rate: f64, fc: f64, q: f64) -> Self {
        let mut b = Self::IDENTITY;
        b.set_matched(rate, fc, [1.0, 0.0, 0.0], 1.0 / q.max(0.05));
        b
    }

    /// First-order low shelf: `gain_db` below `fc`, unity above; `(s + g) / (s + 1)`.
    pub fn set_low_shelf(&mut self, rate: f64, fc: f64, gain_db: f64) {
        let g = 10f64.powf(gain_db / 20.0);
        let fc = fc.max(1.0);
        let x = rate / 2.0 / fc;
        let (b0, b1, a1) = first_order_matched(rate, fc, g * g, (x * x + g * g) / (x * x + 1.0));
        self.b0 = b0;
        self.b1 = b1;
        self.b2 = 0.0;
        self.a1 = a1;
        self.a2 = 0.0;
    }

    /// Complex response at `hz`, for tests and normalisation.
    pub fn response(&self, rate: f64, hz: f64) -> (f64, f64) {
        let w = 2.0 * PI * hz / rate;
        let (c1, s1) = (w.cos(), -w.sin());
        let (c2, s2) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let num = (self.b0 + self.b1 * c1 + self.b2 * c2, self.b1 * s1 + self.b2 * s2);
        let den = (1.0 + self.a1 * c1 + self.a2 * c2, self.a1 * s1 + self.a2 * s2);
        let d = den.0 * den.0 + den.1 * den.1;
        (
            (num.0 * den.0 + num.1 * den.1) / d,
            (num.1 * den.0 - num.0 * den.1) / d,
        )
    }

    pub fn magnitude(&self, rate: f64, hz: f64) -> f64 {
        let (re, im) = self.response(rate, hz);
        re.hypot(im)
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// A one-pole low-pass, matched at its corner. `open()` is a wire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnePole {
    b0: f64,
    b1: f64,
    a1: f64,
    x1: f64,
    y1: f64,
}

impl Default for OnePole {
    fn default() -> Self {
        Self::open()
    }
}

impl OnePole {
    pub const fn open() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            a1: 0.0,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// `1 / (s + 1)`, matched at DC and Nyquist. An infinite corner is a wire.
    pub fn set_lowpass(&mut self, rate: f64, fc: f64) {
        if !fc.is_finite() {
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.a1 = 0.0;
            return;
        }
        let fc = fc.max(1.0);
        let x = rate / 2.0 / fc;
        let (b0, b1, a1) = first_order_matched(rate, fc, 1.0, 1.0 / (1.0 + x * x));
        self.b0 = b0;
        self.b1 = b1;
        self.a1 = a1;
    }

    /// `w / (s + w_leak)`-style leaky integrator: gain `peak` at DC, falling at
    /// 6 dB/octave above `corner`, unity crossing near `peak * corner`.
    pub fn set_leaky_integrator(&mut self, rate: f64, corner: f64, peak: f64) {
        self.set_lowpass(rate, corner);
        self.b0 *= peak;
        self.b1 *= peak;
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    pub fn magnitude(&self, rate: f64, hz: f64) -> f64 {
        let w = 2.0 * PI * hz / rate;
        let (c, s) = (w.cos(), -w.sin());
        let num = (self.b0 + self.b1 * c, self.b1 * s);
        let den = (1.0 + self.a1 * c, self.a1 * s);
        num.0.hypot(num.1) / den.0.hypot(den.1)
    }
}

/// A first-order section with its pole at `fc` by impulse invariance, and a numerator
/// matching the analog magnitude-squared at DC (`dc2`) and Nyquist (`nyquist2`).
fn first_order_matched(rate: f64, fc: f64, dc2: f64, nyquist2: f64) -> (f64, f64, f64) {
    let a1 = -(-2.0 * PI * fc / rate).exp();
    let r0 = (dc2 * (1.0 + a1).powi(2)).sqrt();
    let r1 = (nyquist2 * (1.0 - a1).powi(2)).sqrt();
    (0.5 * (r0 + r1), 0.5 * (r0 - r1), a1)
}

/// Length of the shared propagation delay line. At 192 kHz this holds 10.6 ms: every
/// relative path difference a 1 m microphone range and the largest cabinet can make.
pub const DELAY_LEN: usize = 2048;

/// One circular buffer, read at any number of fractional delays.
#[derive(Clone, Debug, PartialEq)]
pub struct DelayLine {
    buf: [f64; DELAY_LEN],
    pos: usize,
}

impl DelayLine {
    pub fn new() -> Self {
        Self {
            buf: [0.0; DELAY_LEN],
            pos: 0,
        }
    }

    #[inline]
    pub fn write(&mut self, x: f64) {
        self.pos = (self.pos + 1) & (DELAY_LEN - 1);
        self.buf[self.pos] = x;
    }

    #[inline]
    fn at(&self, back: usize) -> f64 {
        self.buf[(self.pos + DELAY_LEN - (back & (DELAY_LEN - 1))) & (DELAY_LEN - 1)]
    }

    /// The sample written `delay` samples ago, by third-order Lagrange interpolation.
    /// Delays below one sample use the causal second-order form, so a path with no
    /// relative delay reads the newest sample exactly and adds no latency.
    #[inline]
    pub fn read(&self, delay: f64) -> f64 {
        let delay = delay.clamp(0.0, (DELAY_LEN - 4) as f64);
        let i = delay as usize;
        let f = delay - i as f64;
        if i == 0 {
            let (y0, y1, y2) = (self.at(0), self.at(1), self.at(2));
            return y0 * (f - 1.0) * (f - 2.0) * 0.5 - y1 * f * (f - 2.0) + y2 * f * (f - 1.0) * 0.5;
        }
        // Samples at i-1, i, i+1, i+2 back; interpolate at i + f, i.e. u = f + 1.
        let (ym1, y0, y1, y2) = (self.at(i - 1), self.at(i), self.at(i + 1), self.at(i + 2));
        let u = f + 1.0;
        let l0 = -(u - 1.0) * (u - 2.0) * (u - 3.0) / 6.0;
        let l1 = u * (u - 2.0) * (u - 3.0) / 2.0;
        let l2 = -u * (u - 1.0) * (u - 3.0) / 2.0;
        let l3 = u * (u - 1.0) * (u - 2.0) / 6.0;
        ym1 * l0 + y0 * l1 + y1 * l2 + y2 * l3
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

impl Default for DelayLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Analog second-order responses in dB, for tests to compare against.
pub mod analog {
    fn norm(f: f64, fc: f64) -> (f64, f64) {
        let x = f / fc;
        (x, x * x)
    }

    pub fn peaking_db(f: f64, fc: f64, gain_db: f64, q: f64) -> f64 {
        let a = 10f64.powf(gain_db / 40.0);
        let (x, x2) = norm(f, fc);
        let num = ((1.0 - x2).powi(2) + (x * a / q).powi(2)).sqrt();
        let den = ((1.0 - x2).powi(2) + (x / (a * q)).powi(2)).sqrt();
        20.0 * (num / den).log10()
    }

    pub fn lowpass_db(f: f64, fc: f64, q: f64) -> f64 {
        let (x, x2) = norm(f, fc);
        -10.0 * ((1.0 - x2).powi(2) + (x / q).powi(2)).log10()
    }

    pub fn highpass_db(f: f64, fc: f64, q: f64) -> f64 {
        let (x, x2) = norm(f, fc);
        20.0 * x2.log10() - 10.0 * ((1.0 - x2).powi(2) + (x / q).powi(2)).log10()
    }
}
