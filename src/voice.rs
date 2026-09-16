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

use crate::acoustics::cabinet::CabinetProfile;
use crate::acoustics::mic::{MicPlacement, MicProfile};
use crate::acoustics::speaker::{self, LoadSlots, LoadValues, Mounting, SpeakerProfile};
use crate::acoustics::stage::{AcousticStage, MicSlot};
use crate::circuits::{
    ac30, american312, bigmuff, brit800, cabinet, clipper, console_e, distortion_plus, dr103,
    evh5150, heavy_metal, iron, markiic, metal_zone, neve, plexi, power, preamp, rectifier, rodent, round_fuzz, studio, tone, ts808, tube610, twin,
};
use crate::dsp::ac;
use crate::dsp::netlist::{Circuit as Netlist, DiodeSpec, Fault};
use crate::dsp::oversample::Oversampler;
use crate::dsp::spring::Tank;
use crate::dsp::time::Simulation;
use crate::dsp::tremolo::Tremolo;

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
    /// Mesa Boogie Mark IIC+, the lead channel preamplifier through its
    /// recovery stage.
    Boogie,
    /// Peavey EVH 5150, the lead channel preamplifier.
    Peavey,
    /// Neve 73P microphone preamplifier, two cascaded transistor stages.
    Neve,
    /// Fender Twin Reverb, AB763 -- the circuit the '65 reissue reissues.
    /// Vibrato channel, with the spring tank and the tremolo.
    Twin,
    /// Marshall JCM800 2203, the 1981 master-volume preamplifier. Its matched
    /// power stage is the Brit EL34. See `circuits::brit800`.
    Brit800,
    /// API 312 microphone preamplifier card. See `circuits::american312`.
    American312,
    /// SSL SL 4000 E channel mic amplifier (82E01). See `circuits::console_e`.
    ConsoleE,
    /// Universal Audio 610-A modular console preamplifier. See `circuits::tube610`.
    Tube610,
    /// Marshall 1959 Super Lead (Unicord drawing, 1970), bright channel. Its
    /// matched power stage is the Brit Plexi EL34. See `circuits::plexi`.
    Plexi,
    /// Vox AC30/6 Top Boost, brilliant channel. Cathode-biased EL84s with no
    /// feedback loop behind it. See `circuits::ac30`.
    AC30,
    /// Hiwatt Custom 100 DR103, brilliant channel through its master volume.
    /// See `circuits::dr103`.
    DR103,
    // --- the pedals, as circuits in their own right ----------------------
    // Every pedal in the slot is also a circuit you can select on its own,
    // with no amplifier behind it. The Green 808 and the Ram Fuzz were here
    // first -- they were catalogue voices before the pedal slot existed -- and
    // these are the rest of them, appended so the voice indices above do not
    // move and `CALIBRATION` keeps its meaning.
    /// Ibanez TS9 (`ts808::TS9`).
    Green9,
    /// Pro Co RAT (`circuits::rodent`).
    Rat,
    /// Arbiter Fuzz Face (`circuits::round_fuzz`).
    FuzzFace,
    /// MXR Distortion+ (`circuits::distortion_plus`).
    DistPlus,
    /// Boss HM-2 (`circuits::heavy_metal`).
    Hm2,
    /// Boss MT-2 (`circuits::metal_zone`).
    Mt2,
    /// Mesa/Boogie Dual Rectifier, Rev F, the red channel in its modern setting.
    /// See `circuits::rectifier`.
    Recto,
}

impl Gain {
    // Appended: the calibration table and every chain's circuit slots are laid
    // out in this order.
    pub const ALL: [Gain; 27] = [
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
        Gain::Twin,
        Gain::Brit800,
        Gain::American312,
        Gain::ConsoleE,
        Gain::Tube610,
        Gain::Plexi,
        Gain::AC30,
        Gain::DR103,
        Gain::Recto,
        Gain::Green9,
        Gain::Rat,
        Gain::FuzzFace,
        Gain::DistPlus,
        Gain::Hm2,
        Gain::Mt2,
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
            Gain::Twin => "Twin Reverb",
            Gain::Muff => "Big Muff",
            Gain::Boogie => "Mark IIC+",
            Gain::Peavey => "5150",
            Gain::Neve => "Neve 73P",
            Gain::Brit800 => "JCM800 2203",
            Gain::American312 => "API 312",
            Gain::ConsoleE => "SSL 4000 E",
            Gain::Tube610 => "UA 610-A",
            Gain::Plexi => "1959 Super Lead",
            Gain::AC30 => "AC30 Top Boost",
            Gain::DR103 => "Hiwatt DR103",
            Gain::Recto => "Dual Rectifier",
            Gain::Green9 => "TS9",
            Gain::Rat => "ProCo RAT",
            Gain::FuzzFace => "Fuzz Face",
            Gain::DistPlus => "MXR Distortion+",
            Gain::Hm2 => "Boss HM-2",
            Gain::Mt2 => "Boss MT-2",
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
            Gain::Twin => twin::VOLUME,
            Gain::Muff => bigmuff::SUSTAIN,
            Gain::Brit800 => brit800::VOLUME,
            Gain::American312 => american312::GAIN,
            Gain::ConsoleE => console_e::GAIN,
            Gain::Tube610 => tube610::LEVEL,
            Gain::Plexi => plexi::VOLUME,
            Gain::AC30 => ac30::VOLUME,
            Gain::DR103 => dr103::VOLUME,
            Gain::Recto => rectifier::GAIN,
            Gain::Green9 => ts808::DRIVE,
            Gain::Rat => rodent::DISTORTION,
            Gain::FuzzFace => round_fuzz::FUZZ,
            Gain::DistPlus => distortion_plus::DISTORTION,
            Gain::Hm2 => heavy_metal::DIST,
            Gain::Mt2 => metal_zone::DIST,
            _ => clipper::GAIN,
        }
    }

    /// Whether this amplifier carries the five band graphic equaliser.
    ///
    /// Only the Mark IIC+. It is the part of that amplifier everyone
    /// recognises -- the scooped middle is this network and not the Fender
    /// stack ahead of it -- and it sits late in the preamplifier, so it is
    /// modelled where the drawing puts it rather than as an equaliser bolted
    /// on the end. See `markiic::graphic` and §9.8.
    pub const fn has_graphic(self) -> bool {
        matches!(self, Gain::Boogie)
    }

    /// What the drive knob is called on the device it is turning.
    ///
    /// The same principle as the pedals' single tone control being labelled
    /// TONE rather than TREBLE: a knob is named after what it turns, and what
    /// it turns here is a specific pot on a specific drawing. "Drive" is the
    /// section's job, not every device's word for it.
    ///
    /// The Twin Reverb is the one that matters. Its drive control **is** its
    /// Volume -- an AB763 has no master, so the one pot both sets the level and
    /// decides how hard the amplifier works -- and calling it DRIVE made it
    /// look as though the amplifier's volume knob was missing. It is not
    /// missing; it is this one.
    pub fn drive_name(self) -> &'static str {
        match self {
            Gain::Twin => "VOLUME",
            Gain::Muff => "SUSTAIN",
            Gain::Boogie => "LEAD DRIVE",
            Gain::Peavey => "PRE GAIN",
            Gain::Neve => "GAIN",
            Gain::Brit800 => "PREAMP",
            Gain::American312 | Gain::ConsoleE => "GAIN",
            Gain::Tube610 => "LEVEL",
            Gain::Plexi | Gain::AC30 | Gain::DR103 => "VOLUME",
            Gain::Recto => "GAIN",
            _ => "DRIVE",
        }
    }

    /// The circuit's own output level control, where its drawing has one, and
    /// which of the two simulations it lives in.
    ///
    /// Every device here except the abstract typologies has a knob on its face
    /// that sets how loud it is, and until now every one of them was frozen at
    /// a resting position -- BUG-023's fault, one level up: a control that
    /// exists on the hardware and cannot be reached from the panel.
    ///
    /// Which control counts as "the output" is a judgement per device and it
    /// is written here rather than inferred:
    ///
    /// - the pedals' Level and Volume, which is what they are called;
    /// - the 73P's output trim;
    /// - the 5150's post gain, at its **power stage**: it is after the
    ///   preamplifier, which is the amplifier's whole method -- gain in front,
    ///   level at the back;
    /// - the Mark IIC+'s **Lead Master**, in the preamplifier, because that is
    ///   where the drawing puts it: between V2B and the V2A recovery stage, so
    ///   it sets how hard V2A and the power stage are driven. The amplifier's
    ///   overall MASTER sits after V2A, and the power stage's master pot stands
    ///   in for it at its resting position;
    /// - the Mark IIC+'s Volume 1 is *not* this. It is an input volume and it
    ///   sits ahead of the lead circuit; Lead Drive is already the Drive knob.
    ///
    /// **The Twin Reverb has none**, and that is not an omission. An AB763 has
    /// no master volume and no presence: its channel Volume is the only level
    /// control it owns, and that is already what the Drive knob turns. Giving
    /// it one would hide why its phase inverter sees a hundred volts at Volume
    /// 10 -- see BUG-025.
    pub fn level_control(self) -> Option<Level> {
        match self {
            Gain::Screamer => Some(Level::Circuit(ts808::LEVEL)),
            Gain::Muff => Some(Level::Circuit(bigmuff::VOLUME)),
            Gain::Neve => Some(Level::Circuit(neve::TRIM)),
            Gain::Boogie => Some(Level::Circuit(markiic::LEAD_MASTER)),
            // The 2203's Master Volume is the pot the Brit EL34 stage begins with.
            Gain::Peavey | Gain::Brit800 => Some(Level::Power(power::MASTER)),
            // The DR103's master volume is in the preamplifier, between the
            // stack and the last two triodes, which is where the drawing has it.
            Gain::DR103 => Some(Level::Circuit(dr103::MASTER)),
            // The red channel's master, in the preamplifier as the sheet has it.
            Gain::Recto => Some(Level::Circuit(rectifier::MASTER)),
            Gain::Green9 => Some(Level::Circuit(ts808::LEVEL)),
            Gain::Rat => Some(Level::Circuit(rodent::VOLUME)),
            Gain::FuzzFace => Some(Level::Circuit(round_fuzz::VOLUME)),
            Gain::DistPlus => Some(Level::Circuit(distortion_plus::VOLUME)),
            Gain::Hm2 => Some(Level::Circuit(heavy_metal::LEVEL)),
            Gain::Mt2 => Some(Level::Circuit(metal_zone::LEVEL)),
            _ => None,
        }
    }

    /// The circuit's own tone controls, where it has some: bass, middle,
    /// treble. A Mark IIC+ carries a Fender stack of its own, and using the
    /// plugin's generic one instead of it would be modelling the amplifier
    /// and then ignoring the part everyone recognises.
    pub fn own_tone(self) -> Option<(usize, usize, usize)> {
        match self {
            Gain::Boogie => Some((markiic::BASS, markiic::MIDDLE, markiic::TREBLE)),
            Gain::Twin => Some((twin::BASS, twin::MIDDLE, twin::TREBLE)),
            Gain::Brit800 => Some((brit800::BASS, brit800::MIDDLE, brit800::TREBLE)),
            Gain::Plexi => Some((plexi::BASS, plexi::MIDDLE, plexi::TREBLE)),
            // No middle control: the stack has a 10 k resistor where a Fender
            // stack has that pot, so the panel's Middle knob is greyed out.
            Gain::AC30 => Some((ac30::BASS, usize::MAX, ac30::TREBLE)),
            Gain::DR103 => Some((dr103::BASS, dr103::MIDDLE, dr103::TREBLE)),
            Gain::Recto => Some((rectifier::BASS, rectifier::MIDDLE, rectifier::TREBLE)),
            // The TS808 and the Muff have a single tone control, which the
            // Treble knob takes.
            Gain::Screamer => Some((usize::MAX, usize::MAX, ts808::TONE)),
            Gain::Muff => Some((usize::MAX, usize::MAX, bigmuff::TONE)),
            Gain::Green9 => Some((usize::MAX, usize::MAX, ts808::TONE)),
            // The Rodent's is a **filter**: it runs the other way, and
            // `tone_runs_backwards` is how the knob is made to agree with it.
            Gain::Rat => Some((usize::MAX, usize::MAX, rodent::FILTER)),
            // The Heavy Metal's Colour Mix is a pair, and the Metal Zone has a
            // three band equaliser with a sweep. The panel has bass, middle and
            // treble, so they take the ones that map: the Colour Mix low and
            // high on bass and treble, and the Metal Zone's three bands on all
            // three. Its Mid Freq is reachable only from the pedal slot.
            Gain::Hm2 => Some((heavy_metal::LOW, usize::MAX, heavy_metal::HIGH)),
            Gain::Mt2 => Some((metal_zone::LOW, metal_zone::MIDDLE, metal_zone::HIGH)),
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
    /// A control this circuit has of its own beyond bass, middle and treble,
    /// and what the panel calls it.
    ///
    /// Only the Metal Zone has one: its **Mid Freq**, which moves where the
    /// middle band works rather than how much it does. Selected as a pedal it
    /// gets a knob in the slot's own row; selected as a circuit it needs one
    /// here, or the control would be reachable from one half of the plugin and
    /// not the other.
    pub fn own_sweep(self) -> Option<(usize, &'static str)> {
        match self {
            Gain::Mt2 => Some((metal_zone::MID_FREQ, "MID FREQ")),
            _ => None,
        }
    }

    /// Whether this circuit's own tone control runs the other way from the
    /// knob. Only the Rodent's, whose FILTER darkens as it turns up -- the same
    /// inversion the pedal slot carries for it.
    pub fn tone_runs_backwards(self) -> bool {
        matches!(self, Gain::Rat)
    }

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
    /// Only the three valve guitar amplifiers do. A pedal has no power stage,
    /// and the topologies are shapes rather than particular units, so there
    /// is nothing to put behind them: inventing one would be inventing a
    /// sound. The 73P has an output block of its own and it is not one of
    /// these -- see `build_power`.
    pub fn power_stage(self) -> Option<&'static power::PowerSpec> {
        match self {
            Gain::Boogie => Some(&power::PowerSpec::MARKIIC),
            Gain::Peavey => Some(&power::PowerSpec::EVH5150),
            Gain::Twin => Some(&power::PowerSpec::TWIN),
            Gain::Brit800 => Some(&power::PowerSpec::BRIT_EL34),
            Gain::Plexi => Some(&power::PowerSpec::PLEXI_EL34),
            Gain::AC30 => Some(&power::PowerSpec::AC30_EL84),
            Gain::DR103 => Some(&power::PowerSpec::DR103_EL34),
            Gain::Recto => Some(&power::PowerSpec::RECTO_6L6),
            _ => None,
        }
    }

    /// Whether this is a model of a particular circuit rather than a
    /// topology. The panel says so, because "an overdrive" and "a TS808" are
    /// different kinds of claim.
    pub fn is_modelled(self) -> bool {
        matches!(
            self,
            Gain::Green9
                | Gain::Rat
                | Gain::FuzzFace
                | Gain::DistPlus
                | Gain::Hm2
                | Gain::Mt2
                | Gain::Screamer
                | Gain::Muff
                | Gain::Boogie
                | Gain::Peavey
                | Gain::Neve
                | Gain::Twin
                | Gain::Brit800
                | Gain::American312
                | Gain::ConsoleE
                | Gain::Tube610
                | Gain::Plexi
                | Gain::AC30
                | Gain::DR103
                | Gain::Recto
        )
    }

    /// Whether the choice of amplifying part reaches this circuit.
    ///
    /// Only the preamplifier channels are built around a part that can be
    /// swapped. The guitar circuits are valve cascades by definition -- a
    /// "three cascaded stages" made of op-amps is a different thing with the
    /// same name -- and the pedals are built around their diodes.
    pub const fn has_amplifier(self) -> bool {
        matches!(self, Gain::Console | Gain::Studio)
    }

    /// Whether the diode choice reaches this circuit at all. A valve stage has
    /// no diodes in it, and offering the choice there would be a control that
    /// does nothing -- which is worse than not offering it.
    pub const fn has_diodes(self) -> bool {
        matches!(self, Gain::Overdrive | Gain::Distortion)
    }

    /// Whether this voice has a reverb tank and a tremolo of its own.
    ///
    /// Only the Twin does. The panel greys the three controls everywhere
    /// else rather than leaving knobs that turn nothing -- which is the
    /// defect BUG-023 was about, from the other side.
    pub fn has_reverb_and_tremolo(self) -> bool {
        matches!(self, Gain::Twin)
    }

    pub const fn variants(self) -> usize {
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
/// Counted from `Gain::ALL` rather than written down, so that adding a
/// circuit cannot leave this behind. It was a hand-kept `20`, and adding the
/// Twin made it wrong: the calibration table is `[Calibration; VOICES]`, so a
/// stale count is a table with a missing row and every voice after the new one
/// reading its neighbour's make-up.
/// How many entries the circuit list has, which is what `POWER_TRIM_DB` is
/// indexed by. Tied to the list so that adding a circuit fails to compile
/// rather than indexing past the end of the table at runtime, which is what it
/// did when six were appended.
pub const GAINS: usize = Gain::ALL.len();

pub const VOICES: usize = {
    let mut total = 0;
    let mut i = 0;
    while i < Gain::ALL.len() {
        total += Gain::ALL[i].variants();
        i += 1;
    }
    total
};

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
/// How far the modelled circuits follow the Oversampling control.
///
/// A nonlinear stage makes harmonics, and the ones above half the sample rate
/// fold back down onto frequencies that have nothing to do with the note. That
/// fold-back is the hash a high-gain amplifier adds at 48 kHz, and the only
/// real cure is to solve the circuit faster than the host.
///
/// These circuits are big nonlinear solves and the cure is priced by the
/// factor. Measured at full drive on a 1760 Hz note, as inharmonic energy
/// against the fundamental, alongside what one channel costs of its real-time
/// budget (`examples/oversampling.rs`):
///
/// | | 1x | 2x | 4x | 8x |
/// |---|---|---|---|---|
/// | American 5150 | 18.6 % / 34 % | 6.5 % / 67 % | 3.4 % / 114 % | 1.7 % / 193 % |
/// | Brit 800 | 12.1 % / 21 % | 2.7 % / 40 % | 1.3 % / 71 % | 1.2 % / 142 % |
/// | Cali Rectifier | 7.2 % / 25 % | 2.9 % / 45 % | 0.8 % / 85 % | 0.2 % / 162 % |
/// | Cali IIC+ | 5.1 % / 32 % | 1.0 % / 57 % | 0.3 % / 109 % | 0.2 % / 212 % |
///
/// Two is where the trade sits. It takes about two thirds of the fold-back
/// away for about double the work, and it is the last factor that fits: past
/// it every one of these circuits costs more than the time there is, which is
/// a DAW missing its deadline -- crackle, stuttering live input, playback
/// falling behind. That was what pinning them to 1x was avoiding when the
/// control was first made to skip them; the pin also meant the control did
/// nothing at all for exactly the circuits that alias most, which is this cap
/// instead.
///
/// Every shipped preset on a modelled circuit asks for 1x, so none of them
/// costs any more than it did; this is what the control does when a player
/// turns it up.
pub const MODELLED_MAX_OVERSAMPLING: usize = 2;

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

/// Output stages that are nobody's matched stage, and so have no voice to
/// borrow a resistor-loaded simulation from. They sit after the catalogue in
/// `Chain::powers`, in this order.
pub const EXTRA_POWER_SPECS: [&power::PowerSpec; 1] = [&power::PowerSpec::RECTO_6L6_TUBE];

/// How many of them there are.
pub const EXTRA_POWERS: usize = EXTRA_POWER_SPECS.len();

/// Physical power circuit identity, independent of the preamp catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerModel {
    Cali6L6,
    American6L6Clean,
    American6L6HighGain,
    BritEL34,
    BritPlexiEL34,
    AC30EL84,
    DR103EL34,
    Recto6L6,
    Recto6L6Tube,
}

impl PowerModel {
    pub const ALL: [PowerModel; 9] = [
        PowerModel::Cali6L6,
        PowerModel::American6L6Clean,
        PowerModel::American6L6HighGain,
        PowerModel::BritEL34,
        PowerModel::BritPlexiEL34,
        PowerModel::AC30EL84,
        PowerModel::DR103EL34,
        PowerModel::Recto6L6,
        PowerModel::Recto6L6Tube,
    ];

    pub fn spec(self) -> &'static power::PowerSpec {
        match self {
            Self::Cali6L6 => &power::PowerSpec::MARKIIC,
            Self::American6L6Clean => &power::PowerSpec::TWIN,
            Self::American6L6HighGain => &power::PowerSpec::EVH5150,
            Self::BritEL34 => &power::PowerSpec::BRIT_EL34,
            Self::BritPlexiEL34 => &power::PowerSpec::PLEXI_EL34,
            Self::AC30EL84 => &power::PowerSpec::AC30_EL84,
            Self::DR103EL34 => &power::PowerSpec::DR103_EL34,
            Self::Recto6L6 => &power::PowerSpec::RECTO_6L6,
            Self::Recto6L6Tube => &power::PowerSpec::RECTO_6L6_TUBE,
        }
    }

    fn slot(self) -> usize {
        match self {
            Self::Cali6L6 => 0,
            Self::American6L6Clean => 1,
            Self::American6L6HighGain => 2,
            Self::BritEL34 => 3,
            Self::BritPlexiEL34 => 4,
            Self::AC30EL84 => 5,
            Self::DR103EL34 => 6,
            Self::Recto6L6 => 7,
            Self::Recto6L6Tube => 8,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Cali6L6 => voice_index(Gain::Boogie, Diode::Silicon, Amplifier::Valve),
            Self::American6L6Clean => voice_index(Gain::Twin, Diode::Silicon, Amplifier::Valve),
            Self::American6L6HighGain => {
                voice_index(Gain::Peavey, Diode::Silicon, Amplifier::Valve)
            }
            Self::BritEL34 => voice_index(Gain::Brit800, Diode::Silicon, Amplifier::Valve),
            Self::BritPlexiEL34 => voice_index(Gain::Plexi, Diode::Silicon, Amplifier::Valve),
            Self::AC30EL84 => voice_index(Gain::AC30, Diode::Silicon, Amplifier::Valve),
            Self::DR103EL34 => voice_index(Gain::DR103, Diode::Silicon, Amplifier::Valve),
            Self::Recto6L6 => voice_index(Gain::Recto, Diode::Silicon, Amplifier::Valve),
            // No voice has this one as its own, so it lives in the slot after
            // the catalogue. See `EXTRA_POWER_SPECS`.
            Self::Recto6L6Tube => VOICES,
        }
    }
}

