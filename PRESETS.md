# Preset compatibility and planned library

Existing factory entries are Preset structs; dials() now maps 24 panel values to host
parameters. Seven GUI groups: Studio, Preamp, Crunch, High Gain, Overdrive,
Distortion, Amplifier. Legacy saved JSON has name and normalized values, without a
version. The refactor adds a serde-defaulted `model_ids` map; saved model IDs take
precedence over normalized values when loading. Host enum ids are stable, but legacy normalized enum
positions require a migration if a list is expanded. Do not reorder Circuit::ALL.

Missing `power_amp` now actively restores Matched for both saved presets and host
state, including after a user has selected an override. Future acoustic stages must
likewise default to legacy behavior; those stages have not been implemented.

The requested 21 era-inspired presets remain pending their component models and
level validation. Puppet Master '86 must use an ordinary no-pedal Mark preamp /
British EL34 / closed 4x12 / dynamic mic configuration. Do not label the current
Mark-with-resistive-load chain as that completed preset. The Great Wall '79 needs
DR103, Experienced '67 needs Round Fuzz, Texas Storm '83 needs an independent 808
into Twin, and Blackout '80 needs the British topology. No artist DSP switches.
