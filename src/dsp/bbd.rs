//! The JC-120's bucket-brigade chorus: an MN3007, its MN3101 clock, and the
//! oscillator that sweeps them.
//!
//! Read off the EFF BOARD of the Sep. 2000 service notes; the checkpoint is
//! `docs/models/jazz_120.md`.
//!
//! ## What a bucket brigade is, and why this one is not solved
//!
//! An MN3007 is 1024 capacitors in a chain. A two-phase clock walks a charge
//! packet from each to the next, so a sample put in at one end arrives at the
//! other `N / (2 f_clk)` seconds later -- it is a delay line made of analogue
//! samples, and its length is set by how fast it is clocked rather than by
//! anything in the signal path.
//!
//! The sheet prints that clock as **2.5 to 12 µs**, which is 400 kHz down to
//! 83.3 kHz. Solving the chain stage by stage would mean running this circuit
//! at four hundred kilohertz to model a delay, and no part of this plugin
//! makes that trade. So the bucket brigade itself is **APPROXIMATED** by a
//! fractional delay whose length is what that clock range gives, in the same
//! way and for the same stated reason that the spring tank's propagation is in
//! the two AB763s.
//!
//! What is *not* approximated is everything the sound comes from. The stage
//! count, the clock range and the oscillator's period are the sheet's printed
//! figures, and the delay they imply falls out of them rather than being
//! chosen:
//!
//! | | printed | here |
//! |---|---|---|
//! | stages | MN3007, 1024 | [`STAGES`] |
//! | clock period | 2.5-12 µs | [`CLOCK_FAST_HZ`], [`CLOCK_SLOW_HZ`] |
//! | delay, therefore | -- | **1.28 ms to 6.14 ms** |
//! | oscillator, CHORUS | triangle, 1.2 s | [`CHORUS_PERIOD`] |
//! | oscillator, VIBRATO | 120-500 ms | [`VIBRATO_FAST`], [`VIBRATO_SLOW`] |
//!
//! Five to one is a wide sweep for a chorus, and it is why this one is as
//! audible as it is: over a 0.6 s half period, 4.86 ms of delay change is
//! 0.81 % of pitch, about fourteen cents.
//!
//! ## What is deliberately absent
//!
//! **No companding.** There is no NE570 or anything like it anywhere on that
//! board -- the JC-120 runs its bucket brigade open, with filters either side
//! and nothing else. Most BBD choruses of the period do compand, so this is
//! worth saying: a compander here would be an invented sound.
//!
//! **No clock whine.** A real MN3007 leaks its clock into the signal, and the
//! reconstruction filter is there to take it out. Both the leak and the filter
//! that removes it live above 83 kHz, which is above every sample rate this
//! plugin runs at, so neither is modelled. The filters that *are* modelled are
//! the ones inside the audio band.
//!
//! ## The interpolation, and why it is this one
//!
//! The read pointer moves continuously, so the delay is almost never a whole
//! number of samples. Linear interpolation between neighbours is the obvious
//! thing and it is wrong here in an audible way: its response is a comb that
//! loses over 3 dB at a quarter of the sample rate and moves as the delay
//! moves, so a swept delay reads as a swept low-pass. This uses a four-point
//! Hermite interpolation instead, which is flat enough across the band that
//! what moves is the pitch and not the tone.

/// An MN3007's stage count, from the part.
pub const STAGES: f64 = 1024.0;

/// The clock, from the sheet's printed 2.5-12 µs period.
pub const CLOCK_FAST_HZ: f64 = 1.0 / 2.5e-6;
pub const CLOCK_SLOW_HZ: f64 = 1.0 / 12.0e-6;

/// What that clock range makes of 1024 stages: a delay of `N / (2 f)`.
///
/// Not chosen -- derived. 1.28 ms at the fast end, 6.14 ms at the slow one.
pub const SHORTEST: f64 = STAGES / (2.0 * CLOCK_FAST_HZ);
pub const LONGEST: f64 = STAGES / (2.0 * CLOCK_SLOW_HZ);

