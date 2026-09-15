# Speaker model

Implementation: `src/acoustics/speaker.rs`, `src/circuits/power.rs::build_with_speaker`.
Research, sources and fits: [docs/models/speakers.md](docs/models/speakers.md).

## Electrical and mechanical equivalent (PHYSICS DERIVED)

The driver's impedance analogue is stamped into the power-amplifier netlist in place of
the resistor, at the transformer tap the power spec was drawn for:

```text
spk --Re-- vc1 --(L1 || R1)-- vc2 --(L2 || R2)-- mot --+-- Res  = Bl^2 / Rms --+
                                                        +-- Lces = Bl^2 Cms   --+
                                                        +-- Cmes = Mms / Bl^2 --+-- gnd
                                                        +-- Lbox = Bl^2 Cab -- box -- Rbox --+
```

- Mechanical: `Zm = Rms + s Mms + 1/(s Cms) + Zbox`, with `Rms = 2 pi Fs Mms / Qms` and
  `Cms = 1 / ((2 pi Fs)^2 Mms)`.
- Coupling: `F = Bl i` and `V(mot) = Bl u`, so the electrical side sees `Bl^2 / Zm` in
  series with the voice coil.
- Voice coil: two lossy inductances (eddy-current loss), fitted to Jensen's measured
  |Z|. It levels off above the band.
- Sealed box: `Cab = Vb_i / (rho c^2 Sd^2)` per driver, in series with `Cms`
  mechanically, i.e. in parallel with `Lces` electrically. Leakage is a series
  resistance giving Q_L = 7 at the loaded resonance (ESTIMATED).
- Arrays: N adjacent drivers share radiation mass, `+ (8 rho a^3 / 3)(sqrt N - 1)` per
  driver (PHYSICS DERIVED approximation: one piston of N times the area).
- Referencing: every impedance is multiplied by `k = PowerSpec::speaker / 8`, and
  capacitances divided by it. This is an ideal multi-tap transformer on the matching tap;
  feedback stays on the drawn tap.

## What is coupled (not approximated by a post-EQ)

The speaker is part of the same Newton solve as the phase inverter, output tubes,
bias, supplies, output transformer, negative feedback and presence network. So its
impedance changes:

- the output voltage;
- tube current and clipping;
- the feedback signal;
- the presence network's effect;
- transformer flux (through the reflected load).

Measured: into a V30 in a 34 L box, the output voltage rises x1.6-2.8 at the boxed
resonance and x1.3-2.3 in the inductive top relative to the resistor. Midband is within
7 % (`tests/speaker_load.rs`).

A chain with no power stage (pedal or studio preamp, or Power Amp = Bypass) drives the
same speaker model from a near-ideal voltage source (`speaker::voltage_driven`, 0.01 ohm).

## Radiation

The cone's on-axis pressure is proportional to its acceleration. The chain computes
`p = (Re Mms / Bl^2) dV(mot)/dt` (backward difference at the solver's rate). This equals
the terminal voltage in the mass-controlled band, so the legacy level calibration stays
meaningful. The cabinet and microphone stages work on that signal. Cone breakup is a
three-peak + low-pass voicing EMPIRICALLY TUNED to the manufacturer on-axis SPL plots,
applied in `acoustics::stage`.

## Added high-frequency parasitic

`power::Parasitics`: a primary winding capacitance putting the loaded primary corner at
30 kHz (ESTIMATED, bounded by Hammond 1750U data). It is stamped only in speaker-loaded
netlists. Without it, feedback stages driven far into clipping at high frequency failed
to converge into the inductive load at 96-192 kHz. See the research log for the
measurements that selected it and the rejected alternatives.

## Not implemented yet

- Nonlinear Bl(x), Cms(x), Le(x).
- Thermal Re and compression. The adjustable-part mechanism can change Re at control
  rate, which is the planned route.
- Per-driver mismatch in arrays.
- 16 ohm-specific parameter sets.
