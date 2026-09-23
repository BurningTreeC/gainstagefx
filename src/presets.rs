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

use nih_plug::prelude::Enum;

use crate::params::{
    Amplifier, CabModel, Cabinet, Circuit, Diode, Iron, Mains, MicModel, Oversampling, PedalModel,
    PowerAmp, SpeakerModel, ToneStack,
};

pub struct Preset {
    /// What the amplifier is plugged into. `Mains::Nominal` is the wall, which
    /// is what every preset but a variac one wants.
    pub mains: Mains,
    /// A pedal ahead of the circuit, and its knobs. `pedal_tone` is the first
    /// tone control; the three after it are for the pedals that have more than
    /// one, and sit at their middles otherwise. See `voice::PEDAL_TONES`.
    pub pedal: PedalModel,
    pub pedal_drive: f32,
    pub pedal_tone: f32,
    pub pedal_tone_b: f32,
    pub pedal_tone_c: f32,
    pub pedal_tone_d: f32,
    pub pedal_level: f32,
    /// A circuit's own fourth control, where it has one; noon otherwise.
    /// See `voice::Gain::own_sweep`.
    pub tone_sweep: f32,
    /// The Heavy Metal circuit's Colour Mix pair. These are separate from the
    /// plugin's generic Bass/Treble stack controls and rest at noon for every
    /// other circuit.
    pub hm2_colour_lo: f32,
    pub hm2_colour_hi: f32,
    pub power_amp: PowerAmp,
    /// Speaker, cabinet and microphones. `CabModel::Legacy` keeps the old baked
    /// `cabinet` filter; see `Chain::set_acoustic`.
    pub cab_model: CabModel,
    pub speaker: SpeakerModel,
    pub mic_a: MicModel,
    pub mic_a_position: f32,
    /// Metres.
    pub mic_a_distance: f32,
    /// Degrees.
    pub mic_a_angle: f32,
    pub mic_b: MicModel,
    pub mic_b_position: f32,
    pub mic_b_distance: f32,
    pub mic_b_angle: f32,
    pub mic_blend: f32,
    /// Stereo placement of each microphone, -1 hard left to +1 hard right.
    /// Zero on every shipped preset: the stereo image is the player's choice,
    /// not part of a voicing.
    pub mic_a_pan: f32,
    pub mic_b_pan: f32,
    pub mic_b_invert: bool,
    pub mic_align: bool,
    pub group: &'static str,
    pub name: &'static str,
    pub circuit: Circuit,
    pub diode: Diode,
    pub amplifier: Amplifier,
    pub iron: Iron,
    pub drive: f32,
    /// The circuit's own level control, where its drawing has one. Half is
    /// where the voice was calibrated. See `Chain::set_master`.
    pub master: f32,
    /// The Mark IIC+'s five graphic equaliser sliders, bottom band first.
    /// Centred is flat, and every preset for anything else leaves them there.
    pub graphic: [f32; 5],
    pub tone: ToneStack,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub cabinet: Cabinet,
    /// The three the Twin Reverb has and nothing else does. Carried by every
    /// preset because a preset that does not say leaves them wherever the last
    /// one put them, and a reverb arriving with a patch that never asked for
    /// one is the same defect as a knob that reaches nothing.
    /// American Twin input jack: false = High/1, true = Low/2 (-6 dB).
    pub twin_low_input: bool,
    pub twin_bright: bool,
    pub reverb: f32,
    pub speed: f32,
    pub intensity: f32,
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
        mains: Mains::Nominal,
        pedal: PedalModel::None,
        pedal_drive: 0.5,
        pedal_tone: 0.5,
        pedal_tone_b: 0.5,
        pedal_tone_c: 0.5,
        pedal_tone_d: 0.5,
        pedal_level: 0.5,
        tone_sweep: 0.5,
        hm2_colour_lo: 0.5,
        hm2_colour_hi: 0.5,
        power_amp: PowerAmp::Matched,
        cab_model: CabModel::Legacy,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.3,
        mic_a_distance: 0.025,
        mic_a_angle: 0.0,
        mic_b: MicModel::Off,
        mic_b_position: 0.5,
        mic_b_distance: 0.05,
        mic_b_angle: 0.0,
        mic_blend: 0.5,
        mic_a_pan: 0.0,
        mic_b_pan: 0.0,
        mic_b_invert: false,
        mic_align: false,
        group,
        name,
        circuit: Circuit::Crunch,
        diode: Diode::Silicon,
        amplifier: Amplifier::Jfet,
        iron: Iron::Off,
        drive: 0.5,
        master: 0.5,
        graphic: [0.5; 5],
        tone: ToneStack::Wide,
        bass: 0.5,
        mid: 0.5,
        treble: 0.5,
        cabinet: Cabinet::Off,
        twin_low_input: false,
        twin_bright: true,
        reverb: 0.0,
        speed: 0.4,
        intensity: 0.0,
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
    Preset {
        drive: 0.62,
        circuit: Circuit::Console,
        amplifier: Amplifier::Jfet,
        iron: Iron::Steel,
        tone: ToneStack::Off,
        ..base("Studio", "Console Channel")
    },
    Preset {
        drive: 0.88,
        circuit: Circuit::Console,
        amplifier: Amplifier::Jfet,
        iron: Iron::Nickel,
        tone: ToneStack::Off,
        ..base("Studio", "Console, Driven")
    },
    Preset {
        drive: 0.68,
        circuit: Circuit::Console,
        amplifier: Amplifier::Valve,
        iron: Iron::Nickel,
        tone: ToneStack::Off,
        ..base("Studio", "Valve Mic Pre")
    },
    Preset {
        drive: 0.92,
        circuit: Circuit::Console,
        amplifier: Amplifier::Valve,
        iron: Iron::Steel,
        tone: ToneStack::Off,
        ..base("Studio", "Valve, Pushed")
    },
    Preset {
        drive: 0.55,
        circuit: Circuit::Studio,
        amplifier: Amplifier::OpAmp,
        iron: Iron::Off,
        tone: ToneStack::Off,
        ..base("Studio", "Studio Preamp")
    },
    Preset {
        drive: 0.75,
        circuit: Circuit::Studio,
        amplifier: Amplifier::OpAmp,
        iron: Iron::Steel,
        tone: ToneStack::Off,
        ..base("Studio", "Studio, Iron Out")
    },
    Preset {
        drive: 0.62,
        circuit: Circuit::Studio,
        amplifier: Amplifier::OpAmp,
        iron: Iron::Amorphous,
        tone: ToneStack::Off,
        ..base("Studio", "Clean Line Amp")
    },
    // The one that is nothing but the transformer: an op-amp contributes
    // nothing of its own anywhere in the band, so everything heard here is
    // the core running out of room -- and because flux is the integral of
    // voltage, all of it is at the bottom.
    Preset {
        drive: 0.96,
        circuit: Circuit::Studio,
        amplifier: Amplifier::OpAmp,
        iron: Iron::Nickel,
        tone: ToneStack::Off,
        ..base("Studio", "Transformer Colour")
    },
    Preset {
        drive: 0.60,
        circuit: Circuit::Studio,
        amplifier: Amplifier::Jfet,
        iron: Iron::Off,
        tone: ToneStack::Off,
        ..base("Studio", "JFET Direct")
    },
    Preset {
        drive: 0.80,
        circuit: Circuit::Studio,
        amplifier: Amplifier::Jfet,
        iron: Iron::Steel,
        tone: ToneStack::Off,
        ..base("Studio", "Discrete Colour")
    },
    // --- Preamplifier ----------------------------------------------------
    // The quiet end. A valve stage barely working, which is the sound of a
    // signal having been through something rather than the sound of an
    // effect. Everything here has the cabinet off: a speaker in front of a
    // preamplifier sound is wrong, and it is the commonest way these end up
    // sounding muffled.
    Preset {
        drive: 0.55,
        circuit: Circuit::Clean,
        tone: ToneStack::Off,
        ..base("Preamp", "Valve Colour")
    },
    Preset {
        drive: 0.85,
        circuit: Circuit::Clean,
        tone: ToneStack::Off,
        ..base("Preamp", "Driven Preamp")
    },
    Preset {
        drive: 0.65,
        circuit: Circuit::Clean,
        treble: 0.62,
        bass: 0.45,
        ..base("Preamp", "Bright Front End")
    },
    Preset {
        drive: 0.95,
        circuit: Circuit::Clean,
        tone: ToneStack::Off,
        mix: 0.45,
        ..base("Preamp", "Parallel Warmth")
    },
    // --- Crunch ----------------------------------------------------------
    // Two stages, which is where the harmonics start compounding rather than
    // adding. Everything from here on comes out of a real cabinet: a physical
    // box and driver, driven straight from the circuit (these topologies have
    // no power stage of their own), and a microphone in front of it. The
    // legacy baked Combo/Stack filter is no longer used by any shipped preset.
    Preset {
        drive: 0.55,
        mid: 0.6,
        cab_model: CabModel::AmericanOpen212,
        mic_a_position: 0.35,
        mic_a_distance: 0.03,
        ..base("Crunch", "Edge of Breakup")
    },
    Preset {
        drive: 0.70,
        mid: 0.58,
        cab_model: CabModel::AmericanOpen112,
        mic_a_position: 0.3,
        mic_a_distance: 0.025,
        mic_b: MicModel::Ribbon121,
        mic_b_distance: 0.15,
        mic_blend: 0.35,
        output_trim: 2.0,
        ..base("Crunch", "Blues Crunch")
    },
    Preset {
        drive: 0.85,
        bass: 0.45,
        mid: 0.55,
        treble: 0.62,
        cab_model: CabModel::Brit1960,
        mic_a_position: 0.4,
        mic_a_angle: 15.0,
        ..base("Crunch", "Classic Rock")
    },
    Preset {
        drive: 0.95,
        bass: 0.42,
        treble: 0.68,
        cab_model: CabModel::BritGreen,
        mic_b: MicModel::Ribbon121,
        mic_b_distance: 0.2,
        mic_blend: 0.3,
        ..base("Crunch", "British Crunch")
    },
    Preset {
        drive: 0.78,
        tone: ToneStack::Scooping,
        mid: 0.35,
        cab_model: CabModel::Closed212,
        mic_a_position: 0.5,
        ..base("Crunch", "Hollow Crunch")
    },
    // --- High gain -------------------------------------------------------
    // Three stages, all of them clipping on every note, into closed 4x12s --
    // that much energy above 4 kHz wants a closed box and a close microphone.
    Preset {
        drive: 0.78,
        bass: 0.55,
        mid: 0.5,
        treble: 0.6,
        cab_model: CabModel::BritV30,
        mic_a_position: 0.3,
        mic_a_angle: 10.0,
        ..base("High Gain", "Modern Rhythm")
    },
    // The one the whole exercise was aimed at. Three cascaded stages for the
    // harmonics, the scooping voicing with the mid control right down for the
    // dip -- a resonant leg rather than a shelf at each end, which is why it
    // is a scoop and not just less middle.
    Preset {
        drive: 0.93,
        circuit: Circuit::HighGain,
        tone: ToneStack::Scooping,
        bass: 0.78,
        mid: 0.08,
        treble: 0.82,
        cab_model: CabModel::CaliOversized,
        mic_a_position: 0.25,
        mic_a_distance: 0.02,
        output_trim: 2.5,
        ..base("High Gain", "Scooped Metal")
    },
    Preset {
        drive: 0.96,
        circuit: Circuit::HighGain,
        tone: ToneStack::Scooping,
        bass: 0.7,
        mid: 0.2,
        treble: 0.75,
        cab_model: CabModel::BritClosed,
        mic_a_position: 0.35,
        mic_a_angle: 20.0,
        ..base("High Gain", "Thrash Rhythm")
    },
    Preset {
        drive: 0.85,
        circuit: Circuit::HighGain,
        bass: 0.6,
        mid: 0.62,
        treble: 0.55,
        cab_model: CabModel::Brit1960,
        mic_b: MicModel::Ribbon121,
        mic_b_distance: 0.2,
        mic_blend: 0.4,
        ..base("High Gain", "Lead Sustain")
    },
    Preset {
        drive: 1.00,
        circuit: Circuit::HighGain,
        tone: ToneStack::Scooping,
        bass: 0.85,
        mid: 0.05,
        treble: 0.7,
        cab_model: CabModel::Oversized,
        mic_a_position: 0.2,
        output_trim: 2.0,
        ..base("High Gain", "Everything Up")
    },
    Preset {
        drive: 0.88,
        circuit: Circuit::HighGain,
        bass: 0.35,
        mid: 0.45,
        treble: 0.7,
        cab_model: CabModel::BritV30,
        mic_a: MicModel::Dynamic906,
        mic_a_position: 0.25,
        mic_a_distance: 0.015,
        ..base("High Gain", "Tight Low End")
    },
    // --- Overdrive -------------------------------------------------------
    // Diodes in the loop, which lower the gain rather than stopping the
    // output -- so these keep following what is played and clean up when the
    // playing gets quieter.
    //
    // The two named after a particular green box are that box in the pedal
    // slot, in front of an amplifier, which is where one goes. They were the
    // pedal on its own with nothing after it; the pedal now carries the values
    // its drawings agree on, and an overdrive's point is what it does to the
    // amplifier behind it.
    Preset {
        pedal: PedalModel::Green808,
        pedal_drive: 0.7,
        pedal_tone: 0.5,
        pedal_level: 0.6,
        circuit: Circuit::Twin,
        drive: 0.45,
        tone: ToneStack::Off,
        bass: 0.5,
        mid: 0.5,
        treble: 0.55,
        cab_model: CabModel::AmericanOpen212,
        mic_a_position: 0.3,
        mic_a_distance: 0.03,
        oversampling: Oversampling::Off,
        // Twin Volume now changes real amplifier level. Preserve this preset
        // at its previously calibrated loudness with one fixed preset trim.
        output_trim: -12.0,
        ..base("Overdrive", "Green Overdrive")
    },
    // Little drive and a lot of level, into a British master-volume amplifier
    // already crunching: the boost tightens the bottom and pushes the preamp.
    Preset {
        pedal: PedalModel::Green808,
        pedal_drive: 0.15,
        pedal_tone: 0.6,
        pedal_level: 0.85,
        circuit: Circuit::Brit800,
        drive: 0.65,
        tone: ToneStack::Off,
        bass: 0.45,
        mid: 0.7,
        treble: 0.65,
        cab_model: CabModel::BritV30,
        mic_a_position: 0.35,
        mic_a_angle: 10.0,
        oversampling: Oversampling::Off,
        ..base("Overdrive", "Screamer Boost")
    },
    Preset {
        drive: 0.70,
        circuit: Circuit::Overdrive,
        tone: ToneStack::Off,
        cab_model: CabModel::AmericanOpen112,
        mic_a_position: 0.4,
        mic_a_distance: 0.04,
        oversampling: Oversampling::Four,
        ..base("Overdrive", "Transparent Boost")
    },
    Preset {
        drive: 0.90,
        circuit: Circuit::Overdrive,
        diode: Diode::Germanium,
        mid: 0.6,
        cab_model: CabModel::AmericanOpen112,
        speaker: SpeakerModel::AmericanAlnico,
        mic_a: MicModel::Ribbon121,
        mic_a_position: 0.3,
        mic_a_distance: 0.12,
        oversampling: Oversampling::Four,
        ..base("Overdrive", "Germanium Warmth")
    },
    Preset {
        drive: 0.95,
        circuit: Circuit::Overdrive,
        diode: Diode::Led,
        treble: 0.58,
        cab_model: CabModel::Closed112,
        mic_a_position: 0.3,
        oversampling: Oversampling::Four,
        output_trim: 2.0,
        ..base("Overdrive", "LED Headroom")
    },
    Preset {
        drive: 0.92,
        circuit: Circuit::Overdrive,
        mid: 0.7,
        cab_model: CabModel::AmericanOpen212,
        mic_a_position: 0.35,
        mic_a_distance: 0.05,
        oversampling: Oversampling::Four,
        ..base("Overdrive", "Overdriven Combo")
    },
    // --- Distortion ------------------------------------------------------
    // Diodes to ground: a ceiling the output stops at, so the wave is squared
    // off and the harmonics reach the top of the band. These want a cabinet.
    Preset {
        drive: 0.78,
        circuit: Circuit::Distortion,
        mid: 0.45,
        cab_model: CabModel::Closed212,
        mic_a_position: 0.35,
        oversampling: Oversampling::Four,
        ..base("Distortion", "Classic Distortion")
    },
    Preset {
        drive: 0.90,
        circuit: Circuit::Distortion,
        tone: ToneStack::Scooping,
        bass: 0.7,
        mid: 0.15,
        treble: 0.75,
        cab_model: CabModel::CaliOversized,
        mic_a_position: 0.3,
        oversampling: Oversampling::Four,
        output_trim: 2.0,
        ..base("Distortion", "Scooped Pedal")
    },
    Preset {
        drive: 0.85,
        circuit: Circuit::Distortion,
        diode: Diode::Germanium,
        bass: 0.65,
        cab_model: CabModel::BritGreen,
        mic_a_position: 0.45,
        mic_a_distance: 0.04,
        oversampling: Oversampling::Four,
        ..base("Distortion", "Woolly Fuzz")
    },
    Preset {
        drive: 0.97,
        circuit: Circuit::Distortion,
        diode: Diode::Led,
        bass: 0.55,
        treble: 0.7,
        cab_model: CabModel::BritV30,
        mic_a_position: 0.3,
        oversampling: Oversampling::Four,
        ..base("Distortion", "LED Wall")
    },
    Preset {
        drive: 0.92,
        circuit: Circuit::Distortion,
        mix: 0.6,
        cab_model: CabModel::Closed112,
        mic_a_position: 0.35,
        oversampling: Oversampling::Four,
        output_trim: 15.0,
        ..base("Distortion", "Blended Grit")
    },
    // The modelled four stage fuzz. Its tone control is a scoop rather than a
    // treble cut -- turning it *down* is what makes the notch -- so the stack
    // is off and the Treble knob is the pedal's own.
    Preset {
        drive: 0.90,
        circuit: Circuit::Muff,
        tone: ToneStack::Off,
        treble: 0.35,
        cab_model: CabModel::BritGreen,
        mic_a_position: 0.35,
        mic_b: MicModel::Ribbon121,
        mic_b_distance: 0.25,
        mic_blend: 0.35,
        oversampling: Oversampling::Off,
        ..base("Distortion", "Sustain Fuzz")
    },
    // --- Modelled amplifiers ---------------------------------------------
    // Whole preamplifiers rather than pedals, each on its own power stage, each
    // driving a speaker in a cabinet of the kind it was sold with. Their own
    // tone stacks carry the three knobs, so the plugin's stack is off -- except
    // the 5150, whose stack could not be traced and which borrows the plugin's.
    //
    // The Cali IIC+ now runs its lead return, V2B, Lead Master and V2A (see
    // `circuits::markiic`), so its Master knob is the Lead Master and its
    // Lead Drive distorts far earlier than it did. Both presets were re-voiced
    // for that: less Lead Drive, the Lead Master at five.
    Preset {
        drive: 0.60,
        master: 0.5,
        graphic: [0.62, 0.55, 0.35, 0.60, 0.58],
        circuit: Circuit::Boogie,
        tone: ToneStack::Off,
        bass: 0.55,
        mid: 0.35,
        treble: 0.70,
        cab_model: CabModel::Closed112,
        mic_a_position: 0.3,
        mic_a_distance: 0.03,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Boutique Lead")
    },
    // And the V everybody sets: 750 on the floor, the ends up. On this
    // amplifier that is not an equaliser curve applied to the output -- the
    // sliders are before the power stage, so scooping here changes what the
    // 6L6s are asked to do.
    Preset {
        drive: 0.70,
        master: 0.55,
        graphic: [0.75, 0.50, 0.10, 0.68, 0.70],
        circuit: Circuit::Boogie,
        tone: ToneStack::Off,
        bass: 0.60,
        mid: 0.25,
        treble: 0.75,
        cab_model: CabModel::CaliOversized,
        mic_a_position: 0.25,
        mic_a_angle: 10.0,
        oversampling: Oversampling::Off,
        output_trim: -4.5,
        ..base("Amplifier", "Boutique Rhythm")
    },
    // --- American Twin ------------------------------------------------------
    // The clean reference: chime, headroom, and the spring tank underneath.
    // The completed AB763 was re-measured rather than keeping the old knob
    // numbers: Volume 0.24 gives about 2 % core THD at nominal input, with the
    // familiar Bass 6 / Middle 3.5 / Treble 6.5 blackface scoop and Reverb 0.30
    // measuring about -12 dB against the dry path. Into an open-back 2x12 with
    // ceramic drivers, a 57 close and
    // a ribbon a little back. The Steel iron these carried is gone: with a real
    // power stage and speaker the amplifier already has its own output
    // transformer, and on the physical path the Iron control would sit in front
    // of the power stage, which an AB763 does not have.
    Preset {
        // Re-measured against the completed AB763 at 48 kHz: this is the
        // nearest one-percent Volume position to 2 % core THD at nominal input.
        drive: 0.24,
        circuit: Circuit::Twin,
        tone: ToneStack::Off,
        bass: 0.60,
        mid: 0.35,
        treble: 0.65,
        // Measured with the corrected 4AB3C1B pickup sensitivity: about
        // -11.9 dB spring return against the dry path.
        reverb: 0.30,
        cab_model: CabModel::AmericanOpen212,
        mic_a_position: 0.3,
        mic_a_distance: 0.03,
        mic_b: MicModel::Ribbon121,
        mic_b_position: 0.4,
        mic_b_distance: 0.3,
        mic_blend: 0.3,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Blackface Clean")
    },
    // The other AB763. Same stack, same spring, same optical tremolo, a
    // quarter of the power -- so where the Twin stays clean this one starts to
    // move, and that is the whole reason to have both. One 12 in an open back,
    // which is what the amplifier is.
    //
    // Volume sits low because the Drive parameter here is the physical channel
    // Volume pot, as on the Twin: it is the amplifier's own control, not a
    // normalised gain.
    Preset {
        drive: 0.30,
        circuit: Circuit::Deluxe,
        tone: ToneStack::Off,
        // No Middle control on this amplifier: the stack grounds through a
        // fixed 6.8 k resistor, so the panel's Middle knob is greyed out and
        // the value here is inert.
        bass: 0.55,
        mid: 0.5,
        treble: 0.65,
        reverb: 0.32,
        cab_model: CabModel::AmericanOpen112,
        // The C12N, which is what a blackface Deluxe of this period shipped
        // with -- not the alnico P12R. See docs/models/american_deluxe.md.
        speaker: SpeakerModel::AmericanCeramic,
        mic_a_position: 0.3,
        mic_a_distance: 0.03,
        // Measured by `examples/presetlevel`, not chosen: this is what puts
        // the preset on the catalogue's mean.
        output_trim: -1.0,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Blackface Deluxe")
    },
    // The sound the amplifier is known for: the Volume far enough up that the
    // 6V6s and the GZ34 are both working, so picking harder moves the tone and
    // not just the level. The tremolo running underneath it.
    Preset {
        drive: 0.62,
        circuit: Circuit::Deluxe,
        tone: ToneStack::Off,
        bass: 0.50,
        mid: 0.5,
        treble: 0.60,
        reverb: 0.25,
        speed: 0.42,
        intensity: 0.80,
        cab_model: CabModel::AmericanOpen112,
        // The C12N, which is what a blackface Deluxe of this period shipped
        // with -- not the alnico P12R. See docs/models/american_deluxe.md.
        speaker: SpeakerModel::AmericanCeramic,
        mic_a_position: 0.35,
        mic_a_distance: 0.04,
        // The Volume is twice as far up as the preset above and the power
        // stage is working, so it needs more of a trim. Measured.
        output_trim: -5.0,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Deluxe Breakup")
    },
    // The other end of the catalogue entirely: the first solid-state
    // amplifier here, and the one whose reputation is for never doing
    // anything. CH-1 wide open is 43 dB of gain with no valve compression
    // behind it, so the Volume sits where the channel is loud and still
    // linear, and the amplifier's own three-band stack does the voicing --
    // it has far more authority than a Fender's, which is why Bass and
    // Treble are nearer noon here than on the two blackface presets.
    //
    // No power stage: the JC-120's two 60 W transistor amplifiers are not
    // built yet, so Matched resolves to nothing and the channel hands
    // straight over. The cabinet is the nearest open-back 2x12 and a ceramic
    // 12 in it; the Roland 30-103D is APPROXIMATED by the C12N until its own
    // driver is measured. See docs/models/jazz_120.md.
    Preset {
        drive: 0.55,
        circuit: Circuit::Jazz120,
        tone: ToneStack::Off,
        // BRI off, which is how Roland's own test points are taken and how
        // the amplifier is usually played. It is not a dead control here --
        // SW2 shorts R4 and the 330 pF couples fully -- so it is worth
        // reaching for, but not as the default voicing.
        twin_bright: false,
        bass: 0.5,
        mid: 0.5,
        treble: 0.5,
        cab_model: CabModel::AmericanOpen212,
        speaker: SpeakerModel::AmericanCeramic,
        mic_a_position: 0.35,
        mic_a_distance: 0.05,
        mic_b: MicModel::Ribbon121,
        mic_b_position: 0.5,
        mic_b_distance: 0.25,
        mic_blend: 0.25,
        // The other direction: no power stage behind this one, so the chain
        // loses what every valve amplifier here gains. Measured, not guessed.
        output_trim: 4.0,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Jazz Clean")
    },
    // The AB763's other channel, which is the amplifier with everything taken
    // off it: no bright capacitor across the Volume, no reverb, no tremolo.
    // What is left is the stack, the two stages and the 6V6s, which is why
    // this is the one to put a pedal in front of. Treble sits higher than on
    // the Vibrato-channel presets because there is no 47 pF helping it along.
    Preset {
        drive: 0.45,
        circuit: Circuit::DeluxeNormal,
        tone: ToneStack::Off,
        // No Middle control on this amplifier either: the value is inert.
        bass: 0.55,
        mid: 0.5,
        treble: 0.70,
        cab_model: CabModel::AmericanOpen112,
        speaker: SpeakerModel::AmericanCeramic,
        mic_a_position: 0.3,
        mic_a_distance: 0.04,
        // Measured. The Normal channel is the quieter of the two -- no 47 pF
        // driving the second stage -- so it wants a little more, not less.
        output_trim: 1.0,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Blackface Normal")
    },
    // The same amplifier with its tremolo running: the completed circuit puts
    // its ~2 % clean-core point at Volume 0.17 with this tone/mic voicing.
    // Reverb 0.38 is about -10 dB wet/dry, and Intensity 0.94 measures about
    // 8 dB optical modulation at the shipped ~3.7 Hz speed.
    Preset {
        // The slightly different tone/mic voicing reaches the same clean core
        // target lower on the Volume control.
        drive: 0.17,
        circuit: Circuit::Twin,
        tone: ToneStack::Off,
        bass: 0.55,
        mid: 0.40,
        treble: 0.60,
        // A little more spring than Clean: measured about -10.0 dB wet/dry.
        reverb: 0.38,
        speed: 0.40,
        // Measured on the completed optical cell for about 8 dB modulation
        // depth at the shipped ~3.7 Hz speed.
        intensity: 0.94,
        cab_model: CabModel::AmericanOpen212,
        mic_a_position: 0.35,
        mic_a_distance: 0.04,
        oversampling: Oversampling::Off,
        // The Twin no longer normalises channel Volume dynamically. At this
        // 0.17 setting the fixed 0.24 calibration reference is 3.64 dB lower
        // than the old drive-following make-up. +2.1 dB keeps the shipped
        // preset at the same calibrated level relative to Blackface Clean.
        output_trim: 2.1,
        ..base("Amplifier", "Blackface Throb")
    },
    Preset {
        drive: 0.90,
        circuit: Circuit::Peavey,
        tone: ToneStack::Scooping,
        bass: 0.70,
        mid: 0.15,
        treble: 0.75,
        cab_model: CabModel::CaliOversized,
        mic_a_position: 0.3,
        mic_a_angle: 15.0,
        oversampling: Oversampling::Off,
        output_trim: 2.5,
        ..base("Amplifier", "Ultra Lead")
    },
    Preset {
        drive: 0.75,
        circuit: Circuit::Peavey,
        tone: ToneStack::Wide,
        bass: 0.45,
        mid: 0.50,
        treble: 0.65,
        cab_model: CabModel::Oversized,
        mic_a_position: 0.3,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Ultra Rhythm")
    },
    // --- Brit 800 -------------------------------------------------------------
    // The 1981 master-volume British lead amplifier on its own EL34 power
    // stage. Middle up and bass back is how the stack is usually run, into a
    // closed 4x12: T75s for the stock cabinet of the time, V30s for the lead.
    Preset {
        drive: 0.45,
        circuit: Circuit::Brit800,
        tone: ToneStack::Off,
        bass: 0.5,
        mid: 0.65,
        treble: 0.6,
        cab_model: CabModel::Brit1960,
        mic_a_position: 0.35,
        mic_a_distance: 0.025,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Brit Crunch")
    },
    Preset {
        drive: 0.90,
        circuit: Circuit::Brit800,
        tone: ToneStack::Off,
        bass: 0.45,
        mid: 0.8,
        treble: 0.7,
        cab_model: CabModel::BritV30,
        mic_a_position: 0.3,
        mic_a_angle: 10.0,
        mic_b: MicModel::Ribbon121,
        mic_b_distance: 0.2,
        mic_blend: 0.3,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Brit Lead")
    },
    // --- Brit Plexi -----------------------------------------------------------
    // The late-60s non-master British 100 W head on its own EL34 stage. No master,
    // so Volume is how hard the power valves work: moderate for crunch, most of
    // the way up for the cranked sound, into a 4x12 of 25 W greenbacks.
    Preset {
        drive: 0.40,
        circuit: Circuit::Plexi,
        tone: ToneStack::Off,
        bass: 0.45,
        mid: 0.7,
        treble: 0.6,
        cab_model: CabModel::BritGreen,
        mic_a_position: 0.35,
        mic_a_distance: 0.025,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Plexi Crunch")
    },
    Preset {
        drive: 0.85,
        circuit: Circuit::Plexi,
        tone: ToneStack::Off,
        bass: 0.4,
        mid: 0.75,
        treble: 0.55,
        cab_model: CabModel::BritGreen,
        mic_a_position: 0.3,
        mic_b: MicModel::Ribbon121,
        mic_b_distance: 0.3,
        mic_blend: 0.3,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Plexi Cranked")
    },
    // --- Brit AC30 -------------------------------------------------------------
    // The 30 W British combo: a cathode-biased EL84 stage with no feedback loop,
    // so it compresses rather than tightens as it is pushed. Treble and Bass
    // only; the Middle knob is greyed out because the amplifier has no such
    // control. Into its own 2x12 of alnico-voiced speakers, taken here as an
    // open-backed 2x12.
    Preset {
        drive: 0.35,
        circuit: Circuit::AC30,
        tone: ToneStack::Off,
        bass: 0.5,
        treble: 0.6,
        cab_model: CabModel::AmericanOpen212,
        speaker: SpeakerModel::AmericanAlnico,
        mic_a_position: 0.4,
        mic_a_distance: 0.04,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Chime Clean")
    },
    Preset {
        drive: 0.85,
        circuit: Circuit::AC30,
        tone: ToneStack::Off,
        bass: 0.45,
        treble: 0.55,
        cab_model: CabModel::AmericanOpen212,
        speaker: SpeakerModel::AmericanAlnico,
        mic_a_position: 0.3,
        mic_a_distance: 0.03,
        mic_b: MicModel::Ribbon160,
        mic_b_position: 0.5,
        mic_b_distance: 0.25,
        mic_blend: 0.3,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Chime Edge")
    },
    // --- Brit DR103 -------------------------------------------------------------
    // The 100 W head built for headroom: a master volume, 22 k grid stoppers and
    // an inverter that cannot shift its own bias. Clean at settings where the
    // others are breaking up, and it takes a pedal in front of it without
    // falling apart.
    Preset {
        drive: 0.45,
        master: 0.45,
        circuit: Circuit::DR103,
        tone: ToneStack::Off,
        bass: 0.6,
        mid: 0.55,
        treble: 0.6,
        cab_model: CabModel::BritClosed,
        mic_a_position: 0.4,
        mic_a_distance: 0.03,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Hi-Headroom Clean")
    },
    Preset {
        drive: 0.8,
        master: 0.6,
        circuit: Circuit::DR103,
        tone: ToneStack::Off,
        bass: 0.55,
        mid: 0.6,
        treble: 0.65,
        cab_model: CabModel::BritClosed,
        mic_a_position: 0.3,
        mic_b: MicModel::Condenser87,
        mic_b_distance: 0.4,
        mic_blend: 0.25,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Hi-Headroom Pushed")
    },
    // --- Cali Rectifier ----------------------------------------------------------
    // The red channel of the two-channel American head, in its modern setting:
    // five triodes with the third run cold on 39 k, into an oversized 4x12. The
    // gain control is what the knob turns; the red channel's own master is the
    // Master knob.
    Preset {
        drive: 0.55,
        master: 0.5,
        circuit: Circuit::Recto,
        tone: ToneStack::Off,
        bass: 0.55,
        mid: 0.35,
        treble: 0.6,
        cab_model: CabModel::CaliOversized,
        mic_a_position: 0.3,
        mic_a_distance: 0.02,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Recto Rhythm")
    },
    Preset {
        drive: 0.85,
        master: 0.55,
        circuit: Circuit::Recto,
        tone: ToneStack::Off,
        bass: 0.5,
        mid: 0.45,
        treble: 0.65,
        cab_model: CabModel::CaliOversized,
        mic_a_position: 0.25,
        mic_b: MicModel::Dynamic421,
        mic_b_position: 0.45,
        mic_b_distance: 0.03,
        mic_blend: 0.4,
        oversampling: Oversampling::Off,
        ..base("Amplifier", "Recto Lead")
    },
    // The console microphone preamplifier: two cascaded transistor stages with
    // an input transformer. Clean at low drive, warming into subtle harmonic
    // saturation as the gain increases. No cabinet -- this is a preamplifier,
    // not an amplifier.
    Preset {
        drive: 0.35,
        circuit: Circuit::Neve,
        tone: ToneStack::Off,
        cabinet: Cabinet::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "British 73 Pre")
    },
    Preset {
        drive: 0.55,
        circuit: Circuit::Neve,
        tone: ToneStack::Off,
        cabinet: Cabinet::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "British 73, Driven")
    },
    // The other three console microphone preamplifiers, each modelled from its
    // drawing: a transformer, one discrete op-amp and a transformer; a 1:10
    // transformer into two op-amps around one pot; and a valve channel with
    // feedback round both pairs of triodes. No cabinet on any of them.
    Preset {
        drive: 0.55,
        circuit: Circuit::American312,
        tone: ToneStack::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "American Console Pre")
    },
    Preset {
        drive: 0.90,
        circuit: Circuit::American312,
        tone: ToneStack::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "American Console, Pushed")
    },
    Preset {
        drive: 0.55,
        circuit: Circuit::ConsoleE,
        tone: ToneStack::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "British Desk Pre")
    },
    Preset {
        drive: 0.90,
        circuit: Circuit::ConsoleE,
        tone: ToneStack::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "British Desk, Driven")
    },
    Preset {
        drive: 0.60,
        circuit: Circuit::Tube610,
        tone: ToneStack::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "Valve Console Pre")
    },
    Preset {
        drive: 0.92,
        circuit: Circuit::Tube610,
        tone: ToneStack::Off,
        oversampling: Oversampling::Off,
        ..base("Preamp", "Valve Console, Pushed")
    },
    // --- Era presets -----------------------------------------------------
    // Configurations of the modular chain, named after the records whose
    // guitar sound they are aimed at. Nothing here is special-cased in the DSP;
    // what each one is built from, and how firmly its rig is documented, is in
    // PRESETS.md.
    //
    // The Cali IIC+ on its own 6L6 power stage. The producer stated in 2018 that
    // most of the album's guitars used the Boogie power stage and the JCM800 only
    // served as a power amplifier on some solos, so the "IIC+ slaved into a
    // Marshall" story does not describe the rhythm tracks. Lead Drive and Volume
    // high, treble high, middle low; two 4x12s, a 57 in the cone and a valve
    // condenser about a metre out at 45 degrees.
    //
    // Bass low and the low end from the graphic, which is Mesa's own rule for
    // this amplifier: "As gain goes up, Bass should come down", with BASS "at 3.0
    // or well below" at a high Volume 1, and extra low end taken from the two
    // lowest graphic bands, which sit after the gain stages. This preset shipped
    // with Bass at 6.5 and the 80 Hz slider most of the way up, on a Bass pot
    // that was then modelled linear, and it was far too heavy. The V is a
    // moderate one: the manual warns against scooping 750 Hz until nothing is
    // left.
    Preset {
        circuit: Circuit::Boogie,
        power_amp: PowerAmp::Matched,
        drive: 0.80,
        master: 0.55,
        graphic: [0.65, 0.45, 0.25, 0.65, 0.70],
        bass: 0.20,
        mid: 0.35,
        treble: 0.75,
        tone: ToneStack::Off,
        cab_model: CabModel::BritClosed,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.2,
        mic_a_distance: 0.025,
        mic_a_angle: 0.0,
        mic_b: MicModel::TubeCondenser67,
        mic_b_position: 0.5,
        mic_b_distance: 1.0,
        mic_b_angle: 45.0,
        mic_blend: 0.25,
        // A hot passive humbucker rather than the nominal guitar.
        input_trim: 3.0,
        // A green overdrive in front, low drive and high output: the setting a
        // player uses to tighten an amplifier rather than to distort it.
        //
        // **This departs from the evidence on purpose.** The producer says the
        // rhythm tracks went straight in -- "a lot of the distortion would come
        // from the amp itself" -- and no source puts a pedal on this record.
        // The owner asked for one; it is here as a sound rather than as a
        // claim, and PRESETS.md says so in the same words.
        //
        // It does less than it looks like it should, and the reason is the
        // reason the producer gave: with Drive at 0.80 and a hot humbucker the
        // Boogie is already saturated, so the boost has little left to push.
        // Measured on a low E and its fifth through the whole chain: total
        // distortion 18.9 % -> 19.3 %, and the low-to-high balance 7.2 -> 6.8
        // dB, which is the tightening. The lever for a bigger change is the
        // amplifier's Drive or the graphic, not this.
        pedal: PedalModel::Green808,
        pedal_drive: 0.15,
        pedal_tone: 0.5,
        pedal_level: 0.85,
        oversampling: Oversampling::Off,
        ..base("Metal / Heavy", "Puppet Master '86")
    },
    // Two guitars in B standard through a Boss HM-2 with **every knob at ten**,
    // into a small solid-state combo. The pedal is the whole sound here: the
    // amplifier is clean and the chainsaw is entirely the HM-2's three gyrators
    // and its coring gate. See PRESETS.md.
    Preset {
        pedal: PedalModel::HeavyMetal,
        // All four dialled hard right, which is the documented setting and the
        // one the record is named for in every retelling.
        pedal_drive: 1.0,
        pedal_tone: 1.0,
        pedal_tone_b: 1.0,
        pedal_level: 0.5,
        // A solid-state Peavey Studio Pro 40 is not modelled. The nearest
        // thing the plugin has to "a clean amplifier that only makes it
        // louder" is the blackface channel with its power stage bypassed --
        // a guitar circuit at a guitar's level, which the Clean voice is not:
        // that one is a line-level preamplifier and putting a cabinet on it is
        // a different mistake. APPROXIMATED.
        circuit: Circuit::Twin,
        power_amp: PowerAmp::Bypass,
        drive: 0.30,
        bass: 0.45,
        mid: 0.50,
        treble: 0.55,
        tone: ToneStack::Off,
        // A 1x12 combo, closed.
        cab_model: CabModel::Closed112,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.25,
        mic_a_distance: 0.03,
        mic_a_angle: 0.0,
        mic_b: MicModel::Off,
        mic_blend: 0.0,
        // Tuned to B, so the source is lower and hotter than nominal.
        input_trim: 2.0,
        // An HM-2 with everything at ten into a clean amplifier is the loudest
        // thing in the catalogue; this is the trim that keeps it level with the
        // rest. Measured with `examples/presetlevel.rs`.
        output_trim: -11.5,
        oversampling: Oversampling::Off,
        ..base("Metal / Heavy", "Swedish Death '90")
    },
    // The documented chain is a Boss HM-2 **into** a Boss MT-2 -- the first for
    // the buzz, the second to tighten it -- which is a thing this plugin can
    // only express now that the pedals are also circuits: the HM-2 in the pedal
    // slot, the MT-2 as the circuit behind it. See PRESETS.md.
    Preset {
        pedal: PedalModel::HeavyMetal,
        pedal_drive: 0.85,
        pedal_tone: 0.75,
        pedal_tone_b: 0.80,
        pedal_level: 0.25,
        circuit: Circuit::Mt2,
        power_amp: PowerAmp::Bypass,
        // The Metal Zone's own Dist, and its equaliser doing the tightening:
        // the middle up and swept into the low mids, which is what that control
        // is for.
        drive: 0.55,
        bass: 0.45,
        mid: 0.60,
        treble: 0.65,
        tone: ToneStack::Off,
        cab_model: CabModel::BritClosed,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.3,
        mic_a_distance: 0.03,
        mic_a_angle: 0.0,
        mic_b: MicModel::Dynamic421,
        mic_b_position: 0.45,
        mic_b_distance: 0.08,
        mic_b_angle: 0.0,
        mic_blend: 0.3,
        // Two pedals in series, each attenuating after it clips, so the chain
        // arrives quiet. Measured with `examples/presetlevel.rs`.
        output_trim: 0.0,
        oversampling: Oversampling::Off,
        ..base("Metal / Heavy", "Slaughter '95")
    },
    // A 100 W non-master British head turned up only as far as the song needed,
    // into a Marshall 4x12, each guitar on two large-diaphragm valve/FET
    // condensers placed on different speakers. See PRESETS.md.
    Preset {
        circuit: Circuit::Plexi,
        power_amp: PowerAmp::Matched,
        drive: 0.55,
        bass: 0.40,
        mid: 0.70,
        treble: 0.60,
        tone: ToneStack::Off,
        cab_model: CabModel::Brit1960,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::TubeCondenser67,
        mic_a_position: 0.35,
        mic_a_distance: 0.10,
        mic_a_angle: 0.0,
        mic_b: MicModel::Condenser87,
        mic_b_position: 0.6,
        mic_b_distance: 0.15,
        mic_b_angle: 0.0,
        mic_blend: 0.5,
        oversampling: Oversampling::Off,
        ..base("Classic Rock", "Blackout '80")
    },
    // A germanium fuzz in front of a British 100 W head played loud, into a 4x12
    // of greenbacks; a ribbon straight at the cone and a valve condenser. See
    // PRESETS.md.
    Preset {
        pedal: PedalModel::RoundFuzz,
        pedal_drive: 0.85,
        pedal_level: 0.6,
        circuit: Circuit::Plexi,
        power_amp: PowerAmp::Matched,
        drive: 0.70,
        bass: 0.50,
        mid: 0.60,
        treble: 0.60,
        tone: ToneStack::Off,
        cab_model: CabModel::BritGreen,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Ribbon160,
        mic_a_position: 0.3,
        mic_a_distance: 0.05,
        mic_a_angle: 0.0,
        mic_b: MicModel::TubeCondenser67,
        mic_b_position: 0.5,
        mic_b_distance: 0.5,
        mic_b_angle: 0.0,
        mic_blend: 0.35,
        oversampling: Oversampling::Off,
        ..base("Psychedelic / Lead", "Experienced '67")
    },
    // A four-transistor fuzz with its sustain well down, into a 100 W head with
    // headroom to spare and a 4x12. The amplifier does not distort; the pedal
    // does, and the power stage only compresses it. See PRESETS.md.
    Preset {
        pedal: PedalModel::BigMuff,
        pedal_drive: 0.35,
        pedal_tone: 0.55,
        pedal_level: 0.75,
        circuit: Circuit::DR103,
        power_amp: PowerAmp::Matched,
        drive: 0.45,
        master: 0.5,
        bass: 0.6,
        mid: 0.55,
        treble: 0.6,
        tone: ToneStack::Off,
        cab_model: CabModel::BritClosed,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic421,
        mic_a_position: 0.35,
        mic_a_distance: 0.04,
        mic_b: MicModel::Condenser87,
        mic_b_position: 0.5,
        mic_b_distance: 0.5,
        mic_blend: 0.3,
        // A fuzz into a hundred watts of headroom would otherwise be one of the
        // loudest things in the catalogue; this is the trim that keeps it level
        // with the rest. Measured with `examples/presetlevel.rs`, and measured
        // again when the pedal slot's resting level moved under it.
        output_trim: -1.0,
        oversampling: Oversampling::Off,
        ..base("Psychedelic / Lead", "The Great Wall '79")
    },
    // A late-60s British 100 W head run from a variac at about seventy per cent
    // of its mains, everything on the stop, into a 4x12. The variac is the
    // documented part of this rig and the reason it sounds the way it does:
    // every rail comes down together, so the amplifier is soft and compressed at
    // a setting where it would otherwise be brutal. See PRESETS.md.
    Preset {
        circuit: Circuit::Plexi,
        power_amp: PowerAmp::Matched,
        mains: Mains::Seventy,
        drive: 1.0,
        bass: 0.55,
        mid: 0.65,
        treble: 0.7,
        tone: ToneStack::Off,
        cab_model: CabModel::BritGreen,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.3,
        mic_a_distance: 0.02,
        mic_a_angle: 0.0,
        mic_b: MicModel::Condenser87,
        mic_b_position: 0.5,
        mic_b_distance: 0.9,
        mic_b_angle: 30.0,
        mic_blend: 0.3,
        // A variac takes the whole amplifier down, so this one is quiet.
        output_trim: 3.0,
        oversampling: Oversampling::Off,
        ..base("Classic Rock", "Brown '78")
    },
    // The same amplifier and the same variac six years later, with the guitar
    // brighter and the room further back.
    Preset {
        circuit: Circuit::Plexi,
        power_amp: PowerAmp::Matched,
        mains: Mains::Seventy,
        drive: 1.0,
        bass: 0.5,
        mid: 0.7,
        treble: 0.75,
        tone: ToneStack::Off,
        cab_model: CabModel::BritGreen,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.35,
        mic_a_distance: 0.03,
        mic_b: MicModel::Ribbon121,
        mic_b_position: 0.5,
        mic_b_distance: 0.4,
        mic_blend: 0.35,
        // A variac takes the whole amplifier down, so this one is quiet.
        output_trim: 3.0,
        oversampling: Oversampling::Off,
        ..base("Classic Rock", "Brown '84")
    },
    // A germanium distortion pedal kept on all the time, into a stock 100 W
    // British head run from a variac at about ninety volts, into 4x12s. See
    // PRESETS.md.
    Preset {
        pedal: PedalModel::YellowDist,
        pedal_drive: 0.6,
        pedal_level: 0.7,
        circuit: Circuit::Plexi,
        power_amp: PowerAmp::Matched,
        mains: Mains::Eighty,
        drive: 0.85,
        bass: 0.5,
        mid: 0.6,
        treble: 0.7,
        tone: ToneStack::Off,
        cab_model: CabModel::BritClosed,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.3,
        mic_a_distance: 0.02,
        mic_b: MicModel::Condenser87,
        mic_b_position: 0.5,
        mic_b_distance: 0.6,
        mic_blend: 0.3,
        // A variac again, so this one is quiet.
        output_trim: 4.0,
        oversampling: Oversampling::Off,
        ..base("Classic Rock", "Blizzard '80")
    },
    // One master-volume British head and one 4x12, unchanged for every record
    // the band made, with the guitar's own volume doing the rest. See PRESETS.md.
    Preset {
        circuit: Circuit::Brit800,
        power_amp: PowerAmp::Matched,
        drive: 0.65,
        master: 0.7,
        bass: 0.45,
        mid: 0.55,
        treble: 0.7,
        tone: ToneStack::Off,
        cab_model: CabModel::Oversized,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.25,
        mic_a_distance: 0.02,
        mic_b: MicModel::Dynamic421,
        mic_b_position: 0.4,
        mic_b_distance: 0.03,
        mic_blend: 0.35,
        output_trim: -11.0,
        oversampling: Oversampling::Off,
        ..base("Alternative", "Machine Rage '92")
    },
    // A four-transistor fuzz into the low input of an early-80s British
    // master-volume head with its master wide open, the preamp volume doing the
    // adjusting. Two 4x12s' worth of the same take, layered on the record; here
    // it is one. See PRESETS.md.
    Preset {
        pedal: PedalModel::BigMuff,
        pedal_drive: 0.7,
        pedal_tone: 0.45,
        pedal_level: 0.55,
        circuit: Circuit::Brit800,
        power_amp: PowerAmp::Matched,
        drive: 0.35,
        master: 1.0,
        bass: 0.5,
        mid: 0.6,
        treble: 0.6,
        tone: ToneStack::Off,
        cab_model: CabModel::Brit1960,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.35,
        mic_a_distance: 0.02,
        mic_b: MicModel::Ribbon121,
        mic_b_position: 0.45,
        mic_b_distance: 0.15,
        mic_blend: 0.35,
        // A fuzz into a master-volume head with the master wide open is the
        // loudest thing here by a distance; this is what levels it.
        output_trim: -24.0,
        oversampling: Oversampling::Off,
        ..base("Alternative", "Pumpkin Dream '93")
    },
    // A Tube Screamer with little drive and a lot of level, into a blackface
    // AB763 channel on the edge of breaking up, into an open-back combo.
    Preset {
        pedal: PedalModel::Green808,
        pedal_drive: 0.25,
        pedal_tone: 0.55,
        pedal_level: 0.85,
        circuit: Circuit::Twin,
        drive: 0.55,
        bass: 0.45,
        mid: 0.60,
        treble: 0.55,
        reverb: 0.12,
        tone: ToneStack::Off,
        cab_model: CabModel::AmericanOpen112,
        speaker: SpeakerModel::AmericanCeramic,
        mic_a: MicModel::Dynamic57,
        mic_a_position: 0.25,
        mic_a_distance: 0.03,
        mic_a_angle: 0.0,
        oversampling: Oversampling::Off,
        // Preserve the shipped level after making Twin Volume a real level
        // control instead of a gain-normalised Drive control.
        output_trim: -14.2,
        ..base("Blues", "Texas Storm '83")
    },
];

