//! Rayleigh surface integral for a circular piston in a rigid baffle.
//!
//! Surface rings centred below the receiver turn dS/r into dphi*dr. Integrating
//! the visible arc of each ring gives a compact causal pressure impulse response,
//! including near-field interference and the signed far-field Bessel lobes.
//! The second kernel is the reactive 1/r² velocity term of the pressure gradient.
//! No distance make-up or near/far switching is applied.

use super::filters::{DelayLine, DELAY_LEN};
use super::speaker::{RHO, SPEED_OF_SOUND};
use std::f64::consts::{PI, TAU};

const RAMP: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tap {
    pressure: f64,
    velocity: f64,
    target: [f64; 2],
    step: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct PressureField {
    taps: Box<[Tap]>,
    first: usize,
    end: usize,
    ramp: usize,
}

impl Default for PressureField {
    fn default() -> Self {
        Self {
            taps: vec![Tap::default(); DELAY_LEN].into_boxed_slice(),
            first: 0,
            end: 0,
            ramp: 0,
        }
    }
}

impl PressureField {
    /// A surface element carrying a fraction of a radiator's volume
    /// acceleration, m³/s². Used to integrate the perforated front panel.
    #[allow(clippy::too_many_arguments)]
    pub fn add_volume_element(
        &mut self,
        rate: f64,
        receiver: [f64; 3],
        axis: [f64; 3],
        pattern: (f64, f64),
        proximity: f64,
        reference: f64,
        fraction: f64,
    ) {
        let distance = receiver.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-5);
        let cosine = -receiver.iter().zip(axis).map(|(x, a)| x * a).sum::<f64>() / distance;
        let scale = RHO / TAU * fraction / distance;
        self.deposit(
            (distance - reference).max(0.0) / SPEED_OF_SOUND * rate,
            scale * (pattern.0 + pattern.1 * cosine),
            scale * pattern.1 * cosine * proximity * SPEED_OF_SOUND / distance,
        );
    }
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        self.taps.copy_from_slice(&source.taps);
        self.first = source.first;
        self.end = source.end;
        self.ramp = source.ramp;
    }
    pub fn begin(&mut self) {
        for tap in self.taps.iter_mut() {
            tap.target = [0.0; 2];
        }
    }

    /// Nearest point of the disc, in metres. `receiver` is relative to its centre.
    pub fn first_arrival(radius: f64, receiver: [f64; 3]) -> f64 {
        let outside = (receiver[0].hypot(receiver[1]) - radius).max(0.0);
        receiver[2].hypot(outside)
    }

    /// Add a coherently driven cone. Output is Pa for acceleration in m/s² and
    /// velocity in m/s. `reference` only removes a common propagation time;
    /// it does not change geometric attenuation or relative arrival times.
    #[allow(clippy::too_many_arguments)]
    pub fn add_piston(
        &mut self,
        rate: f64,
        radius: f64,
        receiver: [f64; 3],
        axis: [f64; 3],
        pattern: (f64, f64),
        proximity: f64,
        reference: f64,
    ) {
        let b = receiver[0].hypot(receiver[1]);
        let z = receiver[2].abs().max(1e-5);
        let near = Self::first_arrival(radius, receiver);
        let far = z.hypot(b + radius);
        let dr = SPEED_OF_SOUND / rate;
        // Integrate within each delay sample, then deposit the fractional time
        // with fifth-order Lagrange weights. Eight midpoint samples also resolve the
        // square-root arc endpoints without any surface mesh aliasing.
        let pieces = ((far - near) / dr * 8.0).ceil().max(8.0) as usize;
        let delta = (far - near) / pieces as f64;
        let lateral_axis = if b > 1e-12 {
            (receiver[0] * axis[0] + receiver[1] * axis[1]) / b
        } else {
            0.0
        };
        for i in 0..pieces {
            let r = near + (i as f64 + 0.5) * delta;
            let q = (r * r - z * z).max(0.0).sqrt();
            let theta = if b < 1e-12 || b + q <= radius {
                PI
            } else {
                ((b * b + q * q - radius * radius) / (2.0 * b * q))
                    .clamp(-1.0, 1.0)
                    .acos()
            };
            let average_cos = if theta < 1e-12 {
                1.0
            } else {
                theta.sin() / theta
            };
            let incidence = (-q * average_cos * lateral_axis - z * axis[2]) / r;
            let area_over_r = 2.0 * theta * delta;
            let pressure = RHO / TAU * area_over_r * (pattern.0 + pattern.1 * incidence);
            let velocity =
                RHO / TAU * area_over_r * pattern.1 * incidence * proximity * SPEED_OF_SOUND / r;
            self.deposit((r - reference).max(0.0) / dr, pressure, velocity);
        }
    }

    fn deposit(&mut self, delay: f64, pressure: f64, velocity: f64) {
        let i = delay.floor() as usize;
        let start = i.saturating_sub(2);
        let u = delay - start as f64;
        let weights: [f64; 6] = std::array::from_fn(|j| {
            (0..6)
                .filter(|&m| m != j)
                .map(|m| (u - m as f64) / (j as f64 - m as f64))
                .product()
        });
        assert!(
            start + 6 <= self.taps.len(),
            "radiation delay exceeds preallocated history"
        );
        for (tap, weight) in self.taps[start..start + 6].iter_mut().zip(weights) {
            tap.target[0] += pressure * weight;
            tap.target[1] += velocity * weight;
        }
    }

    pub fn commit(&mut self, snap: bool) {
        self.first = DELAY_LEN;
        self.end = 0;
        for (i, tap) in self.taps.iter_mut().enumerate() {
            if snap {
                tap.pressure = tap.target[0];
                tap.velocity = tap.target[1];
            }
            tap.step = [
                (tap.target[0] - tap.pressure) / RAMP as f64,
                (tap.target[1] - tap.velocity) / RAMP as f64,
            ];
            if tap.pressure != 0.0 || tap.velocity != 0.0 || tap.target != [0.0; 2] {
                self.first = self.first.min(i);
                self.end = i + 1;
            }
        }
        self.first = self.first.min(self.end);
        self.ramp = if snap { 0 } else { RAMP };
    }

    pub fn process(&mut self, acceleration: &DelayLine, velocity: &DelayLine) -> f64 {
        let taps = &mut self.taps[self.first..self.end];
        // The ramp is the same for every tap, so move them all first and keep
        // the branch out of the sum.
        if self.ramp == 1 {
            for tap in taps.iter_mut() {
                tap.pressure = tap.target[0];
                tap.velocity = tap.target[1];
            }
        } else if self.ramp > 1 {
            for tap in taps.iter_mut() {
                tap.pressure += tap.step[0];
                tap.velocity += tap.step[1];
            }
        }
        let mut pressure = 0.0;
        for (offset, tap) in taps.iter().enumerate() {
            let delay = self.first + offset;
            pressure += tap.pressure * acceleration.read_integer(delay)
                + tap.velocity * velocity.read_integer(delay);
        }
        self.ramp = self.ramp.saturating_sub(1);
        pressure
    }
}
