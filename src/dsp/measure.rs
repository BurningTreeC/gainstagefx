//! Measuring what a circuit does, once and correctly.
//!
//! The previous version wrote this by hand in six places and got it wrong in
//! three different ways, each of which read as behaviour rather than as a
//! mistake:
//!
//! * a tone that did not land exactly on a bin leaked across its neighbours
//!   and put a floor of about 0.8 % under every distortion reading;
//! * restarting the input's phase between settling and measuring put a step in
//!   the signal, and the splash off that step measured as 2.6 % distortion on
//!   a network made of two resistors and a capacitor;
//! * a transformer measured at 220 Hz and a treble booster measured at 220 Hz
//!   both read zero, because neither does anything there.
//!
//! The first two are arithmetic and are fixed here for good. The third is a
//! judgement about the circuit, so [`Tone`] carries the frequency with it and
//! whoever asks has to say where they are looking.

use super::complex::C;

/// A test tone that lands exactly on a DFT bin.
///
/// Bin alignment is not a nicety. A tone between bins spreads its energy over
/// the neighbouring ones, and everything measured against it -- harmonics,
/// noise, a filter's own response -- inherits that skirt.
#[derive(Clone, Copy, Debug)]
pub struct Tone {
    pub rate: f64,
    /// Length of the analysis window, in samples.
    pub window: usize,
    /// Which bin. The frequency follows from it.
    pub bin: f64,
    pub amplitude: f64,
}

impl Tone {
    /// The bin nearest a wanted frequency, so callers can think in hertz and
    /// still get an aligned tone.
    pub fn near(rate: f64, window: usize, hz: f64, amplitude: f64) -> Self {
        let bin = (hz * window as f64 / rate).round().max(1.0);
        Self {
            rate,
            window,
            bin,
            amplitude,
        }
    }

    pub fn hz(&self) -> f64 {
        self.rate * self.bin / self.window as f64
    }

    /// The tone at a sample index. The index is absolute, which is what keeps
    /// the phase continuous across the settle boundary.
    pub fn at(&self, i: usize) -> f64 {
        let w = std::f64::consts::TAU * self.bin / self.window as f64;
        self.amplitude * (w * i as f64).sin()
    }
}

/// What came out, as a spectrum of the bins that matter.
#[derive(Clone, Debug)]
pub struct Measured {
    pub tone: Tone,
    samples: Vec<f64>,
}

impl Measured {
    /// The complex amplitude at a bin.
    pub fn bin(&self, bin: f64) -> C {
        let n = self.samples.len();
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &x) in self.samples.iter().enumerate() {
            let a = std::f64::consts::TAU * bin * i as f64 / n as f64;
            re += x * a.cos();
            im -= x * a.sin();
        }
        C::new(2.0 * re / n as f64, 2.0 * im / n as f64)
    }

    /// The fundamental.
    pub fn fundamental(&self) -> C {
        self.bin(self.tone.bin)
    }

    /// Output level relative to the input, in dB.
    pub fn gain_db(&self) -> f64 {
        20.0 * (self.fundamental().magnitude() / self.tone.amplitude).log10()
    }

    /// Total harmonic distortion as a percentage of the fundamental, over the
    /// harmonics that fit below Nyquist.
    pub fn thd_percent(&self) -> f64 {
        let n = self.samples.len();
        let f1 = self.fundamental().magnitude().max(1e-30);
        let mut sum = 0.0;
        let mut h = 2.0;
        while self.tone.bin * h < n as f64 / 2.0 {
            let m = self.bin(self.tone.bin * h).magnitude();
            sum += m * m;
            h += 1.0;
        }
        sum.sqrt() / f1 * 100.0
    }

    /// Everything that came out which is not the fundamental and not one of
    /// its harmonics, as a percentage of the fundamental.
    ///
    /// This is the aliasing measure. A nonlinear stage makes harmonics, and
    /// harmonics above half the sample rate do not simply vanish -- they fold
    /// back down and land on frequencies that have nothing to do with the note
    /// being played. `thd_percent` cannot see any of it, because it looks only
    /// at the bins where harmonics belong, so a circuit can alias badly and
    /// still measure exactly the same total.
    ///
    /// What it sounds like is not more distortion. Folded partials are
    /// inharmonic, so they do not thicken the note; they sit beside it and
    /// buzz, and they move the wrong way when the note does. `CLAUDE.md` §22
    /// asks for this to be inspected rather than assumed.
    ///
    /// A tone at a bin, run through something time-invariant, puts all of its
    /// legitimate output on multiples of that bin. So everything in between is
    /// either fold-back or numerical noise, and the two are told apart by
    /// their size.
    pub fn inharmonic_percent(&self) -> f64 {
        let n = self.samples.len();
        let f1 = self.fundamental().magnitude().max(1e-30);
        let step = self.tone.bin.max(1.0);
        let mut sum = 0.0;
        let mut bin = 1.0;
        while bin < n as f64 / 2.0 {
            // Skip the fundamental and its harmonics, and the bin either side
            // of each: a window that is not an exact number of cycles leaks a
            // little into its neighbours, and that leakage is not aliasing.
            let nearest = (bin / step).round() * step;
            if (bin - nearest).abs() > 1.5 {
                let m = self.bin(bin).magnitude();
                sum += m * m;
            }
            bin += 1.0;
        }
        sum.sqrt() / f1 * 100.0
    }

    /// One harmonic, as a percentage of the fundamental.
    pub fn harmonic_percent(&self, which: u32) -> f64 {
        let f1 = self.fundamental().magnitude().max(1e-30);
        self.bin(self.tone.bin * which as f64).magnitude() / f1 * 100.0
    }
}

/// Runs a tone through something and measures what comes back.
///
/// `settle` is how many samples to run before the window opens -- coupling
/// capacitors have to charge and a bypass has to reach the gain the stage will
/// actually have. The sample index runs unbroken through both, because
/// restarting it puts a step in the input and the splash off that step is
/// indistinguishable from distortion.
pub fn run(tone: Tone, settle: usize, mut step: impl FnMut(f64) -> f64) -> Measured {
    for i in 0..settle {
        step(tone.at(i));
    }
    let samples = (0..tone.window)
        .map(|i| step(tone.at(settle + i)))
        .collect();
    Measured { tone, samples }
}
