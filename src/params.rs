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
    #[name = "Green 808"]
    Screamer,
    #[id = "bigmuff"]
    #[name = "Ram Fuzz"]
    Muff,
    #[id = "markiic"]
    #[name = "Cali IIC+"]
    Boogie,
    #[id = "evh5150"]
    #[name = "American 5150"]
    Peavey,
    #[id = "neve"]
    #[name = "British 73"]
    Neve,
    #[id = "twin"]
    #[name = "American Twin"]
    Twin,
    // Appended, like everything above. Host state stores these ids, so sessions
    // are unaffected; a saved preset old enough to carry no ids is migrated in
    // `presets::migrate`. See `LEGACY_CIRCUIT_COUNT`.
    #[id = "amp_jcm800_2203"]
    #[name = "Brit 800"]
    Brit800,
    #[id = "pre_api_312"]
    #[name = "American 312"]
    American312,
    #[id = "pre_ssl_4000e"]
    #[name = "British 4K E"]
    ConsoleE,
    #[id = "pre_ua_610a"]
    #[name = "Tube 610"]
    Tube610,
    #[id = "amp_marshall_1959"]
    #[name = "Brit Plexi"]
    Plexi,
    #[id = "amp_vox_ac30_tb"]
    #[name = "Brit AC30"]
    AC30,
    #[id = "amp_hiwatt_dr103"]
    #[name = "Brit DR103"]
    DR103,
    #[id = "amp_dual_rectifier"]
    #[name = "Cali Rectifier"]
    Recto,
    // The rest of the pedals, as circuits in their own right. The Green 808 and
    // the Ram Fuzz have been here since before the pedal slot existed; these
    // are appended, because a saved automation lane stores a normalised value
    // and inserting one mid-list would move every other selection.
    #[id = "pedal_ts9_circuit"]
    #[name = "Green 9"]
    Green9,
    #[id = "pedal_rat_circuit"]
    #[name = "Rodent"]
    Rat,
    #[id = "pedal_fuzz_face_circuit"]
    #[name = "Round Fuzz"]
    FuzzFace,
    #[id = "pedal_mxr_dist_plus_circuit"]
    #[name = "Yellow Dist"]
    DistPlus,
    #[id = "pedal_boss_hm2_circuit"]
    #[name = "Heavy Metal"]
    Hm2,
    #[id = "pedal_boss_mt2_circuit"]
    #[name = "Metal Zone"]
    Mt2,
    // Appended, like everything above it. Beside the American Twin: the same
    // AB763 circuit at a quarter of the power.
    #[id = "amp_fender_deluxe_ab763"]
    #[name = "American Deluxe"]
    Deluxe,
    // Appended. The first solid-state amplifier in the list, and the first
    // with no valve power stage to match it to.
    #[id = "amp_roland_jc120"]
    #[name = "Jazz 120"]
    Jazz120,
    // Appended. The same amplifier as `Deluxe`, through its other channel.
    #[id = "amp_fender_deluxe_ab763_normal"]
    #[name = "American Deluxe Normal"]
    DeluxeNormal,
    // Appended. The JCM800 2205's boost channel: the same family as the Brit
    // 800 and a different preamplifier, with its own 50 W power stage.
    #[id = "amp_jcm800_2205"]
    #[name = "Brit 2205"]
    Brit2205,
}

/// How long the circuit list was before the Brit 800 was appended. A saved
/// preset written before presets carried stable ids stores the circuit as
/// `index / (LEGACY_CIRCUIT_COUNT - 1)`.
pub const LEGACY_CIRCUIT_COUNT: usize = 13;

/// What a modelled circuit is, for the heading it is listed under.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Pedal,
    Amplifier,
    MicPre,
}

impl Kind {
    /// In the order the signal meets them.
    pub const ALL: [Kind; 3] = [Kind::Pedal, Kind::Amplifier, Kind::MicPre];

