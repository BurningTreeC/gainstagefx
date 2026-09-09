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

use crate::circuits::{
    bigmuff, cabinet, clipper, evh5150, iron, markiic, neve, power, preamp, studio, tone, ts808,
};
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
    /// A step-up transformer into a discrete stage. Not a guitar
    /// preamplifier turned down: built so as *not* to run out of room, so
    /// everything interesting happens in the last few decibels before it does.
    Console,
    /// The same channel without the input transformer, and with far more
    /// headroom. The one to reach for when the point is not to hear the
    /// preamplifier.
    Studio,
    // --- modelled from schematics ---------------------------------------
    // The ones above are topologies: a valve cascade, a clipper, a channel.
    // These are particular circuits, parts and values off a drawing, and each
    // is checked against arithmetic done from that drawing.
    /// Ibanez TS808.
    Screamer,
    /// Electro-Harmonix Big Muff Pi, the 1973 Ram's Head.
    Muff,
    /// Mesa Boogie Mark IIC+, the lead channel preamplifier.
    Boogie,
    /// Peavey EVH 5150, the lead channel preamplifier.
    Peavey,
    /// Neve 73P microphone preamplifier, two cascaded transistor stages.
    Neve,
}

impl Gain {
    pub const ALL: [Gain; 12] = [
        Gain::Clean,
        Gain::Crunch,
        Gain::HighGain,
        Gain::Overdrive,
        Gain::Distortion,
        Gain::Console,
        Gain::Studio,
        Gain::Screamer,
        Gain::Muff,
        Gain::Boogie,
        Gain::Peavey,
        Gain::Neve,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Gain::Clean => "Clean",
            Gain::Crunch => "Crunch",
            Gain::HighGain => "High Gain",
            Gain::Overdrive => "Overdrive",
            Gain::Distortion => "Distortion",
            Gain::Console => "Console",
            Gain::Studio => "Studio",
            Gain::Screamer => "TS808",
            Gain::Muff => "Big Muff",
            Gain::Boogie => "Mark IIC+",
            Gain::Peavey => "5150",
            Gain::Neve => "Neve 73P",
        }
    }

    /// Which control on this circuit the Drive knob turns.
    ///
    /// The topologies all put it first because they were written that way.
    /// A modelled circuit puts its controls where the drawing does, and the
    /// Mark IIC+ has treble, bass and middle before its lead drive -- so a
    /// Drive knob that always turned control zero would be turning its treble.
    pub fn drive_control(self) -> usize {
        match self {
            Gain::Boogie => markiic::LEAD_DRIVE,
            Gain::Peavey => evh5150::PRE,
            Gain::Neve => neve::GAIN,
            Gain::Screamer => ts808::DRIVE,
            Gain::Muff => bigmuff::SUSTAIN,
            _ => clipper::GAIN,
        }
    }

    /// The circuit's own tone controls, where it has some: bass, middle,
    /// treble. A Mark IIC+ carries a Fender stack of its own, and using the
    /// plugin's generic one instead of it would be modelling the amplifier
    /// and then ignoring the part everyone recognises.
    pub fn own_tone(self) -> Option<(usize, usize, usize)> {
        match self {
            Gain::Boogie => Some((markiic::BASS, markiic::MIDDLE, markiic::TREBLE)),
            // The TS808 and the Muff have a single tone control, which the
            // Treble knob takes.
            Gain::Screamer => Some((usize::MAX, usize::MAX, ts808::TONE)),
            Gain::Muff => Some((usize::MAX, usize::MAX, bigmuff::TONE)),
            Gain::Neve => None,
            _ => None,
        }
    }

    /// Which of the three tone knobs this circuit carries one of its own for:
    /// bass, middle, treble.
    ///
    /// The panel needs this and `own_tone` will not do, because a control that
    /// is not there is written `usize::MAX` and a caller that forgets to check
    /// sets control number eighteen quintillion.
    pub fn own_tone_knobs(self) -> [bool; 3] {
        match self.own_tone() {
            Some((b, m, t)) => [b != usize::MAX, m != usize::MAX, t != usize::MAX],
            None => [false; 3],
        }
    }

    /// Whether the circuit's only tone control is the single knob a pedal has.
    ///
    /// A TS808 and a Big Muff each have one, and it is not a treble control:
    /// the Screamer's is a shelf either side of a fixed corner and the Muff's
    /// is a blend between a low-pass and a high-pass with a scoop in the
    /// middle. Both land on the third knob, and the panel says TONE there
    /// rather than TREBLE so the knob is named after what it turns.
    pub fn single_tone(self) -> bool {
        self.own_tone_knobs() == [false, false, true]
    }

    /// The *valve* power amplifier behind this circuit, where it has one.
    ///
    /// Only the two guitar amplifiers do. A pedal has no power stage, and the
    /// topologies are shapes rather than particular units, so there is nothing
    /// to put behind them: inventing one would be inventing a sound. The 73P
    /// has an output block of its own and it is not one of these -- see
    /// `build_power`.
    pub fn power_stage(self) -> Option<&'static power::PowerSpec> {
        match self {
            Gain::Boogie => Some(&power::PowerSpec::MARKIIC),
            Gain::Peavey => Some(&power::PowerSpec::EVH5150),
            _ => None,
        }
    }

    /// Whether this is a model of a particular circuit rather than a
    /// topology. The panel says so, because "an overdrive" and "a TS808" are
    /// different kinds of claim.
    pub fn is_modelled(self) -> bool {
        matches!(
            self,
            Gain::Screamer | Gain::Muff | Gain::Boogie | Gain::Peavey | Gain::Neve
        )
    }

    /// Whether the choice of amplifying part reaches this circuit.
    ///
    /// Only the preamplifier channels are built around a part that can be
    /// swapped. The guitar circuits are valve cascades by definition -- a
    /// "three cascaded stages" made of op-amps is a different thing with the
    /// same name -- and the pedals are built around their diodes.
    pub fn has_amplifier(self) -> bool {
        matches!(self, Gain::Console | Gain::Studio)
    }

    /// Whether the diode choice reaches this circuit at all. A valve stage has
    /// no diodes in it, and offering the choice there would be a control that
    /// does nothing -- which is worse than not offering it.
    pub fn has_diodes(self) -> bool {
        matches!(self, Gain::Overdrive | Gain::Distortion)
    }

    /// How many circuits this one covers: one per part it can be built with.
    fn variants(self) -> usize {
        if self.has_diodes() || self.has_amplifier() {
            3
        } else {
            1
        }
    }
}

