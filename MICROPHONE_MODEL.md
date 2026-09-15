# Microphone model

Implementation: `src/acoustics/mic.rs` (profiles, patterns, placement),
`src/acoustics/stage.rs` (processor). Research, digitised charts and fits:
[docs/models/microphones.md](docs/models/microphones.md), `tools/speaker_fit/fit_mics.py`.

## Separation

- `MicProfile` (immutable):
  - pattern and transducer family;
  - on-axis response (HP2 + 4 peaks + LP2, fitted to published charts);
  - off-axis HF loss;
  - proximity strength;
  - capsule depth.
- `MicPlacement`: position 0..1 (cone centre to edge), distance 0.01..1 m from the
  grille cloth, angle 0..90 degrees. Values are clamped and non-finite input is replaced.
- `AcousticStage` microphone channel: per-path delays, gains, filters, proximity
  integrator, response EQ, trims and saturation. Mutable, per channel, allocation-free.

## Geometry (3-D)

The capsule sits at `(x_t + outward * position * a, y_t, distance + capsule_depth)`,
in front of the target driver (top-left of a 4x12, left of a 2x12). The capsule axis is
tilted by `angle` towards the cone's centre. Every driver of the cabinet is
propagated to it (see `CABINET_MODEL.md`); the GUI's three controls map to this 3-D point.

## Polar patterns (PHYSICS DERIVED)

`P(psi) = a + b cos(psi)` with signed gradient term. Cardioid (.5, .5), supercardioid
(.366, .634), hypercardioid (.25, .75, null 109.5 deg), figure-8 (0, 1), omni (1, 0).
Omni and figure-8 rear-lobe inversion are tested (`tests/acoustics.rs`). Mic-specific
off-axis darkening uses the profile's loss at 90 deg / 10 kHz, scaled by `sin^2(psi)`.

## Proximity (PHYSICS DERIVED form, per-mic strength)

Only the gradient term gets the near-field rise `1 + omega_p / (j omega)`, implemented
as the gradient sum plus a leaky integrator. Details:

- `omega_p = k_p c / r`, with `r = sqrt(L_min^2 + (0.7 a)^2)`: never smaller than the
  radiating cone.
- The integrator's DC gain is capped at 8 (+18 dB).
- Omni microphones get none.
- `k_p` comes from the SM57 statement, the MD 409 distance curves and the M 160
  distance curves; otherwise it is ESTIMATED.

Tested: the ribbon gains more bass from far to close than the SM57, and the ideal omni
gains less.

## Position, distance, angle

- **Position:** low frequencies use the nearest point of the cone, so the level of the
  low mids barely changes. The high-frequency radiating region is the centre (radius 0.4 a).
  An edge placement is therefore off that region's axis, and piston directivity darkens it.
  Tested: 3-12 dB darker at 5 kHz, low mids within 3 dB.
  - **Source conflict recorded:** Shure's SM57 placement table describes "edge of cone"
    as higher frequency, while the specification and common studio descriptions say
    centre is brighter. The model follows the specification.
- **Distance:**
  - level (bounded geometric law, half made up);
  - proximity;
  - propagation delay;
  - array and rear-wave contributions;
  - baffle step.
- **Angle:** polar weight plus microphone-specific off-axis HF loss. Tested: SM57 darker
  at 45 deg; R-121 more consistent; figure-8 side null greater than 15 dB.

## Dual microphones

Mic A (Bypass = ideal omni, never off) and Mic B (Off / Bypass / models). Blend is
ramped. Polarity inverts B. Phase Align removes each microphone's own first-arrival
delay; otherwise the physical relative delay is kept, and comb filtering stays part of
the sound. Tested: identical inverted microphones cancel below -180 dB; a 30 cm offset
gives a 0.30/343 s delay unless aligned.

## Transient character and nonlinearity

- Transient differences are carried by the fitted response (diaphragm resonance and
  HF roll-off); nothing extra is added.
- Subtle saturation is TUNED, not measured:
  - odd `v / sqrt(1 + 2 k v^2)` with k = .004 dynamic, .006 ribbon, .002 FET, .005 tube;
  - plus a DC-free second-order term for the tube family.
- At nominal level this is well below 0.1 % and runs at host rate without oversampling.

## Phase

All sections are minimum phase (matched second-order and first-order designs). The only
deliberate phase is propagation. No arbitrary phase rotation is applied.