/// The groups, in the order they should be shown: quietest first, so the list
/// itself reads as a range rather than as an alphabetical accident.
pub const GROUPS: [&str; 12] = [
    "Studio",
    "Preamp",
    "Crunch",
    "High Gain",
    "Overdrive",
    "Distortion",
    // Last because it is a different kind of entry: not a topology set up to
    // make a sound, but a particular amplifier modelled from its drawing.
    "Amplifier",
    // Modular chains aimed at particular records. See PRESETS.md.
    "Classic Rock",
    "Psychedelic / Lead",
    "Alternative",
    "Metal / Heavy",
    "Blues",
];

impl Preset {
    /// What the chain is set to when this preset is loaded, as the plugin
    /// would build it from the parameters: for measuring presets exactly as they
    /// play, including their pedal, power stage and cabinet.
    pub fn settings(&self) -> crate::voice::Settings {
        use crate::acoustics::mic::MicPlacement;
        let place = |position: f32, distance: f32, angle: f32| MicPlacement {
            position: position as f64,
            distance: distance as f64,
            angle: angle as f64,
        };
        crate::voice::Settings {
            mains: self.mains.fraction(),
            pedal: crate::voice::PedalSettings {
                pedal: self.pedal.voice(),
                drive: self.pedal_drive as f64,
                tone: [
                    self.pedal_tone as f64,
                    self.pedal_tone_b as f64,
                    self.pedal_tone_c as f64,
                    self.pedal_tone_d as f64,
                ],
                level: self.pedal_level as f64,
            },
            power_amp: self.power_amp.voice(),
            acoustic: crate::voice::AcousticSettings {
                cabinet: self.cab_model.voice(),
                speaker: self.speaker.voice(),
                mic_a: self.mic_a.voice(false),
                mic_b: self.mic_b.voice(true),
                place_a: place(self.mic_a_position, self.mic_a_distance, self.mic_a_angle),
                place_b: place(self.mic_b_position, self.mic_b_distance, self.mic_b_angle),
                blend: self.mic_blend as f64,
                pan_a: self.mic_a_pan as f64,
                pan_b: self.mic_b_pan as f64,
                invert_b: self.mic_b_invert,
                align: self.mic_align,
            },
            gain: self.circuit.voice(),
            diode: if self.circuit.has_diodes() {
                self.diode.voice()
            } else {
                Diode::Silicon.voice()
            },
            amplifier: if self.circuit.has_amplifier() {
                self.amplifier.voice()
            } else {
                Amplifier::Valve.voice()
            },
            iron: self.iron.voice(),
            tone: self.tone.voice(),
            cabinet: self.cabinet.voice(),
            drive: self.drive as f64,
            master: self.master as f64,
            graphic: self.graphic.map(|g| g as f64),
            bass: self.bass as f64,
            mid: self.mid as f64,
            treble: self.treble as f64,
            tone_sweep: if self.circuit == Circuit::Mt2 {
                self.tone_sweep as f64
            } else {
                0.5
            },
            hm2_colour_lo: if self.circuit == Circuit::Hm2 {
                self.hm2_colour_lo as f64
            } else {
                0.5
            },
            hm2_colour_hi: if self.circuit == Circuit::Hm2 {
                self.hm2_colour_hi as f64
            } else {
                0.5
            },
            twin_low_input: self.twin_low_input,
            twin_bright: self.twin_bright,
            reverb: self.reverb as f64,
            speed: self.speed as f64,
            intensity: self.intensity as f64,
            oversampling: self.oversampling.factor(),
        }
    }

