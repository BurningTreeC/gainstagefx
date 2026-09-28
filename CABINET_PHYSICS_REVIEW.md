# Cabinet, speaker, and microphone physics review

Reviewed 2026-09-26 against v0.28.0 (`9055add`). The target is improved physical
realism, including intentional changes to the sound of speakers, cabinets and
microphones. The accompanying CI repair changes no production audio code.
This document specifies a redesign; it does not claim that redesign is implemented.

## What GainStageFx is

GainStageFx is a Rust CLAP/VST3 effect, with an optional standalone host. It
models pedals, guitar amplifiers and microphone preamps as component netlists,
then combines independently selectable power stages, speakers, cabinets and
microphones. There are 32 circuit choices, a separate nine-pedal slot, mono and
stereo host layouts, presets, automation, oversampling and a Vizia editor.

- `src/circuits/`: component values and circuit topologies.
- `src/dsp/`: modified nodal analysis, nonlinear device models, transient and AC
  solving, oversampling and time-dependent effects.
- `src/voice.rs`: circuit routing, calibration, speaker-loaded power stages,
  state transitions and fixed 66-sample latency.
- `src/acoustics/`: speaker equivalents, cabinet geometry, radiation and mics.
- `src/plugin.rs`, `params.rs`, `presets.rs`, `editor/`: host integration and UI.
- `tests/` and `examples/`: numerical regressions, measurements and diagnostics.

The physical acoustic path is distinct from the old `Legacy` cabinet filters.
The findings below concern the physical path.

## 1. Pressure, air, and dynamics

**Present:** `speaker::LoadValues` represents coil resistance, lossy inductance,
moving mass, suspension compliance, mechanical loss and motor coupling. A sealed
box adds an air spring. This impedance participates in the same solve as the
power amplifier, so load changes can affect clipping and feedback. This is an
important foundation to retain.

In `voice.rs`, the motional voltage is differentiated and scaled by
`Re Mms / Bl²`. `AcousticStage` receives this normalized radiation proxy. It
applies breakup filters, propagation delays, geometric levels, cabinet filters,
mic responses and small heuristic mic nonlinearities.

**Missing or approximate:**

- The radiation signal is not calibrated pressure in pascals. Microphone gain
  is normalized at 1 kHz and speaker voicing at 600 Hz. There is no complete
  volts -> cone velocity -> pascals -> microphone volts calibration.
- Air is fixed at `rho = 1.204 kg/m³`, `c = 343 m/s`. There is no atmospheric
  absorption dependent on frequency, temperature, humidity and pressure.
- The geometric law is bounded near the cone, then half its loss in dB is
  automatically compensated (`DISTANCE_MAKEUP = 0.5`). Using the V30 radius in
  the source, the geometric term alone falls 5.83 dB from 0.5 to 1 m; after
  makeup it falls 2.91 dB. This calculation excludes the separate baffle,
  directivity and proximity filters. Far away its net pressure amplitude tends
  toward `1/sqrt(r)`, rather than `1/r`.
- Speaker parameters are linear and fixed. There is no `Bl(x)`, `Kms(x)`,
  nonlinear inductance, voice-coil heating, thermal compression or per-driver
  excursion state driving those parameters. The amplifier is nonlinear; that
  does not substitute for loudspeaker large-signal behavior.
- Mic saturation depends on digital sample level and broad transducer family,
  not measured pressure/voltage headroom for the particular microphone.

