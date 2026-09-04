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

use crate::params::{Amplifier, Cabinet, Circuit, Diode, Iron, Oversampling, ToneStack};

pub struct Preset {
    pub group: &'static str,
    pub name: &'static str,
    pub circuit: Circuit,
    pub diode: Diode,
    pub amplifier: Amplifier,
    pub iron: Iron,
    pub drive: f32,
    pub tone: ToneStack,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub cabinet: Cabinet,
    pub input_trim: f32,
    pub output_trim: f32,
    pub mix: f32,
    /// How much faster than the host this sound needs the circuit solved.
    ///
    /// Carried by the preset, which is a correction. It was left out on the
    /// grounds that it is about what the plugin costs rather than what it
    /// sounds like, and measured against a reference at 3 kHz that is simply
    /// untrue for the pedals: a diode clipper to ground aliases 16 dB down at
    /// one times and 31 at two, and needs the four it has. The valve cascades
    /// are the opposite -- 55 dB down at two times, and they are the ones that
    /// cost anything, so paying for four there buys nothing audible.
    pub oversampling: Oversampling,
}

/// A preset with everything at its resting position, which the entries below
/// then vary.
///
/// The drive figures are read off `examples/curve.rs`, which measures what the
/// circuit's own distortion does against knob position at a nominal input.
/// They were chosen by eye before that existed, and the catalogue was far
/// cleaner than its names claimed: a preset called Classic Distortion sat at
/// under 8 per cent and Scooped Metal at 3. Written this way so that reading one says only what makes it
/// different, and so that adding a control later does not mean editing every
/// preset in the file.
const fn base(group: &'static str, name: &'static str) -> Preset {
    Preset {
        group,
        name,
        circuit: Circuit::Crunch,
        diode: Diode::Silicon,
        amplifier: Amplifier::Jfet,
        iron: Iron::Off,
        drive: 0.5,
        tone: ToneStack::Wide,
        bass: 0.5,
        mid: 0.5,
        treble: 0.5,
        cabinet: Cabinet::Off,
        input_trim: 0.0,
        output_trim: 0.0,
        mix: 1.0,
        oversampling: Oversampling::Two,
    }
}