/// Which part does the amplifying, for the channels built around one.
///
/// The axis the hardware actually varies along, and the one the panel has to
/// expose: a console channel with a bottle in it instead of a transistor is a
/// different and much-argued-about box built from the same schematic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amplifier {
    Valve,
    Jfet,
    OpAmp,
}

impl Amplifier {
    pub const ALL: [Amplifier; 3] = [Amplifier::Valve, Amplifier::Jfet, Amplifier::OpAmp];

    pub fn name(self) -> &'static str {
        match self {
            Amplifier::Valve => "Valve",
            Amplifier::Jfet => "JFET",
            Amplifier::OpAmp => "Op-amp",
        }
    }

    fn index(self) -> usize {
        match self {
            Amplifier::Valve => 0,
            Amplifier::Jfet => 1,
            Amplifier::OpAmp => 2,
        }
    }

    fn spec(self) -> studio::Amplifier {
        match self {
            Amplifier::Valve => studio::Amplifier::Valve,
            Amplifier::Jfet => studio::Amplifier::Jfet,
            Amplifier::OpAmp => studio::Amplifier::OpAmp,
        }
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

    fn index(self) -> usize {
        match self {
            Diode::Silicon => 0,
            Diode::Germanium => 1,
            Diode::Led => 2,
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

/// Every gain circuit that can be selected, as one flat list.
///
/// The plugin builds all of them once and then switches by index, so changing
/// the circuit while playing allocates nothing. Laid out by walking `Gain::ALL`
/// and giving each topology one slot per part it can be built with, so adding
/// a circuit does not move the ones already there.
pub const VOICES: usize = 20;

/// Where a topology's first slot is.
fn first_of(gain: Gain) -> usize {
    Gain::ALL
        .iter()
        .take_while(|g| **g != gain)
        .map(|g| g.variants())
        .sum()
}

/// Where a (circuit, part) combination lives in that list.
pub fn voice_index(gain: Gain, diode: Diode, amplifier: Amplifier) -> usize {
    let within = if gain.has_diodes() {
        diode.index()
    } else if gain.has_amplifier() {
        amplifier.index()
    } else {
        0
    };
    first_of(gain) + within
}

/// The combination at an index, which is the inverse of the above and exists
/// so a table can be built by walking the list.
pub fn voice_at(index: usize) -> (Gain, Diode, Amplifier) {
    let mut at = 0;
    for gain in Gain::ALL {
        let n = gain.variants();
        if index < at + n {
            let within = index - at;
            return (
                gain,
                if gain.has_diodes() {
                    Diode::ALL[within]
                } else {
                    Diode::Silicon
                },
                if gain.has_amplifier() {
                    Amplifier::ALL[within]
                } else {
                    Amplifier::Valve
                },
            );
        }
        at += n;
    }
    (Gain::Clean, Diode::Silicon, Amplifier::Valve)
}

/// The block behind a voice's gain circuit, where it has one.
///
/// Three voices do, and they are not the same kind of thing. The two guitar
/// amplifiers get a push-pull valve power stage from `power.rs`, built from a
/// `PowerSpec`. The 73P gets its own OUTPUT block -- two BC109C into a TIP3055
/// and the VTB1148 -- because a microphone preamplifier's line driver is not a
/// power amplifier and sharing the machinery would mean sharing a topology it
/// does not have.
///
/// Separate netlists rather than one joined onto the end of the other, and the
/// reason is cost. Solving one circuit of thirty-five nodes is not the same
/// work as solving two of eighteen: the factorisation goes as the cube of the
/// size, so joining them costs about three times as much as keeping them
/// apart. What that separation gives up is the loading between the two, and
/// there is almost none to give up -- a coupling capacitor into a high
/// impedance, with a level control in between.
pub fn build_power(gain: Gain) -> Option<Result<Netlist, Fault>> {
    match gain {
        // The card's own load is the line it drives. Ten kilohms is what a
        // modern input presents; the 73P was designed for six hundred, and
        // R43's 1.5 k across the secondary means the difference is small.
        Gain::Neve => Some(neve::output(LOAD, 10_000.0)),
        _ => gain.power_stage().map(|spec| power::build(spec, 10_000.0)),
    }
}

pub fn build_voice(gain: Gain, diode: Diode, amplifier: Amplifier) -> Result<Netlist, Fault> {
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
        Gain::Console | Gain::Studio => {
            let mut v = if gain == Gain::Console {
                studio::CONSOLE
            } else {
                studio::STUDIO
            };
            v.amplifier = amplifier.spec();
            studio::build(&v, SOURCE, LOAD)
        }
        // The modelled circuits take their own source and load, because those
        // are part of what the drawing specifies.
        Gain::Screamer => ts808::build(10_000.0, 470_000.0),
        Gain::Muff => bigmuff::build(&bigmuff::RAMS_HEAD, 10_000.0, 470_000.0),
        Gain::Boogie => markiic::build(10_000.0, 1_000_000.0),
        // Not a nominal load: the 5150's tone stack really does hang 33 k on
        // the far side of R89, and leaving it out would flatter the model by
        // twenty four decibels.
        Gain::Peavey => evh5150::build(10_000.0, evh5150::TONE_STACK_INPUT),
        Gain::Neve => neve::build(150.0, 10_000.0),
    }
}

/// The output transformer, which is a control rather than a property of a
/// circuit: iron belongs after a distortion pedal exactly as much as after a
/// console channel, and there is no reason to offer it on one and not the
/// other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Iron {
    Off,
    /// Bends earliest and most gently -- the only one that colours a quiet
    /// signal at all.
    Nickel,
    /// Stays out of the way and then arrives hard, and goes furthest.
    Steel,
    /// Still clean where the other two are well into it.
    Amorphous,
}

impl Iron {
    pub const ALL: [Iron; 4] = [Iron::Off, Iron::Nickel, Iron::Steel, Iron::Amorphous];

    pub fn name(self) -> &'static str {
        match self {
            Iron::Off => "Off",
            Iron::Nickel => "Nickel",
            Iron::Steel => "Steel",
            Iron::Amorphous => "Amorphous",
        }
    }

    fn index(self) -> Option<usize> {
        match self {
            Iron::Off => None,
            Iron::Nickel => Some(0),
            Iron::Steel => Some(1),
            Iron::Amorphous => Some(2),
        }
    }

    fn values(self) -> Option<iron::Values> {
        let core = match self {
            Iron::Off => return None,
            Iron::Nickel => crate::dsp::netlist::CoreSpec::NICKEL,
            Iron::Steel => crate::dsp::netlist::CoreSpec::STEEL,
            Iron::Amorphous => crate::dsp::netlist::CoreSpec::AMORPHOUS,
        };
        Some(iron::Values {
            core,
            ..iron::OUTPUT
        })
    }
}