    pub fn heading(self) -> &'static str {
        match self {
            Kind::Pedal => "PEDALS",
            Kind::Amplifier => "AMPLIFIERS",
            Kind::MicPre => "MICROPHONE PREAMPS",
        }
    }
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
            Circuit::Screamer => "Green 808",
            Circuit::Muff => "Ram Fuzz",
            Circuit::Boogie => "Cali IIC+",
            Circuit::Peavey => "American 5150",
            Circuit::Neve => "British 73",
            Circuit::Twin => "American Twin",
            Circuit::Brit800 => "Brit 800",
            Circuit::American312 => "American 312",
            Circuit::ConsoleE => "British 4K E",
            Circuit::Tube610 => "Tube 610",
            Circuit::Plexi => "Brit Plexi",
            Circuit::AC30 => "Brit AC30",
            Circuit::DR103 => "Brit DR103",
            Circuit::Recto => "Cali Rectifier",
            Circuit::Green9 => "Green 9",
            Circuit::Rat => "Rodent",
            Circuit::FuzzFace => "Round Fuzz",
            Circuit::DistPlus => "Yellow Dist",
            Circuit::Hm2 => "Heavy Metal",
            Circuit::Mt2 => "Metal Zone",
            Circuit::Deluxe => "American Deluxe",
            Circuit::Jazz120 => "Jazz 120",
            Circuit::DeluxeNormal => "American Deluxe Normal",
            Circuit::Brit2205 => "Brit 2205",
        }
    }
    pub const ALL: [Circuit; 31] = [
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
        Circuit::Peavey,
        Circuit::Neve,
        Circuit::Twin,
        Circuit::Brit800,
        Circuit::American312,
        Circuit::ConsoleE,
        Circuit::Tube610,
        Circuit::Plexi,
        Circuit::AC30,
        Circuit::DR103,
        Circuit::Recto,
        Circuit::Green9,
        Circuit::Rat,
        Circuit::FuzzFace,
        Circuit::DistPlus,
        Circuit::Hm2,
        Circuit::Mt2,
        Circuit::Deluxe,
        Circuit::Jazz120,
        Circuit::DeluxeNormal,
        Circuit::Brit2205,
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
            Circuit::Peavey => voice::Gain::Peavey,
            Circuit::Neve => voice::Gain::Neve,
            Circuit::Twin => voice::Gain::Twin,
            Circuit::Brit800 => voice::Gain::Brit800,
            Circuit::American312 => voice::Gain::American312,
            Circuit::ConsoleE => voice::Gain::ConsoleE,
            Circuit::Tube610 => voice::Gain::Tube610,
            Circuit::Plexi => voice::Gain::Plexi,
            Circuit::AC30 => voice::Gain::AC30,
            Circuit::DR103 => voice::Gain::DR103,
            Circuit::Recto => voice::Gain::Recto,
            Circuit::Green9 => voice::Gain::Green9,
            Circuit::Rat => voice::Gain::Rat,
            Circuit::FuzzFace => voice::Gain::FuzzFace,
            Circuit::DistPlus => voice::Gain::DistPlus,
            Circuit::Hm2 => voice::Gain::Hm2,
            Circuit::Mt2 => voice::Gain::Mt2,
            Circuit::Deluxe => voice::Gain::Deluxe,
            Circuit::Jazz120 => voice::Gain::Jazz120,
            Circuit::DeluxeNormal => voice::Gain::DeluxeNormal,
            Circuit::Brit2205 => voice::Gain::Brit2205,
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

    /// Which kind of thing a modelled circuit is, for the heading it sits
    /// under in the list.
    ///
    /// The modelled list is long enough now that it needs them, and the three
    /// kinds want different things of a player: a pedal goes in front of
    /// something, an amplifier is the something, and a microphone preamplifier
    /// is not a guitar circuit at all. The order of the parameter cannot change
    /// -- a saved automation lane is a normalised number against it -- so the
    /// menu groups what it draws and leaves the list where it is.
    pub fn kind(self) -> Kind {
        match self.voice() {
            voice::Gain::Screamer
            | voice::Gain::Muff
            | voice::Gain::Green9
            | voice::Gain::Rat
            | voice::Gain::FuzzFace
            | voice::Gain::DistPlus
            | voice::Gain::Hm2
            | voice::Gain::Mt2 => Kind::Pedal,
            voice::Gain::Neve
            | voice::Gain::American312
            | voice::Gain::ConsoleE
            | voice::Gain::Tube610 => Kind::MicPre,
            _ => Kind::Amplifier,
        }
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

    /// Which of the three tone knobs this circuit carries one of its own for.
    ///
    /// The panel needs it to decide which knobs are live. A knob that turns
    /// and reaches nothing is indistinguishable from a fault, and so is a
    /// greyed-out knob that would have reached something -- which is what the
    /// TS808 and the Big Muff had: a tone control on the drawing, wired up in
    /// the voice, and greyed on the panel because the plugin's own stack was
    /// switched out.
    pub fn own_tone_knobs(self) -> [bool; 3] {
        self.voice().own_tone_knobs()
    }

    /// Whether this circuit's only tone control is the single knob a pedal
    /// has, which the panel calls TONE rather than TREBLE.
    pub fn single_tone(self) -> bool {
        self.voice().single_tone()
    }

    /// Whether the Reverb, Speed and Intensity knobs reach anything here.
    ///
    /// Only the Twin has a tank and a tremolo. The panel greys them
    /// everywhere else, for the same reason it greys the diode control on a
    /// valve stage: a knob that turns and reaches nothing is
    /// indistinguishable from a fault.
    pub fn has_reverb_and_tremolo(self) -> bool {
        self.voice().has_reverb_and_tremolo()
    }
}

/// Full output circuit, independent of the selected preamp. Append future IDs.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum PowerAmp {
    #[id = "matched"]
    #[name = "Matched"]
    Matched,
    #[id = "bypass"]
    #[name = "Bypass"]
    Bypass,
    #[id = "power_mark_iic"]
    #[name = "Cali 6L6"]
    Cali6L6,
    #[id = "power_twin_ab763"]
    #[name = "American 6L6 Clean"]
    American6L6Clean,
    #[id = "power_5150"]
    #[name = "American 6L6 High-Gain"]
    American6L6HighGain,
    #[id = "power_2203_el34"]
    #[name = "Brit EL34"]
    BritEL34,
    #[id = "power_1959_el34"]
    #[name = "Brit Plexi EL34"]
    BritPlexiEL34,
    #[id = "power_ac30_el84"]
    #[name = "AC30 EL84"]
    AC30EL84,
    #[id = "power_dr103_el34"]
    #[name = "DR103 EL34"]
    DR103EL34,
    #[id = "power_recto_6l6"]
    #[name = "Recto 6L6"]
    Recto6L6,
    #[id = "power_recto_6l6_tube"]
    #[name = "Recto 6L6 Tube"]
    Recto6L6Tube,
    #[id = "power_deluxe_6v6"]
    #[name = "American Deluxe 6V6"]
    AmericanDeluxe6V6,
    #[id = "power_2205_el34"]
    #[name = "Brit 2205 EL34"]
    Brit2205EL34,
}