pub const PRESETS: &[Preset] = &[
    // --- Studio ----------------------------------------------------------
    // Not guitar amplifiers turned down. A guitar preamplifier is supposed to
    // run out of room -- that is where the sound is -- and these are built so
    // that they do not, so what they have happens in the last few decibels
    // before they do, or in the iron rather than the gain stage. The cabinet
    // is off throughout: a speaker in front of a microphone preamplifier makes
    // no sense at all.
    Preset { drive: 0.62, circuit: Circuit::Console, amplifier: Amplifier::Jfet, iron: Iron::Steel, tone: ToneStack::Off, ..base("Studio", "Console Channel") },
    Preset { drive: 0.88, circuit: Circuit::Console, amplifier: Amplifier::Jfet, iron: Iron::Nickel, tone: ToneStack::Off, ..base("Studio", "Console, Driven") },
    Preset { drive: 0.68, circuit: Circuit::Console, amplifier: Amplifier::Valve, iron: Iron::Nickel, tone: ToneStack::Off, ..base("Studio", "Valve Mic Pre") },
    Preset { drive: 0.92, circuit: Circuit::Console, amplifier: Amplifier::Valve, iron: Iron::Steel, tone: ToneStack::Off, ..base("Studio", "Valve, Pushed") },
    Preset { drive: 0.55, circuit: Circuit::Studio, amplifier: Amplifier::OpAmp, iron: Iron::Off, tone: ToneStack::Off, ..base("Studio", "Studio Preamp") },
    Preset { drive: 0.75, circuit: Circuit::Studio, amplifier: Amplifier::OpAmp, iron: Iron::Steel, tone: ToneStack::Off, ..base("Studio", "Studio, Iron Out") },
    Preset { drive: 0.62, circuit: Circuit::Studio, amplifier: Amplifier::OpAmp, iron: Iron::Amorphous, tone: ToneStack::Off, ..base("Studio", "Clean Line Amp") },
    // The one that is nothing but the transformer: an op-amp contributes
    // nothing of its own anywhere in the band, so everything heard here is
    // the core running out of room -- and because flux is the integral of
    // voltage, all of it is at the bottom.
    Preset { drive: 0.96, circuit: Circuit::Studio, amplifier: Amplifier::OpAmp, iron: Iron::Nickel, tone: ToneStack::Off, ..base("Studio", "Transformer Colour") },
    Preset { drive: 0.60, circuit: Circuit::Studio, amplifier: Amplifier::Jfet, iron: Iron::Off, tone: ToneStack::Off, ..base("Studio", "JFET Direct") },
    Preset { drive: 0.80, circuit: Circuit::Studio, amplifier: Amplifier::Jfet, iron: Iron::Steel, tone: ToneStack::Off, ..base("Studio", "Discrete Colour") },

    // --- Preamplifier ----------------------------------------------------
    // The quiet end. A valve stage barely working, which is the sound of a
    // signal having been through something rather than the sound of an
    // effect. Everything here has the cabinet off: a speaker in front of a
    // preamplifier sound is wrong, and it is the commonest way these end up
    // sounding muffled.
    Preset { drive: 0.55, circuit: Circuit::Clean, tone: ToneStack::Off, ..base("Preamp", "Valve Colour") },
    Preset { drive: 0.85, circuit: Circuit::Clean, tone: ToneStack::Off, ..base("Preamp", "Driven Preamp") },
    Preset { drive: 0.65, circuit: Circuit::Clean, treble: 0.62, bass: 0.45, ..base("Preamp", "Bright Front End") },
    Preset { drive: 0.95, circuit: Circuit::Clean, tone: ToneStack::Off, mix: 0.45, ..base("Preamp", "Parallel Warmth") },

    // --- Crunch ----------------------------------------------------------
    // Two stages, which is where the harmonics start compounding rather than
    // adding. The cabinet comes in here: past this point there is enough
    // going on at the top of the band that leaving it bare is audible.
    Preset { drive: 0.55, mid: 0.6, cabinet: Cabinet::Combo, ..base("Crunch", "Edge of Breakup") },
    Preset { drive: 0.70, mid: 0.58, cabinet: Cabinet::Combo, ..base("Crunch", "Blues Crunch") },
    Preset { drive: 0.85, bass: 0.45, mid: 0.55, treble: 0.62, cabinet: Cabinet::Stack, ..base("Crunch", "Classic Rock") },
    Preset { drive: 0.95, bass: 0.42, treble: 0.68, cabinet: Cabinet::Stack, ..base("Crunch", "British Crunch") },
    Preset { drive: 0.78, tone: ToneStack::Scooping, mid: 0.35, cabinet: Cabinet::Combo, ..base("Crunch", "Hollow Crunch") },

    // --- High gain -------------------------------------------------------
    // Three stages, all of them clipping on every note.
    Preset { drive: 0.78, bass: 0.55, mid: 0.5, treble: 0.6, cabinet: Cabinet::Stack, ..base("High Gain", "Modern Rhythm") },
    // The one the whole exercise was aimed at. Three cascaded stages for the
    // harmonics, the scooping voicing with the mid control right down for the
    // dip -- a resonant leg rather than a shelf at each end, which is why it
    // is a scoop and not just less middle -- and a stack behind it, because
    // that much energy above 4 kHz is unlistenable without a speaker.
    Preset {
        drive: 0.93,
        circuit: Circuit::HighGain,
        tone: ToneStack::Scooping,
        bass: 0.78,
        mid: 0.08,
        treble: 0.82,
        cabinet: Cabinet::Stack,
        output_trim: -1.5,
        ..base("High Gain", "Scooped Metal")
    },
    Preset { drive: 0.96, circuit: Circuit::HighGain, tone: ToneStack::Scooping, bass: 0.7, mid: 0.2, treble: 0.75, cabinet: Cabinet::Stack, ..base("High Gain", "Thrash Rhythm") },
    Preset { drive: 0.85, circuit: Circuit::HighGain, bass: 0.6, mid: 0.62, treble: 0.55, cabinet: Cabinet::Stack, ..base("High Gain", "Lead Sustain") },
    Preset { drive: 1.00, circuit: Circuit::HighGain, tone: ToneStack::Scooping, bass: 0.85, mid: 0.05, treble: 0.7, cabinet: Cabinet::Stack, output_trim: -2.0, ..base("High Gain", "Everything Up") },
    Preset { drive: 0.88, circuit: Circuit::HighGain, bass: 0.35, mid: 0.45, treble: 0.7, cabinet: Cabinet::Stack, ..base("High Gain", "Tight Low End") },

    // --- Overdrive -------------------------------------------------------
    // Diodes in the loop, which lower the gain rather than stopping the
    // output -- so these keep following what is played and clean up when the
    // playing gets quieter. Mostly with the cabinet off, because an overdrive
    // is usually in front of an amplifier rather than instead of one.
    // The name was always describing a particular green box, and now it is
    // one: the modelled TS808 rather than the generic loop clipper. Its tone
    // control is the circuit's own, so the plugin's stack is out of the path
    // and the Treble knob is what the pedal has.
    Preset { drive: 0.85, circuit: Circuit::Screamer, tone: ToneStack::Off, treble: 0.5, oversampling: Oversampling::Off, ..base("Overdrive", "Green Overdrive") },
    Preset { drive: 0.40, circuit: Circuit::Screamer, tone: ToneStack::Off, treble: 0.65, output_trim: 1.5, oversampling: Oversampling::Off, ..base("Overdrive", "Screamer Boost") },
    Preset { drive: 0.70, circuit: Circuit::Overdrive, tone: ToneStack::Off, oversampling: Oversampling::Four, ..base("Overdrive", "Transparent Boost") },
    Preset { drive: 0.90, circuit: Circuit::Overdrive, diode: Diode::Germanium, mid: 0.6, oversampling: Oversampling::Four, ..base("Overdrive", "Germanium Warmth") },
    Preset { drive: 0.95, circuit: Circuit::Overdrive, diode: Diode::Led, treble: 0.58, oversampling: Oversampling::Four, ..base("Overdrive", "LED Headroom") },
    Preset { drive: 0.92, circuit: Circuit::Overdrive, mid: 0.7, cabinet: Cabinet::Combo, oversampling: Oversampling::Four, ..base("Overdrive", "Overdriven Combo") },

    // --- Distortion ------------------------------------------------------
    // Diodes to ground: a ceiling the output stops at, so the wave is squared
    // off and the harmonics reach the top of the band. These want a cabinet.
    Preset { drive: 0.78, circuit: Circuit::Distortion, mid: 0.45, cabinet: Cabinet::Combo, oversampling: Oversampling::Four, ..base("Distortion", "Classic Distortion") },
    Preset { drive: 0.90, circuit: Circuit::Distortion, tone: ToneStack::Scooping, bass: 0.7, mid: 0.15, treble: 0.75, cabinet: Cabinet::Stack, oversampling: Oversampling::Four, ..base("Distortion", "Scooped Pedal") },
    Preset { drive: 0.85, circuit: Circuit::Distortion, diode: Diode::Germanium, bass: 0.65, cabinet: Cabinet::Combo, oversampling: Oversampling::Four, ..base("Distortion", "Woolly Fuzz") },
    Preset { drive: 0.97, circuit: Circuit::Distortion, diode: Diode::Led, bass: 0.55, treble: 0.7, cabinet: Cabinet::Stack, oversampling: Oversampling::Four, ..base("Distortion", "LED Wall") },
    Preset { drive: 0.92, circuit: Circuit::Distortion, mix: 0.6, cabinet: Cabinet::Combo, oversampling: Oversampling::Four, ..base("Distortion", "Blended Grit") },
    // The modelled four stage fuzz. Its tone control is a scoop rather than a
    // treble cut -- turning it *down* is what makes the notch -- so the stack
    // is off and the Treble knob is the pedal's own.
    Preset { drive: 0.90, circuit: Circuit::Muff, tone: ToneStack::Off, treble: 0.35, cabinet: Cabinet::Stack, oversampling: Oversampling::Off, ..base("Distortion", "Sustain Fuzz") },

    // --- Modelled amplifiers ---------------------------------------------
    // Whole preamplifiers rather than pedals: the circuits in src/circuits
    // that were built from a drawing, with the controls where the drawing
    // puts them. The Mark IIC+ carries its own tone stack, so the stack is
    // off and the three knobs are the amplifier's; the 5150's stack could not
    // be traced, so it borrows the plugin's.
    Preset { drive: 0.85, circuit: Circuit::Boogie, tone: ToneStack::Off, bass: 0.60, mid: 0.25, treble: 0.70, cabinet: Cabinet::Stack, oversampling: Oversampling::Off, ..base("Amplifier", "Boutique Lead") },
    Preset { drive: 0.95, circuit: Circuit::Boogie, tone: ToneStack::Off, bass: 0.70, mid: 0.15, treble: 0.75, cabinet: Cabinet::Stack, oversampling: Oversampling::Off, ..base("Amplifier", "Boutique Rhythm") },
    Preset { drive: 0.90, circuit: Circuit::Peavey, tone: ToneStack::Scooping, bass: 0.70, mid: 0.15, treble: 0.75, cabinet: Cabinet::Stack, oversampling: Oversampling::Off, ..base("Amplifier", "Ultra Lead") },
    Preset { drive: 0.75, circuit: Circuit::Peavey, tone: ToneStack::Wide, bass: 0.45, mid: 0.50, treble: 0.65, cabinet: Cabinet::Stack, oversampling: Oversampling::Off, ..base("Amplifier", "Ultra Rhythm") },
];