    /// The preset as parameter ids and the values the panel would show, which
    /// is the form the host wants them in.
    ///
    /// Ids rather than fields, because this is what gets pushed out as
    /// ordinary parameter gestures: an automatable, undoable edit like any
    /// other, rather than a set of assignments the host never hears about. It
    /// is also the shape a preset saved to disk would take, so user presets
    /// can join the same path later without any of this changing.
    pub fn dials(&self) -> [(&'static str, f32); 54] {
        [
            ("in_trim", self.input_trim),
            ("noise_reduction", 0.0),
            ("noise_threshold", -60.0),
            (
                "twin_low_input",
                if self.twin_low_input { 1.0 } else { 0.0 },
            ),
            ("twin_bright", if self.twin_bright { 1.0 } else { 0.0 }),
            ("pedal", self.pedal.to_index() as f32),
            ("pedal_drive", self.pedal_drive),
            ("pedal_tone", self.pedal_tone),
            ("pedal_tone_b", self.pedal_tone_b),
            ("pedal_tone_c", self.pedal_tone_c),
            ("pedal_tone_d", self.pedal_tone_d),
            ("pedal_level", self.pedal_level),
            ("tone_sweep", self.tone_sweep),
            ("hm2_colour_lo", self.hm2_colour_lo),
            ("hm2_colour_hi", self.hm2_colour_hi),
            ("circuit", self.circuit.to_index() as f32),
            ("power_amp", self.power_amp.to_index() as f32),
            ("mains", self.mains.to_index() as f32),
            ("diode", self.diode.to_index() as f32),
            ("amplifier", self.amplifier.to_index() as f32),
            ("iron", self.iron.to_index() as f32),
            ("drive", self.drive),
            ("master", self.master),
            ("eq60", self.graphic[0]),
            ("eq240", self.graphic[1]),
            ("eq750", self.graphic[2]),
            ("eq2200", self.graphic[3]),
            ("eq6600", self.graphic[4]),
            ("tone", self.tone.to_index() as f32),
            ("bass", self.bass),
            ("mid", self.mid),
            ("treble", self.treble),
            ("cabinet", self.cabinet.to_index() as f32),
            ("cab_model", self.cab_model.to_index() as f32),
            ("speaker", self.speaker.to_index() as f32),
            ("mic_a", self.mic_a.to_index() as f32),
            ("mic_a_position", self.mic_a_position),
            ("mic_a_distance", self.mic_a_distance),
            ("mic_a_angle", self.mic_a_angle),
            ("mic_b", self.mic_b.to_index() as f32),
            ("mic_b_position", self.mic_b_position),
            ("mic_b_distance", self.mic_b_distance),
            ("mic_b_angle", self.mic_b_angle),
            ("mic_blend", self.mic_blend),
            ("mic_a_pan", self.mic_a_pan),
            ("mic_b_pan", self.mic_b_pan),
            ("mic_b_invert", if self.mic_b_invert { 1.0 } else { 0.0 }),
            ("mic_align", if self.mic_align { 1.0 } else { 0.0 }),
            ("reverb", self.reverb),
            ("speed", self.speed),
            ("intensity", self.intensity),
            ("mix", self.mix),
            ("out_trim", self.output_trim),
            ("oversampling", self.oversampling.to_index() as f32),
        ]
    }
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
use std::ffi::OsString;
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
    /// Stable enum values are authoritative; numeric values retain old-reader support.
    #[serde(default)]
    pub model_ids: BTreeMap<String, String>,
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
///
/// Per platform, because `$XDG_CONFIG_HOME` and `$HOME` are a Unix
/// convention: neither is normally set on Windows, so the Unix rule returned
/// nothing there and saving a preset failed with "no config directory". A host
/// started from a Unix-style shell was worse than that -- it did find `$HOME`,
/// and wrote the presets somewhere no Windows DAW session would look again.
/// Windows keeps per-user application data under `%APPDATA%`, which is the
/// roaming profile, and a preset is exactly the kind of thing that should
/// roam.
pub fn preset_dir() -> Option<PathBuf> {
    // `cfg!` rather than `#[cfg]`, so both rules are compiled on both
    // platforms and the Windows one can be tested from a Linux machine. It is
    // the rule a report arrived about, and a rule only ever compiled on the
    // platform nobody here develops on is a rule nobody can check.
    if cfg!(windows) {
        windows_preset_dir(std::env::var_os("APPDATA"), std::env::var_os("USERPROFILE"))
    } else {
        xdg_preset_dir(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
    }
}

/// `%APPDATA%\GainStageFx\Presets`, falling back to deriving the roaming
/// directory from `%USERPROFILE%` for the rare host that clears `APPDATA`.
///
/// Takes the two variables rather than reading them, so the rule is a pure
/// function and its test does not have to touch the environment every other
/// test in the binary is also reading.
fn windows_preset_dir(appdata: Option<OsString>, userprofile: Option<OsString>) -> Option<PathBuf> {
    let base = appdata
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            userprofile
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join("AppData").join("Roaming"))
        })?;
    Some(base.join("GainStageFx").join("Presets"))
}