impl PowerAmp {
    pub const ALL: [Self; 13] = [
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
        Self::AmericanDeluxe6V6,
        Self::Brit2205EL34,
    ];

    pub fn name(self) -> &'static str {
        Self::variants()[self.to_index()]
    }

    pub fn voice(self) -> voice::PowerAmp {
        match self {
            Self::Matched => voice::PowerAmp::Matched,
            Self::Bypass => voice::PowerAmp::Bypass,
            Self::Cali6L6 => voice::PowerAmp::Cali6L6,
            Self::American6L6Clean => voice::PowerAmp::American6L6Clean,
            Self::American6L6HighGain => voice::PowerAmp::American6L6HighGain,
            Self::BritEL34 => voice::PowerAmp::BritEL34,
            Self::BritPlexiEL34 => voice::PowerAmp::BritPlexiEL34,
            Self::AC30EL84 => voice::PowerAmp::AC30EL84,
            Self::DR103EL34 => voice::PowerAmp::DR103EL34,
            Self::Recto6L6 => voice::PowerAmp::Recto6L6,
            Self::Recto6L6Tube => voice::PowerAmp::Recto6L6Tube,
            Self::AmericanDeluxe6V6 => voice::PowerAmp::AmericanDeluxe6V6,
            Self::Brit2205EL34 => voice::PowerAmp::Brit2205EL34,
        }
    }
}

/// What the amplifier is plugged into. Append new ids only.
///
/// A variac is not an effect: it is how the rig was wired, and it belongs with
/// the other selections rather than on a knob. The first entry is the wall,
/// which is what every session that has never heard of this parameter gets.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum Mains {
    #[id = "mains_nominal"]
    #[name = "Nominal"]
    Nominal,
    #[id = "mains_90"]
    #[name = "90 %"]
    Ninety,
    #[id = "mains_80"]
    #[name = "80 %"]
    Eighty,
    #[id = "mains_70"]
    #[name = "70 %"]
    Seventy,
}

impl Mains {
    pub const ALL: [Self; 4] = [Self::Nominal, Self::Ninety, Self::Eighty, Self::Seventy];

    pub fn name(self) -> &'static str {
        Self::variants()[self.to_index()]
    }

    /// The fraction every supply is multiplied by.
    pub fn fraction(self) -> f64 {
        match self {
            Self::Nominal => 1.0,
            Self::Ninety => 0.9,
            Self::Eighty => 0.8,
            Self::Seventy => 0.7,
        }
    }
}

/// The pedal slot in front of the circuit. Append new ids only.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum PedalModel {
    #[id = "none"]
    #[name = "None"]
    None,
    #[id = "pedal_ts808"]
    #[name = "Green 808"]
    Green808,
    /// Displayed as "Ram Fuzz", after the 1973 "Ram's Head" circuit it is built
    /// from; the id is unchanged. See docs/models/big_muff.md.
    #[id = "pedal_bigmuff_ramshead"]
    #[name = "Ram Fuzz"]
    BigMuff,
    #[id = "pedal_ts9"]
    #[name = "Green 9"]
    Green9,
    // Appended: saved presets carry ids, and host state stores them.
    #[id = "pedal_rat"]
    #[name = "Rodent"]
    Rodent,
    #[id = "pedal_mxr_dist_plus"]
    #[name = "Yellow Dist"]
    YellowDist,
    #[id = "pedal_fuzz_face"]
    #[name = "Round Fuzz"]
    RoundFuzz,
    // Appended, like the ones above it: the declaration order is what a host
    // automation lane recorded against, so a new pedal goes on the end.
    #[id = "pedal_boss_hm2"]
    #[name = "Heavy Metal"]
    HeavyMetal,
    #[id = "pedal_boss_mt2"]
    #[name = "Metal Zone"]
    MetalZone,
}

impl PedalModel {
    pub const ALL: [Self; 9] = [
        Self::None,
        Self::Green808,
        Self::BigMuff,
        Self::Green9,
        Self::Rodent,
        Self::YellowDist,
        Self::RoundFuzz,
        Self::HeavyMetal,
        Self::MetalZone,
    ];

    pub fn name(self) -> &'static str {
        Self::variants()[self.to_index()]
    }

    pub fn voice(self) -> voice::Pedal {
        match self {
            Self::None => voice::Pedal::None,
            Self::Green808 => voice::Pedal::Green808,
            Self::BigMuff => voice::Pedal::BigMuff,
            Self::Green9 => voice::Pedal::Green9,
            Self::Rodent => voice::Pedal::Rodent,
            Self::RoundFuzz => voice::Pedal::RoundFuzz,
            Self::YellowDist => voice::Pedal::YellowDist,
            Self::HeavyMetal => voice::Pedal::HeavyMetal,
            Self::MetalZone => voice::Pedal::MetalZone,
        }
    }
}

