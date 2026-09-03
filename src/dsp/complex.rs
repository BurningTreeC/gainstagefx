//! Just enough complex arithmetic for an AC solve.

use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const ZERO: C = C { re: 0.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub const fn real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }

    pub fn magnitude(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// In dB, with a floor so silence does not become minus infinity.
    pub fn db(self) -> f64 {
        20.0 * (self.magnitude() + 1e-30).log10()
    }

    /// In degrees.
    pub fn phase(self) -> f64 {
        self.im.atan2(self.re).to_degrees()
    }

    pub fn recip(self) -> C {
        let d = self.re * self.re + self.im * self.im;
        C::new(self.re / d, -self.im / d)
    }
}

impl Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

impl Div for C {
    type Output = C;
    #[allow(clippy::suspicious_arithmetic_impl)] // dividing *is* multiplying by the reciprocal
    fn div(self, o: C) -> C {
        self * o.recip()
    }
}

impl Neg for C {
    type Output = C;
    fn neg(self) -> C {
        C::new(-self.re, -self.im)
    }
}
