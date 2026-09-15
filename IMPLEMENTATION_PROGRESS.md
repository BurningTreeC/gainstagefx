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