/// The cabinet after the power stage. `Legacy` (first, and the default for every
/// old session) is the resistor load with the old Combo/Stack filter. The rest are
/// geometry models; see docs/models/cabinets.md. Append new ids only.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum CabModel {
    #[id = "legacy"]
    #[name = "Legacy"]
    Legacy,
    #[id = "bypass"]
    #[name = "Bypass"]
    Bypass,
    #[id = "cab_marshall_1960a"]
    #[name = "Brit 1960 4x12"]
    Brit1960,
    #[id = "cab_mesa_recto_standard"]
    #[name = "Cali Oversized 4x12"]
    CaliOversized,
    #[id = "cab_marshall_1960b"]
    #[name = "Brit Closed 4x12"]
    BritClosed,
    #[id = "cab_marshall_1960ax"]
    #[name = "Brit Green 4x12"]
    BritGreen,
    #[id = "cab_marshall_1960av"]
    #[name = "Brit V30 4x12"]
    BritV30,
    #[id = "cab_generic_oversized_412"]
    #[name = "Oversized 4x12"]
    Oversized,
    #[id = "cab_fender_twin_open_212"]
    #[name = "American Open 2x12"]
    AmericanOpen212,
    #[id = "cab_fender_deluxe_open_112"]
    #[name = "American Open 1x12"]
    AmericanOpen112,
    #[id = "cab_marshall_1912"]
    #[name = "Closed 1x12"]
    Closed112,
    #[id = "cab_marshall_1936"]
    #[name = "Closed 2x12"]
    Closed212,
}

impl CabModel {
    pub const ALL: [Self; 12] = [
        Self::Legacy,
        Self::Bypass,
        Self::Brit1960,
        Self::CaliOversized,
        Self::BritClosed,
        Self::BritGreen,
        Self::BritV30,
        Self::Oversized,
        Self::AmericanOpen212,
        Self::AmericanOpen112,
        Self::Closed112,
        Self::Closed212,
    ];

    pub fn name(self) -> &'static str {
        Self::variants()[self.to_index()]
    }

    pub fn voice(self) -> voice::CabinetChoice {
        use crate::acoustics::cabinet::CabinetProfile as P;
        match self {
            Self::Legacy => voice::CabinetChoice::Legacy,
            Self::Bypass => voice::CabinetChoice::Bypass,
            Self::Brit1960 => voice::CabinetChoice::Model(&P::BRIT_1960),
            Self::CaliOversized => voice::CabinetChoice::Model(&P::CALI_OVERSIZED),
            Self::BritClosed => voice::CabinetChoice::Model(&P::BRIT_CLOSED),
            Self::BritGreen => voice::CabinetChoice::Model(&P::BRIT_GREEN),
            Self::BritV30 => voice::CabinetChoice::Model(&P::BRIT_V30),
            Self::Oversized => voice::CabinetChoice::Model(&P::OVERSIZED),
            Self::AmericanOpen212 => voice::CabinetChoice::Model(&P::AMERICAN_OPEN_212),
            Self::AmericanOpen112 => voice::CabinetChoice::Model(&P::AMERICAN_OPEN_112),
            Self::Closed112 => voice::CabinetChoice::Model(&P::CLOSED_112),
            Self::Closed212 => voice::CabinetChoice::Model(&P::CLOSED_212),
        }
    }
}

/// The loudspeaker. `Matched` is the cabinet's own; `Bypass` is a resistive load and
/// the power stage's terminal voltage. Only implemented drivers are listed.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum SpeakerModel {
    #[id = "matched"]
    #[name = "Matched"]
    Matched,
    #[id = "bypass"]
    #[name = "Bypass"]
    Bypass,
    #[id = "spk_celestion_v30"]
    #[name = "Brit V30"]
    BritV30,
    #[id = "spk_celestion_g12m25"]
    #[name = "Brit Green 25"]
    BritGreen25,
    #[id = "spk_celestion_g12t75"]
    #[name = "Brit T75"]
    BritT75,
    #[id = "spk_jensen_p12r"]
    #[name = "American Vintage 12"]
    AmericanVintage12,
    #[id = "spk_jensen_p10r"]
    #[name = "American Vintage 10"]
    AmericanVintage10,
    #[id = "spk_jensen_c12n"]
    #[name = "American Ceramic"]
    AmericanCeramic,
    #[id = "spk_jensen_p12n"]
    #[name = "American Alnico"]
    AmericanAlnico,
}

impl SpeakerModel {
    pub const ALL: [Self; 9] = [
        Self::Matched,
        Self::Bypass,
        Self::BritV30,
        Self::BritGreen25,
        Self::BritT75,
        Self::AmericanVintage12,
        Self::AmericanVintage10,
        Self::AmericanCeramic,
        Self::AmericanAlnico,
    ];