/// Independent selection of complete output circuits; see params for stable IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PowerAmp {
    #[default]
    Matched,
    Bypass,
    Cali6L6,
    American6L6Clean,
    American6L6HighGain,
    BritEL34,
    BritPlexiEL34,
    AC30EL84,
    DR103EL34,
    Recto6L6,
    Recto6L6Tube,
}

impl PowerAmp {
    pub const ALL: [Self; 11] = [
        Self::Matched,
        Self::Bypass,
        Self::Cali6L6,
        Self::American6L6Clean,
        Self::American6L6HighGain,
        Self::BritEL34,
        Self::BritPlexiEL34,
        Self::AC30EL84,
        Self::DR103EL34,
        Self::Recto6L6,
        Self::Recto6L6Tube,
    ];

    pub fn resolved(self, preamp: Gain) -> Option<PowerModel> {
        match self {
            Self::Matched => match preamp {
                Gain::Boogie => Some(PowerModel::Cali6L6),
                Gain::Twin => Some(PowerModel::American6L6Clean),
                Gain::Peavey => Some(PowerModel::American6L6HighGain),
                Gain::Brit800 => Some(PowerModel::BritEL34),
                Gain::Plexi => Some(PowerModel::BritPlexiEL34),
                Gain::AC30 => Some(PowerModel::AC30EL84),
                Gain::DR103 => Some(PowerModel::DR103EL34),
                Gain::Recto => Some(PowerModel::Recto6L6),
                _ => None,
            },
            Self::Bypass => None,
            Self::Cali6L6 => Some(PowerModel::Cali6L6),
            Self::American6L6Clean => Some(PowerModel::American6L6Clean),
            Self::American6L6HighGain => Some(PowerModel::American6L6HighGain),
            Self::BritEL34 => Some(PowerModel::BritEL34),
            Self::BritPlexiEL34 => Some(PowerModel::BritPlexiEL34),
            Self::AC30EL84 => Some(PowerModel::AC30EL84),
            Self::DR103EL34 => Some(PowerModel::DR103EL34),
            Self::Recto6L6 => Some(PowerModel::Recto6L6),
            Self::Recto6L6Tube => Some(PowerModel::Recto6L6Tube),
        }
    }
}

/// A pedal in front of the preamplifier, independent of the circuit selection.
/// Only modelled pedals are offered; see `docs/MODEL_INVENTORY.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pedal {
    #[default]
    None,
    /// The Ibanez TS-808 with the values its drawings agree on (`ts808::TS808`).
    Green808,
    /// The 1973 Ram's Head Big Muff Pi netlist (`Gain::Muff`).
    BigMuff,
    /// The Ibanez TS9 (`ts808::TS9`).
    Green9,
    /// The Pro Co RAT, LM308 version (`circuits::rodent`).
    Rodent,
    /// The Arbiter Fuzz Face, germanium (`circuits::round_fuzz`).
    RoundFuzz,
    /// The MXR Distortion+ (`circuits::distortion_plus`).
    YellowDist,
    /// The Boss HM-2 (`circuits::heavy_metal`). Four knobs: the first pedal
    /// here with more than one tone control.
    HeavyMetal,
    /// The Boss MT-2 (`circuits::metal_zone`). Five, and three of them tone.
    MetalZone,
}

/// What a guitar puts out for a nominal digital signal: the level every circuit
/// with a guitar in front of it is calibrated at (`examples/calibrate.rs`).
pub const GUITAR_VOLTS: f64 = 0.122;

/// The most tone controls any pedal in the list has: the Metal Zone's four.
///
/// The slot used to carry one, which was fine while every pedal in it had one
/// tone knob or none. It is not fine for a pedal whose entire point is its
/// equaliser -- a Metal Zone with three of its six controls missing is not that
/// pedal -- so the slot carries what the boxes carry.
pub const PEDAL_TONES: usize = 4;

/// One of a pedal's tone controls.
#[derive(Clone, Copy, Debug)]
struct ToneKnob {
    /// Where it is in the pedal's netlist.
    control: usize,
    /// What the panel prints under it. The pedals do not agree on a name --
    /// tone, filter, colour, middle -- and the knob should say what the box
    /// says rather than flattening them all to "tone".
    label: &'static str,
    /// The pedal's control runs the other way from the knob: the Rodent's
    /// FILTER darkens as it turns up.
    inverted: bool,
}

/// Where a pedal's knobs land in its netlist. `None` is a knob it does not have.
#[derive(Clone, Copy, Debug)]
struct PedalControls {
    drive: usize,
    tones: [Option<ToneKnob>; PEDAL_TONES],
    level: usize,
}

/// A pedal with one tone control, which is most of them.
const fn one_tone(control: usize, label: &'static str, inverted: bool) -> [Option<ToneKnob>; PEDAL_TONES] {
    [Some(ToneKnob { control, label, inverted }), None, None, None]
}

/// A pedal with none.
const NO_TONES: [Option<ToneKnob>; PEDAL_TONES] = [None, None, None, None];