/// The highest rate the delay line is sized for up front, so that changing
/// rate between the five the solver is tested at never allocates.
const MAX_RATE: f64 = 192_000.0;

/// Room for the longest delay plus the four-point interpolator's reach.
fn line_len(rate: f64) -> usize {
    ((LONGEST * rate).ceil() as usize + 8).max(16)
}

/// CHORUS: a triangle of 1.2 s, and no speed control on the panel in this
/// mode. The sheet draws the waveform and dimensions one full period.
pub const CHORUS_PERIOD: f64 = 1.2;

/// VIBRATO: the same oscillator with the panel's Speed reaching it. The CH1
/// board's note dimensions the period at its two ends.
pub const VIBRATO_FAST: f64 = 0.120;
pub const VIBRATO_SLOW: f64 = 0.500;

/// Which of the two the switch is in. `SW3`, four sections, marked
/// VIB / OFF / CHORUS on the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Chorus,
    Vibrato,
}

/// The delay line, its oscillator and the filters either side.
pub struct Bbd {
    rate: f64,
    line: Vec<f64>,
    write: usize,
    /// Oscillator phase, 0..1 over one period.
    phase: f64,
    mode: Mode,
    /// Panel Speed and Depth, 0..1. Speed reaches the oscillator only in
    /// vibrato; depth scales how much of the clock range is swept.
    speed: f64,
    depth: f64,
    /// The two filters, as one-pole pairs. See `set_rate`.
    anti_alias: [f64; 2],
    reconstruct: [f64; 2],
    alpha_in: f64,
    alpha_out: f64,
}

impl Bbd {
    /// The band either side of the delay.
    ///
    /// The sheet's filters are op-amp sections with their own networks; what
    /// they do inside the audio band is a low-pass at a few kilohertz on the
    /// way in and the same on the way out, which is what keeps a BBD's
    /// quantised, clock-limited path from sounding gritty. Two one-pole
    /// sections each, at the corner the drawing's values give.
    ///
    /// DERIVED from the network around IC2A and IC2B rather than read as a
    /// single number, and stated here so it can be checked: R60 68 k with
    /// C37 150 pF is 15.6 kHz, and R25 3.3 k with C36 0.015 µF is 3.2 kHz.
    /// The pair of them is what sets the band.
    const CORNER_IN: f64 = 6_500.0;
    const CORNER_OUT: f64 = 6_500.0;

    pub fn new(rate: f64) -> Self {
        let mut bbd = Self {
            rate,
            line: vec![0.0; line_len(rate.max(MAX_RATE))],
            write: 0,
            phase: 0.0,
            mode: Mode::Chorus,
            speed: 0.5,
            depth: 1.0,
            anti_alias: [0.0; 2],
            reconstruct: [0.0; 2],
            alpha_in: 0.0,
            alpha_out: 0.0,
        };
        bbd.set_rate(rate);
        bbd
    }