/// `$XDG_CONFIG_HOME/gainstagefx/presets`, or `~/.config` below it. This is
/// also what macOS gets: it is where presets have always been written there,
/// and moving them would lose everyone's.
fn xdg_preset_dir(config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))?;
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
    for preset in &mut all {
        migrate(preset, params);
    }
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
            model_ids: BTreeMap::new(),
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

/// Enum identities are independent of display labels and normalized list lengths.
fn ids(id: &str) -> Option<&'static [&'static str]> {
    match id {
        "circuit" => Circuit::ids(),
        "power_amp" => PowerAmp::ids(),
        "diode" => Diode::ids(),
        "amplifier" => Amplifier::ids(),
        "iron" => Iron::ids(),
        "tone" => ToneStack::ids(),
        "cabinet" => Cabinet::ids(),
        "cab_model" => CabModel::ids(),
        "pedal" => PedalModel::ids(),
        "speaker" => SpeakerModel::ids(),
        "mic_a" | "mic_b" => MicModel::ids(),
        "oversampling" => Oversampling::ids(),
        _ => None,
    }
}

fn model_ids(values: &BTreeMap<String, f32>, params: &impl Params) -> BTreeMap<String, String> {
    params
        .param_map()
        .into_iter()
        .filter_map(|(id, ptr, _)| {
            let names = ids(&id)?;
            let value = values.get(&id)?;
            // SAFETY: param_map pointers are owned by params for this call.
            let index = unsafe { ptr.preview_plain(*value) } as usize;
            Some((id, names.get(index)?.to_string()))
        })
        .collect()
}