/// The groups, in the order they should be shown: quietest first, so the list
/// itself reads as a range rather than as an alphabetical accident.
pub const GROUPS: [&str; 7] = [
    "Studio",
    "Preamp",
    "Crunch",
    "High Gain",
    "Overdrive",
    "Distortion",
    // Last because it is a different kind of entry: not a topology set up to
    // make a sound, but a particular amplifier modelled from its drawing.
    "Amplifier",
];

impl Preset {
    /// The preset as parameter ids and the values the panel would show, which
    /// is the form the host wants them in.
    ///
    /// Ids rather than fields, because this is what gets pushed out as
    /// ordinary parameter gestures: an automatable, undoable edit like any
    /// other, rather than a set of assignments the host never hears about. It
    /// is also the shape a preset saved to disk would take, so user presets
    /// can join the same path later without any of this changing.
    pub fn dials(&self) -> [(&'static str, f32); 14] {
        [
            ("in_trim", self.input_trim),
            ("circuit", index_in(&Circuit::ALL, self.circuit)),
            ("diode", index_in(&Diode::ALL, self.diode)),
            ("amplifier", index_in(&Amplifier::ALL, self.amplifier)),
            ("iron", index_in(&Iron::ALL, self.iron)),
            ("drive", self.drive),
            ("tone", index_in(&ToneStack::ALL, self.tone)),
            ("bass", self.bass),
            ("mid", self.mid),
            ("treble", self.treble),
            ("cabinet", index_in(&Cabinet::ALL, self.cabinet)),
            ("mix", self.mix),
            ("out_trim", self.output_trim),
            ("oversampling", index_in(&Oversampling::ALL, self.oversampling)),
        ]
    }
}

/// Where a variant sits in its list, which is what an enumerated parameter
/// takes as its plain value.
fn index_in<T: PartialEq + Copy>(all: &[T], value: T) -> f32 {
    all.iter().position(|v| *v == value).unwrap_or(0) as f32
}

/// What the strip shows when no preset has been loaded.
///
/// A plugin that has just been added has not loaded anything, and naming that
/// state after a preset says it has -- and invites the reasonable belief that
/// turning a knob has departed from something. It is a dash: no preset.
pub const NONE: &str = "-";

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

// ---------------------------------------------------------------------------
// Saved presets
// ---------------------------------------------------------------------------

use nih_plug::params::Params;
use nih_plug::prelude::ParamPtr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The group saved presets appear under. Their own section at the foot of the
/// list, rather than mixed into the shipped groups: which of these you can
/// delete and which you cannot is worth being able to see without clicking.
pub const SAVED: &str = "Saved";

/// A preset as the menu handles it: a name and parameter ids against
/// *normalised* values.
///
/// Normalised rather than the plain values the shipped catalogue is written
/// in, because this is also what goes to disk, and a plain value only means
/// something next to the parameter it came from. A range widened in a later
/// version would silently reinterpret every saved file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub name: String,
    pub values: BTreeMap<String, f32>,
    /// Compiled in rather than loaded from disk, so it cannot be overwritten
    /// or deleted.
    #[serde(default, skip)]
    pub built_in: bool,
    /// Which heading it sits under.
    #[serde(default, skip)]
    pub group: &'static str,
}

