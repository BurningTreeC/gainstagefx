//! What the plugin ships knowing how to sound like.
//!
//! Every preset is a full set of panel positions, so loading one and then
//! looking at the panel tells you how the sound is made -- there is nothing
//! hidden behind a name. That is the point of them here: the catalogue is
//! small enough that a preset is a worked example rather than a substitute for
//! understanding the controls.
//!
//! They are grouped by what they are for rather than by which circuit they
//! use, because that is how somebody looking for a sound is thinking.

use crate::params::{Cabinet, Circuit, Diode, ToneStack};

pub struct Preset {
    pub group: &'static str,
    pub name: &'static str,
    pub circuit: Circuit,
    pub diode: Diode,
    pub drive: f32,
    pub tone: ToneStack,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub cabinet: Cabinet,
    pub input_trim: f32,
    pub output_trim: f32,
    pub mix: f32,
}

/// A preset with everything at its resting position, which the entries below
/// then vary. Written this way so that reading one says only what makes it
/// different, and so that adding a control later does not mean editing every
/// preset in the file.
const fn base(group: &'static str, name: &'static str) -> Preset {
    Preset {
        group,
        name,
        circuit: Circuit::Crunch,
        diode: Diode::Silicon,
        drive: 0.5,
        tone: ToneStack::Wide,
        bass: 0.5,
        mid: 0.5,
        treble: 0.5,
        cabinet: Cabinet::Off,
        input_trim: 0.0,
        output_trim: 0.0,
        mix: 1.0,
    }
}

pub const PRESETS: &[Preset] = &[
    // --- Preamplifier ----------------------------------------------------
    // The quiet end. A valve stage barely working, which is the sound of a
    // signal having been through something rather than the sound of an
    // effect. Everything here has the cabinet off: a speaker in front of a
    // preamplifier sound is wrong, and it is the commonest way these end up
    // sounding muffled.
    Preset { drive: 0.30, ..base("Preamp", "Init") },
    Preset { drive: 0.45, circuit: Circuit::Clean, tone: ToneStack::Off, ..base("Preamp", "Valve Colour") },
    Preset { drive: 0.72, circuit: Circuit::Clean, tone: ToneStack::Off, ..base("Preamp", "Driven Preamp") },
    Preset { drive: 0.58, circuit: Circuit::Clean, treble: 0.62, bass: 0.45, ..base("Preamp", "Bright Front End") },
    Preset { drive: 0.85, circuit: Circuit::Clean, tone: ToneStack::Off, mix: 0.45, ..base("Preamp", "Parallel Warmth") },

    // --- Crunch ----------------------------------------------------------
    // Two stages, which is where the harmonics start compounding rather than
    // adding. The cabinet comes in here: past this point there is enough
    // going on at the top of the band that leaving it bare is audible.
    Preset { drive: 0.42, mid: 0.6, cabinet: Cabinet::Combo, ..base("Crunch", "Edge of Breakup") },
    Preset { drive: 0.62, mid: 0.58, cabinet: Cabinet::Combo, ..base("Crunch", "Blues Crunch") },
    Preset { drive: 0.78, bass: 0.45, mid: 0.55, treble: 0.62, cabinet: Cabinet::Stack, ..base("Crunch", "Classic Rock") },
    Preset { drive: 0.88, bass: 0.42, treble: 0.68, cabinet: Cabinet::Stack, ..base("Crunch", "British Crunch") },
    Preset { drive: 0.70, tone: ToneStack::Scooping, mid: 0.35, cabinet: Cabinet::Combo, ..base("Crunch", "Hollow Crunch") },

    // --- High gain -------------------------------------------------------
    // Three stages, all of them clipping on every note.
    Preset { drive: 0.62, bass: 0.55, mid: 0.5, treble: 0.6, cabinet: Cabinet::Stack, ..base("High Gain", "Modern Rhythm") },
    // The one the whole exercise was aimed at. Three cascaded stages for the
    // harmonics, the scooping voicing with the mid control right down for the
    // dip -- a resonant leg rather than a shelf at each end, which is why it
    // is a scoop and not just less middle -- and a stack behind it, because
    // that much energy above 4 kHz is unlistenable without a speaker.
    Preset {
        drive: 0.86,
        circuit: Circuit::HighGain,
        tone: ToneStack::Scooping,
        bass: 0.78,
        mid: 0.08,
        treble: 0.82,
        cabinet: Cabinet::Stack,
        output_trim: -1.5,
        ..base("High Gain", "Scooped Metal")
    },
    Preset { drive: 0.92, circuit: Circuit::HighGain, tone: ToneStack::Scooping, bass: 0.7, mid: 0.2, treble: 0.75, cabinet: Cabinet::Stack, ..base("High Gain", "Thrash Rhythm") },
    Preset { drive: 0.75, circuit: Circuit::HighGain, bass: 0.6, mid: 0.62, treble: 0.55, cabinet: Cabinet::Stack, ..base("High Gain", "Lead Sustain") },
    Preset { drive: 1.0, circuit: Circuit::HighGain, tone: ToneStack::Scooping, bass: 0.85, mid: 0.05, treble: 0.7, cabinet: Cabinet::Stack, output_trim: -2.0, ..base("High Gain", "Everything Up") },
    Preset { drive: 0.80, circuit: Circuit::HighGain, bass: 0.35, mid: 0.45, treble: 0.7, cabinet: Cabinet::Stack, ..base("High Gain", "Tight Low End") },

    // --- Overdrive -------------------------------------------------------
    // Diodes in the loop, which lower the gain rather than stopping the
    // output -- so these keep following what is played and clean up when the
    // playing gets quieter. Mostly with the cabinet off, because an overdrive
    // is usually in front of an amplifier rather than instead of one.
    Preset { drive: 0.55, circuit: Circuit::Overdrive, mid: 0.62, ..base("Overdrive", "Green Overdrive") },
    Preset { drive: 0.35, circuit: Circuit::Overdrive, tone: ToneStack::Off, ..base("Overdrive", "Transparent Boost") },
    Preset { drive: 0.70, circuit: Circuit::Overdrive, diode: Diode::Germanium, mid: 0.6, ..base("Overdrive", "Germanium Warmth") },
    Preset { drive: 0.85, circuit: Circuit::Overdrive, diode: Diode::Led, treble: 0.58, ..base("Overdrive", "LED Headroom") },
    Preset { drive: 0.80, circuit: Circuit::Overdrive, mid: 0.7, cabinet: Cabinet::Combo, ..base("Overdrive", "Overdriven Combo") },

    // --- Distortion ------------------------------------------------------
    // Diodes to ground: a ceiling the output stops at, so the wave is squared
    // off and the harmonics reach the top of the band. These want a cabinet.
    Preset { drive: 0.60, circuit: Circuit::Distortion, mid: 0.45, cabinet: Cabinet::Combo, ..base("Distortion", "Classic Distortion") },
    Preset { drive: 0.85, circuit: Circuit::Distortion, tone: ToneStack::Scooping, bass: 0.7, mid: 0.15, treble: 0.75, cabinet: Cabinet::Stack, ..base("Distortion", "Scooped Pedal") },
    Preset { drive: 0.75, circuit: Circuit::Distortion, diode: Diode::Germanium, bass: 0.65, cabinet: Cabinet::Combo, ..base("Distortion", "Woolly Fuzz") },
    Preset { drive: 0.95, circuit: Circuit::Distortion, diode: Diode::Led, bass: 0.55, treble: 0.7, cabinet: Cabinet::Stack, ..base("Distortion", "LED Wall") },
    Preset { drive: 0.90, circuit: Circuit::Distortion, mix: 0.6, cabinet: Cabinet::Combo, ..base("Distortion", "Blended Grit") },
];