    pub fn name(self) -> &'static str {
        Self::variants()[self.to_index()]
    }

    pub fn voice(self) -> voice::SpeakerChoice {
        use crate::acoustics::speaker::SpeakerProfile as P;
        match self {
            Self::Matched => voice::SpeakerChoice::Matched,
            Self::Bypass => voice::SpeakerChoice::Bypass,
            Self::BritV30 => voice::SpeakerChoice::Model(&P::BRIT_V30),
            Self::BritGreen25 => voice::SpeakerChoice::Model(&P::BRIT_GREEN_25),
            Self::BritT75 => voice::SpeakerChoice::Model(&P::BRIT_T75),
            Self::AmericanVintage12 => voice::SpeakerChoice::Model(&P::AMERICAN_VINTAGE_12),
            Self::AmericanVintage10 => voice::SpeakerChoice::Model(&P::AMERICAN_VINTAGE_10),
            Self::AmericanCeramic => voice::SpeakerChoice::Model(&P::AMERICAN_CERAMIC),
            Self::AmericanAlnico => voice::SpeakerChoice::Model(&P::AMERICAN_ALNICO),
        }
    }
}

/// A microphone. `Off` exists only for the second slot; `Bypass` is an ideal omni
/// at the placement. See docs/models/microphones.md.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug, Data)]
pub enum MicModel {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "ideal"]
    #[name = "Bypass"]
    Ideal,
    #[id = "mic_shure_sm57"]
    #[name = "Dynamic 57"]
    Dynamic57,
    #[id = "mic_sennheiser_md421"]
    #[name = "Dynamic 421"]
    Dynamic421,
    #[id = "mic_sennheiser_e906"]
    #[name = "Dynamic 906"]
    Dynamic906,
    #[id = "mic_sennheiser_md409"]
    #[name = "Dynamic 409"]
    Dynamic409,
    #[id = "mic_royer_r121"]
    #[name = "Ribbon 121"]
    Ribbon121,
    #[id = "mic_beyer_m160"]
    #[name = "Ribbon 160"]
    Ribbon160,
    #[id = "mic_coles_4038"]
    #[name = "Ribbon 38"]
    Ribbon38,
    #[id = "mic_neumann_u87ai"]
    #[name = "Condenser 87"]
    Condenser87,
    #[id = "mic_akg_c414"]
    #[name = "Condenser 414"]
    Condenser414,
    #[id = "mic_neumann_u67"]
    #[name = "Tube Condenser 67"]
    TubeCondenser67,
    #[id = "mic_neumann_u47fet"]
    #[name = "FET Condenser 47"]
    FetCondenser47,
}

impl MicModel {
    pub const ALL: [Self; 13] = [
        Self::Off,
        Self::Ideal,
        Self::Dynamic57,
        Self::Dynamic421,
        Self::Dynamic906,
        Self::Dynamic409,
        Self::Ribbon121,
        Self::Ribbon160,
        Self::Ribbon38,
        Self::Condenser87,
        Self::Condenser414,
        Self::TubeCondenser67,
        Self::FetCondenser47,
    ];

