//! A circuit made playable.
//!
//! Between a netlist and a plugin there is a gap that the first attempt at
//! this fell into twice, so both sides of it are settled here before anything
//! is wired to a knob.
//!
//! **A circuit does not run at digital levels.** A guitar arrives at an
//! interface at a peak of a few tenths of a volt and leaves as a number near
//! one. Feed that number to a valve grid and it is a hundred times too large;
//! feed the same number to a three stage cascade with a small signal gain of
//! nearly six thousand and there is nothing to hear but a square wave. Each
//! voice therefore states the voltage a nominal digital signal should arrive
//! at, and that is the only place the two worlds are joined.
//!
//! **The make-up cannot be measured while playing.** It has to follow the
//! drive control, because otherwise every comparison between two settings is
//! just picking the louder one -- but measuring it costs thousands of solves,
//! and the first attempt spent about fifty five times the entire audio budget
//! doing exactly that on every block. The measurement is real work and it is
//! done here in an example, printed as a table, and pasted in as constants;
//! `tests/voice.rs` re-measures the table and fails if the circuits have moved
//! away from it. So the numbers are measured, and the audio thread only ever
//! interpolates five of them.

use crate::circuits::{cabinet, clipper, preamp, tone};
use crate::dsp::ac;
use crate::dsp::netlist::{Circuit as Netlist, DiodeSpec, Fault};
use crate::dsp::oversample::Oversampler;
use crate::dsp::time::Simulation;

/// The level a plugin should be set up around: hot enough to be well clear of
/// the noise floor, quiet enough to leave headroom for a peak. A guitar
/// tracked sensibly sits about here.
pub const NOMINAL_DBFS: f64 = -18.0;

/// The impedance a stage is driven from and drives into. Real values, so that
/// a stage loads the one before it the way the hardware does rather than
/// every stage pretending it is driven by a perfect source.
pub const SOURCE: f64 = 10_000.0;
pub const LOAD: f64 = 470_000.0;

/// What makes the gain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gain {
    /// One valve stage barely working: the sound of a signal having been
    /// through something rather than the sound of distortion.
    Clean,
    /// Two stages, the second driven by the first.
    Crunch,
    /// Three stages. This is where the gain stops being a texture.
    HighGain,
    /// Diodes in the feedback loop, which lower the gain rather than stopping
    /// the output.
    Overdrive,
    /// Diodes to ground, which are a ceiling.
    Distortion,
}

impl Gain {
    pub const ALL: [Gain; 5] = [
        Gain::Clean,
        Gain::Crunch,
        Gain::HighGain,
        Gain::Overdrive,
        Gain::Distortion,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Gain::Clean => "Clean",
            Gain::Crunch => "Crunch",
            Gain::HighGain => "High Gain",
            Gain::Overdrive => "Overdrive",
            Gain::Distortion => "Distortion",
        }
    }

    /// Whether the diode choice reaches this circuit at all. A valve stage has
    /// no diodes in it, and offering the choice there would be a control that
    /// does nothing -- which is worse than not offering it.
    pub fn has_diodes(self) -> bool {
        matches!(self, Gain::Overdrive | Gain::Distortion)
    }
}

/// Which diodes, for the circuits that have any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diode {
    Silicon,
    Germanium,
    Led,
}

impl Diode {
    pub const ALL: [Diode; 3] = [Diode::Silicon, Diode::Germanium, Diode::Led];

    pub fn name(self) -> &'static str {
        match self {
            Diode::Silicon => "Silicon",
            Diode::Germanium => "Germanium",
            Diode::Led => "LED",
        }
    }

    fn spec(self) -> DiodeSpec {
        match self {
            Diode::Silicon => DiodeSpec::SILICON,
            Diode::Germanium => DiodeSpec::GERMANIUM,
            Diode::Led => DiodeSpec::LED,
        }
    }
}

/// Every gain circuit that can be selected, as one flat list. The plugin
/// builds all of them once and then switches by index, so changing the
/// circuit while playing allocates nothing.
pub const VOICES: usize = 3 + 2 * 3;

/// Where a (circuit, diode) pair lives in that list.
pub fn voice_index(gain: Gain, diode: Diode) -> usize {
    let d = match diode {
        Diode::Silicon => 0,
        Diode::Germanium => 1,
        Diode::Led => 2,
    };
    match gain {
        Gain::Clean => 0,
        Gain::Crunch => 1,
        Gain::HighGain => 2,
        Gain::Overdrive => 3 + d,
        Gain::Distortion => 6 + d,
    }
}