“Dynamics” needs two distinctions. The existing delay lines and resonant filters
already have memory and alter transients. What they mostly lack is calibrated
level-dependent mechanical/thermal behavior. Linear air propagation is a valid
starting point when acoustic perturbations are small; it does not need an
invented compressor to become physical. Atmospheric losses are a lower priority
than radiation geometry at these microphone distances. See the
[linear-acoustics assumptions](https://doc.comsol.com/6.4/doc/com.comsol.help.aco/aco_ug_intro.04.08.html),
[bulk versus boundary losses](https://doc.comsol.com/6.4/doc/com.comsol.help.aco/aco_ug_properties_fluids.17.8.html),
and [Klippel's large-signal parameters](https://docs.klippel.de/db-lab/latest/transducer-parameter-identification/lsi/lsi.html).

**Proposed model:** keep physical units until the final output gain. Derive
pressure from distributed cone velocity, with consistent radiation loading on
the driver. Add measured displacement-dependent motor/suspension behavior and
a thermal state driven by coil dissipation. Provide physical distance attenuation;
any convenience level compensation belongs in an explicit separate control.
Do not invent nonlinear parameter curves merely to make the output more lively.

## 2. Cabinet walls and internal reflections

**Present:** `CabinetProfile` stores dimensions, uniform wall thickness, driver
positions, open fraction, slant volume correction and leakage Q. Thickness
changes internal volume, the three axial frequencies `c/(2L)`, and one back-panel
plate frequency. The plate formula uses bending rigidity
`D = E t³ / [12(1 - nu²)]`, so it has a physical dependence on thickness.

However, `AcousticStage::rebuild` reduces the enclosure to:

- three peaking filters at the first axial modes: depth -2 dB/Q4,
  width and height -1 dB/Q5;
- one back-panel peak: +1 dB/Q3.

All cabinets use the same estimated birch constants (`E = 12.4 GPa`, density
680 kg/m³, Poisson ratio 0.3). There is no material field, per-panel stiffness,
orthotropy, joint model, structural loss factor, brace layout or lining model.
Bracing only subtracts 3% of volume. Slant changes volume, not the 3D orientation
of the drivers or the actual cavity boundary.

These four filters have ringing, but their strength and decay are not derived
from material, boundary absorption, source position or structural coupling.
They filter the radiation proxy after the electrical solve; they do not load
the cones back through cavity pressure. The back panel is not a separately
radiating surface with its own path to each microphone.

Any nonzero open fraction disables the box air spring and all four enclosure
filters. Rear radiation then uses one inverted path per cone around its nearest
edge. Partly open boxes still have cavity and panel behavior in reality; this
binary switch cannot describe a gradual change from sealed to partly open.

**Proposed model:** a coupled cavity/structure system, reduced to a manageable
number of states for audio processing. The rectangular rigid-box frequencies
`f_n = c/2 * sqrt((nx/Lx)² + (ny/Ly)² + (nz/Lz)²)` are a reference case, not the
complete model. Mode excitation depends on driver position; damping depends on
lining, leakage, openings and wall losses. Panel motion depends on material,
thickness, joints and braces. Internal pressure must act back on each cone;
panels and openings must radiate to the microphones separately.

Representing reflections through modes or a derived impulse response is
legitimate. A 3D wave solver does not have to run inside the audio callback.
Use offline/reference acoustic and structural calculations, then a stable,
passive reduced model. Do not count the same sealed air compliance or cone
breakup twice when adding the new coupling. A useful physical reference is
[COMSOL's coupled driver, enclosure and air model](https://doc.comsol.com/6.4/doc/com.comsol.help.models.aco.vented_loudspeaker_enclosure/vented_loudspeaker_enclosure.html).

## 3. Microphone position and multiple speakers

**Present:** every microphone receives contributions from every driver, with
individual distance, fractional delay, signed polar weight, and direction
filtering. Open boxes also contribute inverted rear paths. Two microphones
retain their relative delays unless Phase Align is enabled. Interference is
therefore already present; neighboring speakers are not omitted.

**Limitations that matter for realism:**

1. All paths read one shared cone waveform. The model has one effective driver
   solve, not individual driver states and electrical wiring. Mutual radiation
   is approximated by extra mass dependent on driver count, not a
   frequency- and geometry-dependent coupling matrix.
2. The target cone is always `drivers[0]`. Placement moves radially outward
   along one horizontal line on that cone, 1 cm to 1 m from the grille.
   There is no arbitrary XY position, other target speaker, between-speaker
   placement, rear placement, or actual angled-baffle driver axis.
3. A path is classified as “far” when its lateral offset exceeds the cone
   radius. That identifies another speaker, not the physical far field.
   Near/far validity depends on frequency, aperture size and distance.
   With the code's V30 radius of 0.125 m, the reference Rayleigh distance
   `pi*a²/lambda` is about 0.43 m at 3 kHz and 0.72 m at 5 kHz.
4. The target cone uses one pole and a tuned radius of `0.4 a`. Other cones
   use a fourth-order low-pass approximating the main lobe of a piston. That
   filter does not reproduce the actual Bessel directivity's nulls, signed
   sidelobes, phase, or the cone's spatial breakup modes.
5. Proximity is applied to the combined gradient signal using one effective
   distance derived from the nearest source. The different propagation paths
   do not each produce their own physically consistent pressure gradient.
6. The microphone pattern is an ideal first-order pattern plus a simplified
   HF-loss filter. Published magnitude fits do not establish actual complex
   polar response, diaphragm dynamics or SPL-dependent headroom.
7. Subtracting a common propagation delay is a deliberate plugin latency
   convention. It preserves relative arrival times for static placements,
   but a physical reference should retain absolute travel time too. The
   time-reference policy also matters when positions move.

**Proposed model:** calculate the coherent pressure field first, then let each
microphone sample pressure and the appropriate directional/gradient response.
For a rigid piston in an infinite baffle, a useful reference is the Rayleigh
surface integral, integrating each surface element's delayed acceleration over
its distance. This handles the near field and becomes Bessel directivity in
the far field. Real guitar-cone breakup and finite baffles require measured or
computed corrections beyond that reference. See the
[baffled-piston reference and validation example](https://doc.comsol.com/6.3/doc/com.comsol.help.models.aco.baffled_piston_radiation/baffled_piston_radiation.html).

In frequency-domain notation the linear radiation portion is:

`p_m(f) = sum_i H_mi(f) U_i(f) + panel radiation + opening radiation`

Here `U_i` is each cone's volume velocity and `H_mi` is a complex transfer
including geometry, propagation and diffraction. Sum signed pressure/phase,
not power magnitudes. A derived filter bank or convolution can implement this
linear mapping while the driver, cavity and thermal states provide the coupled
time-domain behavior. Filter interpolation during placement changes must remain
causal, stable and free of discontinuities.

## Implementation order and evidence

1. Establish calibrated units and a reference fixture: one well-documented
   driver, cabinet and microphone. The existing profiles mark many mechanical
   values as estimated. More simulation complexity cannot identify missing
   physical data by itself.
2. Validate radiation and microphone geometry against analytic piston cases
   and measured complex responses over a position/distance/angle grid. Replace
   the tuned near/far split and implicit distance compensation.
3. Add individual driver/wiring states where necessary, shared cavity pressure,
   opening impedance, and panel modes. Keep source, load and radiation models
   consistent; include frequency-dependent mutual loading as accuracy requires.
4. Add measured large-signal and thermal speaker behavior, then calibrated
   microphone dynamics/nonlinearity. Verify that hotter drive changes the
   behavior for measured physical reasons.
5. Fit reduced models outside the callback, preallocate their state, and measure
   realtime cost. Compare held-out microphones/positions/levels, not only the
   curves used for fitting.

Required checks include impedance and excursion, absolute SPL, phase and
arrival time, multi-speaker interference, cavity/panel decay, THD/IMD against
level, thermal compression and recovery, sample-rate agreement, automation,
passivity/stability and allocation-free processing. The existing tests prove
many useful behavioral properties; they are not comprehensive physical
validation. In particular, “close mic resembles one cone within 0.5 dB” is a
regression heuristic for one geometry, not a universal law for every cabinet.

Sound changes from this work are intentional and should be judged against the
physical references. Record those changes explicitly and handle saved-session
compatibility deliberately. The nice-plug port remains a separate engineering
change whose wrapper behavior can be checked for equivalence; see
[NICE_PLUG_ASSESSMENT.md](NICE_PLUG_ASSESSMENT.md).
