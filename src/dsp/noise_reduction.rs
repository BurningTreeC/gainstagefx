//! Optional input expander. One detector/gain serves both host channels before
//! the pedal and amplifier; their nonlinear states still run on every sample.
//! No lookahead, FFT, hard zeroing, or change to the plugin's latency.

pub(crate) struct NoiseReduction {
    envelope: f64,
    gain: f64,
    threshold: f64,
    threshold_db: f32,
    release: f64,
    open: f64,
    close: f64,
    hold_samples: u32,
    hold: u32,
    fade_samples: u32,
    fade_remaining: u32,
    blend: f64,
    blend_step: f64,
    enabled: bool,
}

impl NoiseReduction {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            envelope: 0.0,
            gain: 1.0,
            threshold: 0.001,
            threshold_db: -60.0,
            release: (-1.0 / (0.120 * sample_rate)).exp(),
            open: (-1.0 / (0.0005 * sample_rate)).exp(),
            close: (-1.0 / (0.080 * sample_rate)).exp(),
            hold_samples: (0.025 * sample_rate).round() as u32,
            hold: 0,
            fade_samples: (0.005 * sample_rate).round().max(1.0) as u32,
            fade_remaining: 0,
            blend: 0.0,
            blend_step: 0.0,
            enabled: false,
        }
    }

    /// Called at block boundaries. Gain smoothing also covers threshold edits.
    pub fn configure(&mut self, enabled: bool, threshold_db: f32) {
        if threshold_db != self.threshold_db {
            self.threshold_db = threshold_db;
            self.threshold = 10.0f64.powf(threshold_db as f64 / 20.0);
        }
        if enabled != self.enabled {
            self.enabled = enabled;
            self.fade_remaining = self.fade_samples;
            self.blend_step = (f64::from(enabled) - self.blend) / self.fade_samples as f64;
        }
    }

    pub fn reset(&mut self, enabled: bool, threshold_db: f32) {
        self.envelope = 0.0;
        // Start open so transport starts do not truncate the first attack.
        self.gain = 1.0;
        self.hold = self.hold_samples;
        self.configure(enabled, threshold_db);
        self.blend = f64::from(enabled);
        self.fade_remaining = 0;
    }

    /// Peak magnitude of the hotter trimmed input channel, once per host frame.
    #[inline]
    pub fn next_gain(&mut self, peak: f64) -> f64 {
        // Do not let a malformed host sample poison the envelope indefinitely.
        let peak = if peak.is_finite() { peak.abs() } else { 0.0 };
        self.envelope = peak.max(self.envelope * self.release);
        if self.envelope >= self.threshold {
            self.hold = self.hold_samples;
        } else {
            self.hold = self.hold.saturating_sub(1);
        }
        let target = if self.hold > 0 {
            1.0
        } else {
            expansion_gain(self.envelope / self.threshold)
        };
        let coefficient = if target > self.gain {
            self.open
        } else {
            self.close
        };
        self.gain = target + coefficient * (self.gain - target);
        if self.fade_remaining > 0 {
            self.fade_remaining -= 1;
            self.blend = if self.fade_remaining == 0 {
                f64::from(self.enabled)
            } else {
                self.blend + self.blend_step
            };
        }
        // Exact unity when off, including after an automated disable. Keep
        // tracking the envelope so enabling during a note does not close it.
        if self.blend == 0.0 {
            1.0
        } else {
            1.0 + self.blend * (self.gain - 1.0)
        }
    }
}

/// 2:1 expansion below threshold - 6.02 dB, a cubic knee up to unity at
/// threshold, and at most 40 dB attenuation. Value and slope join continuously;
/// the polynomial avoids per-sample logarithms/exponentials.
fn expansion_gain(ratio: f64) -> f64 {
    if ratio >= 1.0 {
        1.0
    } else if ratio <= 0.5 {
        ratio.max(0.01)
    } else {
        let t = 2.0 * ratio - 1.0;
        0.5 + 0.5 * t + 0.5 * t * t * (1.0 - t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expander_curve_is_continuous_monotone_and_has_the_expected_ratio() {
        assert_eq!(expansion_gain(0.0), 0.01);
        assert_eq!(expansion_gain(0.1), 0.1); // -20 dB input -> -40 dB output
        assert_eq!(expansion_gain(0.5), 0.5);
        assert_eq!(expansion_gain(1.0), 1.0);
        assert_eq!(expansion_gain(20.0), 1.0);
        let mut last = 0.01;
        for i in 0..=2000 {
            let value = expansion_gain(i as f64 / 1000.0);
            assert!(value >= last && value <= 1.0);
            assert!(value - last <= 0.002);
            last = value;
        }
    }

    #[test]
    fn expander_rates_attenuation_hold_attack_and_bypass() {
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let mut gate = NoiseReduction::new(rate);
            for _ in 0..rate as usize {
                assert_eq!(gate.next_gain(0.0001), 1.0);
            }
            gate.configure(true, -60.0);
            for _ in 0..rate as usize {
                gate.next_gain(0.0001);
            }
            let quiet = gate.next_gain(0.0001);
            assert!((quiet - 0.1).abs() < 1e-5, "rate={rate}, gain={quiet}");
            let first_attack = gate.next_gain(0.1);
            assert!(first_attack > quiet && first_attack < 0.15);
            for _ in 0..(0.005 * rate) as usize {
                gate.next_gain(0.1);
            }
            assert!(gate.next_gain(0.1) > 0.9999);
            for _ in 0..(0.025 * rate) as usize {
                assert!(gate.next_gain(0.0) > 0.9999, "hold rate={rate}");
            }
            for _ in 0..(2.0 * rate) as usize {
                gate.next_gain(0.0);
            }
            assert!((gate.next_gain(0.0) - 0.01).abs() < 1e-5);
            gate.configure(false, -60.0);
            let mut previous = 0.01;
            for _ in 0..gate.fade_samples {
                let value = gate.next_gain(0.0);
                assert!(value >= previous && value - previous < 0.005);
                previous = value;
            }
            assert_eq!(previous, 1.0);
            assert_eq!(gate.next_gain(f64::NAN), 1.0);
            gate.reset(true, -60.0);
            assert_eq!(gate.next_gain(0.1), 1.0);
        }
    }
}