/// The (circuit, diode) pair at an index, which is the inverse of the above
/// and exists so a table can be built by walking the list.
pub fn voice_at(index: usize) -> (Gain, Diode) {
    match index {
        0 => (Gain::Clean, Diode::Silicon),
        1 => (Gain::Crunch, Diode::Silicon),
        2 => (Gain::HighGain, Diode::Silicon),
        i if i < 6 => (Gain::Overdrive, Diode::ALL[i - 3]),
        i => (Gain::Distortion, Diode::ALL[i - 6]),
    }
}

/// Build one gain circuit as a netlist.
pub fn build_voice(gain: Gain, diode: Diode) -> Result<Netlist, Fault> {
    match gain {
        Gain::Clean => preamp::build(&preamp::CLEAN, SOURCE, LOAD),
        Gain::Crunch => preamp::build(&preamp::CRUNCH, SOURCE, LOAD),
        Gain::HighGain => preamp::build(&preamp::HIGH_GAIN, SOURCE, LOAD),
        Gain::Overdrive | Gain::Distortion => {
            let mut v = if gain == Gain::Overdrive {
                clipper::OVERDRIVE
            } else {
                clipper::DISTORTION
            };
            v.diode = diode.spec();
            clipper::build(&v, SOURCE, LOAD)
        }
    }
}

/// The tone section, which can be out of circuit entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Off,
    /// Wide open and roughly flat in the middle: a tone control rather than a
    /// voicing.
    Wide,
    /// The scooped voicing, which is the one that makes the sound the whole
    /// exercise is aimed at.
    Scooping,
}

impl Tone {
    pub const ALL: [Tone; 3] = [Tone::Off, Tone::Wide, Tone::Scooping];

    pub fn name(self) -> &'static str {
        match self {
            Tone::Off => "Off",
            Tone::Wide => "Wide",
            Tone::Scooping => "Scooping",
        }
    }

    pub fn build(self) -> Option<Result<Netlist, Fault>> {
        match self {
            Tone::Off => None,
            Tone::Wide => Some(tone::build(&tone::WIDE, SOURCE, LOAD)),
            Tone::Scooping => Some(tone::build(&tone::SCOOPING, SOURCE, LOAD)),
        }
    }
}

/// The speaker, which can also be out of circuit -- and has to be, because a
/// cabinet in front of a preamplifier sound is wrong and a high gain sound
/// without one is unlistenable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cabinet {
    Off,
    Combo,
    Stack,
}

impl Cabinet {
    pub const ALL: [Cabinet; 3] = [Cabinet::Off, Cabinet::Combo, Cabinet::Stack];

    pub fn name(self) -> &'static str {
        match self {
            Cabinet::Off => "Off",
            Cabinet::Combo => "Combo",
            Cabinet::Stack => "Stack",
        }
    }

    pub fn build(self) -> Option<Result<Netlist, Fault>> {
        match self {
            Cabinet::Off => None,
            Cabinet::Combo => Some(cabinet::build(&cabinet::COMBO, SOURCE)),
            Cabinet::Stack => Some(cabinet::build(&cabinet::STACK, SOURCE)),
        }
    }
}

/// What a nominal digital signal has to become, and what comes back.
///
/// The make-up is a curve across the drive control rather than one number,
/// because the drive control moves the gain of these circuits by about eighty
/// decibels end to end. Nine points, not five: five leaves twenty decibels
/// between neighbours, and a straight line drawn across twenty decibels of a
/// curve is not close enough to call the level held. `tests/voice.rs` checks
/// the interpolation, not just the points.
/// How many points the make-up curve is measured at.
pub const POINTS: usize = 9;

#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    /// Volts at the circuit's input for a signal at `NOMINAL_DBFS`.
    pub drive_volts: f64,
    /// Output make-up in dB at nine evenly spaced drive positions, the first
    /// at 0 and the last at 1.
    pub make_up_db: [f64; POINTS],
}

impl Calibration {
    /// The make-up at a drive position, between the measured points.
    pub fn make_up_db_at(&self, drive: f64) -> f64 {
        let x = drive.clamp(0.0, 1.0) * (POINTS - 1) as f64;
        let i = (x as usize).min(POINTS - 2);
        let f = x - i as f64;
        self.make_up_db[i] * (1.0 - f) + self.make_up_db[i + 1] * f
    }
}

include!("calibration.rs");

