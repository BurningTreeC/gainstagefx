# GainStageFX implementation résumé / handoff

Checkpoint: 2026-09-15. This is an incremental implementation, **not completion of
the master specification**. Changes are uncommitted. Original clean baseline:
`5fda86934d5695cb1995adb88a2377c6cb29d8e5`.

## Start here

Read [architecture audit](ARCHITECTURE.md), [progress log](IMPLEMENTATION_PROGRESS.md)
and [model inventory](MODELS.md). The audit was completed before production source
changes. Its “before refactor” sections intentionally describe the original code.
Research checkpoints and source links are in [docs/models](docs/models/).

## Completed work

- Audited circuit builders, solver, oversampling, buffering, channel state, GUI,
  parameters, presets, latency, tests and existing performance reports.
- Captured pre-refactor outputs for Mark IIC+, 5150, Twin and the 73P studio
  preamp before modifying production DSP.
- Exposed independent complete power circuits: Matched, Bypass, Cali 6L6,
  American 6L6 Clean, American 6L6 High-Gain and Brit EL34.
- Preserved existing preamp boundaries and numerical component values. Matched
  resolves to each existing amplifier's original power circuit. Bypass skips the
  entire power simulation, including its inverter, tubes, feedback and transformer.
- Separated the 73P line driver from guitar power storage; it remains active with
  studio preamp power bypass. Updated rate/reset, DC sharing, diagnostics and
  dormant-channel runtime copying for the new routing. The line driver is boxed
  during construction to avoid adding large test-trace arrays to Chain's stack
  footprint; no callback allocation is needed.
- Added the 1981 2203-inspired Brit EL34 circuit after schematic research:
  inverter, four EL34s represented as parallel pairs, fixed bias, feedback/presence,
  supply approximation and transformer. This does not add a Brit 800 preamp.
- Added the `power_amp` host parameter and GUI selector. Changed five existing
  display names to Green 808, Cali IIC+, American 5150, British 73 and American Twin
  while retaining their serialized IDs and order.
- Added missing-power defaults for old host state and saved presets. Saved presets
  now carry stable `model_ids` alongside normalized values; no factory era presets
  have been added yet.

## Architecture now

```text
selected gain/preamp
  -> Mark graphic, where applicable
  -> 73P line driver, where applicable
  -> selected complete power circuit, or bypass
  -> existing optional iron / makeup / oversampling return
  -> existing Twin effects / latency padding / tone / legacy cabinet
  -> output
```

Cali IIC+ into Brit EL34 is selectable through normal composition. Pedals are still
part of the single gain-model selector: an independent pedal-before-preamp path is
pending. Power circuits still terminate in resistors. There is no reactive speaker
coupling, speaker mechanics, cabinet geometry or microphone processor yet.

Actual amplifier split points and legacy topology deviations are detailed in
[ARCHITECTURE.md](ARCHITECTURE.md). In particular, the 5150 substitute tone remains
after power, Twin effects retain their old placement, and Mark graphic recovery is
a scalar approximation. The refactor does not establish hardware accuracy for
these inherited approximations.

## Research completed

| Model | Evidence and remaining uncertainty |
| --- | --- |
| [Cali IIC+](docs/models/cali_iic_plus.md) | Original archive plus RP10/FINAL community redraws; drawing/code discrepancies retained for compatibility. |
| [American 5150](docs/models/american_5150.md) | Factory 5150 II and alternate scans located; exact legacy circuit lineage and full reconciliation remain unresolved. |
| [American Twin](docs/models/american_twin.md) | AB763 schematic/layout inspected; documented differences from existing values preserved. |
| [Brit EL34](docs/models/brit_el34.md) | 1981 original drawings cross-checked against 1988 drawings; Hammond replacement transformer data and JJ EL34 data. Supply impedances/core behavior remain estimates. |

Every further hardware model requires the user's eight-question research checkpoint
before model code. Research for one family does not authorize guessing another.

## Compatibility reference

