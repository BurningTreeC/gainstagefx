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
//! started at. So the light is a pulse, not a sine. The panel Intensity
//! control is **not** in this lamp drive: it is the 50 k reverse-audio
//! pot whose wiper the LDR shunts later in the signal path.
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

use crate::dsp::netlist::Taper;

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

/// The AB763 Intensity control is not a lamp-drive control. It is a 50 kΩ
/// reverse-audio pot from the V4-B output node to ground, with the LDR tied
/// from its wiper to ground. The Vibrato channel then reaches the phase
/// inverter through 220 kΩ, whose far side sees about 1 MΩ.
///
/// The complete Twin circuit now contains this network directly. The constants
/// below remain only for the analytical attenuation regression tests, which
/// cross-check the optical cell against the former closed-form divider.
const INTENSITY_OHMS: f64 = 50_000.0;
const V4B_SOURCE_OHMS: f64 = 38_000.0;
const CHANNEL_MIX_OHMS: f64 = 220_000.0;
const PI_GRID_LEAK_OHMS: f64 = 1_000_000.0;
const DOWNSTREAM_OHMS: f64 = CHANNEL_MIX_OHMS + PI_GRID_LEAK_OHMS;

#[inline]
fn parallel(a: f64, b: f64) -> f64 {
    a * b / (a + b)
}

#[inline]
fn loaded_node(load: f64) -> f64 {
    load / (V4B_SOURCE_OHMS + load)
}

#[inline]
fn baseline_node_gain() -> f64 {
    loaded_node(parallel(INTENSITY_OHMS, DOWNSTREAM_OHMS))
}

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

    /// The optical cell's resistance this sample.
    ///
    /// The oscillator and neon are upstream of the panel Intensity control on
    /// the AB763. Intensity therefore does **not** change the lamp waveform; it
    /// only changes how strongly this LDR can shunt the audio node. Keep the
    /// argument for API compatibility with the test/example code, but ignore it
    /// deliberately.
    pub fn resistance(&mut self, speed: f64, _intensity: f64) -> f64 {
        let drive = self.oscillator(speed);
        // Neon hysteresis: it strikes at the higher voltage and extinguishes
        // at the lower one, turning the oscillator into asymmetric light
        // pulses before the LDR's own lag is applied.
        const STRIKE: f64 = 0.35;
        const QUENCH: f64 = 0.18;
        self.struck = if self.struck {
            drive > QUENCH
        } else {
            drive > STRIKE
        };
        let target = if self.struck {
            ((drive - QUENCH) / (1.0 - QUENCH)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let coefficient = if target > self.lit { self.fall } else { self.rise };
        self.lit = target + (self.lit - target) * coefficient;
        DARK_OHMS * (self.lit * LOG_CELL_RATIO).exp()
    }

    /// Relative voltage at the AB763 Vibrato-channel hand-off after the
    /// 50 kΩ reverse-audio Intensity pot and its photocell are applied.
    ///
    /// Test/reference closed-form ratio for the passive intensity network. The
    /// production Twin stamps the pot and LDR into MNA directly.
    /// At Intensity zero the wiper is at ground and the LDR cannot change the
    /// signal, but the oscillator still advances. At full Intensity the wiper
    /// approaches the signal node and the lit cell can shunt it strongly.
    #[inline]
    pub fn attenuation(&mut self, speed: f64, intensity: f64) -> f64 {
        let cell = self.resistance(speed, intensity);
        let p = intensity.clamp(0.0, 1.0);

        // A C/reverse-audio track is the mirror of the normal A/audio law.
        // With ground at the CCW end, this expression has the required
        // physical endpoints: zero ohms wiper-to-ground at 0, the full 50 kΩ
        // at 1, with the reverse-log law between them.
        let bottom_fraction = Taper::ReverseAudio.fraction(1.0 - p);
        let bottom = INTENSITY_OHMS * bottom_fraction;
        let top = INTENSITY_OHMS - bottom;
        let wiper_to_ground = if bottom <= f64::EPSILON {
            0.0
        } else {
            parallel(bottom, cell)
        };
        let tremolo_load = top + wiper_to_ground;
        let total_load = parallel(tremolo_load.max(1.0), DOWNSTREAM_OHMS);
        loaded_node(total_load) / baseline_node_gain()
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

    #[test]
    fn intensity_zero_is_exactly_unity_while_the_cell_keeps_advancing() {
        let mut trem = Tremolo::new(48_000.0);
        let mut changed = false;
        let mut previous = trem.resistance(0.4, 0.0);
        for _ in 0..48_000 {
            let gain = trem.attenuation(0.4, 0.0);
            assert!((gain - 1.0).abs() < 2e-15, "zero Intensity must not modulate");
            let now = trem.resistance(0.4, 0.0);
            changed |= (now - previous).abs() > 1e-6;
            previous = now;
        }
        assert!(changed, "the optical oscillator must keep running at zero Intensity");
    }

    #[test]
    fn intensity_does_not_change_the_lamp_or_cell_trajectory() {
        let mut low = Tremolo::new(48_000.0);
        let mut high = Tremolo::new(48_000.0);
        for sample in 0..100_000 {
            let a = low.resistance(0.63, 0.0);
            let b = high.resistance(0.63, 1.0);
            assert_eq!(a.to_bits(), b.to_bits(), "cell trajectory changed at sample {sample}");
        }
    }

    #[test]
    fn full_intensity_produces_real_optical_depth() {
        let mut trem = Tremolo::new(48_000.0);
        let mut min = 1.0f64;
        let mut max = 0.0f64;
        for _ in 0..(48_000 * 3) {
            let gain = trem.attenuation(0.45, 1.0);
            min = min.min(gain);
            max = max.max(gain);
        }
        assert!(max > 0.97, "dark cell should return close to the stock load: {max}");
        assert!(min < 0.55, "lit cell should produce deep AB763 tremolo: {min}");
    }

    #[test]
    fn cached_oscillator_matches_direct_reference_without_intensity_drive() {
        let rate = 48_000.0;
        let speed = 0.40;
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
            let drive = (std::f64::consts::TAU * phase).sin();
            struck = if struck { drive > QUENCH } else { drive > STRIKE };
            let target = if struck {
                ((drive - QUENCH) / (1.0 - QUENCH)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let coefficient = if target > lit { fall } else { rise };
            lit = target + (lit - target) * coefficient;
            let reference = DARK_OHMS * (LIT_OHMS / DARK_OHMS).powf(lit);
            let actual = optimized.resistance(speed, 0.0);
            let relative = ((actual - reference) / reference.max(1.0)).abs();
            worst_relative = worst_relative.max(relative);
        }
        assert!(
            worst_relative < 1e-9,
            "optimized tremolo drifted from the direct model: {worst_relative:e}"
        );
    }
}
