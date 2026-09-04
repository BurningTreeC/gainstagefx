//! The front panel, as parameters.
//!
//! Laid out in the order the signal goes through it, and grouped so that the
//! panel can be read top to bottom without knowing anything about the plugin
//! first. The first attempt grew its controls in the order they were built,
//! which meant the panel recorded the development history rather than the
//! signal path, and nothing about it said what happened before what.
//!
//! Six sections, numbered on the panel:
//!
//! 1. **Input** -- what arrives, and how hard.
//! 2. **Circuit** -- what does the work.
//! 3. **Drive** -- how hard it is worked.
//! 4. **Tone** -- what is taken out afterwards.
//! 5. **Cabinet** -- what it comes out of.
//! 6. **Output** -- what leaves.
//!
//! Plus a Session strip above them for the preset and the settings that are
//! about the plugin rather than the sound.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::Data;
use nih_plug_vizia::ViziaState;
use std::sync::{Arc, Mutex};

use crate::voice;

/// What does the work. One entry per circuit topology, and the name is the
/// sound rather than a manufacturer.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Circuit {
    #[id = "clean"]
    #[name = "Clean"]
    Clean,
    #[id = "crunch"]
    #[name = "Crunch"]
    Crunch,
    #[id = "highgain"]
    #[name = "High Gain"]
    HighGain,
    #[id = "overdrive"]
    #[name = "Overdrive"]
    Overdrive,
    #[id = "distortion"]
    #[name = "Distortion"]
    Distortion,
    // Appended rather than slotted in beside their relatives, so the positions
    // the shipped presets refer to do not move under them.
    #[id = "console"]
    #[name = "Console"]
    Console,
    #[id = "studio"]
    #[name = "Studio"]
    Studio,
    // Models of particular circuits, built from their schematics, rather than
    // topologies. Appended so the positions the shipped presets refer to do
    // not move under them.
    #[id = "ts808"]
    #[name = "TS808"]
    Screamer,
    #[id = "bigmuff"]
    #[name = "Big Muff"]
    Muff,
    #[id = "markiic"]
    #[name = "Mark IIC+"]
    Boogie,
}

impl Circuit {

    /// The name the panel puts on it, which is the same name the host shows.
    pub fn name(self) -> &'static str {
        match self {
            Circuit::Clean => "Clean",
            Circuit::Crunch => "Crunch",
            Circuit::HighGain => "High Gain",
            Circuit::Overdrive => "Overdrive",
            Circuit::Distortion => "Distortion",
            Circuit::Console => "Console",
            Circuit::Studio => "Studio",
            Circuit::Screamer => "TS808",
            Circuit::Muff => "Big Muff",
            Circuit::Boogie => "Mark IIC+",
        }
    }
    pub const ALL: [Circuit; 10] = [
        Circuit::Clean,
        Circuit::Crunch,
        Circuit::HighGain,
        Circuit::Overdrive,
        Circuit::Distortion,
        Circuit::Console,
        Circuit::Studio,
        Circuit::Screamer,
        Circuit::Muff,
        Circuit::Boogie,
    ];

    pub fn voice(self) -> voice::Gain {
        match self {
            Circuit::Clean => voice::Gain::Clean,
            Circuit::Crunch => voice::Gain::Crunch,
            Circuit::HighGain => voice::Gain::HighGain,
            Circuit::Overdrive => voice::Gain::Overdrive,
            Circuit::Distortion => voice::Gain::Distortion,
            Circuit::Console => voice::Gain::Console,
            Circuit::Studio => voice::Gain::Studio,
            Circuit::Screamer => voice::Gain::Screamer,
            Circuit::Muff => voice::Gain::Muff,
            Circuit::Boogie => voice::Gain::Boogie,
        }
    }

    /// Whether the choice of amplifying part reaches this circuit.
    pub fn has_amplifier(self) -> bool {
        self.voice().has_amplifier()
    }

    /// Whether this is a model of a particular circuit rather than a
    /// topology. The panel puts the two on separate rows, because "an
    /// overdrive" and "a TS808" are different kinds of claim.
    pub fn is_modelled(self) -> bool {
        self.voice().is_modelled()
    }

    /// Whether the diode choice reaches this circuit. A valve stage has no
    /// diodes in it, and the panel greys the control out rather than offering
    /// one that does nothing.
    pub fn has_diodes(self) -> bool {
        self.voice().has_diodes()
    }

    /// Which of the two families this belongs to: something with a bottle or
    /// a transistor in it, or something with a pair of diodes.
    pub fn is_valve(self) -> bool {
        !self.has_diodes()
    }
}