/// Resolve the circuit stored in a normalized value map using the real host
/// parameter. Keeping this conversion in one place avoids assuming that enum
/// normalized values or list lengths will stay fixed forever.
fn circuit_in(values: &BTreeMap<String, f32>, params: &impl Params) -> Option<Circuit> {
    let value = *values.get("circuit")?;
    let (_, ptr, _) = params
        .param_map()
        .into_iter()
        .find(|(id, _, _)| id == "circuit")?;
    // SAFETY: the pointer belongs to `params` for the duration of this call.
    let index = unsafe { ptr.preview_plain(value) }.round().max(0.0) as usize;
    Circuit::ALL.get(index).copied()
}

/// Circuit-private controls must never be latent state carried by an unrelated
/// preset. Neutralize them unless their owning circuit is the one being saved
/// or loaded. The values here are normalized and all three parameters are
/// linear 0..1 controls, so 0.5 is exactly noon.
fn isolate_circuit_controls(values: &mut BTreeMap<String, f32>, circuit: Circuit) {
    if circuit != Circuit::Mt2 {
        values.insert("tone_sweep".into(), 0.5);
    }
    if circuit != Circuit::Hm2 {
        values.insert("hm2_colour_lo".into(), 0.5);
        values.insert("hm2_colour_hi".into(), 0.5);
    }
}