impl Pedal {
    pub const ALL: [Pedal; 9] = [
        Pedal::None,
        Pedal::Green808,
        Pedal::BigMuff,
        Pedal::Green9,
        Pedal::Rodent,
        Pedal::RoundFuzz,
        Pedal::YellowDist,
        Pedal::HeavyMetal,
        Pedal::MetalZone,
    ];
    /// How many pedal circuits a chain holds.
    const SLOTS: usize = 8;

    fn slot(self) -> Option<usize> {
        match self {
            Pedal::None => None,
            Pedal::Green808 => Some(0),
            Pedal::BigMuff => Some(1),
            Pedal::Green9 => Some(2),
            Pedal::Rodent => Some(3),
            Pedal::RoundFuzz => Some(4),
            Pedal::YellowDist => Some(5),
            Pedal::HeavyMetal => Some(6),
            Pedal::MetalZone => Some(7),
        }
    }

    /// Whether this pedal is large enough that the chain cannot afford to
    /// oversample it.
    ///
    /// Measured with `examples/pedalcost.rs`, as a percentage of one channel's
    /// realtime budget at 48 kHz, in front of the Cali IIC+:
    ///
    /// | | alone | with the amplifier, 1x | at 2x |
    /// |---|---|---|---|
    /// | Green 808 | 11.6 | 40.8 | 74.5 |
    /// | Rodent | 10.5 | 39.6 | 74.6 |
    /// | Metal Zone | 22.4 | 55.5 | **100.7** |
    /// | Heavy Metal | 41.6 | 75.1 | **132.6** |
    ///
    /// The last two do not fit at twice the host rate, and a chain that does not
    /// fit is a DAW missing its deadline: crackle, stuttering, dropouts. They
    /// are fifty-unknown circuits with a nonlinear device on almost every node,
    /// which is two to four times what the older pedals are, and no amount of
    /// tidying changes that -- it is what those two boxes are.
    ///
    /// So the chain keeps them at the host rate, exactly as it keeps the
    /// modelled amplifiers under `MODELLED_MAX_OVERSAMPLING`, and for the same
    /// reason. See `Chain::set_oversampling`.
    pub fn is_expensive(self) -> bool {
        matches!(self, Pedal::HeavyMetal | Pedal::MetalZone)
    }

    /// Whether the pedal has any tone control at all.
    pub fn has_tone(self) -> bool {
        self.tone_labels()[0].is_some()
    }

    /// What this pedal's tone controls are called, in panel order. The editor
    /// draws a knob for each one that is there and nothing for the rest.
    pub fn tone_labels(self) -> [Option<&'static str>; PEDAL_TONES] {
        let mut labels = [None; PEDAL_TONES];
        if let Some(slot) = self.slot() {
            for (label, knob) in labels.iter_mut().zip(Self::controls(slot).tones) {
                *label = knob.map(|k| k.label);
            }
        }
        labels
    }

    fn build(slot: usize) -> Result<Netlist, Fault> {
        match slot {
            0 => ts808::build_with(&ts808::TS808, 10_000.0, 470_000.0),
            1 => bigmuff::build(&bigmuff::RAMS_HEAD, 10_000.0, 470_000.0),
            2 => ts808::build_with(&ts808::TS9, 10_000.0, 470_000.0),
            3 => rodent::build(10_000.0, 470_000.0),
            4 => round_fuzz::build(10_000.0, 470_000.0),
            5 => distortion_plus::build(10_000.0, 470_000.0),
            6 => heavy_metal::build(10_000.0, 470_000.0),
            _ => metal_zone::build(10_000.0, 470_000.0),
        }
    }

    fn controls(slot: usize) -> PedalControls {
        match slot {
            0 | 2 => PedalControls {
                drive: ts808::DRIVE,
                tones: one_tone(ts808::TONE, "tone", false),
                level: ts808::LEVEL,
            },
            1 => PedalControls {
                drive: bigmuff::SUSTAIN,
                tones: one_tone(bigmuff::TONE, "tone", false),
                level: bigmuff::VOLUME,
            },
            3 => PedalControls {
                drive: rodent::DISTORTION,
                tones: one_tone(rodent::FILTER, "filter", true),
                level: rodent::VOLUME,
            },
            4 => PedalControls {
                drive: round_fuzz::FUZZ,
                tones: NO_TONES,
                level: round_fuzz::VOLUME,
            },
            5 => PedalControls {
                drive: distortion_plus::DISTORTION,
                tones: NO_TONES,
                level: distortion_plus::VOLUME,
            },
            // The first pedal here with two tone controls, and they are a
            // boost-and-cut pair rather than a passive tone: "colour mix" is
            // what the box calls them.
            6 => PedalControls {
                drive: heavy_metal::DIST,
                tones: [
                    Some(ToneKnob { control: heavy_metal::LOW, label: "colour lo", inverted: false }),
                    Some(ToneKnob { control: heavy_metal::HIGH, label: "colour hi", inverted: false }),
                    None,
                    None,
                ],
                level: heavy_metal::LEVEL,
            },
            // Four tone controls, which is what the slot was widened to carry:
            // three bands and the sweep that moves the middle one.
            _ => PedalControls {
                drive: metal_zone::DIST,
                tones: [
                    Some(ToneKnob { control: metal_zone::LOW, label: "low", inverted: false }),
                    Some(ToneKnob { control: metal_zone::MIDDLE, label: "middle", inverted: false }),
                    Some(ToneKnob { control: metal_zone::MID_FREQ, label: "mid freq", inverted: false }),
                    Some(ToneKnob { control: metal_zone::HIGH, label: "high", inverted: false }),
                ],
                level: metal_zone::LEVEL,
            },
        }
    }

    /// Volts at the pedal's input for a nominal digital signal. The three older
    /// pedals keep the calibration of the catalogue voice they share a netlist
    /// with; the newer ones take a guitar's level directly.
    fn input_volts(slot: usize) -> f64 {
        let of = |gain: Gain| CALIBRATION[voice_index(gain, Diode::Silicon, Amplifier::Valve)].drive_volts;
        match slot {
            0 | 2 => of(Gain::Screamer),
            1 => of(Gain::Muff),
            _ => GUITAR_VOLTS,
        }
    }
}

/// The pedal and its knobs. Level's middle is the pedal's calibrated resting
/// position, exactly as the Master knob's is. See `Chain::master_position`.
///
/// `tone` carries as many as the pedal has, in the order its panel has them;
/// entries past that are ignored. See `PEDAL_TONES`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PedalSettings {
    pub pedal: Pedal,
    pub drive: f64,
    pub tone: [f64; PEDAL_TONES],
    pub level: f64,
}

impl PedalSettings {
    /// A pedal with every tone control in the middle, which is what a knob the
    /// caller does not care about should be.
    pub fn centred(pedal: Pedal, drive: f64, level: f64) -> Self {
        Self { pedal, drive, level, ..Self::default() }
    }
}

impl Default for PedalSettings {
    fn default() -> Self {
        Self {
            pedal: Pedal::None,
            drive: 0.5,
            tone: [0.5; PEDAL_TONES],
            level: 0.5,
        }
    }
}

/// Which cabinet is after the power stage. `Legacy` is the old resistive load and
/// baked Combo/Stack filter, exactly as before any of this existed; every old
/// session resolves to it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum CabinetChoice {
    #[default]
    Legacy,
    /// A driver on an infinite baffle: speaker and microphone, no box.
    Bypass,
    Model(&'static CabinetProfile),
}

/// Which driver. `Matched` is the cabinet's own; `Bypass` is a resistor and the
/// terminal voltage (a DI of the power stage).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum SpeakerChoice {
    #[default]
    Matched,
    Bypass,
    Model(&'static SpeakerProfile),
}

/// Everything after the power stage, as plain values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcousticSettings {
    pub cabinet: CabinetChoice,
    pub speaker: SpeakerChoice,
    /// `Ideal` is an ideal omni at the placement ("Bypass"); it can never be `Off`.
    pub mic_a: MicSlot,
    pub mic_b: MicSlot,
    pub place_a: MicPlacement,
    pub place_b: MicPlacement,
    pub blend: f64,
    pub invert_b: bool,
    pub align: bool,
}

impl Default for AcousticSettings {
    fn default() -> Self {
        Self {
            cabinet: CabinetChoice::Legacy,
            speaker: SpeakerChoice::Matched,
            mic_a: MicSlot::Profile(&MicProfile::DYNAMIC_57),
            mic_b: MicSlot::Off,
            place_a: MicPlacement::default(),
            place_b: MicPlacement::default(),
            blend: 0.5,
            invert_b: false,
            align: false,
        }
    }
}

impl AcousticSettings {
    /// The driver actually radiating, if the physical path is in use.
    pub fn resolved_speaker(&self) -> Option<&'static SpeakerProfile> {
        let cabinet = match self.cabinet {
            CabinetChoice::Legacy => return None,
            CabinetChoice::Bypass => None,
            CabinetChoice::Model(cab) => Some(cab),
        };
        match self.speaker {
            SpeakerChoice::Bypass => None,
            SpeakerChoice::Model(profile) => Some(profile),
            SpeakerChoice::Matched => Some(
                cabinet
                    .map(|cab| cab.default_speaker)
                    .unwrap_or(&SpeakerProfile::BRIT_V30),
            ),
        }
    }

    pub fn resolved_cabinet(&self) -> Option<&'static CabinetProfile> {
        match self.cabinet {
            CabinetChoice::Model(cab) => Some(cab),
            _ => None,
        }
    }

    fn mounting(&self) -> Mounting {
        self.resolved_cabinet()
            .map(CabinetProfile::mounting)
            .unwrap_or(Mounting::BAFFLE)
    }
}

/// A speaker-loaded simulation and where to read its cone.
struct Loaded {
    sim: Simulation,
    slots: LoadSlots,
    motional: usize,
}

/// The block behind a voice's gain circuit, where it has one.
///
/// Four voices do, and they are not the same kind of thing. The three valve
/// guitar amplifiers get a push-pull power stage from `power.rs`, built from
/// a `PowerSpec`. The 73P gets its own OUTPUT block -- two BC109C into a
/// TIP3055 and the VTB1148 -- because a microphone preamplifier's line driver
/// is not a power amplifier and sharing the machinery would mean sharing a
/// topology it does not have.
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
        // Loaded by the phase inverter's grid leak, which is where the
        // drawing hands over. See `twin.rs`.
        Gain::Twin => twin::build(10_000.0, 1_000_000.0),
        Gain::Muff => bigmuff::build(&bigmuff::RAMS_HEAD, 10_000.0, 470_000.0),
        Gain::Boogie => markiic::build(10_000.0, 1_000_000.0),
        // Not a nominal load: the 5150's tone stack really does hang 33 k on
        // the far side of R89, and leaving it out would flatter the model by
        // twenty four decibels.
        Gain::Peavey => evh5150::build(10_000.0, evh5150::TONE_STACK_INPUT),
        Gain::Neve => neve::build(150.0, 10_000.0),
        // Loaded by the Master Volume's 1 M, which the power stage begins with.
        Gain::Brit800 => brit800::build(10_000.0, 1_000_000.0),
        // A 150 ohm microphone into the card, and a modern line input after it.
        Gain::American312 => american312::build(150.0, 10_000.0),
        Gain::ConsoleE => console_e::build(150.0, 10_000.0),
        // The 610-A's microphone connection is the O-1's 50 ohm winding, and its
        // output transformer is wound for a 600 ohm line.
        Gain::Tube610 => tube610::build(50.0, 600.0),
        Gain::Plexi => plexi::build(10_000.0, 1_000_000.0),
        Gain::AC30 => ac30::build(10_000.0, 1_000_000.0),
        Gain::DR103 => dr103::build(10_000.0, 1_000_000.0),
        Gain::Recto => rectifier::build(10_000.0, 1_000_000.0),
        // The pedals as circuits: the same netlists the slot builds, on the
        // same source and load their drawings specify.
        Gain::Green9 => ts808::build_with(&ts808::TS9, 10_000.0, 470_000.0),
        Gain::Rat => rodent::build(10_000.0, 470_000.0),
        Gain::FuzzFace => round_fuzz::build(10_000.0, 470_000.0),
        Gain::DistPlus => distortion_plus::build(10_000.0, 470_000.0),
        Gain::Hm2 => heavy_metal::build(10_000.0, 470_000.0),
        Gain::Mt2 => metal_zone::build(10_000.0, 470_000.0),
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
/// iron stage sits after the make-up and sees a known level already.
///
/// Set so that a nominal signal at a guitar's **low E** lands about at
/// steel's knee.
///
/// It was set at 40 Hz, and 40 Hz is below the lowest note the instrument
/// has. Flux goes as `V / f`, so by low E (82 Hz) there was half that flux
/// and by A (110 Hz) less again -- and the Iron control did nothing audible
/// anywhere in the instrument's range. Measured on the iron alone at a
/// nominal signal, the spread between the three cores was:
///
/// | | 40 Hz | 82 Hz | 110 Hz | 220 Hz |
/// |---|---|---|---|---|
/// | at 24 V | 0.8 pts | **0.2** | **0.1** | **0.0** |
/// | at 96 V | 42.4 pts | **20.5** | **4.8** | 0.1 |
///
/// Reported from a DAW as "the iron models seem not to change anything", and
/// they did not.
///
/// Four times, not more. At six the cores still differ by 22.9 points at
/// 110 Hz, which is a transformer that never stops saturating; at four they
/// separate on the bottom two strings and are out of the way above them. The
/// reason not to chase an audible difference at 220 Hz is that no transformer
/// has one -- getting flux to the knee there needs five times again, which
/// would put 40 Hz past 75 per cent distortion. It would stop being iron and
/// start being a waveshaper.
///
/// See `examples/ironvolts.rs` for the measurement behind the choice.
pub const IRON_VOLTS: f64 = 96.0;