/// The clipping part, chosen separately from the circuit it sits in.
///
/// This is the axis the hardware actually varies along -- swapping the diodes
/// in a pedal is the commonest modification there is, and it makes a different
/// sound from the same schematic. Two decisions, so two controls.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Diode {
    #[id = "silicon"]
    #[name = "Silicon"]
    Silicon,
    #[id = "germanium"]
    #[name = "Germanium"]
    Germanium,
    #[id = "led"]
    #[name = "LED"]
    Led,
}

impl Diode {

    /// The name the panel puts on it, which is the same name the host shows.
    pub fn name(self) -> &'static str {
        match self {
            Diode::Silicon => "Silicon",
            Diode::Germanium => "Germanium",
            Diode::Led => "LED",
        }
    }
    pub const ALL: [Diode; 3] = [Diode::Silicon, Diode::Germanium, Diode::Led];

    pub fn voice(self) -> voice::Diode {
        match self {
            Diode::Silicon => voice::Diode::Silicon,
            Diode::Germanium => voice::Diode::Germanium,
            Diode::Led => voice::Diode::Led,
        }
    }
}

/// The part that does the amplifying, on the channels built around one.
///
/// The axis the hardware varies along: a console channel with a bottle in it
/// instead of a transistor is a different and much-argued-about box built from
/// the same schematic. Two decisions, so two controls.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Amplifier {
    #[id = "valve"]
    #[name = "Valve"]
    Valve,
    #[id = "jfet"]
    #[name = "JFET"]
    Jfet,
    #[id = "opamp"]
    #[name = "Op-amp"]
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

    pub fn voice(self) -> voice::Amplifier {
        match self {
            Amplifier::Valve => voice::Amplifier::Valve,
            Amplifier::Jfet => voice::Amplifier::Jfet,
            Amplifier::OpAmp => voice::Amplifier::OpAmp,
        }
    }
}

/// An output transformer, which every circuit here can have.
///
/// A control rather than a property of a circuit: iron belongs after a
/// distortion pedal exactly as much as after a console channel, and there is
/// no reason to offer it on one and not the other. What it does is level and
/// frequency dependent together -- flux is the integral of voltage, so it
/// gives up on a bass note long before anything else notices.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Iron {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "nickel"]
    #[name = "Nickel"]
    Nickel,
    #[id = "steel"]
    #[name = "Steel"]
    Steel,
    #[id = "amorphous"]
    #[name = "Amorphous"]
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

    pub fn voice(self) -> voice::Iron {
        match self {
            Iron::Off => voice::Iron::Off,
            Iron::Nickel => voice::Iron::Nickel,
            Iron::Steel => voice::Iron::Steel,
            Iron::Amorphous => voice::Iron::Amorphous,
        }
    }
}

/// The tone section, which can be out of circuit entirely.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum ToneStack {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "wide"]
    #[name = "Wide"]
    Wide,
    #[id = "scooping"]
    #[name = "Scooping"]
    Scooping,
}

impl ToneStack {

    /// The name the panel puts on it, which is the same name the host shows.
    pub fn name(self) -> &'static str {
        match self {
            ToneStack::Off => "Off",
            ToneStack::Wide => "Wide",
            ToneStack::Scooping => "Scooping",
        }
    }
    pub const ALL: [ToneStack; 3] = [ToneStack::Off, ToneStack::Wide, ToneStack::Scooping];

    pub fn voice(self) -> voice::Tone {
        match self {
            ToneStack::Off => voice::Tone::Off,
            ToneStack::Wide => voice::Tone::Wide,
            ToneStack::Scooping => voice::Tone::Scooping,
        }
    }
}

/// The speaker.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Cabinet {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "combo"]
    #[name = "Combo"]
    Combo,
    #[id = "stack"]
    #[name = "Stack"]
    Stack,
}

impl Cabinet {

    /// The name the panel puts on it, which is the same name the host shows.
    pub fn name(self) -> &'static str {
        match self {
            Cabinet::Off => "Off",
            Cabinet::Combo => "Combo",
            Cabinet::Stack => "Stack",
        }
    }
    pub const ALL: [Cabinet; 3] = [Cabinet::Off, Cabinet::Combo, Cabinet::Stack];

    pub fn voice(self) -> voice::Cabinet {
        match self {
            Cabinet::Off => voice::Cabinet::Off,
            Cabinet::Combo => voice::Cabinet::Combo,
            Cabinet::Stack => voice::Cabinet::Stack,
        }
    }
}