/// Upgrade routing defaults and resolve saved stable IDs on the UI/state thread.
/// Older enum lists remain unchanged, so their normalized legacy values still load.
pub fn migrate(preset: &mut Stored, params: &impl Params) {
    // A saved preset from before stable ids stored its circuit against the
    // thirteen-entry list. Re-express that position against the current list.
    // Shipped presets are converted from plain indices and never need this.
    if !preset.built_in && !preset.model_ids.contains_key("circuit") {
        if let Some(value) = preset.values.get_mut("circuit") {
            let last = (crate::params::LEGACY_CIRCUIT_COUNT - 1) as f32;
            let index = (value.clamp(0.0, 1.0) * last).round();
            *value = index / (Circuit::ALL.len() - 1) as f32;
        }
    }
    preset.values.entry("power_amp".into()).or_insert(0.0);
    // Normalized threshold: -60 dBFS is halfway through [-90, -30].
    preset.values.entry("noise_reduction".into()).or_insert(0.0);
    preset.values.entry("noise_threshold".into()).or_insert(0.5);
    // Legacy is the first cabinet model, so an old preset keeps its baked filter.
    preset.values.entry("cab_model".into()).or_insert(0.0);
    // No pedal is the first pedal entry.
    preset.values.entry("pedal".into()).or_insert(0.0);
    // And the wall is the first mains entry, which is what an amplifier that
    // has never heard of a variac is plugged into.
    preset.values.entry("mains".into()).or_insert(0.0);
    // A pedal's second, third and fourth tone controls arrived with the pedals
    // that have them. A preset saved before that leaves them where a knob
    // nobody has touched belongs, which is the middle.
    for id in ["pedal_tone_b", "pedal_tone_c", "pedal_tone_d", "tone_sweep"] {
        preset.values.entry(id.into()).or_insert(0.5);
    }
    // Microphone panning arrived after these presets were written. Centre is
    // 0.5 normalised on a -1..+1 range, and centre is what the plugin did
    // before the controls existed, so an old preset sounds the same.
    for id in ["mic_a_pan", "mic_b_pan"] {
        preset.values.entry(id.into()).or_insert(0.5);
    }
    for (id, ptr, _) in params.param_map() {
        if let (Some(names), Some(saved)) = (ids(&id), preset.model_ids.get(&id)) {
            if let Some(index) = names.iter().position(|name| *name == saved) {
                // SAFETY: pointers belong to params and outlive this operation.
                preset
                    .values
                    .insert(id, unsafe { ptr.preview_normalized(index as f32) });
            }
        }
    }

    // Heavy Metal originally borrowed the generic Bass/Treble parameters for
    // its Colour Mix. Preserve the sound of an old HM-2 user preset by copying
    // those two values into the new dedicated controls on first migration.
    // Keep Bass/Treble too: if the plugin stack was also enabled, the old
    // preset drove both layers from the same knobs and should still do so.
    let circuit = circuit_in(&preset.values, params).unwrap_or(Circuit::Crunch);
    if circuit == Circuit::Hm2 {
        let low = preset.values.get("bass").copied().unwrap_or(0.5);
        let high = preset.values.get("treble").copied().unwrap_or(0.5);
        preset.values.entry("hm2_colour_lo".into()).or_insert(low);
        preset.values.entry("hm2_colour_hi".into()).or_insert(high);
    } else {
        preset.values.entry("hm2_colour_lo".into()).or_insert(0.5);
        preset.values.entry("hm2_colour_hi".into()).or_insert(0.5);
    }
    isolate_circuit_controls(&mut preset.values, circuit);
    preset.model_ids = model_ids(&preset.values, params);
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
    let mut values = live_values(params);
    let circuit = circuit_in(&values, params).unwrap_or(Circuit::Crunch);
    isolate_circuit_controls(&mut values, circuit);
    Stored {
        model_ids: model_ids(&values, params),
        name: name.trim().to_string(),
        values,
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

/// The MS-DOS device names, which Windows still resolves before it looks at
/// the filesystem. `CON.json` is not a file there: it is the console, and
/// opening it for writing fails whatever directory you are in. The extension
/// does not save you -- the rule is applied to the stem.
const WINDOWS_DEVICE_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Turns a preset name into something safe to use as a file name.
///
/// Everything but letters, digits, space, hyphen and underscore becomes an
/// underscore, which covers the characters Windows forbids as well as the ones
/// Unix minds. Two Windows-only rules on top of that, because a preset called
/// `Aux` or `Clean.` is a name somebody will really use:
///
/// Windows' reserved device names then need one rule of their own: a stem that
/// *is* one gets an underscore appended. Its other rule -- that a trailing dot
/// or space is stripped silently, which would leave the file under a name
/// nothing looks for -- needs no code, because `trim` has taken the spaces and
/// the map above has already turned the dot into an underscore.
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
        return "preset".to_string();
    }
    if WINDOWS_DEVICE_NAMES
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(device))
    {
        return format!("{stem}_");
    }
    stem
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a Windows host actually looks.
    ///
    /// The rule went in after saving a preset there failed with "no config
    /// directory": `preset_dir` used the Unix `$XDG_CONFIG_HOME`/`$HOME` rule,
    /// and neither is normally set on Windows. Worse, a host started from a
    /// Unix-style shell *did* find `$HOME` and wrote presets somewhere no
    /// Windows DAW session would look again.
    ///
    /// Checked with a path that is absolute on whatever platform is running
    /// the test, because `Path::is_absolute` answers by the host's rules and a
    /// `C:\` path is not absolute to a Linux build. What is under test here is
    /// the shape of the result, not that check.
    #[test]
    fn the_windows_preset_directory_is_under_appdata() {
        let appdata = std::env::temp_dir().join("Roaming");
        let dir = windows_preset_dir(Some(appdata.clone().into_os_string()), None)
            .expect("APPDATA is enough on its own");
        assert_eq!(dir, appdata.join("GainStageFx").join("Presets"));
    }

    /// And the fallback, for the rare host that clears `APPDATA`.
    #[test]
    fn a_cleared_appdata_falls_back_to_the_user_profile() {
        let profile = std::env::temp_dir().join("user");
        let from_profile =
            windows_preset_dir(None, Some(profile.clone().into_os_string())).expect("a fallback");
        let from_appdata = windows_preset_dir(
            Some(profile.join("AppData").join("Roaming").into_os_string()),
            Some(profile.into_os_string()),
        )
        .expect("the direct route");
        assert_eq!(
            from_profile, from_appdata,
            "deriving the roaming directory from USERPROFILE has to land in the \
             same place APPDATA points at"
        );
        assert_eq!(
            windows_preset_dir(None, None),
            None,
            "with neither variable set there is nowhere to save, and saying so \
             beats writing to the current directory"
        );
    }

    /// A relative value is a host bug, and following it would scatter presets
    /// through whatever directory the DAW happened to start in.
    #[test]
    fn a_relative_setting_is_refused_rather_than_followed() {
        assert_eq!(
            windows_preset_dir(Some(OsString::from("AppData")), None),
            None
        );
        assert_eq!(xdg_preset_dir(Some(OsString::from(".config")), None), None);
    }

    /// Unix and macOS keep the rule they have always had, because moving it
    /// would lose everyone's saved presets.
    #[test]
    fn the_unix_preset_directory_is_unchanged() {
        let home = std::env::temp_dir().join("home");
        assert_eq!(
            xdg_preset_dir(Some(home.join(".config").into_os_string()), None),
            Some(home.join(".config").join("gainstagefx").join("presets"))
        );
        assert_eq!(
            xdg_preset_dir(None, Some(home.clone().into_os_string())),
            Some(home.join(".config").join("gainstagefx").join("presets"))
        );
    }

    /// The file name a preset gets, for the names Windows will not accept.
    ///
    /// `CON`, `AUX`, `NUL`, `COM1`..`LPT9` are not file names there: Win32
    /// resolves them to hardware before the filesystem sees them, and the
    /// `.json` does not help.
    #[test]
    fn reserved_names_become_usable_file_names() {
        for reserved in ["CON", "con", "AUX", "nul", "COM1", "lpt9", "PRN"] {
            let stem = file_stem(reserved);
            assert!(
                stem.ends_with('_'),
                "{reserved} is a device on Windows, not a file: got {stem}"
            );
        }
        // Not reserved, only beginning with one.
        assert_eq!(file_stem("Console"), "Console");
        assert_eq!(file_stem("Aux Send"), "Aux Send");
    }

    /// And the characters no filesystem here will take. A trailing dot needs
    /// no rule of its own: it is not alphanumeric, so it has already become an
    /// underscore by the time Windows would have stripped it.
    #[test]
    fn awkward_characters_become_underscores() {
        assert_eq!(file_stem("Lead / Solo: \"hot\""), "Lead _ Solo_ _hot_");
        assert_eq!(file_stem("Clean."), "Clean_");
        assert_eq!(file_stem("  Clean  "), "Clean");
        assert_eq!(file_stem("   "), "preset");
        assert_eq!(file_stem("..."), "___");
    }
}