    pub fn name(self) -> &'static str {
        Self::variants()[self.to_index()]
    }

    /// The slot; `off_allowed` is false for microphone A, which falls back to Bypass.
    pub fn voice(self, off_allowed: bool) -> crate::acoustics::stage::MicSlot {
        use crate::acoustics::mic::MicProfile as P;
        use crate::acoustics::stage::MicSlot;
        match self {
            Self::Off if off_allowed => MicSlot::Off,
            Self::Off | Self::Ideal => MicSlot::Ideal,
            Self::Dynamic57 => MicSlot::Profile(&P::DYNAMIC_57),
            Self::Dynamic421 => MicSlot::Profile(&P::DYNAMIC_421),
            Self::Dynamic906 => MicSlot::Profile(&P::DYNAMIC_906),
            Self::Dynamic409 => MicSlot::Profile(&P::DYNAMIC_409),
            Self::Ribbon121 => MicSlot::Profile(&P::RIBBON_121),
            Self::Ribbon160 => MicSlot::Profile(&P::RIBBON_160),
            Self::Ribbon38 => MicSlot::Profile(&P::RIBBON_38),
            Self::Condenser87 => MicSlot::Profile(&P::CONDENSER_87),
            Self::Condenser414 => MicSlot::Profile(&P::CONDENSER_414),
            Self::TubeCondenser67 => MicSlot::Profile(&P::TUBE_CONDENSER_67),
            Self::FetCondenser47 => MicSlot::Profile(&P::FET_CONDENSER_47),
        }
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
    /// Optional gentle input expander; off preserves legacy sessions exactly.
    #[id = "noise_reduction"]
    pub noise_reduction: BoolParam,
    #[id = "noise_threshold"]
    pub noise_threshold: FloatParam,
    /// Which of the American Twin Vibrato channel's two stock input jacks is
    /// used. `false` is Jack 1 / High; `true` is Jack 2 / Low (-6 dB). The
    /// parameter is ignored by every other circuit.
    #[id = "twin_low_input"]
    pub twin_low_input: BoolParam,
    /// Stock 120 pF Bright switch across the Twin channel Volume control.
    #[id = "twin_bright"]
    pub twin_bright: BoolParam,

    // --- 2 Circuit -------------------------------------------------------
    #[id = "pedal"]
    pub pedal: EnumParam<PedalModel>,
    #[id = "pedal_drive"]
    pub pedal_drive: FloatParam,
    #[id = "pedal_tone"]
    pub pedal_tone: FloatParam,
    /// A pedal's second, third and fourth tone controls, for the ones that have
    /// them. Appended after `pedal_tone`, which stays the first: ids are never
    /// renamed, and a session saved before these existed simply leaves them at
    /// their middles. See `voice::PEDAL_TONES`.
    #[id = "pedal_tone_b"]
    pub pedal_tone_b: FloatParam,
    #[id = "pedal_tone_c"]
    pub pedal_tone_c: FloatParam,
    #[id = "pedal_tone_d"]
    pub pedal_tone_d: FloatParam,
    #[id = "pedal_level"]
    pub pedal_level: FloatParam,
    /// A fourth control for a circuit that has one of its own past bass, middle
    /// and treble. Only the Metal Zone does: its Mid Freq, which moves where
    /// its middle band works. Appended, and it rests at noon for every circuit
    /// that has no such control. See `voice::Gain::own_sweep`.
    #[id = "tone_sweep"]
    pub tone_sweep: FloatParam,
    /// The Heavy Metal circuit's two Colour Mix controls. These used to share
    /// the generic Bass/Treble parameters, which meant a hidden HM-2 setting
    /// could be carried by a preset that was not an HM-2 at all. Keep them as
    /// circuit-scoped host parameters instead; presets neutralise them for
    /// every other circuit.
    #[id = "hm2_colour_lo"]
    pub hm2_colour_lo: FloatParam,
    #[id = "hm2_colour_hi"]
    pub hm2_colour_hi: FloatParam,
    #[id = "circuit"]
    pub circuit: EnumParam<Circuit>,
    #[id = "power_amp"]
    pub power_amp: EnumParam<PowerAmp>,
    #[id = "mains"]
    pub mains: EnumParam<Mains>,
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

    /// The circuit's own output level control, where its drawing has one: a
    /// pedal's Level or Volume, the 73P's trim, an amplifier's master.
    ///
    /// **Half is where the voice was calibrated**, not half the pot's travel.
    /// Each circuit rests its level control somewhere different -- 0.70 for the
    /// pedals, 0.85 for the 73P, 0.66 for a 5150's post gain, 0.30 for a
    /// Mark IIC+'s Lead Master -- and those are the positions the make-up table
    /// was measured at. A knob that read the pot straight would move every
    /// voice off its voicing the moment it was selected. See
    /// `Chain::set_master`.
    ///
    /// Greyed for the circuits whose drawing has no such control, which
    /// includes the Twin Reverb: an AB763 has no master volume and its channel
    /// Volume is already the Drive knob.
    #[id = "master"]
    pub master: FloatParam,

    // --- 3b The Mark IIC+'s graphic equaliser -----------------------------
    // Five sliders, bottom band first, centred is flat. Only that amplifier
    // has them; the panel greys them for everything else. They are here rather
    // than in the tone section because on the amplifier they are late in the
    // preamplifier and before the power stage, and the panel reads in signal
    // order. See `markiic::graphic` and §9.8.
    #[id = "eq60"]
    pub eq60: FloatParam,
    #[id = "eq240"]
    pub eq240: FloatParam,
    #[id = "eq750"]
    pub eq750: FloatParam,
    #[id = "eq2200"]
    pub eq2200: FloatParam,
    #[id = "eq6600"]
    pub eq6600: FloatParam,

    /// The plugin's processing in or out of circuit.
    ///
    /// `make_bypass` tells the host this is *the* bypass, so a DAW can put it
    /// on its own strip and automate it, and a controller can drop a Screamer
    /// in for a solo.
    ///
    /// Switched out, the plugin is a **wire**: the host's samples are handed
    /// back exactly as they arrived, without summing, trims, delay or mix.
    /// That is a deliberate departure from what a latent plugin is usually
    /// asked to do -- nih-plug's note is to keep the delay in the bypassed
    /// path so the track does not jump when the plugin is switched out -- and
    /// it means the reported latency stops describing the output while the
    /// plugin is off. The point of the button is that "off" makes the signal
    /// pass through unchanged rather than moving it, and the shift that costs
    /// is written into `Plugin::process` alongside the branch that does it.
    #[id = "bypass"]
    pub bypass: BoolParam,

    // --- 4 Tone ----------------------------------------------------------
    #[id = "tone"]
    pub tone: EnumParam<ToneStack>,
    #[id = "bass"]
    pub bass: FloatParam,
    #[id = "mid"]
    pub mid: FloatParam,
    #[id = "treble"]
    pub treble: FloatParam,

    // --- 4b The Twin's own three -----------------------------------------
    // A Twin Reverb carries a reverb and a tremolo that nothing else in the
    // catalogue has. The panel greys them for every other circuit rather than
    // leaving knobs that turn nothing. See `Gain::has_reverb_and_tremolo`.
    /// How much of the tank comes back. 100 k **linear** on the drawing, which
    /// is unusual for a Fender and is why it does so much early in its travel.
    #[id = "reverb"]
    pub reverb: FloatParam,
    /// The tremolo oscillator's rate: about 1.8 Hz to 11 Hz.
    #[id = "speed"]
    pub speed: FloatParam,
    /// How strongly the optical cell is coupled to the audio node by the
    /// stock 50 k reverse-audio Intensity pot. The oscillator and neon keep
    /// running independently; at zero the wiper sits at ground so the LDR
    /// cannot modulate the signal.
    #[id = "intensity"]
    pub intensity: FloatParam,

    // --- 4c The Jazz 120's chorus ----------------------------------------
    /// SW3's OFF..CHORUS. Zero is both power amplifiers dry, one is the
    /// second one carrying the bucket brigade. Greyed for every circuit
    /// without a chorus. See `Gain::has_chorus`.
    #[id = "chorus"]
    pub chorus: FloatParam,

    // --- 5 Cabinet -------------------------------------------------------
    #[id = "cabinet"]
    pub cabinet: EnumParam<Cabinet>,
    #[id = "cab_model"]
    pub cab_model: EnumParam<CabModel>,
    #[id = "speaker"]
    pub speaker: EnumParam<SpeakerModel>,
    #[id = "mic_a"]
    pub mic_a: EnumParam<MicModel>,
    #[id = "mic_a_position"]
    pub mic_a_position: FloatParam,
    #[id = "mic_a_distance"]
    pub mic_a_distance: FloatParam,
    #[id = "mic_a_angle"]
    pub mic_a_angle: FloatParam,
    #[id = "mic_b"]
    pub mic_b: EnumParam<MicModel>,
    #[id = "mic_b_position"]
    pub mic_b_position: FloatParam,
    #[id = "mic_b_distance"]
    pub mic_b_distance: FloatParam,
    #[id = "mic_b_angle"]
    pub mic_b_angle: FloatParam,
    #[id = "mic_blend"]
    pub mic_blend: FloatParam,
    /// Where each microphone sits in the stereo field. Centre is the whole
    /// signal on both sides, so a default session is unchanged; moving the two
    /// apart is how one cabinet becomes a stereo image.
    #[id = "mic_a_pan"]
    pub mic_a_pan: FloatParam,
    #[id = "mic_b_pan"]
    pub mic_b_pan: FloatParam,
    #[id = "mic_b_invert"]
    pub mic_b_invert: BoolParam,
    #[id = "mic_align"]
    pub mic_align: BoolParam,

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

impl GainStageParams {
    /// Whether the speaker, cabinet and microphone controls are in use.
    pub fn physical_cabinet(&self) -> bool {
        self.cab_model.value() != CabModel::Legacy && self.speaker.value() != SpeakerModel::Bypass
    }

    pub fn master_enabled(&self) -> bool {
        match self.power_amp.value() {
            PowerAmp::Matched => self.circuit.value().voice().level_control().is_some(),
            PowerAmp::Bypass => matches!(
                self.circuit.value().voice().level_control(),
                Some(voice::Level::Circuit(_))
            ),
            _ => true,
        }
    }
}

/// A control that runs 0 to 1 and reads as a percentage, which is what every
/// knob on a piece of hardware like this actually is.
fn position(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_unit(" %")
        .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 100.0)))
        .with_string_to_value(Arc::new(|s| {
            s.trim().parse::<f32>().ok().map(|v| v / 100.0)
        }))
}

