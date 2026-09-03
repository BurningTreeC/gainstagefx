//! GainStageFx: circuit modelled gain stages, from a preamplifier barely
//! working to a scooped high gain sound.
//!
//! Built after a first attempt taught, mostly by measurement, which mistakes
//! this kind of plugin invites. The two structural answers are here from the
//! start: circuits are described by *name* and checked before use, and the
//! solver can be asked for a network's frequency response directly rather than
//! by running audio through it and taking a spectrum.

pub mod dsp;