/// Where on the Drive control the iron is matched to the stage driving it.
///
/// The transformer is handed `IRON_VOLTS` exactly here, more above and less
/// below -- see `Chain::iron_drive`. Three quarters rather than the middle,
/// because that is where a player who has reached for a transformer is likely
/// to be, and because it leaves the top of the travel with somewhere to go.
pub const IRON_REFERENCE_DRIVE: f64 = 0.75;

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

/// How many points the make-up curve is measured at.
///
/// The make-up is a curve across the drive control rather than one number,
/// because the drive control moves the gain of these circuits by about eighty
/// decibels end to end. Where those points sit across the control is what
/// `KNOT_SHAPE` decides; the *number* of them decides how much of the curve a
/// straight line between two of them has to describe. It was nine, and then
/// seventeen, and the increase at each step was because a curve that goes as
/// `log(position)` near the bottom and flattens near the top cannot be
/// described by a straight line between evenly spaced samples -- see
/// `KNOT_SHAPE` for the shape. `tests/voice.rs` checks the interpolation
/// between the points, not just the points themselves.
pub const POINTS: usize = 33;

/// How the make-up's sample points are spread across the drive control.
///
/// Not evenly, and the reason is that no drive control is even. A pot's law is
/// logarithmic and the stage after it saturates, so a circuit's gain climbs
/// most of its range inside the first eighth of the travel and then flattens:
/// the Mark IIC+ moves 47 dB between a shut Lead Drive and an eighth of a turn,
/// and 9 dB over the remaining seven eighths. Evenly spaced points sample that
/// first cliff too sparsely for a straight line to describe it -- with nine
/// of them, the line between the two samples nearest the bottom was **24 dB**
/// away from the curve at a knob position of 0.03, which is heard as the level
/// lurching as the knob comes off its stop, and was reported as "at 0% Gain
/// the meter goes way UP".
///
/// More even points do not fix it. The curve goes as `log(position)` near the
/// bottom, so a straight line across the first segment is wrong by an amount
/// that does not shrink usefully however narrow the segment gets.
///
/// So the points are spread by a cube law: knot `i` sits at
/// `(i / (POINTS - 1))^3`. That puts seventeen of the thirty-three inside the
/// first eighth of the travel, where the gain is, and leaves the flat top end
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
    /// Output make-up in dB at the drive positions `knot_position(i)` gives,
    /// the first at 0 and the last at 1.
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
include!("power_trim.rs");

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
/// oversampling off, 56 samples at two times, 64 at four, 66 at eight -- so
/// the honest figure would change as the control moves. The CLAP specification
/// asks that it does not, and a host that has to renegotiate its delay
/// compensation mid-stream will click. So one figure is reported and the
/// shorter settings are padded up to it.
///
/// 66 samples is 1.38 ms at 48 kHz and 1.50 ms at 44.1 kHz, both inside the
/// plugin's latency budget and inside the 9 ms ceiling everywhere. The
/// filters that get there are shorter and less steep than the ones this used
/// to carry -- 56 taps with a 90 dB stopband against 160 with 136 -- and the
/// reason the trade is worth making is that the plugin is a guitar amplifier.
/// What the extra attenuation bought was the octave above fifteen kilohertz,
/// which a speaker cone never reaches, and what it cost was two and a half
/// milliseconds of a three-point-seven-five millisecond budget.
pub const LATENCY: u32 = 66;

/// Number of samples to crossfade when switching circuits or presets.
/// At 48 kHz this is about 5.3 ms -- long enough to mask the capacitor
/// reset discontinuity even for high-gain circuits like the Mark IIC+,
/// short enough to be imperceptible as a level dip.
const FADE_LEN: usize = 256;

/// A whole number of samples of delay.
struct Delay {
    // A dormant channel may still have its old padding length when it wakes.
    // Reserve the maximum inline so copying any legal history cannot allocate.
    buf: [f64; LATENCY as usize],
    len: usize,
    pos: usize,
}

impl Delay {
    fn new(len: usize) -> Self {
        assert!(len <= LATENCY as usize);
        Self {
            buf: [0.0; LATENCY as usize],
            len,
            pos: 0,
        }
    }

    fn copy_runtime_state_from(&mut self, source: &Self) {
        self.buf.copy_from_slice(&source.buf);
        self.len = source.len;
        self.pos = source.pos;
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
        assert!(len <= self.buf.len());
        if len != self.len {
            self.buf[..len].fill(0.0);
            self.len = len;
            self.pos = 0;
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        if self.len == 0 {
            return x;
        }
        let out = self.buf[self.pos];
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % self.len;
        out
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
}


/// The middle of the Master knob: the position each circuit was voiced at.
pub const MASTER_MIDDLE: f64 = 0.5;

/// How much the top of the Master knob adds once the pot has run out.
///
/// Six decibels, because the pot's own contribution above the voicing ranges
/// from three to eight across the catalogue and this brings every voice's
/// upward half into the same order as its downward one, which is about twenty.
/// Larger would make the knob mostly digital gain; smaller would leave the
/// 73P's upper half doing nothing. See `Chain::master_lift`.
const MASTER_LIFT_DB: f64 = 6.0;

/// Where a level control sits when the circuit declares one but rests it
/// nowhere. Nothing in the catalogue does; this is so a circuit added later
/// that forgets `Netlist::rest` is quiet rather than wrong.
const DEFAULT_MASTER_REST: f64 = 0.7;

/// Where the Master knob's middle puts a power stage that has no master of its
/// own, when it is driven by a different preamplifier. See `Chain::set_master`.
pub const OVERRIDE_MASTER_REST: f64 = 0.30;

/// Where a circuit's output level control lives. See `Gain::level_control`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// In the gain circuit: a pedal's Level or Volume, the 73P's trim.
    Circuit(usize),
    /// In the power amplifier: a master volume.
    Power(usize),
}

/// What every solve a voice runs did, added up. See `Chain::solver_health`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SolverHealth {
    pub solves: u64,
    pub passes: u64,
    /// Solves that ran out of passes still holding a partly converged answer.
    /// These are at once the least accurate samples and the most expensive
    /// ones, which is what a deadline miss is made of.
    pub unsettled: u64,
    pub backtracks: u64,
    pub fallbacks: u64,
    pub nonfinite: u64,
    pub replans: u64,
    pub attack_predictor_suppressions: u64,
    pub continuation_attempts: u64,
    pub continuation_midpoint_successes: u64,
    pub continuation_successes: u64,
    pub continuation_actual_rescues: u64,
}

impl SolverHealth {
    pub fn passes_per_solve(&self) -> f64 {
        if self.solves == 0 {
            0.0
        } else {
            self.passes as f64 / self.solves as f64
        }
    }

    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            solves: self.solves.saturating_sub(before.solves),
            passes: self.passes.saturating_sub(before.passes),
            unsettled: self.unsettled.saturating_sub(before.unsettled),
            backtracks: self.backtracks.saturating_sub(before.backtracks),
            fallbacks: self.fallbacks.saturating_sub(before.fallbacks),
            nonfinite: self.nonfinite.saturating_sub(before.nonfinite),
            replans: self.replans.saturating_sub(before.replans),
            attack_predictor_suppressions: self
                .attack_predictor_suppressions
                .saturating_sub(before.attack_predictor_suppressions),
            continuation_attempts: self
                .continuation_attempts
                .saturating_sub(before.continuation_attempts),
            continuation_midpoint_successes: self
                .continuation_midpoint_successes
                .saturating_sub(before.continuation_midpoint_successes),
            continuation_successes: self
                .continuation_successes
                .saturating_sub(before.continuation_successes),
            continuation_actual_rescues: self
                .continuation_actual_rescues
                .saturating_sub(before.continuation_actual_rescues),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SolverBreakdown {
    pub pedal: SolverHealth,
    pub line: SolverHealth,
    pub gain: SolverHealth,
    pub power: SolverHealth,
    pub iron: SolverHealth,
    pub reverb_return: SolverHealth,
}

impl SolverBreakdown {
    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            pedal: self.pedal.saturating_delta(before.pedal),
            line: self.line.saturating_delta(before.line),
            gain: self.gain.saturating_delta(before.gain),
            power: self.power.saturating_delta(before.power),
            iron: self.iron.saturating_delta(before.iron),
            reverb_return: self.reverb_return.saturating_delta(before.reverb_return),
        }
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
    /// A pedal ahead of the circuit. Defaults to none.
    pub pedal: PedalSettings,
    pub power_amp: PowerAmp,
    /// Speaker, cabinet and microphones. Defaults to the legacy path.
    pub acoustic: AcousticSettings,
    pub gain: Gain,
    pub diode: Diode,
    pub amplifier: Amplifier,
    pub iron: Iron,
    pub tone: Tone,
    pub cabinet: Cabinet,
    pub drive: f64,
    /// The circuit's own level control, where it has one. Half is where the
    /// voice was calibrated; see `Chain::set_master`.
    pub master: f64,
    /// The Mark IIC+'s five graphic equaliser sliders, bottom band first.
    /// Centred is flat; only that amplifier has them.
    pub graphic: [f64; 5],
    pub bass: f64,
    pub mid: f64,
    pub treble: f64,
    /// The three the Twin Reverb has and nothing else does. Ignored by every
    /// other voice; the panel greys them out. See `Gain::extra_controls`.
    pub reverb: f64,
    pub speed: f64,
    pub intensity: f64,
    pub oversampling: usize,
    /// What the amplifier is plugged into, as a fraction of its own mains. One
    /// is the wall; less is a variac. See `Chain::set_mains`.
    pub mains: f64,
    /// A circuit's own fourth control, where it has one. See `Gain::own_sweep`.
    pub tone_sweep: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pedal: PedalSettings::default(),
            power_amp: PowerAmp::Matched,
            acoustic: AcousticSettings::default(),
            gain: Gain::Crunch,
            diode: Diode::Silicon,
            amplifier: Amplifier::Jfet,
            iron: Iron::Off,
            tone: Tone::Wide,
            cabinet: Cabinet::Off,
            drive: 0.5,
            master: MASTER_MIDDLE,
            graphic: [0.5; 5],
            bass: 0.5,
            mid: 0.5,
            reverb: 0.0,
            speed: 0.4,
            intensity: 0.0,
            treble: 0.5,
            oversampling: 2,
            mains: 1.0,
            tone_sweep: 0.5,
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
    /// The pedal slot's own circuits, separate from the catalogue so the same
    /// pedal can sit in front of itself. Indexed by `Pedal::slot`.
    pedals: Vec<Simulation>,
    pedal: Option<usize>,
    /// Which pedal that slot holds, for the policies that need to know what it
    /// is rather than merely that there is one. See `Pedal::is_expensive`.
    selected_pedal: Pedal,
    /// Volts per unit of digital input when a pedal takes the guitar first.
    pedal_into: f64,
    /// What the pedal's output is multiplied by on the way into the circuit:
    /// `into / pedal_into`.
    ///
    /// A pedal is always handed a guitar, because that is what it was built for
    /// and where its diodes sit; a circuit is handed the level *it* was built
    /// for, which is what `into` is and which is a guitar for the amplifiers
    /// and anything from five millivolts to a volt for the consoles and the
    /// clean stages. Without this the pedal's guitar-level output went straight
    /// into whatever followed, so putting a pedal in front of the clean circuit
    /// drove it nineteen decibels under its calibration point and the plugin
    /// went quiet. One for the amplifiers, where the two levels are the same.
    pedal_hand_off: f64,
    /// What every supply is running at, as a fraction of nominal.
    mains: f64,
    /// Each guitar power circuit at its own amplifier's catalogue index.
    powers: Vec<Option<Simulation>>,
    /// Zero is an empty catalogue slot, used for complete power bypass.
    power: usize,
    power_selection: PowerAmp,
    /// The 73P output driver belongs to the preamp, never to guitar power bypass.
    /// Allocate at construction: test trace storage makes Simulation large and
    /// embedding another one would overflow normal test-thread stacks.
    line: Box<Simulation>,
    /// Each output circuit again, driving a loudspeaker instead of a resistor,
    /// indexed by `PowerModel::slot`. See `power::build_with_speaker`.
    loaded: Vec<Loaded>,
    /// A loudspeaker driven straight from the preamplifier, for chains with no
    /// power stage.
    driven: Box<Loaded>,
    /// The cabinet and microphones. Boxed: a delay line lives inline in it.
    acoustic: Box<AcousticStage>,
    acoustic_settings: AcousticSettings,
    /// The physical path is in use: the load is a speaker and the output is the
    /// cone's radiation through `acoustic`, not the terminal voltage.
    radiating: bool,
    /// `Re Mms / Bl^2`, and the motional voltage one sample ago.
    pressure_scale: f64,
    motional_previous: f64,
    /// The three output transformers. Nonlinear, so unlike the tone stack and
    /// the cabinet these cannot be normalised by asking the AC solver: their
    /// trim is measured and baked with the voices.
    irons: Vec<Simulation>,
    /// Each linear section with the trim that undoes its own loss.
    tones: Vec<(Simulation, f64)>,
    cabinets: Vec<(Simulation, f64)>,
    gain: usize,
    /// Which voice that index names, so the two parts that are not circuits --
    /// the tank and the tremolo -- can tell whether they are in the path.
    voice: Gain,
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
    /// The reverb tank and the tremolo, which are not circuits and cannot be
    /// in a netlist. See `dsp::spring` and `dsp::tremolo`. Built for every
    /// chain rather than only for the Twin, because building one inside
    /// `apply` would be an allocation on the audio thread.
    tank: Tank,
    /// The recovery stage and the Reverb control, which *are* a circuit and so
    /// are solved like one. Only the tank between them is not.
    tail: Simulation,
    tremolo: Tremolo,
    reverb: f64,
    speed: f64,
    intensity: f64,
    /// The make-up at the Drive control's reference position, for the iron.
    /// See `iron_drive`.
    iron_reference: f64,
    /// The host's rate. Modelled circuits always run at this rate; see
    /// `set_oversampling`.
    rate: f64,
    drive: f64,
    /// Where the panel's Master knob is. See `set_master`.
    master: f64,
    /// The Mark IIC+'s five band graphic equaliser, between its preamplifier
    /// and its power stage. Linear, so it costs one substitution a sample.
    /// Only that amplifier has one; see `Gain::has_graphic`.
    graphic: Simulation,
    /// What the top half of that knob adds beyond the pot. See `master_lift`.
    master_lift: f64,
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
    /// A requested oversampling factor waiting to be installed at the first
    /// sample of a fresh crossfade. Changing factor resets the FIR histories;
    /// scheduling that reset at the fade boundary keeps the discontinuity
    /// underneath the same switch fade instead of dropping it into live audio.
    deferred_oversample: Option<usize>,
}

