# Implementation progress

## 2026-09-14 audit checkpoint

Phase 1 architecture inspection complete before source changes. See ARCHITECTURE,
MODELS, DSP, SPEAKER_MODEL, CABINET_MODEL, MICROPHONE_MODEL and PRESETS at repository
root (docs/* is globally ignored in the existing .gitignore).

Discovered: existing separate Mark/5150/Twin preamp and full power netlists; Neve
line driver uses the same storage category but must not become bypassed guitar
power. No required British amps, physical loads/geometry/microphones exist yet.
Identified existing post-power 5150 tone and Twin effect approximations, 1x pinning,
fixed 66-sample latency, normalized-preset compatibility hazard, unbounded worker
spin, historical failed solves/deadline misses. All need honest tracking.

Baseline: cargo test --release --tests launched before modifications, output at
/tmp/gainstagefx-baseline-tests.log. Result pending. No DSP changes yet.

## Next implementation checkpoint

Capture deterministic pre-refactor renders, then expose existing power stages via
Matched / Bypass / explicit existing families. Preserve Neve output driver.
Research Mark/5150/Twin sources before model-routing edits; do not change circuit
values as a side effect of modularization. Validate exact legacy output first.
Subsequent phases remain: researched British power and amp models; independent
pedal path; coupled speakers; geometry cabinets; microphones; UI/state; factory
presets; multi-rate/block/format/CPU validation. Full master specification is NOT
complete. Record each incremental result here.

## Pre-refactor render captured

Source commit 5fda86934d5695cb1995adb88a2377c6cb29d8e5, unchanged production code.
Added tests/legacy_baseline.rs and captured 819,200 f32 samples covering Mark,
5150, Twin, 73P at 44.1/48/88.2/96/192 kHz, amplitudes .001/.03/.126/.7, impulse,
log sweep, stepped sine, seeded white and lowpass-colored noise. The colored noise
is a reproducible low-frequency stress signal, not precision 1/f pink noise.
Fixture SHA256: 0ce9d86405fb3df58b30c7b892ab2656341a3c8f99ec043faf1fdc7e65bf3f76.
Capture passed in 6.11 s. The baseline generator is ignored in normal tests.

Research checkpoint for routing existing Mark/5150/Twin models:
1. Existing RP10A/60W C+ interpretation, Peavey 5150-II drawing lineage, Twin AB763.
2. Original manufacturer/archival scans located; Mark RP10 and FINAL redraws are
   NOT original manufacturer drawings (their reference numbers say unofficial).
3. Mark archival schematicheaven scan; Peavey factory 5150II PDF; Fender AB763 scan.
4. Mark local RP10 vs FINAL plus archived preamp; Peavey alternate EL34World scan;
   Twin schematic and layout. Revision/value discrepancies remain, recorded below.
5. Existing netlist boundaries, full PI/tubes/NFB/OT blocks, and 73P line driver.
6. All existing numerical topology/component values and Matched processing order
   remain exact relative to baseline; this is a routing refactor.
7. Retain legacy fixed interstage impedances, graphic recovery scalar, resistor
   load, substitute 5150 stack, Twin effects and estimated transformer/supply values.
8. Correcting these now would defeat the independent compatibility checkpoint.
No new hardware model is claimed by exposing existing output simulations.

## Routing checkpoint validated

Baseline suite: 255 passed, 0 failed, 5 ignored in 34 test binaries.
After routing: legacy baseline passed, runtime state 7 passed, controls 3 passed,
saved store 1 passed. New modular power tests 6 passed; state migration tests 3
passed. Full source/tests/examples check passes (pre-existing debug timing warnings).
New Power Amp parameter and GUI selector rows; display-only names changed.
Saved presets now carry stable model_ids and actively default missing power_amp
to Matched; host filter_state does the same. Existing enum IDs/order unchanged.
Neve output driver now separate from guitar power storage, with complete rate,
reset, runtime/DC copy and diagnostic tracking. All old DSP component values remain.

Brit EL34 eight-question checkpoint completed in docs/models/brit_el34.md BEFORE
implementation. Original 1981 drawings and 1988 alternate were visually inspected.
Use complete circuit topology with sourced replacement-transformer data; estimates
and revision differences are explicitly recorded. No Brit preamp yet.

## 2026-09-15 continuation / resume checkpoint

Created IMPLEMENTATION_RESUME.md with the actual implemented scope, file inventory,
research links, regression fixture details, limitations and ordered continuation
tasks. Full master specification remains incomplete.

Brit EL34 validation completed: frozen legacy baseline 1 passed / 1 intentionally
ignored; modular power 6 passed; modular state migration 3 passed. The selector now
has six choices across three rows. Source formatting applied to the changed Rust
files only; their scoped rustfmt check passes. Repository-wide cargo fmt --check
also found existing formatting differences in untouched solver/test/worker files;
these were not reformatted as part of this work. git diff --check passes.

cargo clippy --tests --examples exits successfully, with existing unreachable-code
warnings in debug-only timing checks, one new chunks_exact style suggestion in the
baseline test, and upstream xcb future-compatibility notice. This is not a
warning-free lint result. Full post-refactor release suite is running; its final
result will be appended below. No CLAP/VST3 host or new-model CPU claims yet.

Review found two items for the next DSP checkpoint: custom power Master ownership
can retain a prior pedal/studio circuit-level setting; the shared presence builder
uses an audio taper whose agreement with the 2203 control law remains unverified.
These are recorded explicitly rather than silently treated as validated behavior.

The full release suite then failed with SIGABRT: stack overflow in
`plugin::channel_layout_tests::first_different_stereo_block_wakes_right_chain_and_latches_stereo`.
The isolated test reproduced on the default stack and passed with
`RUST_MIN_STACK=8388608`. Simulation includes large inline trace arrays under
cfg(test); adding an inline line-driver Simulation increased Chain construction
stack pressure. Moved only the new line driver into Box storage allocated during
construction. The production callback still performs no allocation for this state.
Full suite rerun uses the normal stack; outcome pending below.

## 2026-09-15 (session 3): suite result, inventory, speaker load

Pending full-suite result from the previous checkpoint, rerun on the unchanged tree:
`cargo test --release --tests` **265 passed, 0 failed, 6 ignored** (37 binaries). The
line-driver Box change resolved the stack overflow.

The user issued a correction directive: the prompt model lists are not exhaustive, and the
repository is the inventory. Broad implementation paused. Created
[docs/MODEL_INVENTORY.md](docs/MODEL_INVENTORY.md), un-ignored in `.gitignore`, from a
content search of src/tests/examples/docs/root docs/assets/packaging/git history/local
schematics. It covers existing, partial, referenced-only (Boss HM-2/MT-2 drawings) and
planned models, the three-name policy, serialization constraints, solver gaps (no PNP, no
op-amp GBW/slew, no cathode-bias power) and a prioritized queue.

Queue item 1, speaker electrical load:
- Research checkpoint written first: [docs/models/speakers.md](docs/models/speakers.md).
  Jensen: full manufacturer T/S + measured curves. Celestion: partial data only; the
  remaining values are ESTIMATED with a documented method and analogue-driver bounds.
  Fitting code in `tools/speaker_fit/` (downloads Jensen data, not committed).
- Solver extension: `Part::Adjustable` (R/L/C whose value is set by
  `Simulation::set_value`). Rebuild carries reactive state across value changes, as it
  already did for pots. AC solver, structure masks and counts updated.
  `Circuit::unknown_named` exposes a second node for reading.
- `src/acoustics/speaker.rs`: 7 immutable profiles, `Mounting` (sealed volume / leakage Q /
  array air mass), `LoadValues` referred to the amplifier tap, analytic impedance,
  netlist stamp, voltage-driven netlist for chains without power.
- `power::build_with_speaker`: stamps the driver at the resistor's position in part order,
  plus an ESTIMATED primary winding capacitance (30 kHz corner rule). The resistive
  netlists are unchanged; the legacy fixture passes.
- Stability investigation (documented in the research log): LR-2 coil + NFB oscillated at
  96-192 kHz. Tube capacitances made it worse. Lossy two-inductance coil (better fit) +
  winding capacitance made every case bounded.
- `tests/speaker_load.rs` (6 tests) passes: netlist == analytic |Z| to 1e-6; model within
  15 % of Jensen measured |Z| and resonance peaks; sealed box resonance shift; time domain
  matches AC with no heap use on driver swaps; every power stage's output follows |Z|
  (x1.4+ at resonance, midband within 15 %); abuse test bounded at 48/192 kHz.
  Legacy baseline, modular power/state, netlist and AC suites pass.

Not yet done: Chain integration, cabinet acoustics, microphones, parameters, GUI, presets.
Temporary `examples/zz_probe.rs` exists for measurements and must be removed before
completion.

## 2026-09-15 (session 3, continued): acoustic chain, pedal slot, fixes

**Cabinets and microphones (queue items 2-3).**
- Research checkpoints written first: [docs/models/cabinets.md](docs/models/cabinets.md),
  [docs/models/microphones.md](docs/models/microphones.md) (manufacturer charts
  digitised and fitted, `tools/speaker_fit/fit_mics.py`, 0.05-0.40 dB rms).
- `acoustics::{filters, cabinet, mic, stage}`:
  - matched second-order designs (bilinear cramping measured 3.8 dB at 12 kHz and was
    replaced);
  - shared fractional delay line with 3rd-order Lagrange that is exact at zero delay;
  - per-driver paths using the nearest cone point for LF and the centre for HF directivity;
  - first-order polar patterns with signed gradient, proximity integrator, dipole rear
    wave for open backs, baffle step;
  - dual mic with blend / polarity / alignment; subtle family saturation.
- `tests/acoustics.rs`: 13 tests pass. Six failures on the first run were root-caused:
  - the filter cramping above;
  - a point-source cone moving the low mids 5.5 dB;
  - an off-axis corner law ineffective at 45 degrees;
  - an RC phase lag cancelling the dipole;
  - two test bugs.

**Chain integration (queue item 4).**
- `AcousticSettings` in `Settings`; `CabinetChoice::Legacy` (the default) keeps the
  resistor load and baked filter bit-for-bit.
- Physical path: speaker-loaded twin of each power stage (adjustable load values), or a
  voltage-driven speaker when there is no power stage; cone radiation
  `Re Mms/Bl^2 dV(mot)/dt`; the acoustic stage runs at host rate.
- Speaker Bypass with a cabinet model is a power-stage DI.
- Lifecycle covered: runtime copy, DC sharing, reset, rates, pass ceiling, health.
- New params with stable ids: `cab_model` (legacy first), `speaker`, `mic_a`/`mic_b`,
  placement floats in m/deg, `mic_blend`, `mic_b_invert`, `mic_align`.
- `filter_state` inserts `cab_model=legacy`, `pedal=none`. Preset migration defaults both.
- `tests/acoustic_chain.rs`: 8 tests pass (legacy bit-identical, DI identity, driver
  difference, all power x cabinets bounded, stereo wake, live switching with no heap,
  rates and blocks).

**Measured cost (48 kHz, one channel).**
- Acoustic stage alone: 0.46 % (one mic), 0.77 % (two).
- Full chain, legacy to physical: Mark 20 to 22.6 %, Twin 15 to 17 %, 5150 33 to 30 %
  (fewer passes into the speaker load).
- Level vs legacy Stack: -2.6 to +4.9 dB. `examples/acousticcost.rs`.

**Power-override level and master (Phase 2 leftovers).**
- Measured override level changes of up to +-20 dB. Added `POWER_TRIM_DB` (voice x
  override, measured by `examples/powertrim.rs`, re-verified by `tests/power_trim.rs`
  within 1 dB at the calibration drive).
- Residual: mean 0.8 / 1.2 dB, worst ~9 dB at drive 0.15 / 0.9. Known limitation.
- Master ownership: under an override the preamp's own level control returns to its
  calibrated rest.

**User-reported bug: American 5150 + American 6L6 Clean dropouts.**
- Reproduced: 7.4 power passes/sample, ~2,400 fallbacks per 4 s, deadline misses at
  128-sample blocks.
- Cause: the Twin stage has no master (rest 1.0), so in a custom chain the 5150 preamp
  slammed the ECC81 inverter.
- Fix: `OVERRIDE_MASTER_REST = 0.30` for no-master stages in custom chains only (the
  matched Twin is untouched). Measured 3.2 passes, 0 fallbacks, no misses at normal
  settings. The trim table was re-measured.

**GUI.**
- Panel width 640 to 780 px (user request).
- Input section gains the pedal row (stepper + drive/tone/level).
- Cabinet section rebuilt: cabinet, legacy filter, speaker with physical summary, mic A/B
  steppers, 7 placement knobs, polarity/time toggles.
- New `Stepper` widget for long lists.
- Screenshots checked with `shot.sh` (legacy default state).

**Pedal slot (queue item 5) and TS808 / TS9 (item 6).**
- Research logs: [green_808.md](docs/models/green_808.md) (three independent sources agree
  against the legacy netlist's tone shunt, level feed and C1),
  [green_9.md](docs/models/green_9.md), [big_muff.md](docs/models/big_muff.md).
- `ts808::{LEGACY, TS808, TS9}`: the legacy catalogue voice is unchanged; the pedal slot's
  Green 808 uses the verified values, and Green 9 uses the 470 ohm / 100 k output.
- `Pedal` slot in `Chain` (guitar-level input, output volts into the circuit, own
  drive/tone/level with calibrated-middle Level).
- `tests/pedal_slot.rs`: 7 tests pass.

**Owner decisions outstanding.**
1. Versioned correction of the catalogue `ts808` voice to the verified values.
2. Generic display name for Big Muff.

**Next.** Factory presets needing only available components (Puppet Master '86, Texas
Storm '83 with the pedal slot) after rig research. Then Brit 800 preamp research and
implementation, Rodent (op-amp GBW/slew), Round Fuzz (PNP mirror), AC30/EL84, DR103,
Rectifier, 312, 610.

## Session 4 (2026-09-15): owner requests

**Catalogue Green 808 corrected (owner decision: "correct it everywhere").**
- `ts808::build`/`tap` now use `ts808::TS808`; `LEGACY` is kept for measurement only.
- `src/calibration.rs` regenerated with `examples/calibrate`. Only the TS808 make-up row
  changed (by at most 0.08 dB), plus a rounding in one Studio comment.
- Existing sessions and presets using `circuit = ts808` now sound different (accepted).
- Research log resolution updated in [green_808.md](docs/models/green_808.md).

**User-reported bug: American Twin on Matched power, reverb only.**
- Cause: the Twin power stage's master was left at the override position after that
  stage had run behind another preamp. That was 27 dB down, with the reverb path
  unaffected.
- Fix: `Chain::set_master` returns the resolved power stage's master to its rest
  whenever the level control is not the power stage's own.
- Regression test: `tests/modular_power.rs::a_power_stage_released_by_an_override_returns_to_its_own_master_position`.
- Probed after every power override, resistor-loaded and speaker-loaded: matched level
  identical to a fresh chain.

**GUI: dropdowns and no plugin On/Off switch (user request).**
- New `src/editor/dropdown.rs`:
  - `DropButton` shows the current entry plus a caret; the wheel steps through entries.
  - One overlay list at a time, drawn after the panel so it covers later sections. It
    opens below the button, or above it if there is no room, and scrolls only when
    longer than 360 px.
  - Choosing an entry emits a Begin/Set/End parameter gesture.
- Dropdowns: pedal, topology, modelled circuit, clipping, amplifier, iron, power amp,
  cabinet, speaker, mic A (no Off), mic B.
  - Topology and modelled are one `circuit` parameter; the button that does not hold
    the current circuit shows "--".
- The circuit section is now a two-column grid (296 to 128 px); the window is
  780 x 912.
- The output section's ON/OFF selector is removed. The host bypass parameter stays.
- The `Stepper` widget was removed (unused).
- Verified with `shot.sh`, including open menus (temporary hook, removed).

**Docs.** README rewritten (chain, pedal slot, modelled circuits, power stages,
speaker/cabinet/microphones, presets, costs, tests). MODELS.md, ARCHITECTURE.md and
docs/MODEL_INVENTORY.md statuses brought up to date. `examples/presetcost.rs` now
applies each preset's full `Settings` (pedal, power, cabinet, mics).

**Owner decision outstanding.** Generic display name for Big Muff.

## Session 5 (2026-09-15): Brit 800, Mark IIC+ correction, catalogue update

**Owner requests.**
- Generic panel descriptions ("modeled after ...").
- Implement and wire up the Brit 800.
- Update every existing preset.
- Check the Puppet Master '86 facts ("way too less distortion").
- Next: the missing microphone preamps.

**Panel text.** The modelled-circuit descriptions now say "Modeled after a/an
<generic description>". The panel no longer shows any hardware brand or model name
(the Drive/graphic notes were fixed too). Engineering references stay in the developer
documents.

**Mark IIC+ correction (`circuits::markiic`).**
- Both local drawings return `LEAD OUTPUT` into V2B, then the Lead Master (250 k shunt
  rheostat), the 150 k/4.7 k pad and the V2A recovery stage before `TO EQ`. The model
  stopped at `LEAD OUTPUT`.
- Calibrated distortion at a guitar's level: 39.1 % → 61.1 %.
- The panel Master is now the Lead Master (`Level::Circuit(markiic::LEAD_MASTER)`, rest
  0.5); the power master pot stands in for MASTER at rest 0.30.
- The Mark blocks of `tests/legacy_baseline.rs` are no longer compared (documented in
  the test); the other voices still are.
- Research log updated: `docs/models/cali_iic_plus.md`.

**Puppet Master '86 fact check.**
- Producer Flemming Rasmussen (29 June 2018): "It is only on some solos we use the
  JCM 800 as poweramp. Most guitars are with the Boogie powerstage!"
- The preset moved from Brit EL34 to Matched (Cali 6L6).
- Mics per the producer's interview: two SM57s in the cone, AKG tube mics at 45 degrees,
  3-4 ft.
- Knob settings in secondary sources conflict; recorded in PRESETS.md.

**Brit 800 (`circuits::brit800`, `Gain::Brit800`, `Circuit::Brit800` id `amp_jcm800_2203`).**
- Built from the 1981 Marshall drawing. The 1988 Marshall drawing resolves middle 22 k,
  treble 220 k and bass log.
- Rails from the drawing's own voltage table (330 V node 10, with the 10 k droppers and
  50 uF reservoirs). The operating point is within 4 % of the table at every plate and
  cathode.
- Matched = Brit EL34, now at the Brit 800's own catalogue slot (no appended power slot).
- The circuit enum append comes with a migration for saved presets that carry no ids
  (`params::LEGACY_CIRCUIT_COUNT`).
- Calibration: 24.7 % at full Preamp Volume.
- Tests: `tests/brit800.rs` (5), migration test in `tests/modular_state.rs`.

**Tables.** `src/calibration.rs` and `src/power_trim.rs` were regenerated (14 voices).
`examples/powertrim.rs` sizes the table from `Gain::ALL`.

**Catalogue.**
- Every guitar preset now uses a physical cabinet, speaker and microphones; the legacy
  baked filter is unused.
- Topologies drive the speaker directly (no power stage inside the oversampler).
- Green Overdrive and Screamer Boost use the pedal slot (into the American Twin and the
  Brit 800).
- Mark and Twin presets were re-voiced.
- New: Brit Crunch, Brit Lead.
- Level span 10.1 dB at 220 Hz. Worst cost 32 % of realtime per channel
  (Texas Storm '83).
- New test: `tests/presets.rs::guitar_presets_use_a_physical_cabinet`.

**Open.**
- The Mark graphic model spans only about +2.7 dB of boost against Mesa's ±12 dB.
- Generic display name for Big Muff.

## Session 6 (2026-09-15): microphone preamps, graphic EQ, Ram Fuzz

**Owner requests.**
- Implement the missing mic preamps, including the SSL 4000 E.
- Correct the Mark graphic EQ.
- Give the Big Muff a distinctive name.
- Look deeper for the Tube 610 schematic.

**Research.**
- American 312: API's card drawing, API transformer sheet and API 2520 data sheet
  (waltzingbear archive).
- British 4K E: SSL drawing T82001-71 rev 16 (ka-electronics), cross-checked with
  82E01-710-8208-12 (gyraf.dk); Jensen JE-115K-E/JT-115K-E data sheets.
- Tube 610: Hinson's redraw of UA C-10068, found through Gearspace and retrieved from the
  Internet Archive (the GroupDIY copy is members-only). UTC 1963 catalogue, UTC 1952
  terminal arrangements (the 50 ohm tap on pins 3-4), RCA 12AY7 1953 data.
- Research logs: `docs/models/american_312.md`, `british_4k_e.md`, `tube_610.md`.

**Implementation.**
- `circuits::american312`, `circuits::console_e`, `circuits::tube610`; `Gain`/`Circuit`
  appended with ids `pre_api_312`, `pre_ssl_4000e`, `pre_ua_610a`.
- `TriodeSpec::T12AY7` fitted exactly to the RCA points (`tools/tube_fit/fit_12ay7.py`).
- Transformer internals are fitted to the data-sheet figures (test-held):
  - Jensen: 2.5 Hz/90 kHz −3 dB, 1 % at 20 Hz −2.5 dBu;
  - UTC O-1: ±1 dB, 1 % at +8 dBm;
  - API 2622: ±0.5 dB, 1 % at 0 dBm.
- Gain ranges:

  | Preamp | Gain |
  |---|---|
  | American 312 | 30-64 dB, even in dB |
  | British 4K E | 19-69 dB |
  | Tube 610 | to 70 dB, 2nd-harmonic led |

- Presets: six in the Preamp group. The two Neve presets were renamed "British 73 Pre" /
  "British 73, Driven".
- Tests: `tests/american312.rs`, `tests/console_e.rs`, `tests/tube610.rs`
  (`tests/support/micpre.rs`).

**Mark IIC+ graphic EQ.** Rebuilt as the feedback equaliser both drawings show (tracks
between the amplifier's two inputs), with the 16.6 dB make-up removed and 50 k sliders on a
dual-slope law. Now ±10-15 dB and flat when centred. New `Taper::Symmetric`.

**Ram Fuzz.** The Big Muff's display name on the circuit and pedal lists; ids unchanged.

**Tables.** Calibration and power trim regenerated for 17 voices.

## Session 7 (2026-09-15): pedals, a solver fix, the Mark IIC+ Bass

**Pedals.**
- Rodent (Pro Co RAT, LM308) and Round Fuzz (germanium Fuzz Face) in the pedal slot, ids
  `pedal_rat` and `pedal_fuzz_face`.
  - Logs: [rodent.md](docs/models/rodent.md), [round_fuzz.md](docs/models/round_fuzz.md).
- Solver additions: `Part::Bipolar { pnp }` and `Part::Transconductor` (a saturating VCCS
  for op-amp gain-bandwidth and slew).
- The panel greys the Tone knob for pedals without one.

**User-reported bug: Rodent in front of a distorted Cali IIC+ crackles and drops out.**
- Cause: after Distortion came down while playing, the op-amp loop (pole near 1 MHz) drove
  the transconductor into saturation. Its tangent linearisation is flat there, so every
  solve failed and the reactances froze on the last good answer.
- Fix:
  - chord linearisation once saturated (`Transconductor::stamp`);
  - `lm308` referenced to ground instead of the bias divider;
  - diode swing clamps with a linear follower instead of the two-state rail switch.
- Result: 2,943 unsettled solves and 29,095 fallbacks became 2 and 0 on the same sweep.
  Worst block through the chain went from 394 % to 76 % of its deadline.
- Regression test: `tests/rodent.rs::distortion_can_be_turned_down_while_playing`.

**User report: Puppet Master '86 has far too much low end; Mark IIC+ schematics.**
- Three independent drawings compared: the Mesa-labelled 1985 drawing, jrb32's FINAL, and
  the SLOCLONE v2 trace. They agree on topology and on nearly every value.
  Differences are recorded in [cali_iic_plus.md](docs/models/cali_iic_plus.md).
- Correction: the Bass pot is **audio taper** (SLOCLONE "250k A"; Mesa part A250K for
  "MK II/III Bass"). It was linear.
- Preset re-voiced per Mesa's IIC+ manual ("As gain goes up, Bass should come down"):
  - Bass 0.20;
  - graphic [0.65, 0.45, 0.25, 0.65, 0.70] (was 80 Hz at 0.85, about +9 dB);
  - 57 at 2.5 cm.
  Low octaves are 5.5 dB lower relative to 1-4 kHz.

**Brit Plexi (1959 Super Lead) and its power stage.**
- Research log [brit_plexi.md](docs/models/brit_plexi.md). Sources: Unicord 70-6-11 (July
  1970) for values, Marshall's c. 1967 drawing for voltages, Marshall's 1988 1959 STD
  drawings for pot laws and the later changes.
- `circuits::plexi` (bright channel, id `amp_marshall_1959`) and `PowerSpec::PLEXI_EL34`
  (id `power_1959_el34`, a new `PowerModel` slot and power-amp choice).
- The supply chain is solved self-consistently: the inverter node the preamplifier's
  droppers settle at (319.4 V) is the one the inverter is fed from.
- Idle 16 W per EL34 at -37 V (ESTIMATED).
- Calibration and power-trim tables regenerated (18 voices, 6 override columns).
- Presets: Plexi Crunch, Plexi Cranked (Amplifier); album presets Blackout '80 (Classic
  Rock) and Experienced '67 (Psychedelic / Lead), with evidence tables in PRESETS.md.
- Brown '78/'84 stay waiting: the documented rig ran the amplifier from a variac at
  80-89 V, and there is no mains-voltage supply model.
- Tests: `tests/plexi.rs`.

**Tests.** 355 pass (`cargo test --release --no-fail-fast`).

## Session 8 (2026-09-16): two more amplifiers, and what they needed from the solver

**Brit AC30 (Vox AC30/6 Top Boost).**
- Research log [brit_ac30.md](docs/models/brit_ac30.md). Sources: the Dallas 1974 factory
  sheet Sc/V/1313 and the Vox Sound Limited 1971 sheet, both from voxac30.org.uk's archive
  of original documents, with Ampbooks as the cross-check.
- `circuits::ac30` (id `amp_vox_ac30_tb`) and `PowerSpec::AC30_EL84` (id
  `power_ac30_el84`).
- Two tone knobs, not three: the stack has a 10 k resistor where a Fender stack has its
  Middle pot, so the panel greys that knob out.
- The output valves idle at 10.7 V of cathode bias against the factory sheet's 10 V,
  which is 16 W a valve -- an AC30 runs EL84s past their rating, and that is the
  amplifier.
- Tests: `tests/ac30.rs`.

**Brit DR103 (Hiwatt Custom 100).**
- Research log [brit_dr103.md](docs/models/brit_dr103.md). Sources: Hiwatt's own 1994-95
  factory sheets, with Circuit Codex (a redraw of a late-60s amp) and Ampbooks' analysis
  of this inverter as cross-checks.
- `circuits::dr103` (id `amp_hiwatt_dr103`) and `PowerSpec::DR103_EL34` (id
  `power_dr103_el34`). The master volume is in the preamplifier, where the drawing has it.
- The inverter is **direct-coupled** to the driver's cathode follower, which is where this
  amplifier's headroom comes from: no capacitor to charge up and shift the bias.
- It lands on the published operating point: bias -2.15 V (published -2.1 V), 286 V
  plate-to-cathode (285 V), 1.40 mA a triode (1.56 mA), and V2a's plate at 140.2 V against
  the sheet's own 140 V.
- Tests: `tests/dr103.rs`.

**Solver and builder work.**
- `power.rs` grew four ways of being a different amplifier, each keyed off a zero:
  - `cathode_bias` / `cathode_bypass`: a shared cathode resistor instead of a fixed bias
    supply, with the grid leaks returning to ground;
  - `feedback: 0.0`: no loop and no presence network;
  - `pi_tail_lower: 0.0`: the tail returns to ground; `pi_tail: 0.0`: the leaks, the
    cathode resistor and the feedback meet at one node;
  - `pi_couple: 0.0` with `driver_volts`: a direct-coupled inverter.
  - `cut_pot` / `cut_cap`: a cut control across the inverter's outputs.
- `Netlist::input_at`: an input carrying a direct voltage as well as a signal, so one
  block can be direct-coupled to the next across the model's preamp/power boundary.

**Presets.**
- Amplifier group: Chime Clean, Chime Edge, Hi-Headroom Clean, Hi-Headroom Pushed.
- Album presets: The Great Wall '79 (Psychedelic / Lead) and Pumpkin Dream '93
  (Alternative), each with its evidence table in PRESETS.md.

**Cali Rectifier (Mesa/Boogie Dual Rectifier, Rev F).**
- Research log [cali_rectifier.md](docs/models/cali_rectifier.md). Sources: Mesa's own
  "DUAL RECTIFIER PREAMP RF-1F" (6-93) and "DUAL RECTIFIER POWER AMP" sheets, with the
  Rectifier Guide for which revision is which.
- `circuits::rectifier` (id `amp_dual_rectifier`) and `PowerSpec::RECTO_6L6` (id
  `power_recto_6l6`). The red channel's master is in the preamplifier.
- Every node voltage the two sheets mark is matched within 6 %, including the inverter's
  own plates and cathodes.
- The stage that makes this amplifier: V2B on a 39 k unbypassed cathode resistor, biased
  at 4.9 V and clipping one side of the wave at a guitar's level.
- **The switchable valve rectifier is not modelled**; the supply is the silicon setting,
  and the panel description says so.
- Presets: Recto Rhythm, Recto Lead. Tests: `tests/rectifier.rs`.

## Session 9 (2026-09-16): the rectifier valve, the mains, and a bigger preset list

**The preset browser.** 330 px tall for a catalogue that has grown to sixty-nine presets
in twelve groups -- about seventeen hundred pixels of list -- so the groups near the
bottom were four screens down. It now uses the panel's height.

**A rectifier valve** (`Part::Rectifier`). Child's law: the drop goes as the two-thirds
power of the current, which is what sag is. Measured against a GZ34's data sheet figure:
10.1 V at 49 mA, 28.6 V at 232 mA (`tests/devices.rs`).
- The **AC30** now has its valve rectifier rather than a resistance standing in for it. It
  still idles at 9.7 V of cathode bias against the factory sheet's 10 V. What that
  amplifier does *not* do is sag much -- a cathode-biased stage near class A draws the same
  current either way -- and what moves instead is its bias, which the test now says.
- The **Rectifier** has both of its settings, because it has a switch: Recto 6L6 (silicon)
  and Recto 6L6 Tube (two 5U4GB). Measured: 482 V idling with 59 V of sag against 471 V
  and 83 V. Output stages that belong to no voice now have their own slots
  (`EXTRA_POWER_SPECS`).

**The mains** (`Simulation::set_supply_scale`, the `mains` parameter, a dropdown in the
CIRCUIT section). Every supply in the circuit and its power stage is multiplied together,
which is all a variac does. Measured on the plexi at 70 %: rails fall in proportion, the
power stage's distortion at the same drive goes from 13.6 % to 30.9 %, and the whole thing
is quieter.

**Presets.** Brown '78 and Brown '84 (Classic Rock), which were waiting on exactly that --
"he was using the Variac to run his Marshall at 85 volts instead of 120" (Donn Landee,
Tape Op). Sixty-nine presets in all.

**Yellow Dist (MXR Distortion+).** Added because the research for Blizzard of Ozz named it
as the pedal Rhoads "almost always kept on", and it is a small, fully documented circuit:
one 741 with germanium diodes to ground. Log: [yellow_dist.md](docs/models/yellow_dist.md).
Measured against the published analysis's own arithmetic -- 5.8 dB and 45.2 dB against its
3.5 and 46.5, clipping at 0.295 V against its 0.35 -- and given a log gain track so the
control sweeps evenly instead of doing everything in its last few millimetres.
Tests: `tests/distortion_plus.rs`.

**More album presets.** Blizzard '80 (Classic Rock) and Machine Rage '92 (Alternative),
each with its evidence table. Seventy-one presets.

**Rig research, recorded rather than built.** Several of the remaining titles turn out to
need amplifiers nobody has modelled, and PRESETS.md now says which:
- *Dirty Chains '92*: three amps at once (Bogner Fish/VHT, Bogner Ecstasy, Rockman),
  split and recombined -- Dave Jerden's own description;
- *Unknown Garden '94*: a Sunn Model T for much of it;
- *Spiral '96*: a modified Marshall Super Bass with its channels jumpered, alongside a
  Rectifier;
- *Never Mind '91*, *Seattle Ten '91*, *Desert Deaf '02*, *Californicated '99*,
  *Blood Sugar '91*: a Mesa Studio .22 preamp, a Marshall Major, an Ampeg VT;
- *Appetite '87*: a rented, modified 1959T, which the stock Brit Plexi is not.

**Still open.** The amplifiers those presets would need, if they are ever wanted.

## Session 10 (2026-09-16): the pedal slot gets the level matching the circuits have

Asked whether the level matching that holds the circuit list together (`CALIBRATION`) and
the power overrides (`POWER_TRIM_DB`) also holds the pedal slot. It did not, in two
separate ways, and `examples/pedallevel.rs` is the measurement.

**Every pedal rested at the same position on its own track**, which is a different amount
of gain in each of them: with the knobs where they are, switching from a Round Fuzz to a
Rodent moved the level by 8.5 dB, and every pedal but one was *quieter* than no pedal at
all. Each pedal's resting position is now its **measured unity point** -- Green 808 0.626,
Ram Fuzz 0.565, Rodent 0.543, Round Fuzz 0.747, Yellow Dist 0.631 -- so the panel's Level
knob at noon means unity through that pedal. It is a position rather than a hidden
make-up: it is where a player sets a pedal's output anyway.

**The pedal's output went straight into the circuit behind it.** A pedal is handed a
guitar's 0.122 V, which is what it was built for and where its diodes sit; but a circuit is
calibrated at the level *it* was built for, which is a guitar for the amplifiers and
anything from 4.7 mV to 1.12 V for the consoles, the studio preamplifiers and the clean
stage. So putting any pedal in front of the clean circuit drove it 19 dB under its
calibration point and the plugin went quiet. `Chain::pedal_hand_off` is the missing
rescale, and it is one for every guitar amplifier in the list, where the two levels are the
same number.

Measured after both, at the knobs' middles, against no pedal at all:

| | into Clean | into Crunch | into Twin Reverb |
|---|---|---|---|
| before | -8.0 to -17.9 dB | +1.4 to +8.6 dB | +1.3 to +8.5 dB |
| after | -2.3 to +2.8 dB | -2.3 to +2.7 dB | -3.7 to +3.9 dB |

What is left is the pedals being different pedals: a fuzz at half its drive makes more
than a clean boost does, and flattening that would be flattening the pedals.

Test: `the_pedal_slot_is_level_matched_in_front_of_any_circuit` in `tests/pedal_slot.rs`,
which holds the departure under 6 dB and asserts that a pedal departs by the same amount in
front of a guitar-level circuit and a line-level one -- that is the hand-off, rather than a
coincidence. `src/calibration.rs` and `src/power_trim.rs` were regenerated, because two of
the pedals are also voices in the circuit list.

`examples/presetlevel.rs` prints the whole preset catalogue loudest first, with the pedal
each one carries, which is how the output trims are chosen. One of them was stale after
this change -- The Great Wall '79 had been trimmed 5 dB down against the old resting level
and had become the quietest entry in the list -- and is now -1. Seven presets carry a
pedal and they average half a decibel below the rest of the catalogue, which spans 10.8 dB.

## Session 11 (2026-09-16): the hash at high gain, and what Bypass means

**The distortion settings added noise, and the control that would have fixed it was
switched off for exactly the circuits that needed it.** A nonlinear stage makes harmonics,
and the ones above half the sample rate fold back onto frequencies that have nothing to do
with the note. At 48 kHz the American 5150 at full drive puts **18.6 %** of its fundamental
back as inharmonic hash. `Chain::set_oversampling` was pinning every modelled circuit to 1x
whatever the panel said (commit 73adad0, "modelled circuits: always run at host rate"),
because at 4x they miss a DAW's deadline -- true, and measured again here -- but the pin
also meant the Oversampling control did nothing at all for the circuits that alias most.

Measured at full drive on a 1760 Hz note, inharmonic energy against the fundamental, with
what one channel costs of its real-time budget (`examples/oversampling.rs`, new):

| | 1x | 2x | 4x | 8x |
|---|---|---|---|---|
| American 5150 | 18.6 % / 34 % | 6.5 % / 67 % | 3.4 % / 114 % | 1.7 % / 193 % |
| Brit 800 | 12.1 % / 21 % | 2.7 % / 40 % | 1.3 % / 71 % | 1.2 % / 142 % |
| Cali Rectifier | 7.2 % / 25 % | 2.9 % / 45 % | 0.8 % / 85 % | 0.2 % / 162 % |
| Cali IIC+ | 5.1 % / 32 % | 1.0 % / 57 % | 0.3 % / 109 % | 0.2 % / 212 % |

So the pin became a **cap**: `voice::MODELLED_MAX_OVERSAMPLING = 2`. Two thirds of the
fold-back goes away for about double the work, and it is the last factor that fits -- past
it every one of these circuits costs more than the time there is. Every shipped preset on a
modelled circuit already asks for host rate, so **no preset costs any more than it did**;
this is what the control does when a player turns it up. The panel row now offers the
factors it can deliver, live, and lights the one in use rather than being greyed at Off
(`Selector::capped`, which clamps the lit segment instead of `Selector::pinned`, now gone).
Test: `modelled_circuits_are_capped_not_pinned`.

**Why Legacy/Off and cabinet Bypass sound different.** They are not the same control.
Cabinet **Bypass** means *no box*: `CabinetChoice::Bypass` still resolves a driver, so the
speaker radiates on an open baffle, a microphone picks it up, and the power stage sees the
speaker's reactive impedance instead of a resistor. The way past the acoustic path is the
**speaker** row's Bypass, which is the terminal voltage into a resistor -- and that is
**bit-identical** to Legacy with its filter off (measured: difference 0.000e0, and the
vectors compare equal). Locked as
`cabinet_bypass_is_no_box_and_speaker_bypass_is_the_di` in `tests/acoustic_chain.rs`.

**README:** a new "What each name is" section, one table per panel section, mapping every
display name to the hardware it was built from with its research log.
