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
       nearest point on the cone disc -> distance L_i, arrival delay L_i/c, incidence psi_i
       level  g_i = sqrt((a^2 + 0.05^2) / (a^2 + L_i^2))       (bounded near field, no 1/0)
       piston directivity: one-pole at 2.2 c / (2 pi (0.4 a) sin theta), theta to cone centre
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
| Mode/panel frequencies, baffle-step corner, compliance, delays, piston directivity form | PHYSICS DERIVED |
| Mode/panel levels, HF radiating radius 0.4 a, rear gain rule, diffraction corner | TUNED / APPROXIMATED |

**Known limitation (found 2026-09-24).** Piston directivity is one pole. For a
microphone close to one cone of a multi-driver cabinet, a *neighbouring* cone -- a few
tens of centimetres away and nearly 90 degrees off its axis -- is then only about 8 dB
down at 2-5 kHz, where a real 12-inch piston is more than 20 dB down, and its delayed
arrival draws comb-filter notches into the close-miked response. Fitting the Jazz 12 to a
measured JC-120 found them (notches at 0.6, 2.0 and 3.4 kHz against a measured peak at
3.6 kHz); see `docs/models/speakers.md`. It affects every multi-driver cabinet at close
range and is not changed here, because changing it moves every existing preset.

## Advanced controls (prepared, not exposed)

`CabinetProfile` holds width, height, depth, wall thickness, driver count and layout,
open fraction, slant and leakage Q. A future advanced panel can build a profile at
control rate; the processor and the load already take arbitrary profiles.

## Cost

All cabinet/microphone processing is linear host-rate arithmetic: about 9 biquads, a
delay line and up to 8 paths per microphone (Lagrange interpolation + three
one-poles), with no oversampling. Measured CPU is recorded in `IMPLEMENTATION_PROGRESS.md`.
