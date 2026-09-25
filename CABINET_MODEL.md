# Cabinet model

Implementation: `src/acoustics/cabinet.rs` (immutable geometry),
`src/acoustics/stage.rs` (per-channel processor),
`src/acoustics/speaker.rs::Mounting` (box load on the driver).
Research and value provenance: [docs/models/cabinets.md](docs/models/cabinets.md).

## Legacy cabinets (unchanged)

`Cabinet Model = Legacy` (the default, and what every old session resolves to) keeps the
original resistor-loaded power stage and the baked Combo/Stack filters:

- Combo: 110 Hz/Q .9, 8.5 kHz/Q .72, 14 kHz pole.
- Stack: 85 Hz/Q 1.35, 7 kHz/Q .8, 11 kHz pole.

They are empirical responses with no microphone stage. They are never stacked with the
physical path.

## Physical path

```text
cone radiation -> speaker breakup voicing (3 peaks + LP, normalised at 600 Hz)
  -> closed box only: standing-wave dips at c/2L (depth -2 dB Q4, width/height -1 dB Q5)
                      back-panel plate fundamental (+1 dB Q3)
  -> shared fractional delay line
  -> per driver i, per microphone:
       the cone the mic is in front of (near field):
         point of the cone straight below the capsule -> distance L_i, delay L_i/c, incidence psi_i
         directivity: one-pole at 2.2 c / (2 pi (0.4 a) sin theta), theta to cone centre (TUNED)
       any other cone (far field):
         the cone's centre -> distance L_i, delay L_i/c, incidence psi_i
         directivity: 4th-order Butterworth at 2.2 c / (2 pi a sin theta) -- the rigid piston's
         2 J1(x)/x main lobe of the whole cone
       level  g_i = sqrt((a^2 + 0.05^2) / (a^2 + L_i^2))       (bounded near field, no 1/0)
       polar weight: omni part a g_i, gradient part b g_i cos(psi_i)
       capsule off-axis loss: one-pole, profile dB at 90 deg / 10 kHz scaled by sin^2 psi
  -> open back only, per driver: rear wave, inverted, gain min(1, 2 x open) g(L_rear),
       path = internal depth + distance to nearest edge + edge-to-mic,
       diffraction one-pole at c / (pi (depth + edge))
  -> relative delays: the common minimum over both microphones is removed (no added
     latency); Phase Align removes each microphone's own minimum instead
```

## Box and array behaviour

- **Sealed boxes** stiffen the driver's suspension inside the power-stage solve, raising
  the resonance by `sqrt(1 + Cms/Cab)`. A 1960 4x12 with G12M-25 moves 75 Hz to about
  120 Hz. Tested in `tests/speaker_load.rs`.
- **1x12 / 2x12 / 4x12:** physical driver positions give each microphone
  different distances, delays, directivity and polar incidence per driver. Close up, the
  target cone dominates. At a distance the array adds coherently at low frequencies and
  interferes and beams at high frequencies. Tested: a 4x12's bass-to-treble balance at
  1 m differs from a 1x12's by more than 3 dB (`tests/acoustics.rs`).
- **Open back:** dipole cancellation from the inverted rear path. Tested: at 1 m, 70 Hz
  relative to 1 kHz is more than 2 dB lower than for a closed 1x12.
- **Baffle step:** first-order low shelf down to -6 dB below `c / (pi W)`, blended in
  as `d / (d + W/2)` so close microphones hear half-space radiation.

## Evidence labels

| Quantity | Label |
|---|---|
| Outer dimensions, factory speaker complements | DOCUMENTED (Marshall, Mesa); SECONDARY (Twin reproduction) |
| Marshall 5/8 in birch ply | SECONDARY |
| Hole layout | DERIVED (283 mm cutout, equal gaps) |
| Slant volume (8 %), bracing (3 %), driver displacement (2.5 L), leakage Q (7), open fractions, material constants | ESTIMATED |
| Mode/panel frequencies, baffle-step corner, compliance, delays, far-cone piston directivity (whole-cone `2 J1(x)/x`, 4th-order main lobe) | PHYSICS DERIVED |
| Mode/panel levels, near-cone HF radiating radius 0.4 a (one pole), rear gain rule, diffraction corner | TUNED / APPROXIMATED |

**Near and far cones (fixed 2026-09-24).** Until then every cone's directivity was one
pole on the breakup radius, and every cone's level and delay were taken from its nearest
rim. For a microphone close to one cone of a multi-driver cabinet that left each
neighbour -- a few tens of centimetres away and nearly 90 degrees off its axis -- only
about 8 dB down, and in the treble barely attenuated at all, so its delayed arrival
comb-filtered the close cone: a close-miked 4x12 deviated from its own single cone by
**9 to 13.5 dB** above 2.5 kHz, the 2x12s by 4 to 5 dB. The two cases are now separated:

- the cone the microphone is in front of keeps the near-field treatment above,
  unchanged, so centre-to-edge placement behaves as it did;
- every other cone is a far-field source at its centre with the rigid piston's own
  directivity, `2 J1(x)/x` of the whole cone, as a 4th-order low-pass at the same
  -3 dB point -- the form this document already labelled PHYSICS DERIVED, which one
  pole was not.

No placement moves a cone across that line (the position is clamped to the target
cone, and cones cannot overlap), so automation cannot make it jump. Close-miked, every
multi-driver cabinet now follows its own cone within **0.16 dB** above 2.5 kHz
(`tests/acoustics.rs::a_close_microphone_hears_its_own_cone_above_2_5_khz`).

**Evidence.** The one measurement of a real multi-driver cabinet this repository has --
a JC-120 at 4 cm from one of its two cones, see `docs/models/speakers.md` -- is the
check. With the fitted Jazz 12 the two-cone model follows it to **1.58 dB rms**, against
2.67 dB before, and against 1.16 dB for the near cone alone
(`tests/jazz_cabinet.rs::the_whole_two_cone_cabinet_follows_the_measurement`). Among
far-field radii of 0.6, 0.8 and 1.0 times the cone's, the whole cone agreed best.

**What remains approximated.** Above breakup a real cone radiates from less of itself
than a rigid piston, so a far cone at a moderate angle -- the view a distant microphone
has of the other cones in a 4x12 -- may beam somewhat more above about 3 kHz here than
in reality. Not measured. Below 1 kHz a neighbour is still a little louder than a
baffled piston's near and far fields would make it (the shared distance law gives
10.5 dB under the close cone at 4 cm where the textbook ratio is about 14); in the
JC-120 check that is where the remaining two-cone error sits, at +-3 dB around 300 Hz.

**Levels.** The neighbours had been adding low-frequency energy they should not, so the
fix made 45 close-miked presets 0.5 to 3.1 dB quieter. Each one's output trim was
raised by exactly what it lost (`examples/presetlevel.rs`, before and after), so the
catalogue's level matching is as it was; the sound is what changed.

## Advanced controls (prepared, not exposed)

`CabinetProfile` holds width, height, depth, wall thickness, driver count and layout,
open fraction, slant and leakage Q. A future advanced panel can build a profile at
control rate; the processor and the load already take arbitrary profiles.

## Cost

All cabinet/microphone processing is linear host-rate arithmetic: about 9 biquads, a
delay line and up to 8 paths per microphone (Lagrange interpolation + three
one-poles), with no oversampling. Measured CPU is recorded in `IMPLEMENTATION_PROGRESS.md`.