/// One output transformer as a netlist, for the calibration example and the
/// chain, which have to build the same thing.
pub fn build_iron(material: Iron) -> Result<Netlist, Fault> {
    let values = material.values().unwrap_or(iron::OUTPUT);
    iron::build(&values, 600.0, 10_000.0)
}

/// How many volts a unit of digital signal becomes at the iron stage.
///
/// The one place in the plugin where a level has to be chosen rather than
/// measured. Flux is the integral of voltage, so what the core does depends
/// on how many volts it is handed -- and unlike the gain circuits, which are
/// calibrated so a nominal signal drives them the way their name says, the
/// iron stage sits after the make-up and sees a known level already. Set so
/// that a nominal signal at 40 Hz lands about at steel's knee: nickel is
/// already bending there, amorphous has not started, and pushing the drive
/// into it takes all three further.
pub const IRON_VOLTS: f64 = 24.0;

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
pub const POINTS: usize = 17;

/// How the make-up's sample points are spread across the drive control.
///
/// Not evenly, and the reason is that no drive control is even. A pot's law is
/// logarithmic and the stage after it saturates, so a circuit's gain climbs
/// most of its range inside the first eighth of the travel and then flattens:
/// the Mark IIC+ moves 47 dB between a shut Lead Drive and an eighth of a turn,
/// and 9 dB over the remaining seven eighths. Nine points evenly spaced sample
/// that first cliff exactly twice, and a straight line between those two
/// samples was **24 dB** away from the curve at a knob position of 0.03 --
/// which is heard as the level lurching as the knob comes off its stop, and
/// was reported as "at 0% Gain the meter goes way UP".
///
/// More even points do not fix it. The curve goes as `log(position)` near the
/// bottom, so a straight line across the first segment is wrong by an amount
/// that does not shrink usefully however narrow the segment gets.
///
/// So the points are spread by a cube law: knot `i` sits at
/// `(i / (POINTS - 1))^3`. That puts eight of the seventeen inside the first
/// eighth of the travel, where the gain is, and leaves the flat top end
/// sampled coarsely, where coarse is all it needs.
const KNOT_SHAPE: f64 = 3.0;

