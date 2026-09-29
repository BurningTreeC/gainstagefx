//! GainStageFx: circuit modelled gain stages, from a preamplifier barely
//! working to a scooped high gain sound.
//!
//! Built after a first attempt taught, mostly by measurement, which mistakes
//! this kind of plugin invites. The two structural answers are here from the
//! start: circuits are described by *name* and checked before use, and the
//! solver can be asked for a network's frequency response directly rather than
//! by running audio through it and taking a spectrum.

pub mod acoustics;
pub mod circuits;
pub mod dsp;
pub mod editor;
pub mod meters;
pub mod params;
pub mod plugin;
pub mod presets;
pub mod rt_trace;
pub mod stage_worker;
mod stereo_worker;
pub mod voice;

// ---------------------------------------------------------------------------
// macOS Audio Unit v2 export
// ---------------------------------------------------------------------------
//
// AUv2 uses FourCC identifiers rather than the CLAP/VST3 identifiers.
//
// IMPORTANT: once these identifiers have shipped, do not change them. Hosts
// use them to identify the plugin in existing projects.
//
// Manufacturer: BrTC = BurningTreeC
// Subtype:      GSfx = GainStageFx
//
// GainStageFx is an audio effect, so its AU component type is "aufx".

#[cfg(target_os = "macos")]
use nice_plug_au2::{nice_export_au2, Au2Category, Au2Plugin};

#[cfg(target_os = "macos")]
impl Au2Plugin for plugin::GainStageFx {
    const AU2_CATEGORY: Au2Category = Au2Category::Effect;
    const AU2_MANUFACTURER: [u8; 4] = *b"BrTC";
    const AU2_SUBTYPE: [u8; 4] = *b"GSfx";
    const AU2_NAME: &'static str = "GainStageFx";
}

#[cfg(target_os = "macos")]
nice_export_au2!(plugin::GainStageFx);