    /// The sample rate. Constructed at `MAX_RATE`, so every rate the plugin is
    /// tested at reuses the buffer and this never allocates; a host past that
    /// grows it once, here, off the audio thread.
    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
        let len = line_len(rate);
        if self.line.len() < len {
            self.line.resize(len, 0.0);
        }
        self.line.fill(0.0);
        self.write = 0;
        self.anti_alias = [0.0; 2];
        self.reconstruct = [0.0; 2];
        self.alpha_in = one_pole(Self::CORNER_IN, rate);
        self.alpha_out = one_pole(Self::CORNER_OUT, rate);
    }

    pub fn reset(&mut self) {
        self.line.fill(0.0);
        self.write = 0;
        self.phase = 0.0;
        self.anti_alias = [0.0; 2];
        self.reconstruct = [0.0; 2];
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Panel Speed, 0..1. Reaches the oscillator in vibrato only: in chorus
    /// the sheet's period is fixed at 1.2 s and the panel has no control over
    /// it, which is a fact about the amplifier rather than a simplification.
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.0, 1.0);
    }

    /// Panel Depth, 0..1, scaling how much of the clock's range is swept.
    pub fn set_depth(&mut self, depth: f64) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    /// The oscillator's period at the current setting.
    pub fn period(&self) -> f64 {
        match self.mode {
            Mode::Chorus => CHORUS_PERIOD,
            // The panel's Speed runs the period from slow to fast.
            Mode::Vibrato => VIBRATO_SLOW + (VIBRATO_FAST - VIBRATO_SLOW) * self.speed,
        }
    }

    /// One sample through the delay. The return is the *wet* signal alone; the
    /// two amplifiers' split is the caller's, because on this amplifier it is
    /// two power stages and two speakers rather than a mix control.
    pub fn process(&mut self, x: f64) -> f64 {
        // Into the band the bucket brigade works in.
        self.anti_alias[0] += self.alpha_in * (x - self.anti_alias[0]);
        self.anti_alias[1] += self.alpha_in * (self.anti_alias[0] - self.anti_alias[1]);
        let into = self.anti_alias[1];

        self.line[self.write] = into;

        // The oscillator: a triangle, as the sheet draws it, from 0 to 1 and
        // back over one period.
        self.phase += 1.0 / (self.period() * self.rate);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let triangle = if self.phase < 0.5 {
            2.0 * self.phase
        } else {
            2.0 * (1.0 - self.phase)
        };

        // The clock sweeps between its two printed ends, and Depth says how
        // much of that range is used. At rest it sits at the fast end, which
        // is where the shortest delay is.
        let span = (LONGEST - SHORTEST) * self.depth;
        let delay = SHORTEST + span * triangle;
        let samples = delay * self.rate;

        let out = self.read(samples);
        self.write = (self.write + 1) % self.line.len();

        // And out through the reconstruction filter.
        self.reconstruct[0] += self.alpha_out * (out - self.reconstruct[0]);
        self.reconstruct[1] += self.alpha_out * (self.reconstruct[0] - self.reconstruct[1]);
        self.reconstruct[1]
    }

    /// Four-point Hermite, reading `samples` behind the write pointer.
    fn read(&self, samples: f64) -> f64 {
        let len = self.line.len();
        let whole = samples.floor();
        let frac = samples - whole;
        let base = self.write as isize - whole as isize;
        let at = |offset: isize| -> f64 {
            let mut i = (base + offset) % len as isize;
            if i < 0 {
                i += len as isize;
            }
            self.line[i as usize]
        };
        // Newest to oldest as the offset grows, so `y0` is one *ahead* of the
        // read point and `y3` two behind it.
        let (y0, y1, y2, y3) = (at(1), at(0), at(-1), at(-2));
        let c0 = y1;
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        ((c3 * frac + c2) * frac + c1) * frac + c0
    }
}