`tests/fixtures/legacy-matched.f32le` contains 819,200 f32 samples (3,276,800 bytes).
SHA256: `0ce9d86405fb3df58b30c7b892ab2656341a3c8f99ec043faf1fdc7e65bf3f76`.
It covers 44.1/48/88.2/96/192 kHz; amplitudes .001/.03/.126/.7; impulse, log sweep,
stepped sine, seeded white and lowpass-colored noise. Colored noise is not precision
pink noise. The existing guitar DI was not included in this frozen capture.

**Do not regenerate the fixture against changed production code.** The ignored
capture test overwrites it. Normal testing compares samples using tolerance
`2e-6 * (1 + abs(expected))`.

New power-value IDs: `matched`, `bypass`, `power_mark_iic`, `power_twin_ab763`,
`power_5150`, `power_2203_el34`. Existing Circuit IDs and enum positions are unchanged.

## Validation checkpoint

- Original release suite: 255 passed, 0 failed, 5 ignored across 34 test binaries.
- After initial routing: frozen baseline, runtime-state tests (7), settings (3)
  and saved-store test (1) passed; source/tests/examples check passed.
- With Brit EL34: frozen baseline passed; modular power tests 6 passed; modular
  state tests 3 passed. Allocation guards cover routing/reset/processing/state-copy
  paths. Rate tests cover all five rates above and sample partitions
  1/7/32/64/127/256/512; these are not equivalent to a DAW format validation.
- Chain latency tests still report 66 host samples. No acoustic delay exists yet.
- CLAP/VST3 host validation, GUI visual checks, new-model CPU/alias measurements,
  hard-drive electrical characterization and listening/level calibration remain.

The progress log records the subsequent full-suite, formatting and lint outcomes.
Test logs in `/tmp/gainstagefx-*.log` are local evidence, not durable artifacts.

## Known issues and next work

1. Complete the Phase 2 validation before adding further families: inspect custom
   master behavior on studio/pedal preamps and explicit Twin selection, review
   Brit presence taper against the schematic, measure idle/hard-drive solver
   behavior, calibration, aliasing and CPU. Existing presence builder uses an
   audio taper; equivalence to the 2203 control law has not been established.
2. Audit master ownership when switching from Matched to an override: the current
   override routes Master to power and can leave a preamp's circuit-level control
   at its previous setting. This needs a deterministic policy and regression test.
3. Resolve the inherited stereo-worker unbounded spin and historical solver/deadline
   failures before claiming complete real-time compliance.
4. Implement researched speaker electrical/mechanical models and load interaction,
   then physical cabinets and microphone placement/response. Preserve legacy
   acoustic output for old sessions through explicit defaults/migration.
5. Add the independent pedal path and missing researched models: American 312,
   Tube 610, Green 9, Rodent, Round Fuzz, Brit 800 preamp, Rectifier, DR103, AC30,
   and remaining power families. Do not expose unavailable variants as implemented.
6. Add era-inspired presets only when their reusable components exist; validate
   levels and document approximations. No artist-specific DSP.
7. Finish automation, all-rate/block, allocation, CPU, CLAP/VST3 and listening
   validation; update the final report with measured results.

## Changed files

Production: `.gitignore`, `src/circuits/power.rs`, `src/voice.rs`, `src/params.rs`,
`src/plugin.rs`, `src/presets.rs`, `src/editor/mod.rs`, `src/editor/style.rs`.

Tests: `tests/settings.rs`, `tests/legacy_baseline.rs`, `tests/modular_power.rs`,
`tests/modular_state.rs`, `tests/fixtures/legacy-matched.f32le`.

Documentation: this file, `ARCHITECTURE.md`, `MODELS.md`, `DSP.md`,
`SPEAKER_MODEL.md`, `CABINET_MODEL.md`, `MICROPHONE_MODEL.md`, `PRESETS.md`,
`IMPLEMENTATION_PROGRESS.md`, and the four research logs linked above.

No commits, plugin installation or deployment have been performed.