impl Chain {
    /// Wake a dormant, identically configured channel from this stream's exact
    /// history. Call before applying the first divergent block's new settings.
    /// Both chains must have received the same preceding `apply` calls.
    ///
    /// Only selected sections need copying: changing a gain/iron/tone/cabinet
    /// selection resets that section before use, and changing the gain resets
    /// its power stage, graphic EQ and Twin effects. Within a Twin, effects can
    /// be disabled without resetting them, so copy their tails even when off.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.rate, source.rate);
        debug_assert_eq!(self.gain, source.gain);
        debug_assert_eq!(self.voice, source.voice);
        debug_assert_eq!(self.power, source.power);
        debug_assert_eq!(self.power_selection, source.power_selection);
        debug_assert_eq!(self.iron, source.iron);
        debug_assert_eq!(self.tone, source.tone);
        debug_assert_eq!(self.cabinet, source.cabinet);
        debug_assert_eq!(self.requested_oversampling, source.requested_oversampling);
        debug_assert_eq!(self.drive, source.drive);
        debug_assert_eq!(self.master, source.master);
        debug_assert_eq!(self.out_of_target, source.out_of_target);
        debug_assert_eq!(self.reverb, source.reverb);
        debug_assert_eq!(self.speed, source.speed);
        debug_assert_eq!(self.intensity, source.intensity);