/// Where the make-up's `i`th sample point sits on the drive control.
pub fn knot_position(i: usize) -> f64 {
    (i as f64 / (POINTS - 1) as f64).powf(KNOT_SHAPE)
}

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
        // Into knot space, which is where the points are evenly spaced. The
        // cube root is the inverse of `knot_position`.
        let x = drive.clamp(0.0, 1.0).powf(1.0 / KNOT_SHAPE) * (POINTS - 1) as f64;
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

/// What the plugin tells the host it delays by, whatever the oversampling is
/// set to.
///
/// The filters are shorter at the lower settings -- nothing at all with
/// oversampling off, 160 samples at two times, 176 at four, 180 at eight -- so
/// the honest figure would change as the control moves. The CLAP specification
/// asks that it does not, and a host that has to renegotiate its delay
/// compensation mid-stream will click. So one figure is reported and the
/// shorter settings are padded up to it.
pub const LATENCY: u32 = 180;

/// Number of samples to crossfade when switching circuits or presets.
/// At 48 kHz this is about 5.3 ms -- long enough to mask the capacitor
/// reset discontinuity even for high-gain circuits like the Mark IIC+,
/// short enough to be imperceptible as a level dip.
const FADE_LEN: usize = 256;

/// A whole number of samples of delay.
struct Delay {
    buf: Vec<f64>,
    pos: usize,
}

impl Delay {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len],
            pos: 0,
        }
    }

    /// A length of zero means no delay at all, and has to mean that.
    ///
    /// Rounding it up to one sample instead is not a rounding: at eight times
    /// oversampling the padding is exactly zero, so the wet path came out one
    /// sample later than the dry path it is mixed against and one later than
    /// the host had been told. A one sample offset between two copies of the
    /// same signal is a comb filter, and the reported latency was wrong at
    /// that setting and right at every other.
    fn set_len(&mut self, len: usize) {
        if len != self.buf.len() {
            self.buf.clear();
            self.buf.resize(len, 0.0);
            self.pos = 0;
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        if self.buf.is_empty() {
            return x;
        }
        let out = self.buf[self.pos];
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
}

/// Every panel setting that reaches the circuits, as plain values.
///
/// This exists so the step from "what the panel says" to "what the chain is
/// set to" is one function that can be tested. It was not, and the four
/// controls that move a circuit -- drive, bass, mid, treble -- were silently
/// dropped from the plugin's block loop in an edit. Nothing noticed, because
/// every test drove the chain directly and none of them went through the
/// plugin at all: the knobs simply did nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub gain: Gain,
    pub diode: Diode,
    pub amplifier: Amplifier,
    pub iron: Iron,
    pub tone: Tone,
    pub cabinet: Cabinet,
    pub drive: f64,
    pub bass: f64,
    pub mid: f64,
    pub treble: f64,
    pub oversampling: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gain: Gain::Crunch,
            diode: Diode::Silicon,
            amplifier: Amplifier::Jfet,
            iron: Iron::Off,
            tone: Tone::Wide,
            cabinet: Cabinet::Off,
            drive: 0.5,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            oversampling: 2,
        }
    }
}