/// How much faster than the host the circuit is solved.
///
/// Kept as a setting rather than fixed because the cost is real and the
/// benefit depends entirely on the voice: a Clean stage folds almost nothing
/// back into the band and a squared-off Distortion folds a great deal.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Oversampling {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "x2"]
    #[name = "2x"]
    Two,
    #[id = "x4"]
    #[name = "4x"]
    Four,
    #[id = "x8"]
    #[name = "8x"]
    Eight,
}

impl Oversampling {

    /// The name the panel puts on it, which is the same name the host shows.
    pub fn name(self) -> &'static str {
        match self {
            Oversampling::Off => "Off",
            Oversampling::Two => "2x",
            Oversampling::Four => "4x",
            Oversampling::Eight => "8x",
        }
    }
    pub const ALL: [Oversampling; 4] = [
        Oversampling::Off,
        Oversampling::Two,
        Oversampling::Four,
        Oversampling::Eight,
    ];

    pub fn factor(self) -> usize {
        match self {
            Oversampling::Off => 1,
            Oversampling::Two => 2,
            Oversampling::Four => 4,
            Oversampling::Eight => 8,
        }
    }
}

#[derive(Params)]
pub struct GainStageParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    /// The name of the preset showing in the strip, kept with the state so a
    /// reopened session still says what it was set from.
    #[persist = "preset-name"]
    pub preset_name: Mutex<String>,

    // --- 1 Input ---------------------------------------------------------
    /// What arrives. A guitar taken straight into an interface can land
    /// anywhere between a whisper and clipping depending on the pickups, and
    /// every circuit here is calibrated around a nominal level -- so this is
    /// the control that makes the rest of the panel mean what it says.
    #[id = "in_trim"]
    pub input_trim: FloatParam,

    // --- 2 Circuit -------------------------------------------------------
    #[id = "circuit"]
    pub circuit: EnumParam<Circuit>,
    #[id = "diode"]
    pub diode: EnumParam<Diode>,
    #[id = "amplifier"]
    pub amplifier: EnumParam<Amplifier>,
    #[id = "iron"]
    pub iron: EnumParam<Iron>,

    // --- 3 Drive ---------------------------------------------------------
    /// How hard the circuit is worked. All the way up is the sound the voice
    /// is named for; down from there only cleans up.
    #[id = "drive"]
    pub drive: FloatParam,

    // --- 4 Tone ----------------------------------------------------------
    #[id = "tone"]
    pub tone: EnumParam<ToneStack>,
    #[id = "bass"]
    pub bass: FloatParam,
    #[id = "mid"]
    pub mid: FloatParam,
    #[id = "treble"]
    pub treble: FloatParam,

    // --- 5 Cabinet -------------------------------------------------------
    #[id = "cabinet"]
    pub cabinet: EnumParam<Cabinet>,

    // --- 6 Output --------------------------------------------------------
    /// How much of the processed signal is heard against the dry one.
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "out_trim"]
    pub output_trim: FloatParam,

    // --- Session ---------------------------------------------------------
    #[id = "oversampling"]
    pub oversampling: EnumParam<Oversampling>,
}

/// A control that runs 0 to 1 and reads as a percentage, which is what every
/// knob on a piece of hardware like this actually is.
fn position(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_unit(" %")
        .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 100.0)))
        .with_string_to_value(Arc::new(|s| s.trim().parse::<f32>().ok().map(|v| v / 100.0)))
}

fn decibels(name: &str, span: f32) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: -span, max: span })
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_unit(" dB")
        .with_step_size(0.1)
}

impl Default for GainStageParams {
    fn default() -> Self {
        Self {
            editor_state: crate::editor::default_state(),
            preset_name: Mutex::new(String::from(crate::presets::NONE)),

            input_trim: decibels("Input", 24.0),

            circuit: EnumParam::new("Circuit", Circuit::Crunch),
            diode: EnumParam::new("Diode", Diode::Silicon),
            amplifier: EnumParam::new("Amplifier", Amplifier::Jfet),
            iron: EnumParam::new("Iron", Iron::Off),

            drive: position("Drive", 0.5),

            tone: EnumParam::new("Tone", ToneStack::Wide),
            bass: position("Bass", 0.5),
            mid: position("Mid", 0.5),
            treble: position("Treble", 0.5),

            cabinet: EnumParam::new("Cabinet", Cabinet::Off),

            mix: position("Mix", 1.0),
            output_trim: decibels("Output", 24.0),

            oversampling: EnumParam::new("Oversampling", Oversampling::Two),
        }
    }
}