        let effective_rate_changed = self.over.factor() != source.over.factor();
        self.over.copy_runtime_state_from(&source.over);
        // The active chain installs deferred quality changes in `process`.
        // The dormant one has not reached that sample yet. Agree on the same
        // effective timestep, then install the ready caches below; do not reset
        // the FIRs or rebuild/settle a simulation to catch up.
        if effective_rate_changed {
            self.sync_oversampled_rates();
        }
        self.deferred_oversample = source.deferred_oversample;
        self.pad.copy_runtime_state_from(&source.pad);
        self.dry.copy_runtime_state_from(&source.dry);
        debug_assert_eq!(self.pedal, source.pedal);
        self.gains[self.gain].copy_runtime_state_from(&source.gains[source.gain]);
        if let Some(i) = self.pedal {
            self.pedals[i].copy_runtime_state_from(&source.pedals[i]);
        }
        debug_assert_eq!(self.radiating, source.radiating);
        debug_assert_eq!(self.acoustic_settings, source.acoustic_settings);
        if let (Some(dst), Some(src)) = (self.active_power_mut(), source.active_power()) {
            dst.copy_runtime_state_from(src);
        }
        if self.radiating {
            if self.resolved_power_amp().is_none() {
                self.driven.sim.copy_runtime_state_from(&source.driven.sim);
            }
            self.acoustic.copy_runtime_state_from(&source.acoustic);
            self.motional_previous = source.motional_previous;
        }
        if self.voice == Gain::Neve {
            self.line.copy_runtime_state_from(&source.line);
        }
        if let Some(i) = self.iron {
            self.irons[i].copy_runtime_state_from(&source.irons[i]);
        }
        if let Some(i) = self.tone {
            self.tones[i].0.copy_runtime_state_from(&source.tones[i].0);
        }
        if let Some(i) = self.cabinet {
            self.cabinets[i]
                .0
                .copy_runtime_state_from(&source.cabinets[i].0);
        }
        if self.voice.has_graphic() {
            self.graphic.copy_runtime_state_from(&source.graphic);
        }
        if self.voice.has_reverb_and_tremolo() {
            self.tank.copy_runtime_state_from(&source.tank);
            self.tail.copy_runtime_state_from(&source.tail);
            self.tremolo.copy_runtime_state_from(&source.tremolo);
        }
        self.out_of = source.out_of;
        self.fade_remaining = source.fade_remaining;
        self.prev_output = source.prev_output;
    }

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
        let (gains, powers): (Vec<Simulation>, Vec<Option<Simulation>>) =
            std::thread::scope(|scope| {
                let gains = scope.spawn(|| {
                    (0..VOICES)
                        .map(|i| {
                            let (gain, diode, amplifier) = voice_at(i);
                            Simulation::new(
                                build_voice(gain, diode, amplifier).expect("catalogue builds"),
                                rate,
                            )
                        })
                        .collect()
                });
                let powers = scope.spawn(|| {
                    // One per voice, and then the output stages that belong to
                    // no voice of their own: an amplifier's second rectifier
                    // setting is the same preamplifier's power stage twice over,
                    // so it needs a slot here rather than a catalogue entry.
                    (0..VOICES + EXTRA_POWERS)
                        .map(|i| {
                            if i >= VOICES {
                                let spec = EXTRA_POWER_SPECS[i - VOICES];
                                let built = power::build(spec, 10_000.0).expect("catalogue builds");
                                return Some(Simulation::new(built, rate));
                            }
                            let gain = voice_at(i).0;
                            // Studio line output is kept with its preamp independently.
                            gain.power_stage()
                                .map(|spec| power::build(spec, 10_000.0))
                                .map(|built| {
                                    let mut sim =
                                        Simulation::new(built.expect("catalogue builds"), rate);
                                    // The Twin power stage is the one measured circuit where the
                                    // shortest line-search trials can become a numerical spiral. The
                                    // 1/16 and 1/32 trials barely move an ordinary solve, then the
                                    // following Newton pass searches again. Four trials stop at 1/8 and
                                    // were reference-checked at the same -190..-205 dB error floor,
                                    // while materially reducing realtime misses.  Do not apply this
                                    // to the 5150: its solver genuinely needs the shorter steps and
                                    // the same cap was measured at about -45.7 dB from reference.
                                    if gain == Gain::Twin {
                                        sim.set_backtracks(4);
                                    }
                                    // Source continuation is deliberately Twin-only. The 5150
                                    // already settles every measured sample on the proven normal
                                    // path, and the tight rescue fired only once in 384k samples
                                    // without rescuing anything. Keeping it disabled there restores
                                    // the exact pre-continuation hot loop. The Twin, by contrast,
                                    // measurably benefits from one late midpoint steering step.
                                    if gain == Gain::Twin {
                                        sim.set_late_continuation(true);
                                    }
                                    sim
                                })
                        })
                        .collect()
                });
                (
                    gains.join().expect("gain catalogue builds"),
                    powers.join().expect("power catalogue builds"),
                )
            });
        let initial = LoadValues::new(&SpeakerProfile::BRIT_V30, &Mounting::BAFFLE, 1.0);
        let loaded = PowerModel::ALL
            .iter()
            .map(|model| {
                let spec = model.spec();
                let values = LoadValues::new(
                    &SpeakerProfile::BRIT_V30,
                    &Mounting::BAFFLE,
                    power::speaker_scale(spec),
                );
                let (circuit, slots) = power::build_with_speaker(spec, 10_000.0, &values)
                    .expect("speaker-loaded power builds");
                let motional = circuit
                    .unknown_named(speaker::MOTIONAL)
                    .expect("the driver has a motional node");
                let mut sim = Simulation::new(circuit, rate);
                if *model == PowerModel::American6L6Clean {
                    // The same measured Twin solver settings as the resistive stage.
                    sim.set_backtracks(4);
                    sim.set_late_continuation(true);
                }
                Loaded {
                    sim,
                    slots,
                    motional,
                }
            })
            .collect();
        let (driven_circuit, driven_slots) =
            speaker::voltage_driven(&initial).expect("voltage-driven speaker builds");
        let driven_motional = driven_circuit.output;
        let mut chain = Self {
            mains: 1.0,
            gains,
            pedals: (0..Pedal::SLOTS)
                .map(|slot| Simulation::new(Pedal::build(slot).expect("pedal builds"), rate))
                .collect(),
            pedal: None,
            selected_pedal: Pedal::None,
            pedal_into: 1.0,
            pedal_hand_off: 1.0,
            powers,
            loaded,
            driven: Box::new(Loaded {
                sim: Simulation::new(driven_circuit, rate),
                slots: driven_slots,
                motional: driven_motional,
            }),
            acoustic: Box::new(AcousticStage::new(rate)),
            acoustic_settings: AcousticSettings::default(),
            radiating: false,
            pressure_scale: initial.pressure_scale(),
            motional_previous: 0.0,
            power: 0,
            power_selection: PowerAmp::Matched,
            line: Box::new(Simulation::new(
                neve::output(LOAD, 10_000.0).expect("73P output builds"),
                rate,
            )),
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
            voice: Gain::ALL[0],
            iron: None,
            tone: None,
            cabinet: None,
            over: Oversampler::new(4),
            requested_oversampling: 4,
            pad: Delay::new(1),
            dry: Delay::new(LATENCY as usize),
            tank: Tank::accutronics(rate),
            tail: Simulation::new(
                twin::reverb_return(10_000.0, 1_000_000.0).expect("catalogue builds"),
                rate,
            ),
            tremolo: Tremolo::new(rate),
            reverb: 0.0,
            speed: 0.5,
            intensity: 0.0,
            iron_reference: 1.0,
            rate,
            drive: 0.5,
            master: MASTER_MIDDLE,
            master_lift: 1.0,
            graphic: Simulation::new(
                markiic::graphic(SOURCE, LOAD).expect("the graphic EQ builds"),
                rate,
            ),
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
    ///
    /// The graphic equaliser is reset unconditionally, whether or not the
    /// voice being switched to has one. It is only *in* the path for the
    /// Boogie, but it is a single simulation shared across all voices -- so
    /// switching Boogie -> Crunch -> Boogie without a transport stop would
    /// otherwise carry the Boogie's LC state across the round trip, and the
    /// second Boogie would start with audio the first Boogie left in the
    /// filter from seconds ago. Resetting it on every voice change costs one
    /// rebuild and one DC hunt on a five-band LC network, and removes a way
    /// for state to leak between voices that share nothing else.
    pub fn set_voice(&mut self, gain: Gain, diode: Diode, amplifier: Amplifier) {
        let index = voice_index(gain, diode, amplifier);
        self.voice = gain;
        if index != self.gain {
            self.gain = index;
            self.tank.reset();
            self.tremolo.reset();
            self.tail.reset_deferred();
            self.gains[index].reset_deferred();
            // And its power stage, for the same reason and more so. A power
            // stage sits at four hundred volts with its output transformer
            // carrying the plates' standing current, and `set_voice` used to
            // leave all of it alone: switching back to an amplifier handed a
            // freshly settled preamp to a power stage still holding whatever
            // charge and flux it had when it was last switched away from.
            self.update_power(true);
            if gain == Gain::Neve {
                self.line.reset_deferred();
            }
            // The graphic equaliser, unconditionally. See the doc comment.
            self.graphic.reset_deferred();
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

    fn update_power(&mut self, reset: bool) {
        let next = self
            .power_selection
            .resolved(self.voice)
            .map(PowerModel::index)
            .unwrap_or(0);
        if next != self.power || reset {
            self.power = next;
            if let Some(sim) = self.powers[next].as_mut() {
                sim.reset_deferred();
            }
            match self.power_selection.resolved(self.voice) {
                Some(model) => self.loaded[model.slot()].sim.reset_deferred(),
                None => self.driven.sim.reset_deferred(),
            }
            self.motional_previous = 0.0;
            self.fade_remaining = FADE_LEN;
        }
    }

    /// The power simulation actually in the path: the speaker-loaded one when the
    /// physical path is in use, the resistor-loaded one otherwise.
    fn active_power(&self) -> Option<&Simulation> {
        if self.radiating {
            self.resolved_power_amp()
                .map(|model| &self.loaded[model.slot()].sim)
        } else {
            self.powers[self.power].as_ref()
        }
    }

    fn active_power_mut(&mut self) -> Option<&mut Simulation> {
        if self.radiating {
            match self.power_selection.resolved(self.voice) {
                Some(model) => Some(&mut self.loaded[model.slot()].sim),
                None => None,
            }
        } else {
            self.powers[self.power].as_mut()
        }
    }

    /// The speaker driven with no power stage, when that is what is in the path.
    fn active_driven(&self) -> bool {
        self.radiating && self.resolved_power_amp().is_none()
    }

    /// Speaker, cabinet and microphones. A change of driver, box or path resets the
    /// power stage's load and is covered by the switch fade; placement is ramped.
    pub fn set_acoustic(&mut self, a: &AcousticSettings) {
        let speaker = a.resolved_speaker();
        let previous = self.acoustic_settings;
        let load_changed = speaker.is_some() != self.radiating
            || previous.cabinet != a.cabinet
            || previous.speaker != a.speaker;
        if load_changed {
            self.radiating = speaker.is_some();
            if let Some(profile) = speaker {
                let mounting = a.mounting();
                for (model, loaded) in PowerModel::ALL.iter().zip(self.loaded.iter_mut()) {
                    let values = LoadValues::new(profile, &mounting, power::speaker_scale(model.spec()));
                    loaded.slots.apply(&mut loaded.sim, &values);
                    loaded.sim.reset_deferred();
                }
                let values = LoadValues::new(profile, &mounting, 1.0);
                self.driven.slots.apply(&mut self.driven.sim, &values);
                self.driven.sim.reset_deferred();
                self.pressure_scale = values.pressure_scale();
            }
            if let Some(sim) = self.powers[self.power].as_mut() {
                sim.reset_deferred();
            }
            self.motional_previous = 0.0;
            self.fade_remaining = FADE_LEN;
        }
        if let Some(profile) = speaker {
            let before = (previous.mic_a, previous.mic_b);
            self.acoustic
                .configure(a.resolved_cabinet(), profile, a.mic_a, a.mic_b);
            if load_changed || before != (a.mic_a, a.mic_b) {
                self.fade_remaining = FADE_LEN;
            }
            self.acoustic
                .set_placement(a.place_a, a.place_b, a.blend, a.invert_b, a.align);
        }
        self.acoustic_settings = *a;
        // The master control may now live in a different simulation.
        self.set_master(self.master);
    }

    pub fn is_radiating(&self) -> bool {
        self.radiating
    }

    /// The pedal slot. A pedal takes the guitar at guitar level and hands its
    /// output volts straight to the circuit's input, as a cable would; with none
    /// selected the path is exactly what it was.
    pub fn set_pedal(&mut self, s: &PedalSettings) {
        let next = s.pedal.slot();
        let changed = next != self.pedal;
        if changed {
            if let Some(i) = next {
                self.pedals[i].reset_deferred();
            }
            self.pedal = next;
            self.selected_pedal = s.pedal;
            self.fade_remaining = FADE_LEN;
        }
        if let Some(i) = self.pedal {
            let controls = Pedal::controls(i);
            let sim = &mut self.pedals[i];
            sim.set_control(controls.drive, s.drive.clamp(0.0, 1.0));
            for (knob, &value) in controls.tones.iter().zip(s.tone.iter()) {
                let Some(knob) = knob else { continue };
                let value = value.clamp(0.0, 1.0);
                sim.set_control(knob.control, if knob.inverted { 1.0 - value } else { value });
            }
            let rest = sim
                .resting_position(controls.level)
                .unwrap_or(DEFAULT_MASTER_REST);
            sim.set_control(controls.level, Self::master_position(rest, s.level));
            self.pedal_into = Pedal::input_volts(i) / 10f64.powf(NOMINAL_DBFS / 20.0);
            self.pedal_hand_off = self.into / self.pedal_into;
        }
        if changed {
            // A pedal arriving or leaving can change what the chain can afford
            // to oversample. See `Pedal::is_expensive`.
            self.set_oversampling(self.requested_oversampling);
        }
    }

    pub fn set_power_amp(&mut self, selection: PowerAmp) {
        if selection != self.power_selection {
            self.power_selection = selection;
            self.update_power(false);
            // The make-up belongs to the path, so it changes with it rather than
            // gliding. See `set_voice` for why a glide between paths is wrong.
            self.set_drive(self.drive);
            self.out_of = self.out_of_target;
        }
    }

    /// The make-up correction for a power stage other than the voice's own.
    ///
    /// The calibration table was measured on each voice's own path. Behind a
    /// different output stage the level moved by up to twenty decibels,
    /// because every stage has its own gain, master resting position and
    /// saturation, and a hot preamplifier saturates a clean power stage into
    /// compression. `POWER_TRIM_DB` is that difference, measured at the
    /// calibration drive with `examples/powertrim.rs`; `tests/power_trim.rs`
    /// re-measures it. It is make-up, like the table it corrects, and not part
    /// of any circuit.
    fn power_trim(&self) -> f64 {
        let column = match self.power_selection {
            PowerAmp::Matched => return 1.0,
            PowerAmp::Bypass => 0,
            PowerAmp::Cali6L6 => 1,
            PowerAmp::American6L6Clean => 2,
            PowerAmp::American6L6HighGain => 3,
            PowerAmp::BritEL34 => 4,
            PowerAmp::BritPlexiEL34 => 5,
            PowerAmp::AC30EL84 => 6,
            PowerAmp::DR103EL34 => 7,
            PowerAmp::Recto6L6 => 8,
            PowerAmp::Recto6L6Tube => 9,
        };
        let row = Gain::ALL.iter().position(|g| *g == self.voice).unwrap_or(0);
        10f64.powf(-POWER_TRIM_DB[row][column] / 20.0)
    }

    pub fn resolved_power_amp(&self) -> Option<PowerModel> {
        self.power_selection.resolved(self.voice)
    }

    /// Which output transformer, if any.
    pub fn set_iron(&mut self, iron: Iron) {
        let next = iron.index();
        if next != self.iron {
            if let Some(i) = next {
                self.irons[i].reset_deferred();
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
                self.tones[i].0.reset_deferred();
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
                self.cabinets[i].0.reset_deferred();
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
    /// Where the panel's Master knob puts the circuit's own level control.
    ///
    /// Not the pot's rotation directly, and the reason is the make-up table.
    /// Every voice's `make_up_db` was measured with its level control at the
    /// position the circuit rests it at -- 0.70 for the pedals, 0.85 for the
    /// 73P, 0.66 for a 5150's post gain, 0.30 for a Mark IIC+'s Lead Master.
    /// Those differ because the voicings differ, and a knob that read the pot
    /// straight would put every circuit somewhere it was not voiced the moment
    /// it was selected, shifting its level and widening a catalogue spread
    /// that is already thirteen decibels.
    ///
    /// So the middle of this knob is **where the circuit was calibrated**, and
    /// either side of it is louder or quieter than the voicing. Two straight
    /// segments through (0, 0), (0.5, rest) and (1, 1): at 0.5 every voice is
    /// bit-identical to what it was before there was a knob at all, which
    /// `tests/master.rs` holds, and the ends still reach the ends of the real
    /// pot's travel.
    fn master_position(rest: f64, knob: f64) -> f64 {
        let knob = knob.clamp(0.0, 1.0);
        if knob <= 0.5 {
            rest * knob * 2.0
        } else {
            rest + (1.0 - rest) * (knob - 0.5) * 2.0
        }
    }

    /// What the knob adds once the pot has nothing left to give.
    ///
    /// The pot alone makes a lopsided control. A device rests its level knob
    /// near the top of its travel when that is where it was voiced -- the 73P
    /// rests its output trim at **0.85** -- so putting the voicing at twelve
    /// o'clock leaves fifteen per cent of the track spread across the whole
    /// upper half of the knob. Measured over the full sweep: the 73P gave
    /// **-21.1 dB down and +3.3 dB up**, and the Mark IIC+ +3.3 dB up for a
    /// different reason -- its Lead Master rests at 0.30, but by the top the
    /// power amplifier is saturating and the extra rotation buys level it
    /// cannot deliver. Both read as a knob that does nothing when turned up.
    ///
    /// So above the middle the knob carries the pot to its stop *and* leans on
    /// the plugin's own make-up, by up to `MASTER_LIFT_DB`. **This part is not
    /// the circuit**: past the pot's stop there is no more level in the
    /// hardware, and what the knob is turning is the same output gain the
    /// make-up table already applies. It is an approximation in the sense of
    /// §24.1 category 4, made for the control to be useful across its travel,
    /// and it is why the lift is small and stated rather than large and
    /// silent. Below the middle, and at it, this is exactly one.
    fn master_lift(knob: f64) -> f64 {
        let above = (knob.clamp(0.0, 1.0) - 0.5).max(0.0) * 2.0;
        10f64.powf(above * MASTER_LIFT_DB / 20.0)
    }

    /// The five graphic equaliser sliders, bottom band first.
    pub fn set_graphic(&mut self, bands: [f64; 5]) {
        if !voice_at(self.gain).0.has_graphic() {
            return;
        }
        for (band, &position) in bands.iter().enumerate() {
            self.graphic.set_control(band, position);
        }
    }

    /// The circuit's own output level control, from the panel's Master knob.
    pub fn set_master(&mut self, knob: f64) {
        self.master = knob;
        let voice = voice_at(self.gain).0;
        let overridden = self.power_selection != PowerAmp::Matched && self.power != 0;
        let level = if overridden {
            // In a custom chain the selected output stage owns the master. The
            // preamplifier's own level control -- a pedal's Level, the 73P's
            // trim -- goes back to the position it was calibrated at, rather
            // than staying wherever the knob last left it before the override.
            if let Some(Level::Circuit(which)) = voice.level_control() {
                let sim = &mut self.gains[self.gain];
                let rest = sim.resting_position(which).unwrap_or(DEFAULT_MASTER_REST);
                sim.set_control(which, rest);
            }
            Some(Level::Power(power::MASTER))
        } else {
            voice.level_control()
        };
        let resolved = self.power_selection.resolved(self.voice);
        // A power stage the knob does not own goes back to where it rests. Without
        // this, a Twin power stage that had been driven from another preamplifier
        // kept that chain's master position (0.30) when the Twin's own voice took it
        // back on Matched -- where nothing sets it, because an AB763 has no master --
        // and its dry signal came out 27 dB down under the reverb.
        if !matches!(level, Some(Level::Power(_))) {
            let loaded = resolved.map(|model| &mut self.loaded[model.slot()].sim);
            for sim in self.powers[self.power].as_mut().into_iter().chain(loaded) {
                if let Some(rest) = sim.resting_position(power::MASTER) {
                    sim.set_control(power::MASTER, rest);
                }
            }
        }
        let Some(level) = level else {
            self.master_lift = 1.0;
            return;
        };
        let (sim, which) = match level {
            Level::Circuit(which) => (Some(&mut self.gains[self.gain]), which),
            Level::Power(which) => (self.powers[self.power].as_mut(), which),
        };
        let Some(sim) = sim else {
            self.master_lift = 1.0;
            return;
        };
        self.master_lift = Self::master_lift(knob);
        let mut rest = sim.resting_position(which).unwrap_or(DEFAULT_MASTER_REST);
        // A power stage whose amplifier has no master (the Twin's rests wide open)
        // cannot rest wide open behind somebody else's preamplifier. Measured with
        // the 5150 preamplifier into it: 7.4 Newton passes a sample, 2,400
        // fallbacks and missed deadlines, the inverter driven far into grid
        // current. At 0.30, which is where the Mark IIC+ and 2203 stages rest, the
        // same combination solves in 3.2 passes with no fallbacks. So in a custom
        // chain the knob's middle puts it there -- the position of an attenuating
        // return rather than a control that amplifier has.
        if overridden && matches!(level, Level::Power(_)) && rest >= 0.99 {
            rest = OVERRIDE_MASTER_REST;
        }
        let position = Self::master_position(rest, knob);
        sim.set_control(which, position);
        // The speaker-loaded twin of that power stage carries the same control.
        if let (Level::Power(which), Some(model)) = (level, resolved) {
            self.loaded[model.slot()].sim.set_control(which, position);
        }
    }

    pub fn set_drive(&mut self, drive: f64) {
        self.drive = drive.clamp(0.0, 1.0);
        let which = voice_at(self.gain).0.drive_control();
        self.gains[self.gain].set_control(which, self.drive);
        let calibration = CALIBRATION[self.gain];
        // A nominal digital signal has to arrive as the stated voltage.
        let nominal = 10f64.powf(NOMINAL_DBFS / 20.0);
        self.into = calibration.drive_volts / nominal;
        // The circuit's own input level moved, so what the pedal hands it does
        // too. See `pedal_hand_off`.
        self.pedal_hand_off = self.into / self.pedal_into;
        // The Master knob's lift rides with the make-up, because that is what
        // it is: the same output gain, turned by hand. See `master_lift`.
        let trim = self.power_trim();
        self.out_of_target = 10f64.powf(calibration.make_up_db_at(self.drive) / 20.0) / self.into
            * self.master_lift
            * trim;
        // What the make-up would be with the Drive control at its reference
        // position. See `iron_drive`.
        self.iron_reference =
            10f64.powf(calibration.make_up_db_at(IRON_REFERENCE_DRIVE) / 20.0) / self.into * trim;
    }

    /// How much harder than usual the Drive control is pushing the iron.
    ///
    /// One, at the reference position; more above it, less below.
    ///
    /// The make-up holds the output level constant whatever the Drive knob is
    /// doing -- that is its whole job -- so anything sitting behind it is
    /// handed the same level at every setting and is never driven any harder.
    /// The power stage was moved in front of the make-up for exactly that
    /// reason and the comment there says so; the iron was left behind it and
    /// the same argument was never applied. Reported from a DAW as the Iron
    /// control not responding to Drive, and it did not.
    ///
    /// Handing the iron the raw pre-make-up signal instead would be the
    /// obvious fix and is wrong: the make-up spans about sixty decibels across
    /// the catalogue, so a transformer scaled for the TS808 would be inert on
    /// a Clean voice and destroyed on the 5150. A transformer is *matched to
    /// the stage that drives it* -- nobody bolts a 5150's output transformer
    /// on to a Tube Screamer -- so the reference is per voice, and what varies
    /// is how far the knob has moved from it.
    pub fn iron_drive(&self) -> f64 {
        if self.out_of > 0.0 {
            self.iron_reference / self.out_of
        } else {
            1.0
        }
    }

    pub fn set_tone(&mut self, which: usize, position: f64) {
        if let Some(i) = self.tone {
            self.tones[i].0.set_control(which, position);
        }
    }

    /// The three tone knobs, sent to whichever stack is actually in the path:
    /// the circuit's own where it has one, the plugin's otherwise.
    fn set_tone_knobs(&mut self, bass: f64, mid: f64, treble: f64, sweep: f64) {
        let voice = voice_at(self.gain).0;
        if let Some((b, m, t)) = voice.own_tone() {
            let backwards = voice.tone_runs_backwards();
            let sim = &mut self.gains[self.gain];
            for (which, value) in [(b, bass), (m, mid), (t, treble)] {
                if which != usize::MAX {
                    sim.set_control(which, if backwards { 1.0 - value } else { value });
                }
            }
        }
        if let Some((which, _)) = voice.own_sweep() {
            self.gains[self.gain].set_control(which, sweep.clamp(0.0, 1.0));
        }
        self.set_tone(crate::circuits::tone::BASS, bass);
        self.set_tone(crate::circuits::tone::MID, mid);
        self.set_tone(crate::circuits::tone::TREBLE, treble);
    }

    /// Keep every circuit that executes inside `Oversampler::process` on the
    /// oversampler's effective timestep.
    ///
    /// This includes the gain circuit, the optional power amplifier, the Mark
    /// graphic equaliser and the transformer. The modelled amplifier voices
    /// are currently pinned to 1x, but keeping all four tied to the actual
    /// factor prevents a latent sample-rate bug the moment that policy changes.
    fn sync_oversampled_rates(&mut self) {
        let inner = self.rate * self.over.factor() as f64;
        for sim in self
            .gains
            .iter_mut()
            .chain(self.powers.iter_mut().flatten())
            .chain(self.irons.iter_mut())
            .chain(std::iter::once(self.line.as_mut()))
            .chain(self.loaded.iter_mut().map(|l| &mut l.sim))
            .chain(std::iter::once(&mut self.driven.sim))
            .chain(self.pedals.iter_mut())
        {
            sim.set_rate(inner);
        }
        self.graphic.set_rate(inner);
    }

    /// Sets the oversampling factor -- and tells every circuit running inside
    /// it about the resulting timestep.
    ///
    /// These two have to move together. The oversampler hands its callback
    /// several samples for every one the host sent, so a circuit still solving
    /// with the host's timestep has every capacitor too slow and every corner
    /// frequency too high. The factor change itself also resets the halfband
    /// FIR histories, so it is installed on the first sample of a fresh
    /// crossfade instead of in the middle of otherwise continuous audio.
    /// The modelled circuits are capped at `MODELLED_MAX_OVERSAMPLING` rather
    /// than following the control all the way up. See that constant.
    pub fn set_oversampling(&mut self, factor: usize) {
        let requested_changed = factor != self.requested_oversampling;
        self.requested_oversampling = factor;
        let mut factor = if voice_at(self.gain).0.is_modelled() {
            factor.min(MODELLED_MAX_OVERSAMPLING)
        } else {
            factor
        };
        // ...and the pedal slot counts too. The circuit is not the only thing
        // inside the oversampler: the pedal runs there as well, so a large one
        // is paid for at the same multiple. See `Pedal::is_expensive`.
        if self.pedal.is_some() && self.selected_pedal.is_expensive() {
            factor = 1;
        }

        if !requested_changed && factor == self.over.factor() && self.deferred_oversample.is_none()
        {
            // `Chain::new` constructs the oversampler at its default factor
            // before the simulations know that effective rate. Even when the
            // requested factor is already active, these two derived pieces
            // of state still have to be brought into agreement with it.
            // Keeping this invariant here also makes restoring/reapplying a
            // quality setting idempotent instead of depending on constructor
            // details.
            self.pad
                .set_len((LATENCY - self.over.latency().min(LATENCY)) as usize);
            self.sync_oversampled_rates();
            return;
        }

        if factor != self.over.factor() {
            // `Oversampler::set_factor` clears the FIR history. Restart the
            // switch fade and perform that reset at its first sample, where
            // `process` can cover the discontinuity deliberately.
            self.deferred_oversample = Some(factor);
            self.fade_remaining = FADE_LEN;
        } else {
            // A request can change back before the next sample. Do not leave
            // the old factor queued after the panel has returned to the factor
            // that is already active.
            self.deferred_oversample = None;
        }

        self.pad
            .set_len((LATENCY - self.over.latency().min(LATENCY)) as usize);
        self.sync_oversampled_rates();
    }

    /// Retune the complete chain for a new host rate.
    ///
    /// The plugin currently rebuilds `Chain` in `initialize` when the host rate
    /// changes, so this is a reconfiguration surface rather than something the
    /// audio callback calls. Rebuilding the spring tank is intentional: its
    /// physical delay lengths are stored as sample counts and cannot be made
    /// correct at a new rate by changing a scalar alone.
    pub fn set_rate(&mut self, rate: f64) {
        if (self.rate - rate).abs() <= 1e-9 {
            return;
        }
        self.rate = rate;
        self.sync_oversampled_rates();
        for (sim, _) in self.tones.iter_mut().chain(self.cabinets.iter_mut()) {
            sim.set_rate(rate);
        }
        self.tail.set_rate(rate);
        self.tremolo.set_rate(rate);
        self.tank = Tank::accutronics(rate);
        self.acoustic.set_rate(rate);
    }

    /// The factor actually used by the nonlinear gain path. A modelled circuit
    /// reports at most `MODELLED_MAX_OVERSAMPLING`, however high the control is
    /// set, so that it stays safe to play live.
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

    /// Every solve the selected voice runs, added up.
    ///
    /// `passes_per_sample` reports the gain circuit alone, which for a voice
    /// with a power amplifier is a third of its solver work -- and the third
    /// that is usually not the problem. The 5150's power stage was measured at
    /// 2.78 passes a sample and 33 % of realtime while it was actually taking
    /// 6.17 and 71 %, because it was being fed a clean tone instead of what
    /// its own preamplifier sends it.
    ///
    /// Returns solves, passes, unsettled, backtracks, fallbacks, non-finite
    /// corrections and abandoned pivot orders, over the gain circuit, the
    /// power amplifier and the transformer together.
    pub fn solver_breakdown(&self) -> SolverBreakdown {
        fn health(sim: &Simulation) -> SolverHealth {
            let (solves, passes, unsettled, _) = sim.statistics();
            let (backtracks, fallbacks, nonfinite) = sim.health();
            let (
                attack_predictor_suppressions,
                continuation_attempts,
                continuation_midpoint_successes,
                continuation_successes,
                continuation_actual_rescues,
            ) = sim.continuation_health();
            SolverHealth {
                solves,
                passes,
                unsettled,
                backtracks,
                fallbacks,
                nonfinite,
                replans: sim.replans(),
                attack_predictor_suppressions,
                continuation_attempts,
                continuation_midpoint_successes,
                continuation_successes,
                continuation_actual_rescues,
            }
        }

        let gain = health(&self.gains[self.gain]);
        let power = if self.active_driven() {
            health(&self.driven.sim)
        } else {
            self.active_power().map(health).unwrap_or_default()
        };
        let iron = self
            .iron
            .map(|index| health(&self.irons[index]))
            .unwrap_or_default();
        let reverb_return = if self.voice.has_reverb_and_tremolo() && self.reverb > 0.0 {
            health(&self.tail)
        } else {
            SolverHealth::default()
        };

        SolverBreakdown {
            pedal: self.pedal.map(|i| health(&self.pedals[i])).unwrap_or_default(),
            line: if self.voice == Gain::Neve {
                health(&self.line)
            } else {
                SolverHealth::default()
            },
            gain,
            power,
            iron,
            reverb_return,
        }
    }

    #[cfg(test)]
    pub fn power_solver_trace(&self) -> &[crate::dsp::time::SolverTrace] {
        self.active_power()
            .map(Simulation::solver_trace)
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub fn unsettled_power_solver_trace(&self) -> &[crate::dsp::time::SolverTrace] {
        self.active_power()
            .map(Simulation::unsettled_solver_trace)
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub fn power_solver_unknown_name(&self, at: usize) -> Option<&str> {
        self.active_power()
            .map(|simulation| simulation.solver_unknown_name(at))
    }

    pub fn solver_health(&self) -> SolverHealth {
        let mut h = SolverHealth::default();
        let reverb = if self.voice.has_reverb_and_tremolo() && self.reverb > 0.0 {
            Some(&self.tail)
        } else {
            None
        };
        let sims = std::iter::once(&self.gains[self.gain])
            .chain(self.pedal.map(|i| &self.pedals[i]))
            .chain(self.active_power())
            .chain(self.active_driven().then_some(&self.driven.sim))
            .chain((self.voice == Gain::Neve).then_some(self.line.as_ref()))
            .chain(self.iron.map(|i| &self.irons[i]))
            .chain(reverb);
        for sim in sims {
            let (solves, passes, unsettled, _) = sim.statistics();
            let (backtracks, fallbacks, nonfinite) = sim.health();
            h.solves += solves;
            h.passes += passes;
            h.unsettled += unsettled;
            h.backtracks += backtracks;
            h.fallbacks += fallbacks;
            h.nonfinite += nonfinite;
            h.replans += sim.replans();
            let (suppressed, attempts, midpoint_successes, successes, actual_rescues) =
                sim.continuation_health();
            h.attack_predictor_suppressions += suppressed;
            h.continuation_attempts += attempts;
            h.continuation_midpoint_successes += midpoint_successes;
            h.continuation_successes += successes;
            h.continuation_actual_rescues += actual_rescues;
        }
        h
    }

    #[cfg(test)]
    pub(crate) fn test_power(&self) -> Option<&Simulation> {
        self.active_power()
    }

    /// Puts a whole panel's worth of settings onto the chain.
    ///
    /// Every one of them, in one place. Anything that reaches a circuit has to
    /// go through here, so that forgetting one is a change to this function
    /// rather than a line quietly missing from a loop somewhere.
    pub fn apply(&mut self, s: &Settings) {
        self.set_mains(s.mains);
        self.set_pedal(&s.pedal);
        self.set_voice(s.gain, s.diode, s.amplifier);
        // After `set_voice`, because which control this reaches depends on
        // which circuit is selected, and before `set_drive`, because both
        // touch the same simulation and the order they dirty it in should not
        // matter but reading in signal order is how this function is checked.
        self.set_power_amp(s.power_amp);
        self.set_acoustic(&s.acoustic);
        self.set_master(s.master);
        self.set_graphic(s.graphic);
        self.set_iron(s.iron);
        self.set_tone_section(s.tone);
        self.set_cabinet(s.cabinet);
        self.set_oversampling(s.oversampling);
        self.set_drive(s.drive);
        self.set_tone_knobs(s.bass, s.mid, s.treble, s.tone_sweep);
        self.set_reverb_and_tremolo(s);
    }

    /// What the amplifier is plugged into, as a fraction of its own mains.
    ///
    /// One is the wall. Less is a variac, which is a thing players did and some
    /// records depend on: every rail in the amplifier comes down together, so
    /// the valves run out of room earlier and the whole thing goes soft and
    /// compressed at a volume it used to be clean at. It reaches every
    /// simulation that has a supply in it -- preamplifier, power stage, the
    /// speaker-loaded twins and the studio line output -- because a variac feeds
    /// the amplifier, not one stage of it.
    ///
    /// It is a setting rather than a knob: it is how the rig was wired, and
    /// nobody sweeps it while playing.
    pub fn set_mains(&mut self, fraction: f64) {
        if (self.mains - fraction).abs() < 1e-9 {
            return;
        }
        self.mains = fraction;
        for sim in self
            .gains
            .iter_mut()
            .chain(self.pedals.iter_mut())
            .chain(self.powers.iter_mut().flatten())
            .chain(self.loaded.iter_mut().map(|l| &mut l.sim))
            .chain(std::iter::once(&mut self.driven.sim))
            .chain(std::iter::once(self.line.as_mut()))
            .chain(std::iter::once(&mut self.tail))
        {
            sim.set_supply_scale(fraction);
        }
        // Every rail moved, so every operating point did.
        self.fade_remaining = FADE_LEN;
    }

    pub fn mains(&self) -> f64 {
        self.mains
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
        // FIR-history discontinuity is covered by the fade, and every circuit
        // in the oversampled path sees the new timestep before it sees audio.
        if self.fade_remaining == FADE_LEN {
            if let Some(factor) = self.deferred_oversample.take() {
                self.over.set_factor(factor);
                self.pad
                    .set_len((LATENCY - self.over.latency().min(LATENCY)) as usize);
                self.sync_oversampled_rates();
            }
        }

        // Every nonlinear section before the make-up lives inside the same
        // oversampler callback: gain, optional power amplifier and optional
        // transformer. The Mark graphic EQ is linear but sits there because
        // that is its physical position between preamp and power stage. The
        // plugin tone section and cabinet remain at host rate below.
        // One pole toward the target: about a millisecond at any sample rate
        // the plugin is likely to see.
        self.out_of += (self.out_of_target - self.out_of) * 0.02;
        let iron_reference = self.iron_reference;
        let gain = &mut self.gains[self.gain];
        let iron_trim = self.iron.map(|i| IRON_TRIM[i]).unwrap_or(1.0);
        let mut iron = self.iron.map(|i| &mut self.irons[i]);
        let out_of = self.out_of;
        // The physical path swaps the resistor-loaded power stage for its
        // speaker-loaded twin (or a voltage-driven speaker when there is no power
        // stage) and hands on the cone's radiation instead of the terminal voltage.
        let radiating = self.radiating;
        let pressure_scale = self.pressure_scale;
        let inner_rate = self.rate * self.over.factor() as f64;
        let motional_previous = &mut self.motional_previous;
        let (mut power, mut driven, motional) = if radiating {
            match self.power_selection.resolved(self.voice) {
                Some(model) => {
                    let loaded = &mut self.loaded[model.slot()];
                    (Some(&mut loaded.sim), None, loaded.motional)
                }
                None => (None, Some(&mut self.driven.sim), self.driven.motional),
            }
        } else {
            (self.powers[self.power].as_mut(), None, 0)
        };
        let mut line = (self.voice == Gain::Neve).then_some(self.line.as_mut());
        let graphic = self.voice.has_graphic();
        let self_graphic = &mut self.graphic;
        // The Twin's reverb and tremolo. Neither is a netlist part and both
        // are in the signal path, so they go here rather than nowhere.
        //
        // The reverb is a send and a return: the tank between the driver's
        // transformer and the recovery stage's grid, and the recovery stage
        // solved as the circuit it is. `twin::reverb_return` says what this
        // arrangement departs from on the drawing and why it has to.
        let twin = self.voice.has_reverb_and_tremolo();
        let wet = if twin && self.reverb > 0.0 {
            let sent = self.tank.process(x * self.into * twin::SEND_GAIN);
            // Divided by the path's own gain, because the dry has been through
            // the make-up and this has not. See `twin::RETURN_TRIM`.
            self.tail.process(sent) * twin::RETURN_TRIM
        } else {
            0.0
        };
        // The tremolo's cell shunts the signal where the channel hands over to
        // the phase inverter: a valve plate's own resistance in front of it and
        // the inverter's grid leak behind. `Tremolo::attenuation` is that
        // divider rather than a depth.
        let throb = if twin && self.intensity > 0.0 {
            self.tremolo.attenuation(self.speed, self.intensity)
        } else {
            1.0
        };

        let mut pedal = self.pedal.map(|i| &mut self.pedals[i]);
        let input_scale = if pedal.is_some() { self.pedal_into } else { self.into };
        let hand_off = self.pedal_hand_off;
        let mut y = self.over.process(x * input_scale, &mut |v| {
            // The pedal, when there is one, between the guitar and the circuit:
            // a guitar's level in, and out at the level the circuit behind it
            // was calibrated for. See `pedal_hand_off`.
            let v = match pedal {
                Some(ref mut sim) => sim.process(v) * hand_off,
                None => v,
            };
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
            // naming. The power stage itself executes in this callback, so it
            // must use the same effective sample rate as the preamplifier.
            let mut amplified = gain.process(v);
            // The graphic equaliser, where the drawing puts it: `EQ INPUT` is
            // taken from `LEAD OUTPUT`, which is where the preamplifier above
            // stops, and `EQ OUTPUT` goes to the phase inverter. Late in the
            // preamplifier and *before* the power stage, which is the whole
            // point of it -- a low band lifted here lands on the power section
            // after four gain stages, and the same equaliser at the end of the
            // chain would be a different amplifier (§9.8).
            if graphic {
                // A unity-gain feedback equaliser with its sliders centred; see
                // `markiic::graphic`.
                amplified = self_graphic.process(amplified);
            }
            if let Some(ref mut sim) = line {
                amplified = sim.process(amplified);
            }
            if radiating {
                // A transformer after a loudspeaker has no meaning, so on the
                // physical path the Iron control sits where an interstage
                // transformer would: in front of the output stage. Level neutral,
                // exactly as below.
                if let Some(ref mut sim) = iron {
                    let scale = iron_reference * IRON_VOLTS;
                    amplified = sim.process(amplified * scale) * iron_trim / scale;
                }
                let cone = match (power.as_mut(), driven.as_mut()) {
                    (Some(sim), _) => {
                        sim.process(amplified);
                        sim.voltage_at(motional)
                    }
                    (None, Some(sim)) => sim.process(amplified),
                    (None, None) => 0.0,
                };
                // On-axis pressure is proportional to cone acceleration, and
                // `Re Mms / Bl^2 dV(mot)/dt` is that pressure normalised to one
                // volt at the terminals in the mass-controlled band.
                let pressure = pressure_scale * (cone - *motional_previous) * inner_rate;
                *motional_previous = cone;
                return pressure * out_of;
            }
            if let Some(ref mut sim) = power {
                amplified = sim.process(amplified);
            }
            // The iron goes here, in front of the make-up, for the same
            // reason the power stage does and by the same argument.
            //
            // Handed volts rather than a number near one, because what a core
            // does depends on the flux and flux is in volt seconds. Scaled by
            // `iron_drive`, which is how far the Drive control has moved from
            // the position this voice's transformer is matched at -- so
            // turning up drives the iron harder, which is the one thing a
            // transformer in this position is for. See `IRON_VOLTS`.
            let amplified = match iron {
                Some(ref mut sim) => {
                    // Normalised by the make-up **at the reference drive**
                    // rather than at the current one. At that position this is
                    // exactly what the iron used to be handed; above it the
                    // circuit is putting out more and the transformer gets it,
                    // and below it less.
                    //
                    // The scale factor divides back out, so what survives is
                    // the core's nonlinearity and nothing else -- handing the
                    // raw pre-make-up volts instead put a valve stage's tens
                    // of volts through a ninety-six times multiplier and
                    // measured 132 per cent distortion at the bottom of the
                    // Drive control.
                    let scale = iron_reference * IRON_VOLTS;
                    sim.process(amplified * scale) * iron_trim / scale
                }
                None => amplified,
            };
            amplified * out_of
        });
        // The tremolo shunts the channel's output, and the reverb is summed
        // on to it. Both before the tone section and the cabinet, which is
        // where they sit on the amplifier: the tank and the optical cell are
        // in the preamplifier, and what follows is the speaker.
        y = y * throb + wet;
        // Every setting delays by the same reported amount.
        y = self.pad.process(y);
        if let Some(i) = self.tone {
            let (sim, trim) = &mut self.tones[i];
            y = sim.process(y) * *trim;
        }
        if self.radiating {
            y = self.acoustic.process(y);
        } else if matches!(self.acoustic_settings.cabinet, CabinetChoice::Legacy) {
            if let Some(i) = self.cabinet {
                let (sim, trim) = &mut self.cabinets[i];
                y = sim.process(y) * *trim;
            }
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

    /// The Twin's own three, carried through `apply` like everything else
    /// that reaches a circuit.
    fn set_reverb_and_tremolo(&mut self, s: &Settings) {
        self.reverb = s.reverb;
        self.speed = s.speed;
        self.intensity = s.intensity;
        // The Reverb control is a pot in the recovery stage's own circuit, so
        // it goes where every other control goes: into the simulation, once a
        // block, through `apply`.
        self.tail.set_control(twin::REVERB, s.reverb);
    }

    /// Cap the Newton passes every circuit in this chain may take.
    ///
    /// Layer 5a's lever. See `Simulation::ceiling` for why it is floored and
    /// why this is not the mistake BUG-008 records.
    pub fn set_pass_ceiling(&mut self, passes: usize) {
        for sim in self
            .gains
            .iter_mut()
            .chain(self.powers.iter_mut().flatten())
            .chain(self.irons.iter_mut())
            .chain(std::iter::once(self.line.as_mut()))
            .chain(std::iter::once(&mut self.tail))
            .chain(self.loaded.iter_mut().map(|l| &mut l.sim))
            .chain(std::iter::once(&mut self.driven.sim))
            .chain(self.pedals.iter_mut())
        {
            sim.set_pass_ceiling(passes);
        }
    }

    /// How many samples in this chain finished at the ceiling rather than by
    /// converging, across every circuit it holds.
    pub fn pinched(&self) -> u64 {
        self.gains
            .iter()
            .chain(self.powers.iter().flatten())
            .chain(self.irons.iter())
            .chain(std::iter::once(self.line.as_ref()))
            .chain(std::iter::once(&self.tail))
            .chain(self.loaded.iter().map(|l| &l.sim))
            .chain(std::iter::once(&self.driven.sim))
            .chain(self.pedals.iter())
            .map(|s| s.pinched())
            .sum()
    }

    /// Whether any active nonlinear section is at rest with a dirty matrix and
    /// therefore needs its zero-input DC solution before audio starts.
    ///
    /// This deliberately does not include the linear tone, cabinet or graphic
    /// sections: they rebuild directly on their next sample and have no Newton
    /// operating-point hunt to perform.
    pub fn needs_operating_point(&self) -> bool {
        self.gains[self.gain].needs_operating_point()
            || self.pedal.is_some_and(|i| self.pedals[i].needs_operating_point())
            || self
                .iron
                .is_some_and(|i| self.irons[i].needs_operating_point())
            || (self.voice == Gain::Neve && self.line.needs_operating_point())
            || self.active_power().is_some_and(|s| s.needs_operating_point())
            || (self.active_driven() && self.driven.sim.needs_operating_point())
            || (self.voice.has_reverb_and_tremolo()
                && self.reverb > 0.0
                && self.tail.needs_operating_point())
    }

    /// The operating point of the active voice's power stage, if it has one.
    pub fn power_operating_point(&self) -> Option<&[f64]> {
        if self.active_driven() {
            return Some(self.driven.sim.operating_point());
        }
        self.active_power().map(|s| s.operating_point())
    }

    /// Apply a pre-computed operating point to the active voice's power stage.
    ///
    /// The guard is essential: applying an operating point to a simulation
    /// that already has audio in flight replaces its capacitor/inductor state.
    pub fn share_power_operating_point_from(&mut self, op: &[f64]) {
        let sim = if self.active_driven() {
            Some(&mut self.driven.sim)
        } else {
            self.active_power_mut()
        };
        if let Some(sim) = sim {
            if sim.needs_operating_point() {
                sim.apply_operating_point(op);
            }
        }
    }

    pub fn pedal_operating_point(&self) -> Option<&[f64]> {
        self.pedal.map(|i| self.pedals[i].operating_point())
    }

    pub fn share_pedal_operating_point_from(&mut self, op: &[f64]) {
        if let Some(i) = self.pedal {
            if self.pedals[i].needs_operating_point() {
                self.pedals[i].apply_operating_point(op);
            }
        }
    }

    pub fn line_operating_point(&self) -> Option<&[f64]> {
        (self.voice == Gain::Neve).then(|| self.line.operating_point())
    }

    pub fn share_line_operating_point_from(&mut self, op: &[f64]) {
        if self.voice == Gain::Neve && self.line.needs_operating_point() {
            self.line.apply_operating_point(op);
        }
    }

    /// The Twin recovery stage's operating point, for sharing with another
    /// otherwise identical channel before either channel has processed audio.
    pub fn reverb_operating_point(&self) -> Option<&[f64]> {
        if self.voice.has_reverb_and_tremolo() && self.reverb > 0.0 {
            Some(self.tail.operating_point())
        } else {
            None
        }
    }

    /// Apply a pre-computed Twin recovery-stage operating point when it is
    /// still safe to do so.
    pub fn share_reverb_operating_point_from(&mut self, op: &[f64]) {
        if self.voice.has_reverb_and_tremolo()
            && self.reverb > 0.0
            && self.tail.needs_operating_point()
        {
            self.tail.apply_operating_point(op);
        }
    }

    /// Hunts only the operating points that are actually pending.
    ///
    /// This distinction matters in a running stereo chain. One newly selected
    /// transformer can be at rest while the gain stage is already carrying
    /// audio; solving *all* sections here would erase the gain stage's dynamic
    /// capacitor state just because the transformer needed a DC solution.
    pub fn find_operating_point(&mut self) -> bool {
        let mut settled = true;

        if self.gains[self.gain].needs_operating_point() {
            settled &= self.gains[self.gain].find_operating_point();
        }
        if let Some(i) = self.pedal {
            if self.pedals[i].needs_operating_point() {
                settled &= self.pedals[i].find_operating_point();
            }
        }
        if let Some(i) = self.iron {
            if self.irons[i].needs_operating_point() {
                settled &= self.irons[i].find_operating_point();
            }
        }
        if let Some(sim) = self.active_power_mut() {
            if sim.needs_operating_point() {
                settled &= sim.find_operating_point();
            }
        }
        if self.active_driven() && self.driven.sim.needs_operating_point() {
            settled &= self.driven.sim.find_operating_point();
        }
        if self.voice == Gain::Neve && self.line.needs_operating_point() {
            settled &= self.line.find_operating_point();
        }
        if self.voice.has_reverb_and_tremolo()
            && self.reverb > 0.0
            && self.tail.needs_operating_point()
        {
            settled &= self.tail.find_operating_point();
        }

        settled
    }

    /// Apply a pre-computed operating point to the active gain circuit.
    /// Used when sharing a DC solution between identical channels.
    pub fn share_operating_point_from(&mut self, gain_op: &[f64]) {
        if self.gains[self.gain].needs_operating_point() {
            self.gains[self.gain].apply_operating_point(gain_op);
        }
    }

    /// Apply a pre-computed operating point to the active iron stage.
    pub fn share_iron_operating_point_from(&mut self, iron_op: &[f64]) {
        if let Some(i) = self.iron {
            if self.irons[i].needs_operating_point() {
                self.irons[i].apply_operating_point(iron_op);
            }
        }
    }

    pub fn reset(&mut self) {
        self.out_of = self.out_of_target;
        // A transport reset also ends any fade from the previous stereo
        // stream. No channel-specific output may survive a complete reset.
        self.fade_remaining = if self.deferred_oversample.is_some() {
            FADE_LEN
        } else {
            0
        };
        self.prev_output = 0.0;
        for sim in self
            .gains
            .iter_mut()
            .chain(self.irons.iter_mut())
            .chain(std::iter::once(self.line.as_mut()))
            .chain(self.powers.iter_mut().flatten())
            .chain(self.loaded.iter_mut().map(|l| &mut l.sim))
            .chain(std::iter::once(&mut self.driven.sim))
            .chain(self.pedals.iter_mut())
        {
            sim.reset_deferred();
        }
        self.acoustic.reset();
        self.motional_previous = 0.0;
        for (sim, _) in self.tones.iter_mut().chain(self.cabinets.iter_mut()) {
            sim.reset_deferred();
        }
        // The four things `reset` used to miss. `graphic` is in the Boogie's
        // path and its LC state is large enough to be heard on its own;
        // `tail` is the reverb recovery stage, `tank` the spring and
        // `tremolo` the oscillator -- all three retain signal between calls
        // unless told otherwise. `set_voice` already resets the last three
        // and the graphic as well, but a reset is a bigger operation than a
        // voice switch and everything stateful belongs in it.
        self.graphic.reset_deferred();
        self.tail.reset_deferred();
        self.tank.reset();
        self.tremolo.reset();
        self.over.reset();
        self.pad.reset();
        self.dry.reset();
    }
}