/// Controls that are about the plugin rather than the sound, and so have no
/// business in a preset. There are none at the moment: oversampling was here
/// until it was measured, and for the pedals it is audibly part of the sound.
const EXCLUDED: [&str; 0] = [];

/// Where saved presets live.
pub fn preset_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("gainstagefx").join("presets"))
}

/// Every preset: the shipped ones in their groups, then the saved ones in name
/// order under their own heading.
///
/// A saved preset that happens to share a name with a shipped one does not
/// hide it. The shipped preset is compiled in and cannot be edited, so
/// dropping it from the list would put it permanently out of reach; the two
/// sit in different sections instead, and only yours can be deleted.
pub fn load_all(params: &impl Params) -> Vec<Stored> {
    let mut all = shipped(params);
    all.extend(load_saved());
    all
}

/// The compiled-in catalogue, converted against the real parameters.
fn shipped(params: &impl Params) -> Vec<Stored> {
    let pointers: BTreeMap<String, ParamPtr> = params
        .param_map()
        .into_iter()
        .map(|(id, ptr, _)| (id, ptr))
        .collect();

    PRESETS
        .iter()
        .map(|preset| Stored {
            name: preset.name.to_string(),
            values: preset
                .dials()
                .iter()
                .filter_map(|(id, plain)| {
                    let ptr = pointers.get(*id)?;
                    // SAFETY: the pointers come from the parameters we were
                    // handed, which outlive this function.
                    Some((id.to_string(), unsafe { ptr.preview_normalized(*plain) }))
                })
                .collect(),
            built_in: true,
            group: preset.group,
        })
        .collect()
}

