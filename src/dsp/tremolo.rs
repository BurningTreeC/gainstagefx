//! The AB763 tremolo: an oscillator, a neon bulb and a light-dependent
//! resistor that shunts the signal.
//!
//! Fender called it vibrato and it is not -- vibrato moves pitch, this moves
//! level -- but the panel says Vibrato and so does the drawing.
//!
//! ## Why it is not a sine wave on the output
//!
//! The oscillator is a three-stage phase-shift network round a 12AX7 and its
//! output is roughly sinusoidal. What that drives is a **neon bulb**, which
//! does nothing at all until the voltage across it reaches its striking point
//! and then conducts, and which stops conducting at a *lower* voltage than it
//! started at. So the light is a pulse, not a sine, and its width depends on
//! how hard the oscillator drives it -- which is what the Intensity control
//! sets.
//!
//! That light falls on a **light-dependent resistor**, whose resistance drops
//! quickly when lit and recovers slowly when dark. Measured on real cells the
//! asymmetry is large: milliseconds down, tens to hundreds of milliseconds
//! back. It is the reason an AB763 tremolo has its particular lopsided throb
//! and does not sound like a level control being waggled.
//!
//! And the cell **shunts the signal** at a node in the circuit. Its effect
//! therefore depends on the impedances either side of it, which is why the
//! same optical assembly sounds different in different amplifiers, and why
//! `CLAUDE.md` §42.6 asks for the modulation at the circuit's own control
//! point rather than as an amplitude tremolo on the output.
//!
//! ## What is modelled and what is not
//!
//! The oscillator's *waveform* is taken as sinusoidal rather than solved as a
//! circuit. Solving it would be a second nonlinear system running at audio
//! rate to produce a control signal that moves at ten hertz, and the
//! amplifiers it would sit behind already miss their deadline. The bulb, the
//! cell's asymmetric lag and the shunt are all modelled, because those are
//! what shape it. Marked as the approximation it is.

/// Where the speed control's ends sit. The 3 M reverse-audio pot against the
/// oscillator's network gives roughly this on an AB763.
const SLOWEST_HZ: f64 = 1.8;
const FASTEST_HZ: f64 = 11.0;

/// The cell in the dark and in full light. An NSL-32 style photoresistor of
/// the kind these assemblies use runs to megohms dark and to a few hundred
/// ohms lit; these are estimates of that part, not measurements of one.
const DARK_OHMS: f64 = 5_000_000.0;
#[cfg(test)]
const LIT_OHMS: f64 = 12_000.0;
/// `ln(LIT_OHMS / DARK_OHMS)`. Keeping the logarithm constant turns the old
/// generic `powf` into one `exp` without changing the logarithmic interpolation
/// law of the cell.
const LOG_CELL_RATIO: f64 = -6.032_286_541_628_237;

/// How fast the cell responds. Down quickly, back slowly, and the asymmetry
/// is the point.
const FALL_SECONDS: f64 = 0.006;
const RISE_SECONDS: f64 = 0.090;

/// Recursive oscillator normalization interval. The rotation itself is only
/// four multiplies and two adds per sample; the occasional normalization keeps
/// round-off from changing its amplitude over a long session.
const NORMALIZE_EVERY: u32 = 4096;

pub struct Tremolo {
    rate: f64,
    /// Last speed control value whose oscillator increment is cached.
    speed: f64,
    /// Sin/cos of the oscillator phase. Advancing these with a fixed rotation
    /// is algebraically the same sine oscillator as evaluating `sin(phase)`
    /// every sample, but it avoids a libm call at audio rate.
    oscillator_sin: f64,
    oscillator_cos: f64,
    step_sin: f64,
    step_cos: f64,
    since_normalize: u32,
    /// How lit the cell is, 0 to 1, after its own lag.
    lit: f64,
    /// Whether the bulb is currently struck. Neon has hysteresis: it fires at
    /// one voltage and goes out at a lower one, which is what turns a sine
    /// into a pulse.
    struck: bool,
    fall: f64,
    rise: f64,
}