/// The whole signal path, in the order the signal goes through it.
///
/// It owns *every* circuit that can be selected and switches by index, rather
/// than building the one that is wanted. Choosing a circuit is something a
/// player does while playing, on the audio thread, where allocating is not
/// allowed -- and there are only thirteen small circuits in the catalogue, so
/// holding all of them costs less than the machinery to avoid it would.
pub struct Chain {
    gains: Vec<Simulation>,
    /// The power amplifier behind each voice that has one, indexed alongside
    /// `gains`. Two of them are `Some`; the rest are pedals and topologies,
    /// which have nothing behind them.
    powers: Vec<Option<Simulation>>,
    /// The three output transformers. Nonlinear, so unlike the tone stack and
    /// the cabinet these cannot be normalised by asking the AC solver: their
    /// trim is measured and baked with the voices.
    irons: Vec<Simulation>,
    /// Each linear section with the trim that undoes its own loss.
    tones: Vec<(Simulation, f64)>,
    cabinets: Vec<(Simulation, f64)>,
    gain: usize,
    iron: Option<usize>,
    tone: Option<usize>,
    cabinet: Option<usize>,
    over: Oversampler,
    /// The panel's requested factor. The active factor can be lower for a
    /// modelled circuit, but this is retained so switching back restores the
    /// user's choice.
    requested_oversampling: usize,
    /// Brings the wet path up to `LATENCY` whatever the oversampling is.
    pad: Delay,
    /// Holds the dry signal back by the same amount, so that mixing the two
    /// is a mix rather than a comb filter.
    dry: Delay,
    /// The host's rate. Modelled circuits always run at this rate; see
    /// `set_oversampling`.
    rate: f64,
    drive: f64,
    /// Volts in per unit of digital signal, and digital signal out per volt.
    into: f64,
    out_of: f64,
    /// Where `out_of` is heading. The make-up moves with the drive control,
    /// and the drive control now moves once a block rather than once a sample
    /// -- so the make-up is glided rather than stepped, which costs three
    /// arithmetic operations and saves a click on every automation step.
    out_of_target: f64,
    /// Crossfade state for circuit/preset switches. When a switch is detected,
    /// the output is faded from the last sample of the old circuit to the first
    /// sample of the new one over `FADE_LEN` samples, masking the capacitor
    /// reset discontinuity.
    fade_remaining: usize,
    prev_output: f64,
    /// When a circuit switch also changes the oversampling factor, the
    /// oversampler reset is deferred until after the crossfade completes.
    /// Resetting the oversampler mid-crossfade zeroes its filter histories,
    /// creating a second discontinuity that the crossfade cannot mask.
    deferred_oversample: Option<usize>,
}

impl Chain {
    pub fn new(rate: f64) -> Self {
        let section = |built: Option<Result<Netlist, Fault>>| {
            let netlist = built
                .expect("a section that exists")
                .expect("catalogue builds");
            let controls = vec![0.5; netlist.controls];
            let trim = 1.0 / peak_gain(&netlist, &controls);
            let mut sim = Simulation::new(netlist, rate);
            for (i, p) in controls.iter().enumerate() {
                sim.set_control(i, *p);
            }
            (sim, trim)
        };
        let mut chain = Self {
            gains: (0..VOICES)
                .map(|i| {
                    let (gain, diode, amplifier) = voice_at(i);
                    Simulation::new(
                        build_voice(gain, diode, amplifier).expect("catalogue builds"),
                        rate,
                    )
                })
                .collect(),
            powers: (0..VOICES)
                .map(|i| {
                    build_power(voice_at(i).0)
                        .map(|built| Simulation::new(built.expect("catalogue builds"), rate))
                })
                .collect(),
            irons: [Iron::Nickel, Iron::Steel, Iron::Amorphous]
                .into_iter()
                .map(|i| Simulation::new(build_iron(i).expect("catalogue builds"), rate))
                .collect(),
            tones: [Tone::Wide, Tone::Scooping]
                .into_iter()
                .map(|t| section(t.build()))
                .collect(),
            cabinets: [Cabinet::Combo, Cabinet::Stack]
                .into_iter()
                .map(|c| section(c.build()))
                .collect(),
            gain: 0,
            iron: None,
            tone: None,
            cabinet: None,
            over: Oversampler::new(4),
            requested_oversampling: 4,
            pad: Delay::new(1),
            dry: Delay::new(LATENCY as usize),
            rate,
            drive: 0.5,
            into: 1.0,
            out_of: 1.0,
            out_of_target: 1.0,
            fade_remaining: 0,
            prev_output: 0.0,
            deferred_oversample: None,
        };
        chain.set_oversampling(4);
        chain.set_drive(0.5);
        chain.settle();
        chain
    }