/// The peak gain a linear section has anywhere in the audio band, at the
/// control positions given.
///
/// A passive tone stack and a speaker are both nothing but loss -- a stack can
/// only ever cut, which is why an amplifier has a gain stage after it -- and
/// switching one in would otherwise drop the whole plugin by twenty decibels.
/// Because they are linear, this is not a matter of playing audio through them
/// and taking a spectrum: the solver can be asked for the answer directly, one
/// exact complex number per frequency, in microseconds.
///
/// It is the peak rather than the level at some nominal frequency because the
/// point is headroom: normalising to a dip would push the peak into clipping.
/// This runs when a *section* is chosen, never when a knob moves, so the tone
/// controls shift the level as they do on the hardware.
fn peak_gain(circuit: &Netlist, controls: &[f64]) -> f64 {
    const STEPS: usize = 120;
    let (low, high) = (20.0f64, 16_000.0f64);
    let mut peak: f64 = 0.0;
    for i in 0..=STEPS {
        let hz = low * (high / low).powf(i as f64 / STEPS as f64);
        peak = peak.max(ac::solve(circuit, controls, hz).magnitude());
    }
    peak
}

/// The whole signal path, in the order the signal goes through it.
pub struct Chain {
    gain: Simulation,
    /// Each linear section with the trim that undoes its own loss.
    tone: Option<(Simulation, f64)>,
    cabinet: Option<(Simulation, f64)>,
    over: Oversampler,
    calibration: Calibration,
    /// The host's rate. The gain circuit does not run at it -- see
    /// `set_oversampling`.
    rate: f64,
    drive: f64,
    /// Volts in per unit of digital signal, and digital signal out per volt.
    into: f64,
    out_of: f64,
}

impl Chain {
    pub fn new(gain: Gain, diode: Diode, tone: Tone, cabinet: Cabinet, rate: f64) -> Self {
        let calibration = CALIBRATION[voice_index(gain, diode)];
        let section = |built: Option<Result<Netlist, Fault>>| {
            built.map(|c| {
                let netlist = c.expect("catalogue builds");
                let controls = vec![0.5; netlist.controls];
                let trim = 1.0 / peak_gain(&netlist, &controls);
                let mut sim = Simulation::new(netlist, rate);
                for (i, p) in controls.iter().enumerate() {
                    sim.set_control(i, *p);
                }
                (sim, trim)
            })
        };
        let mut chain = Self {
            gain: Simulation::new(build_voice(gain, diode).expect("catalogue builds"), rate),
            tone: section(tone.build()),
            cabinet: section(cabinet.build()),
            over: Oversampler::new(4),
            calibration,
            rate,
            drive: 0.5,
            into: 1.0,
            out_of: 1.0,
        };
        chain.set_oversampling(4);
        chain.set_drive(0.5);
        chain
    }

    /// The drive control, which moves both the circuit and the make-up that
    /// keeps the comparison honest.
    pub fn set_drive(&mut self, drive: f64) {
        self.drive = drive.clamp(0.0, 1.0);
        self.gain.set_control(clipper::GAIN, self.drive);
        // A nominal digital signal has to arrive as the stated voltage.
        let nominal = 10f64.powf(NOMINAL_DBFS / 20.0);
        self.into = self.calibration.drive_volts / nominal;
        self.out_of =
            10f64.powf(self.calibration.make_up_db_at(self.drive) / 20.0) / self.into;
    }

    pub fn set_tone(&mut self, which: usize, position: f64) {
        if let Some((t, _)) = self.tone.as_mut() {
            t.set_control(which, position);
        }
    }

    /// Sets the oversampling factor -- and tells the circuit about it.
    ///
    /// These two have to move together. The oversampler hands the circuit four
    /// samples for every one the host sent, so a circuit still solving with
    /// the host's timestep has every capacitor in it four times too slow and
    /// every corner frequency four times too high. Measured, that put the
    /// overdrive's mid-hump at 2.9 kHz instead of 720 Hz and cost eleven
    /// decibels at the level the calibration had promised -- and it cost the
    /// valve voices almost nothing, so nothing but the clipper would have
    /// shown it.
    pub fn set_oversampling(&mut self, factor: usize) {
        self.over.set_factor(factor);
        self.gain.set_rate(self.rate * self.over.factor() as f64);
    }

    pub fn latency(&self) -> u32 {
        self.over.latency()
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        // Only the gain circuit can fold anything back into the band, so only
        // the gain circuit runs at the higher rate. The tone stack and the
        // cabinet are linear and cost nothing extra by staying down here.
        let gain = &mut self.gain;
        let mut y = self
            .over
            .process(x * self.into, &mut |v| gain.process(v))
            * self.out_of;
        if let Some((t, trim)) = self.tone.as_mut() {
            y = t.process(y) * *trim;
        }
        if let Some((c, trim)) = self.cabinet.as_mut() {
            y = c.process(y) * *trim;
        }
        y
    }

    pub fn reset(&mut self) {
        self.gain.reset();
        if let Some((t, _)) = self.tone.as_mut() {
            t.reset();
        }
        if let Some((c, _)) = self.cabinet.as_mut() {
            c.reset();
        }
        self.over.reset();
    }
}
