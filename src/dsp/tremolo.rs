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
const LIT_OHMS: f64 = 12_000.0;

/// How fast the cell responds. Down quickly, back slowly, and the asymmetry
/// is the point.
const FALL_SECONDS: f64 = 0.006;
const RISE_SECONDS: f64 = 0.090;

pub struct Tremolo {
    rate: f64,
    phase: f64,
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
    /// Copy oscillator phase, bulb hysteresis and the cell's illumination.
    /// The two oscillators must already have the same sample rate.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.rate, source.rate);
        self.phase = source.phase;
        self.lit = source.lit;
        self.struck = source.struck;
    }

    pub fn new(rate: f64) -> Self {
        Self {
            rate,
            phase: 0.0,
            lit: 0.0,
            struck: false,
            fall: (-1.0 / (FALL_SECONDS * rate)).exp(),
            rise: (-1.0 / (RISE_SECONDS * rate)).exp(),
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
        self.fall = (-1.0 / (FALL_SECONDS * rate)).exp();
        self.rise = (-1.0 / (RISE_SECONDS * rate)).exp();
    }

    /// The cell's resistance this sample.
    ///
    /// `speed` and `intensity` are the two panel controls, 0 to 1. Intensity
    /// sets how hard the oscillator drives the bulb, so at the bottom of its
    /// travel the bulb never strikes and the tremolo is off -- which is what
    /// the control does on the amplifier, rather than fading a depth.
    pub fn resistance(&mut self, speed: f64, intensity: f64) -> f64 {
        let hz = SLOWEST_HZ * (FASTEST_HZ / SLOWEST_HZ).powf(speed.clamp(0.0, 1.0));
        self.phase += hz / self.rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        // The oscillator, driving the bulb harder as Intensity is opened.
        let drive = intensity.clamp(0.0, 1.0) * (std::f64::consts::TAU * self.phase).sin();
        // Neon hysteresis: strikes high, extinguishes lower.
        //
        // There is a real dead zone at the bottom of the Intensity control
        // because of this -- below the striking point the bulb never fires and
        // the tremolo is simply off, which is what the amplifier does and not
        // a fault. Where the strike sits decides how much of the travel that
        // costs, and 0.35 leaves about two thirds of the control useful.
        const STRIKE: f64 = 0.35;
        const QUENCH: f64 = 0.18;
        self.struck = if self.struck {
            drive > QUENCH
        } else {
            drive > STRIKE
        };
        // How *brightly*, not merely whether.
        //
        // A neon bulb past its striking point passes more current the harder
        // it is driven, and a brighter bulb pulls the cell's resistance
        // further down. Modelled as on-or-off, the Intensity control changed
        // only how *long* each pulse lasted and never how deep it went --
        // measured, 12.5 dB at every setting that fired at all, which is not
        // what the control does on the amplifier.
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
        // its travel doing nothing.
        DARK_OHMS * (LIT_OHMS / DARK_OHMS).powf(self.lit)
    }

    /// What the shunt does to the signal at the point it sits.
    ///
    /// The cell is across the signal path to ground, between the stage driving
    /// it and the load it drives. `source` is the driving stage's own output
    /// resistance and `load` is what follows -- on an AB763 that is a valve
    /// plate into the phase inverter's grid leak. The attenuation is then just
    /// the divider those three make, which is the circuit's answer rather than
    /// a depth parameter.
    pub fn attenuation(&mut self, speed: f64, intensity: f64, source: f64, load: f64) -> f64 {
        let cell = self.resistance(speed, intensity);
        let shunt = cell * load / (cell + load);
        shunt / (source + shunt)
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.lit = 0.0;
        self.struck = false;
    }
}
