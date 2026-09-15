//! What happens after the power amplifier: the loudspeaker as a load and a
//! radiator, the cabinet it is mounted in, and the microphone in front of it.
//!
//! Model data (profiles, geometry) is immutable and kept apart from the mutable
//! per-channel processors. See `ARCHITECTURE.md`, `SPEAKER_MODEL.md`,
//! `CABINET_MODEL.md` and `MICROPHONE_MODEL.md`.

pub mod cabinet;
pub mod filters;
pub mod mic;
pub mod speaker;
pub mod stage;