impl Tremolo {
    /// Copy oscillator phase, cached increment, bulb hysteresis and the cell's
    /// illumination. The two oscillators must already have the same sample
    /// rate.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.rate, source.rate);
        self.speed = source.speed;
        self.oscillator_sin = source.oscillator_sin;
        self.oscillator_cos = source.oscillator_cos;
        self.step_sin = source.step_sin;
        self.step_cos = source.step_cos;
        self.since_normalize = source.since_normalize;
        self.lit = source.lit;
        self.struck = source.struck;
    }

    pub fn new(rate: f64) -> Self {
        let mut this = Self {
            rate,
            // Outside the legal 0..=1 control range so the first call caches
            // the requested speed without a separate constructor path.
            speed: -1.0,
            oscillator_sin: 0.0,
            oscillator_cos: 1.0,
            step_sin: 0.0,
            step_cos: 1.0,
            since_normalize: 0,
            lit: 0.0,
            struck: false,
            fall: (-1.0 / (FALL_SECONDS * rate)).exp(),
            rise: (-1.0 / (RISE_SECONDS * rate)).exp(),
        };
        this.cache_speed(0.0);
        this
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
        self.fall = (-1.0 / (FALL_SECONDS * rate)).exp();
        self.rise = (-1.0 / (RISE_SECONDS * rate)).exp();
        // Recompute only the phase increment. Phase itself is continuous.
        let speed = self.speed;
        self.cache_speed(speed);
    }

    #[inline]
    fn cache_speed(&mut self, speed: f64) {
        let speed = speed.clamp(0.0, 1.0);
        self.speed = speed;
        let hz = SLOWEST_HZ * (FASTEST_HZ / SLOWEST_HZ).powf(speed);
        let angle = std::f64::consts::TAU * hz / self.rate;
        let (step_sin, step_cos) = angle.sin_cos();
        self.step_sin = step_sin;
        self.step_cos = step_cos;
    }

    #[inline]
    fn oscillator(&mut self, speed: f64) -> f64 {
        let speed = speed.clamp(0.0, 1.0);
        if speed != self.speed {
            self.cache_speed(speed);
        }

        // Rotate by exactly one cached phase increment. The old code advanced
        // `phase` first and then evaluated the sine, so return the newly
        // advanced sine here too.
        let next_sin = self.oscillator_sin * self.step_cos + self.oscillator_cos * self.step_sin;
        let next_cos = self.oscillator_cos * self.step_cos - self.oscillator_sin * self.step_sin;
        self.oscillator_sin = next_sin;
        self.oscillator_cos = next_cos;
        self.since_normalize += 1;
        if self.since_normalize >= NORMALIZE_EVERY {
            let length = (self.oscillator_sin * self.oscillator_sin
                + self.oscillator_cos * self.oscillator_cos)
                .sqrt();
            if length.is_finite() && length > 0.0 {
                self.oscillator_sin /= length;
                self.oscillator_cos /= length;
            }
            self.since_normalize = 0;
        }
        self.oscillator_sin
    }

    /// The cell's resistance this sample.
    ///
    /// `speed` and `intensity` are the two panel controls, 0 to 1. Intensity
    /// sets how hard the oscillator drives the bulb, so at the bottom of its
    /// travel the bulb never strikes and the tremolo is off -- which is what
    /// the control does on the amplifier, rather than fading a depth.
    pub fn resistance(&mut self, speed: f64, intensity: f64) -> f64 {
        // The oscillator, driving the bulb harder as Intensity is opened.
        let drive = intensity.clamp(0.0, 1.0) * self.oscillator(speed);
        // Neon hysteresis: strikes high, extinguishes lower.
        const STRIKE: f64 = 0.35;
        const QUENCH: f64 = 0.18;
        self.struck = if self.struck {
            drive > QUENCH
        } else {
            drive > STRIKE
        };
        // How *brightly*, not merely whether.
        let target = if self.struck {
            ((drive - QUENCH) / (1.0 - QUENCH)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let coefficient = if target > self.lit {
            self.fall
        } else {
            self.rise
        };
        self.lit = target + (self.lit - target) * coefficient;
        // Log-interpolate, because a photoresistor's resistance is roughly
        // logarithmic in illumination and a linear blend would spend most of
        // its travel doing nothing. This is the same law as
        // `DARK * (LIT / DARK).powf(lit)`, with the constant logarithm removed
        // from the audio-rate path.
        DARK_OHMS * (self.lit * LOG_CELL_RATIO).exp()
    }

    /// What the shunt does to the signal at the point it sits.
    pub fn attenuation(&mut self, speed: f64, intensity: f64, source: f64, load: f64) -> f64 {
        let cell = self.resistance(speed, intensity);
        let shunt = cell * load / (cell + load);
        shunt / (source + shunt)
    }

    pub fn reset(&mut self) {
        self.oscillator_sin = 0.0;
        self.oscillator_cos = 1.0;
        self.since_normalize = 0;
        self.lit = 0.0;
        self.struck = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The optimized oscillator/cell path must remain the same model as the
    /// original direct phase + `sin` + logarithmic `powf` implementation. The
    /// arithmetic is deliberately rearranged to remove libm work from the
    /// audio-rate path, so require numerical agreement rather than bit identity.
    #[test]
    fn cached_oscillator_matches_direct_reference() {
        let rate = 48_000.0;
        let speed = 0.40;
        let intensity = 0.75;
        let mut optimized = Tremolo::new(rate);

        let hz = SLOWEST_HZ * (FASTEST_HZ / SLOWEST_HZ).powf(speed);
        let mut phase = 0.0f64;
        let mut lit = 0.0f64;
        let mut struck = false;
        let fall = (-1.0 / (FALL_SECONDS * rate)).exp();
        let rise = (-1.0 / (RISE_SECONDS * rate)).exp();
        const STRIKE: f64 = 0.35;
        const QUENCH: f64 = 0.18;

        let mut worst_relative = 0.0f64;
        for _ in 0..(48_000 * 10) {
            phase += hz / rate;
            if phase >= 1.0 {
                phase -= 1.0;
            }
            let drive = intensity * (std::f64::consts::TAU * phase).sin();
            struck = if struck {
                drive > QUENCH
            } else {
                drive > STRIKE
            };
            let target = if struck {
                ((drive - QUENCH) / (1.0 - QUENCH)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let coefficient = if target > lit { fall } else { rise };
            lit = target + (lit - target) * coefficient;
            let reference = DARK_OHMS * (LIT_OHMS / DARK_OHMS).powf(lit);
            let actual = optimized.resistance(speed, intensity);
            let relative = ((actual - reference) / reference.max(1.0)).abs();
            worst_relative = worst_relative.max(relative);
        }
        assert!(
            worst_relative < 1e-9,
            "optimized tremolo drifted from the direct model: {worst_relative:e}"
        );
    }
}