/// Microphone distance from the grille cloth, in metres, skewed so the close
/// range where most of the change happens gets most of the travel.
fn distance(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 0.01,
            max: 1.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_smoother(SmoothingStyle::Linear(20.0))
    .with_unit(" cm")
    .with_value_to_string(Arc::new(|v| format!("{:.1}", v * 100.0)))
    .with_string_to_value(Arc::new(|s| {
        s.trim().parse::<f32>().ok().map(|v| v / 100.0)
    }))
}

/// Microphone angle off the cone's axis, degrees.
fn angle(name: &str) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear {
            min: 0.0,
            max: 90.0,
        },
    )
    .with_smoother(SmoothingStyle::Linear(20.0))
    .with_unit(" deg")
    .with_step_size(0.5)
}

/// A microphone's place in the stereo field: hard left through centre to hard
/// right. Reads as L/R rather than as a number, the way a console does.
fn pan(name: &str) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear {
            min: -1.0,
            max: 1.0,
        },
    )
    .with_smoother(SmoothingStyle::Linear(20.0))
    .with_value_to_string(Arc::new(|v| {
        let position = (v * 100.0).round() as i32;
        match position {
            0 => "C".to_string(),
            p if p < 0 => format!("L{}", -p),
            p => format!("R{p}"),
        }
    }))
    .with_string_to_value(Arc::new(|s| {
        let s = s.trim();
        let upper = s.to_ascii_uppercase();
        if upper == "C" || upper.is_empty() {
            return Some(0.0);
        }
        if let Some(rest) = upper.strip_prefix('L') {
            return rest.trim().parse::<f32>().ok().map(|v| -v / 100.0);
        }
        if let Some(rest) = upper.strip_prefix('R') {
            return rest.trim().parse::<f32>().ok().map(|v| v / 100.0);
        }
        s.parse::<f32>().ok().map(|v| v / 100.0)
    }))
}