    /// Which gain circuit is in the path. Switching resets the one being
    /// switched to: it has been sitting with whatever charge was on its
    /// capacitors when it was last used, and a valve plate holds two hundred
    /// volts of it.
    pub fn set_voice(&mut self, gain: Gain, diode: Diode, amplifier: Amplifier) {
        let index = voice_index(gain, diode, amplifier);
        if index != self.gain {
            self.gain = index;
            self.gains[index].reset();
            // And its power stage, for the same reason and more so. A power
            // stage sits at four hundred volts with its output transformer
            // carrying the plates' standing current, and `set_voice` used to
            // leave all of it alone: switching back to an amplifier handed a
            // freshly settled preamp to a power stage still holding whatever
            // charge and flux it had when it was last switched away from.
            if let Some(sim) = self.powers[index].as_mut() {
                sim.reset();
            }
            self.set_oversampling(self.requested_oversampling);
            self.set_drive(self.drive);
            // The make-up belongs to the voice, so it changes with the voice
            // rather than gliding there.
            //
            // `out_of` glides because the make-up moves with the *drive*
            // knob, and a knob is a continuum. Two voices are not: the make-up
            // converts that circuit's own output -- which for an amplifier is
            // amplifier volts, tens of them after the power stage, and for a
            // pedal is something near one -- into the digital domain. Gliding
            // between them applies one voice's conversion to another voice's
            // output for as long as the glide lasts.
            //
            // Measured on Big Muff -> 5150: a peak of 20.25 against a settled
            // 0.125, a hundred and sixty times over, building for four
            // milliseconds and over full scale for three of them. That is the
            // reported spike. `examples/switching.rs` is the measurement.
            self.out_of = self.out_of_target;
            // Crossfade from the old circuit's last output to the new
            // circuit's first output so the capacitor-reset discontinuity
            // is inaudible.
            self.fade_remaining = FADE_LEN;
        }
    }

    /// Which output transformer, if any.
    pub fn set_iron(&mut self, iron: Iron) {
        let next = iron.index();
        if next != self.iron {
            if let Some(i) = next {
                self.irons[i].reset();
            }
            self.iron = next;
            self.fade_remaining = FADE_LEN;
        }
    }

    pub fn set_tone_section(&mut self, tone: Tone) {
        let next = match tone {
            Tone::Off => None,
            Tone::Wide => Some(0),
            Tone::Scooping => Some(1),
        };
        if next != self.tone {
            if let Some(i) = next {
                self.tones[i].0.reset();
            }
            self.tone = next;
            self.fade_remaining = FADE_LEN;
        }
    }

    pub fn set_cabinet(&mut self, cabinet: Cabinet) {
        let next = match cabinet {
            Cabinet::Off => None,
            Cabinet::Combo => Some(0),
            Cabinet::Stack => Some(1),
        };
        if next != self.cabinet {
            if let Some(i) = next {
                self.cabinets[i].0.reset();
            }
            self.cabinet = next;
            self.fade_remaining = FADE_LEN;
        }
    }

    /// The drive control, which moves both the circuit and the make-up that
    /// keeps the comparison honest.
    ///
    /// Call this once a block, not once a sample. Moving a circuit control
    /// invalidates the matrix, and the next sample then rebuilds it and hunts
    /// the operating point again -- which at audio rate is a rebuild and a DC
    /// solve forty-eight thousand times a second. Measured, that is most of
    /// what the plugin costs while a knob is moving.
    pub fn set_drive(&mut self, drive: f64) {
        self.drive = drive.clamp(0.0, 1.0);
        let which = voice_at(self.gain).0.drive_control();
        self.gains[self.gain].set_control(which, self.drive);
        let calibration = CALIBRATION[self.gain];
        // A nominal digital signal has to arrive as the stated voltage.
        let nominal = 10f64.powf(NOMINAL_DBFS / 20.0);
        self.into = calibration.drive_volts / nominal;
        self.out_of_target = 10f64.powf(calibration.make_up_db_at(self.drive) / 20.0) / self.into;
    }

    pub fn set_tone(&mut self, which: usize, position: f64) {
        if let Some(i) = self.tone {
            self.tones[i].0.set_control(which, position);
        }
    }