/// The groups, in the order they should be shown: quietest first, so the list
/// itself reads as a range rather than as an alphabetical accident.
pub const GROUPS: [&str; 5] = ["Preamp", "Crunch", "High Gain", "Overdrive", "Distortion"];

impl Preset {
    /// The preset as parameter ids and the values the panel would show, which
    /// is the form the host wants them in.
    ///
    /// Ids rather than fields, because this is what gets pushed out as
    /// ordinary parameter gestures: an automatable, undoable edit like any
    /// other, rather than a set of assignments the host never hears about. It
    /// is also the shape a preset saved to disk would take, so user presets
    /// can join the same path later without any of this changing.
    pub fn dials(&self) -> [(&'static str, f32); 11] {
        [
            ("in_trim", self.input_trim),
            ("circuit", index_in(&Circuit::ALL, self.circuit)),
            ("diode", index_in(&Diode::ALL, self.diode)),
            ("drive", self.drive),
            ("tone", index_in(&ToneStack::ALL, self.tone)),
            ("bass", self.bass),
            ("mid", self.mid),
            ("treble", self.treble),
            ("cabinet", index_in(&Cabinet::ALL, self.cabinet)),
            ("mix", self.mix),
            ("out_trim", self.output_trim),
        ]
    }
}

/// Where a variant sits in its list, which is what an enumerated parameter
/// takes as its plain value.
fn index_in<T: PartialEq + Copy>(all: &[T], value: T) -> f32 {
    all.iter().position(|v| *v == value).unwrap_or(0) as f32
}

/// Where a named preset sits in the list, or nothing if it has been renamed
/// out from under a saved session.
pub fn index_of(name: &str) -> Option<usize> {
    PRESETS.iter().position(|p| p.name == name)
}

/// The presets in one group, with their positions in the full list.
pub fn in_group(group: &'static str) -> impl Iterator<Item = (usize, &'static Preset)> {
    PRESETS
        .iter()
        .enumerate()
        .filter(move |(_, p)| p.group == group)
}