fn load_saved() -> Vec<Stored> {
    let Some(dir) = preset_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut presets: Vec<Stored> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let mut preset: Stored = serde_json::from_str(&text).ok()?;
            preset.built_in = false;
            preset.group = SAVED;
            // A preset whose file was renamed by hand should follow the file
            // rather than vanish from the list.
            if preset.name.trim().is_empty() {
                preset.name = path.file_stem()?.to_str()?.to_string();
            }
            Some(preset)
        })
        .collect();
    presets.sort_by_key(|preset| preset.name.to_lowercase());
    presets
}

/// The panel as it stands, as parameter ids against normalised values.
///
/// The one thing here that cannot be tested without a host: nih-plug keeps
/// every parameter setter private on purpose, so a test can read the live
/// values but cannot move them. Reading is what this does, and it is one
/// expression -- everything that decides anything is in `same` below, where it
/// can be driven from both sides.
pub fn live_values(params: &impl Params) -> BTreeMap<String, f32> {
    params
        .param_map()
        .into_iter()
        .filter(|(id, _, _)| !EXCLUDED.contains(&id.as_str()))
        // SAFETY: as above, the pointers belong to the parameters we were
        // given.
        .map(|(id, ptr, _)| (id, unsafe { ptr.unmodulated_normalized_value() }))
        .collect()
}

/// Whether two sets of values describe the same panel.
///
/// An id the preset does not mention is not a difference. Presets written
/// before a control existed should keep loading, and the control they say
/// nothing about should stay where it is rather than jumping to a default the
/// preset never chose.
pub fn same(live: &BTreeMap<String, f32>, saved: &BTreeMap<String, f32>) -> bool {
    saved.iter().all(|(id, &value)| match live.get(id) {
        Some(&current) => (current - value).abs() <= 1e-5,
        // An id the panel no longer has cannot differ from anything.
        None => true,
    })
}

/// Take the current panel settings as a preset.
pub fn capture(params: &impl Params, name: &str) -> Stored {
    Stored {
        name: name.trim().to_string(),
        values: live_values(params),
        built_in: false,
        group: SAVED,
    }
}

/// Write a preset out, replacing any file of yours with the same name.
pub fn save(preset: &Stored) -> std::io::Result<PathBuf> {
    if preset.name.trim().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "a preset needs a name",
        ));
    }
    let dir = preset_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no config directory to save presets into",
        )
    })?;
    std::fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", file_stem(&preset.name)));
    let json = serde_json::to_string_pretty(preset)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Remove a saved preset's file.
///
/// Found the way `load_saved` identifies it rather than by deriving a file
/// name from the preset's own, because a file renamed by hand still shows in
/// the list under the name stored inside it. Deleting the row has to remove
/// the file that row actually came from, not a file the name implies.
pub fn delete(name: &str) -> std::io::Result<()> {
    let dir = preset_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no config directory to delete presets from",
        )
    })?;
    let wanted = name.trim().to_lowercase();
    for entry in std::fs::read_dir(&dir)?.filter_map(Result::ok) {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(preset) = serde_json::from_str::<Stored>(&text) else {
            continue;
        };
        let shown = if preset.name.trim().is_empty() {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_string()
        } else {
            preset.name.clone()
        };
        if shown.trim().to_lowercase() == wanted {
            return std::fs::remove_file(&path);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no saved preset by that name",
    ))
}

/// Whether the live parameters still match a preset's values.
///
/// Compared rather than tracked with an edited flag, so that turning a control
/// back to where it was counts as unmodified again -- which is what somebody
/// who has just undone a change expects to see.
pub fn matches(params: &impl Params, values: &BTreeMap<String, f32>) -> bool {
    values.is_empty() || same(&live_values(params), values)
}

/// Whether saving under this name would replace a file of yours.
///
/// Shipped presets deliberately do not count: saving under one of their names
/// writes a new file beside it and replaces nothing, so warning about it would
/// be describing something that does not happen.
pub fn name_taken(name: &str, presets: &[Stored]) -> bool {
    let name = name.trim();
    presets
        .iter()
        .filter(|preset| !preset.built_in)
        .any(|preset| preset.name.trim().eq_ignore_ascii_case(name))
}

/// Turns a preset name into something safe to use as a file name.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "preset".to_string()
    } else {
        stem
    }
}