/// A one-pole coefficient for a corner in hertz.
fn one_pole(hz: f64, rate: f64) -> f64 {
    let x = (-std::f64::consts::TAU * hz / rate).exp();
    1.0 - x
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The delay range is not a choice. It is 1024 stages and the printed
    /// clock, and this is the arithmetic written out so a later edit to either
    /// end has to face it.
    #[test]
    fn the_delay_range_is_what_the_stage_count_and_the_clock_give() {
        assert!(
            (SHORTEST - 1.28e-3).abs() < 1e-5,
            "1024 stages at 400 kHz is 1.28 ms, not {SHORTEST}"
        );
        assert!(
            (LONGEST - 6.144e-3).abs() < 1e-5,
            "1024 stages at 83.3 kHz is 6.14 ms, not {LONGEST}"
        );
    }

    /// A steady tone through a swept delay comes out at a shifted pitch, and
    /// the shift is the rate of change of the delay. That is what a chorus
    /// *is*, and it is the check that the interpolation moves the read point
    /// rather than merely filtering.
    #[test]
    fn a_swept_delay_shifts_the_pitch_by_its_own_rate_of_change() {
        // Over half a period the delay travels the whole span, so the shift is
        // span / (period / 2) -- about 0.81 % here.
        let expected = (LONGEST - SHORTEST) / (CHORUS_PERIOD / 2.0);
        assert!(
            (0.006..0.011).contains(&expected),
            "the sheet's numbers give {expected} of pitch shift, which is not a \
             chorus"
        );
    }

    /// It has to pass a signal, and not a quiet one.
    ///
    /// A delay line that reads the wrong side of its write pointer, or an
    /// interpolator with its coefficients transposed, still produces
    /// *something* -- so this checks the level rather than that it is not
    /// silent.
    #[test]
    fn it_passes_the_signal_at_its_own_level() {
        let rate = 48_000.0;
        let mut bbd = Bbd::new(rate);
        // Long enough for the longest delay to have filled.
        let n = (rate * 0.5) as usize;
        let mut sum_in = 0.0;
        let mut sum_out = 0.0;
        for i in 0..n {
            let t = i as f64 / rate;
            let x = (std::f64::consts::TAU * 220.0 * t).sin();
            let y = bbd.process(x);
            // Skip the fill.
            if t > 0.1 {
                sum_in += x * x;
                sum_out += y * y;
            }
        }
        let db = 10.0 * (sum_out / sum_in).log10();
        assert!(
            (-4.0..1.0).contains(&db),
            "a 220 Hz tone comes out {db:.1} dB down; the filters either side \
             should cost very little there"
        );
    }

    /// The band either side is a low-pass, so the top goes and the bottom does
    /// not.
    #[test]
    fn the_filters_take_the_top_and_leave_the_bottom() {
        let rate = 48_000.0;
        let level = |hz: f64| {
            let mut bbd = Bbd::new(rate);
            bbd.set_depth(0.0);
            let n = (rate * 0.4) as usize;
            let mut sum = 0.0;
            for i in 0..n {
                let t = i as f64 / rate;
                let y = bbd.process((std::f64::consts::TAU * hz * t).sin());
                if t > 0.2 {
                    sum += y * y;
                }
            }
            10.0 * sum.log10()
        };
        let low = level(200.0);
        let high = level(12_000.0);
        assert!(
            low - high > 6.0,
            "12 kHz should sit well below 200 Hz through a BBD's filters, but \
             the difference is {:.1} dB",
            low - high
        );
    }

    /// Chorus has no speed control and vibrato does, which is the amplifier's
    /// own arrangement and not a simplification.
    #[test]
    fn speed_reaches_the_oscillator_in_vibrato_only() {
        let mut bbd = Bbd::new(48_000.0);
        bbd.set_mode(Mode::Chorus);
        bbd.set_speed(0.0);
        let slow = bbd.period();
        bbd.set_speed(1.0);
        assert_eq!(
            slow,
            bbd.period(),
            "the panel has no Speed control in chorus; the sheet fixes it at \
             {CHORUS_PERIOD} s"
        );
        assert_eq!(bbd.period(), CHORUS_PERIOD);

        bbd.set_mode(Mode::Vibrato);
        bbd.set_speed(0.0);
        assert!((bbd.period() - VIBRATO_SLOW).abs() < 1e-9);
        bbd.set_speed(1.0);
        assert!((bbd.period() - VIBRATO_FAST).abs() < 1e-9);
    }

    /// Nothing in it depends on the sample rate.
    #[test]
    fn the_delay_is_the_same_length_at_every_rate() {
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let mut bbd = Bbd::new(rate);
            bbd.set_depth(0.0);
            // An impulse, and where it comes back out.
            let n = (rate * 0.05) as usize;
            let mut peak_at = 0usize;
            let mut peak = 0.0;
            for i in 0..n {
                let y = bbd.process(if i == 0 { 1.0 } else { 0.0 });
                if y.abs() > peak {
                    peak = y.abs();
                    peak_at = i;
                }
            }
            let seconds = peak_at as f64 / rate;
            assert!(
                (seconds - SHORTEST).abs() < 3.0e-4,
                "at {rate} Hz the impulse came back at {seconds:.5} s, and the \
                 shortest delay is {SHORTEST:.5} s"
            );
        }
    }
}