    /// The three tone knobs, sent to whichever stack is actually in the path:
    /// the circuit's own where it has one, the plugin's otherwise.
    fn set_tone_knobs(&mut self, bass: f64, mid: f64, treble: f64) {
        if let Some((b, m, t)) = voice_at(self.gain).0.own_tone() {
            let sim = &mut self.gains[self.gain];
            for (which, value) in [(b, bass), (m, mid), (t, treble)] {
                if which != usize::MAX {
                    sim.set_control(which, value);
                }
            }
        }
        self.set_tone(crate::circuits::tone::BASS, bass);
        self.set_tone(crate::circuits::tone::MID, mid);
        self.set_tone(crate::circuits::tone::TREBLE, treble);
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
        self.requested_oversampling = factor;
        let factor = if voice_at(self.gain).0.is_modelled() {
            1
        } else {
            factor
        };
        // If a crossfade is active, defer the oversampler reset until it
        // completes. Resetting mid-crossfade zeroes the filter histories and
        // creates a second discontinuity that the crossfade cannot mask.
        if self.fade_remaining > 0 && factor != self.over.factor() {
            self.deferred_oversample = Some(factor);
        } else {
            self.over.set_factor(factor);
        }
        self.pad
            .set_len((LATENCY - self.over.latency().min(LATENCY)) as usize);
        let inner = self.rate * self.over.factor() as f64;
        for sim in self.gains.iter_mut().chain(self.irons.iter_mut()) {
            sim.set_rate(inner);
        }
        // The power stage is outside the oversampler, so it keeps the host's
        // rate rather than the inner one. See the note in `process`.
        for sim in self.powers.iter_mut().flatten() {
            sim.set_rate(self.rate);
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
        for (sim, _) in self.tones.iter_mut().chain(self.cabinets.iter_mut()) {
            sim.set_rate(rate);
        }
        self.set_oversampling(self.requested_oversampling);
    }

    /// The factor actually used by the nonlinear gain path. Modelled circuits
    /// deliberately report one: they are kept at the host rate so they remain
    /// safe to use live even when the global control is set higher.
    pub fn effective_oversampling(&self) -> usize {
        self.over.factor()
    }

    /// How many Newton passes the gain circuit is averaging, which is the
    /// number that says whether a setting is expensive because the circuit is
    /// large or because the solve is struggling.
    pub fn passes_per_sample(&self) -> f64 {
        let (solves, passes, _, _) = self.gains[self.gain].statistics();
        if solves == 0 {
            0.0
        } else {
            passes as f64 / solves as f64
        }
    }

    /// Puts a whole panel's worth of settings onto the chain.
    ///
    /// Every one of them, in one place. Anything that reaches a circuit has to
    /// go through here, so that forgetting one is a change to this function
    /// rather than a line quietly missing from a loop somewhere.
    pub fn apply(&mut self, s: &Settings) {
        self.set_voice(s.gain, s.diode, s.amplifier);
        self.set_iron(s.iron);
        self.set_tone_section(s.tone);
        self.set_cabinet(s.cabinet);
        self.set_oversampling(s.oversampling);
        self.set_drive(s.drive);
        self.set_tone_knobs(s.bass, s.mid, s.treble);
    }

    /// One figure, always. See `LATENCY`.
    pub fn latency(&self) -> u32 {
        LATENCY
    }

    /// The dry signal, held back so it lines up with what `process` returns.
    #[inline]
    pub fn delayed_dry(&mut self, x: f64) -> f64 {
        self.dry.process(x)
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        // Apply any deferred oversampler reset at the START of the crossfade,
        // before the oversampler processes this sample. This way the entire
        // reset discontinuity is covered by the crossfade blending.
        if self.fade_remaining == FADE_LEN {
            if let Some(factor) = self.deferred_oversample.take() {
                self.over.set_factor(factor);
                self.pad
                    .set_len((LATENCY - self.over.latency().min(LATENCY)) as usize);
                let inner = self.rate * self.over.factor() as f64;
                for sim in self.gains.iter_mut().chain(self.irons.iter_mut()) {
                    sim.set_rate(inner);
                }
            }
        }

        // Only the gain circuit can fold anything back into the band, so only
        // the gain circuit runs at the higher rate. The tone stack and the
        // cabinet are linear and cost nothing extra by staying down here.
        // The iron runs inside the oversampling with the gain circuit, not
        // after it: a saturating core is as nonlinear as anything else here.
        // One pole toward the target: about a millisecond at any sample rate
        // the plugin is likely to see.
        self.out_of += (self.out_of_target - self.out_of) * 0.02;
        let gain = &mut self.gains[self.gain];
        let iron_trim = self.iron.map(|i| IRON_TRIM[i]).unwrap_or(1.0);
        let mut iron = self.iron.map(|i| &mut self.irons[i]);
        let out_of = self.out_of;
        let mut power = self.powers[self.gain].as_mut();
        let mut y = self.over.process(x * self.into, &mut |v| {
            // The power amplifier goes here, in volts, *before* the make-up.
            //
            // Not after it, which is where the signal order would otherwise
            // put it. The make-up holds the preamplifier's output at a
            // constant level whatever the Drive knob is doing -- that is its
            // whole job -- so a power stage behind it would be handed the same
            // level at every setting and would never be driven any harder.
            // Which is the one thing a power amplifier is for.
            //
            // What that costs is the plugin's tone section, which then sits
            // *after* the power stage rather than before it. For the 5150 that
            // section stands in for the amplifier's own stack, so its loss
            // lands in the wrong place; the master volume absorbs the level,
            // and the voicing being post-power is an approximation worth
            // naming. It also means the power stage runs at the host rate
            // rather than inside the oversampling -- which costs nothing
            // today, because a modelled circuit is pinned to the host rate
            // anyway (see `set_oversampling` and BUG-017), and would have to
            // move if that ever changed.
            let mut amplified = gain.process(v);
            if let Some(ref mut sim) = power {
                amplified = sim.process(amplified);
            }
            let amplified = amplified * out_of;
            match iron {
                Some(ref mut sim) => {
                    // Handed volts rather than a number near one, because what
                    // a core does depends on the flux and flux is in volt
                    // seconds. See `IRON_VOLTS`.
                    sim.process(amplified * IRON_VOLTS) * iron_trim / IRON_VOLTS
                }
                None => amplified,
            }
        });
        // Every setting delays by the same reported amount.
        y = self.pad.process(y);
        if let Some(i) = self.tone {
            let (sim, trim) = &mut self.tones[i];
            y = sim.process(y) * *trim;
        }
        if let Some(i) = self.cabinet {
            let (sim, trim) = &mut self.cabinets[i];
            y = sim.process(y) * *trim;
        }
        // Crossfade from the old circuit's last output to the new one so that
        // a capacitor-reset discontinuity is inaudible.
        if self.fade_remaining > 0 {
            let t = self.fade_remaining as f64 / FADE_LEN as f64;
            self.fade_remaining -= 1;
            y = self.prev_output * t + y * (1.0 - t);
        }
        self.prev_output = y;
        y
    }

    /// Settles the make-up where it is heading, for a chain that has just
    /// been set up and has no glide to do.
    pub fn settle(&mut self) {
        self.out_of = self.out_of_target;
    }

    /// The operating point voltage vector of the active gain circuit, for
    /// sharing with an identical channel.
    pub fn operating_point(&self) -> &[f64] {
        self.gains[self.gain].operating_point()
    }

    /// The operating point voltage vector of the active iron stage, if any.
    ///
    /// A borrow rather than a copy: this is read from `process`, and a `Vec`
    /// there is an allocation on the audio thread.
    pub fn iron_operating_point(&self) -> Option<&[f64]> {
        self.iron.map(|i| self.irons[i].operating_point())
    }

    /// Whether the active gain circuit still has its DC hunt in front of it,
    /// which is the only moment at which its solution is worth sharing. See
    /// `Simulation::needs_operating_point`.
    pub fn needs_operating_point(&self) -> bool {
        self.gains[self.gain].needs_operating_point()
    }

    /// The operating point of the active voice's power stage, if it has one.
    pub fn power_operating_point(&self) -> Option<&[f64]> {
        self.powers[self.gain].as_ref().map(|s| s.operating_point())
    }

    /// Apply a pre-computed operating point to the active voice's power stage.
    pub fn share_power_operating_point_from(&mut self, op: &[f64]) {
        if let Some(sim) = self.powers[self.gain].as_mut() {
            sim.apply_operating_point(op);
        }
    }

    /// Hunts the operating point now rather than on the next sample, so that
    /// an identical channel can be handed the answer.
    pub fn find_operating_point(&mut self) -> bool {
        let gain = self.gains[self.gain].find_operating_point();
        let iron = self.iron.map(|i| self.irons[i].find_operating_point());
        let power = self.powers[self.gain]
            .as_mut()
            .map(|s| s.find_operating_point());
        gain && iron.unwrap_or(true) && power.unwrap_or(true)
    }

    /// Apply a pre-computed operating point to the active gain circuit.
    /// Used when sharing a DC solution between identical channels.
    pub fn share_operating_point_from(&mut self, gain_op: &[f64]) {
        self.gains[self.gain].apply_operating_point(gain_op);
    }

    /// Apply a pre-computed operating point to the active iron stage.
    pub fn share_iron_operating_point_from(&mut self, iron_op: &[f64]) {
        if let Some(i) = self.iron {
            self.irons[i].apply_operating_point(iron_op);
        }
    }

    pub fn reset(&mut self) {
        self.out_of = self.out_of_target;
        for sim in self
            .gains
            .iter_mut()
            .chain(self.irons.iter_mut())
            .chain(self.powers.iter_mut().flatten())
        {
            sim.reset();
        }
        for (sim, _) in self.tones.iter_mut().chain(self.cabinets.iter_mut()) {
            sim.reset();
        }
        self.over.reset();
        self.pad.reset();
        self.dry.reset();
    }
}