fn decibels(name: &str, span: f32) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear {
            min: -span,
            max: span,
        },
    )
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
            noise_reduction: BoolParam::new("Noise Reduction", false),
            noise_threshold: FloatParam::new(
                "Noise Threshold",
                -60.0,
                FloatRange::Linear {
                    min: -90.0,
                    max: -30.0,
                },
            )
            .with_unit(" dBFS")
            .with_step_size(1.0),
            twin_low_input: BoolParam::new("Twin Sensitivity", false),
            // Preserve the sound of sessions made before the switch was exposed:
            // the old model had the 120 pF capacitor permanently connected.
            twin_bright: BoolParam::new("Twin Bright", true),

            pedal: EnumParam::new("Pedal", PedalModel::None),
            pedal_drive: position("Pedal Drive", 0.5),
            pedal_tone: position("Pedal Tone", 0.5),
            pedal_tone_b: position("Pedal Tone 2", 0.5),
            pedal_tone_c: position("Pedal Tone 3", 0.5),
            pedal_tone_d: position("Pedal Tone 4", 0.5),
            pedal_level: position("Pedal Level", 0.5),
            tone_sweep: position("Mid Freq", 0.5),
            hm2_colour_lo: position("HM-2 Colour Low", 0.5),
            hm2_colour_hi: position("HM-2 Colour High", 0.5),
            circuit: EnumParam::new("Circuit", Circuit::Crunch),
            power_amp: EnumParam::new("Power Amp", PowerAmp::Matched),
            mains: EnumParam::new("Mains", Mains::Nominal),
            diode: EnumParam::new("Diode", Diode::Silicon),
            amplifier: EnumParam::new("Amplifier", Amplifier::Jfet),
            iron: EnumParam::new("Iron", Iron::Off),

            drive: position("Drive", 0.5),
            master: position("Master", 0.5),

            eq60: position("EQ 60 Hz", 0.5),
            eq240: position("EQ 240 Hz", 0.5),
            eq750: position("EQ 750 Hz", 0.5),
            eq2200: position("EQ 2.2 kHz", 0.5),
            eq6600: position("EQ 6.6 kHz", 0.5),

            // False is in circuit, which is what a plugin should be when it is
            // dropped on a track.
            bypass: BoolParam::new("Bypass", false).make_bypass(),

            tone: EnumParam::new("Tone", ToneStack::Wide),
            bass: position("Bass", 0.5),
            mid: position("Mid", 0.5),
            treble: position("Treble", 0.5),

            // Reverb off and the tremolo silent by default: a Twin with its
            // reverb up is a choice, not a starting point, and Intensity below
            // the bulb's striking voltage is the tremolo switched off.
            reverb: position("Reverb", 0.0),
            speed: position("Speed", 0.4),
            intensity: position("Intensity", 0.0),

            // Off by default, for the same reason Reverb is: a chorus is a
            // choice rather than a starting point, and it is the one position
            // SW3 has that the amplifier ships in.
            chorus: position("Chorus", 0.0),

            cabinet: EnumParam::new("Cabinet", Cabinet::Off),
            // Legacy by default: an instance, like an old session, starts on the
            // resistor-loaded path it always had. See `filter_state`.
            cab_model: EnumParam::new("Cabinet Model", CabModel::Legacy),
            speaker: EnumParam::new("Speaker", SpeakerModel::Matched),
            mic_a: EnumParam::new("Mic A", MicModel::Dynamic57),
            mic_a_position: position("Mic A Position", 0.3),
            mic_a_distance: distance("Mic A Distance", 0.025),
            mic_a_angle: angle("Mic A Angle"),
            mic_b: EnumParam::new("Mic B", MicModel::Off),
            mic_b_position: position("Mic B Position", 0.5),
            mic_b_distance: distance("Mic B Distance", 0.05),
            mic_b_angle: angle("Mic B Angle"),
            mic_blend: position("Mic Blend", 0.5),
            mic_a_pan: pan("Mic A Pan"),
            mic_b_pan: pan("Mic B Pan"),
            mic_b_invert: BoolParam::new("Mic B Polarity Invert", false),
            mic_align: BoolParam::new("Mic Phase Align", false),

            mix: position("Mix", 1.0),
            output_trim: decibels("Output", 24.0),

            // Off, which is where every shipped preset already sits and where
            // the modelled circuits were pinned when they were calibrated.
            //
            // This was `Two`, and the disagreement cost real callbacks. A
            // player who inserts the plugin and picks an amplifier without
            // loading a preset got 2x. Measured on the American Deluxe, the
            // most expensive voice in the catalogue, at a -6 dBFS input: a
            // **median** of 1072 us against a 1333 us callback -- 80 % before
            // any transient -- and nine missed callbacks in three seconds of
            // plucked attacks. At Off, 544 us and none. Every voice but the
            // Deluxe's Normal channel misses at 2x at that level.
            // See `examples/stutter.rs`, which prints the whole comparison.
            //
            // The control still goes up, and `MODELLED_MAX_OVERSAMPLING` still
            // caps what a modelled circuit will actually use. What changed is
            // that the amplifier no longer starts somewhere its own presets
            // never put it.
            oversampling: EnumParam::new("Oversampling", Oversampling::Off),
        }
    }
}
