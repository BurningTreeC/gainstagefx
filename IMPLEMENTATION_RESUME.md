# GainStageFX implementation résumé / handoff

## Latest follow-up — input expander and rejected bounded-ray rescue

The user subsequently authorized experiments after instrumentation validation,
and selected gentle noise reduction between notes. Production solver remains
**V3.5 unchanged**. Two residual-only failed-ray rescue variants were tried and
removed: broad activation solved sample 59930 in 18 passes but caused one
unsettled solve globally; a >1e6 merit-growth gate added 392 probes without a
single acceptance. Do not restore `GAINSTAGEFX_TEST_FAILED_RAY_RESCUE`: it no
longer exists. DSP.md contains the SUNDIALS/Ceres/Rust-CV source findings and
PROGRESS has the exact experiment counters and decisions. No performance win
has earned production integration in this follow-up.

New input **Noise Reduction Off/On + Threshold** controls drive a stereo-linked
gentle expander before the pedal/amp and dry/wet branch. Defaults Off/-60 dBFS;
legacy host/preset migration preserves the old sound. Added DSP is in
`src/dsp/noise_reduction.rs`, connected through plugin and stereo worker, with
preallocated shared gains. No latency or circuit simplification. Six focused
tests cover the audio path, smooth state changes, five rates, stereo linking,
worker equivalence and no callback allocations. All previous solver validations
were rerun successfully after this addition; counters and output hash remain
identical with noise reduction Off. Final full workspace suite completed
2026-09-21: **445 passed, 8 skipped** (including the six new expander tests).

Fresh three-run Standard Twin baseline, profiling/expander OFF, Ryzen 7 5700G,
CPU 2 / performance governor, 48 kHz / 64: median CPU mean/p99/max
**765.14 / 1667.56 / 1995.76 µs**, worst observed max **2321.90 µs**, median 397
CPU overruns per 6000 blocks, zero unsettled. The 1333.33 µs target remains unmet;
do not treat zero unpaced scheduler deadline misses as zero CPU overruns.
Commands and metadata artifact paths are in the newest PROGRESS entry.

## Instrumentation checkpoint — full sample trajectory, 2026-09-20

Started from clean `e2297db` (`fix clippy errors`), after `a075f71` (LM experiments).
Only test diagnostics and documentation changed. Production remains V3.5; no new
solver optimization is enabled. LM13 remains research-only and dogleg opt-in.
The prior handoff below is historical. See the newest PROGRESS and DSP sections.

`GAINSTAGEFX_TRACE_POWER_SAMPLE=59930` now captures the complete selected Twin
power solve. Relative zero is the first measured power solve after warm-up;
59930 maps to solve 60955, block 936/frame 26. Read the variable once in harness
setup. Print the preallocated records after callbacks. No neighbor flag was added.

```bash
env -u GAINSTAGEFX_ATTACK_PRESET \
GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1 \
GAINSTAGEFX_TRACE_POWER_SAMPLE=59930 \
cargo test --release --lib twin_realtime_recording_solver_trace \
  -- --ignored --nocapture --test-threads=1 \
  2>&1 | tee /tmp/twin-59930-full-trace.log
```

The log contains begin/end records, all predictor unknowns, 166 passes and 173
trials; no overflow, unsettled=0. The source-step ratio is +15.0564 and predictor
scale zero. Five 1/8 fallback steps send the residual as high as 7.995e107;
76 exhausted-tail passes follow, before a 26-pass restart finally converges.
The issue is not merely the final sequence of small damped steps. Inspect the
full log before proposing another solver policy. **Do not optimize as part of
this instrumentation task.**

Clean counters are unchanged with tracing off AND on: Newton 1944584,
trials 613429, backtracks 109464, fallbacks 14107, continuation attempts 840,
restart passes 245, unsettled 0. Complete output hash remains `b2fc6ff9fd0296df`.
All additional residual probes restore the nonlinear evaluation caches and never
supply values to the solver's acceptance/convergence logic.

Formatting, strict release/workspace Clippy, four LM tests, the high-accuracy
Twin reference, Blackface attacks with LM enabled, clean trace and full trace
pass. Two new unit tests cover sample numbering and bit-identical DSP/cache/health
at five rates. Cargo's dependency-only `xcb 0.9.0` future-compatibility notice
remains; there are no project warnings. Full workspace nextest: **439 passed,
8 skipped**.

## Previous checkpoint — Ceres audit, 2026-09-20

HEAD `8e09bb2`, package 0.19.0. The working tree arrived with user LM13-v2,
dogleg, harness and attribution changes; preserve them. V3.5 remains production.
Read the newest IMPLEMENTATION_PROGRESS.md entry and DSP.md Ceres audit first;
older handoffs below are historical and must not override current source.

- **KEEP:** transactional LM device/cache rollback, exact-zero best-trial guard,
  five-rate rollback/reference tests, opt-in dogleg, clean A/B tooling and explicit
  requested/applied preset reporting. None changes production circuit equations.
- **REJECT for production:** original opt-in LM13-v2: one unsettled solve on
  each recording versus V3.5 zero. Still available only under `cfg(test)` for
  research; do not enable by default.
- **REJECT / removed:** cycle-only rescue gate (never exercised) and restart-only
  gate (one useful rescue but no repeated p99/max win). Their switches are gone.
- Existing exact Schur, evaluation cache, reciprocal/fixed13 LU, boundary-major
  recovery, precondensed stamp, fixed13 merit/residual remain. Convergent-search
  release and accepted-search-merit reuse are not current production paths.
- Ceres inspected at `/home/simon/Work/GitHub/ceres-solver`, clean revision
  `fe351d5ab8cbc574f33456376bf5aa90d3162d5d`. Algorithm findings/links in DSP.md.

**Benchmark naming trap:** Ultra Lead is a Peavey preset. The Twin trace ignores
it on circuit mismatch and runs the standard Twin fixture. Blackface Throb is an
actual Twin preset. New output reports this explicitly. Do not call standard
Twin results "Ultra Lead preset" results or silently alter historical input.

Five-pair baseline medians, reported Ryzen 7 5700G/Linux, CPU 2, 48 kHz/block 64,
profiling off: standard Twin CPU mean/p99/max 773.91/1681.18/2054.95 µs,
407 CPU overruns per 6000 blocks; Blackface 638.36/1447.87/1941.31 µs, 88 overruns.
Both have zero unsettled. These exceed 1333.33 µs; realtime is NOT finished.

### Reproduce current baseline and validate an opt-in experiment

```bash
cargo test --release --lib ceres_lm13 -- --nocapture --test-threads=1
cargo test --release --test voice twin_power_four_backtracks_matches_full_reference -- --ignored --nocapture --test-threads=1
cargo nextest run --release --workspace --no-fail-fast --test-threads 4
cargo fmt --check
cargo clippy --all-targets --all-features

# Explicit production-equivalent baseline (old dogleg disable still supported).
GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1 \
GAINSTAGEFX_ATTACK_PRESET="Blackface Throb" \
cargo test --release --lib twin_realtime_recording_solver_trace -- --ignored --nocapture --test-threads=1

# Actual opt-in A/B; CURRENT v2 should fail this script's unsettled gate.
python3 tools/solver_ab.py --output /tmp/lm13-new-ab --preset "Blackface Throb" \
  --enable-switch GAINSTAGEFX_TEST_CERES_LM13 --clean-v35 \
  --cpu 2 --repetitions 5 --control-profile

# Existing exact-arithmetic experiment, with clean solver policy.
python3 tools/solver_ab.py --output /tmp/fixed13-new-ab --preset "Blackface Throb" \
  --disable-switch GAINSTAGEFX_TEST_DISABLE_FIXED_13_STAMPED_MERIT \
  --clean-v35 --cpu 2 --repetitions 5 --assert-control-equal
```

Environment flags are read at construction, not per sample. LM and dogleg are
library-test-only; integration tests compile the dependency without `cfg(test)`.
`GAINSTAGEFX_PROFILE_SOLVER_CONTROL_TAIL=1`,
`GAINSTAGEFX_PROFILE_TWIN_POWER_PHASES=1`, `GAINSTAGEFX_TRACE_UNSETTLED=1` are
diagnostic runs only. Keep their timing separate from production-cost timing.
`GAINSTAGEFX_TEST_NLSOLVE_DOGLEG_TRUST_REGION=1` explicitly opts into old dogleg;
`GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1` overrides it.

Next: instrument the entire trajectory of standard Twin sample 59930 and
Blackface v2 failure 341494; last-eight traces miss the basin transition. Follow
ranked experiments in PROGRESS. Do not broaden LM: 12,519 entries already failed
where one narrow rescue barely changed aggregate work. Model implementation
remains the separate documented roadmap; no new circuit was added this session.

## Historical checkpoint — earlier 2026-09-20 (`gainstagefx(8).zip`)

**Read this section first.** The older 2026-09-15 handoff below is retained as
historical context and no longer describes the current feature/model inventory.
Current git HEAD is `aab1277` (`v0.18.0`) with solver/model work uncommitted.
`IMPLEMENTATION_PROGRESS.md` contains the detailed 2026-09-20 audit and exact A/B
commands.

### Current solver baseline

Retain unless new measurements justify reversal:

- exact nonlinear Schur reduction (Twin common case: 31 full / 13 boundary / 18 internal)
- accepted-trial nonlinear evaluation cache
- reciprocal-pivot reduced LU
- boundary-major recovery
- fixed 13x13 LU with correct pivot-plan invalidation
- precondensed 13x13 stamp base
- phase profiler and solver-control tail profiler

Currently under validation:

1. **fixed 13x13 stamped merit** — expected bit-identical to generic path;
   disable with `GAINSTAGEFX_TEST_DISABLE_FIXED_13_STAMPED_MERIT=1`.
2. **fixed 13x13 trial residual** — new Codex specialization; expected bit-identical;
   disable with `GAINSTAGEFX_TEST_DISABLE_FIXED_13_TRIAL_RESIDUAL=1`.
3. **convergent-search release** — solver-policy experiment, still provisional;
   disable with `GAINSTAGEFX_TEST_DISABLE_CONVERGENT_SEARCH_RELEASE=1`.

Do not reintroduce accepted-search-merit reuse: it was intentionally replaced
because its direct residual arithmetic changed floating-point search decisions.

### Performance target

At 48 kHz / 64 samples the callback budget is 1333.33 us. The target is **both**
CPU p99 and CPU max below that budget, with zero CPU misses and zero unsettled
solves. Mean CPU is secondary to tail latency.

Use `tools/solver_ab.py` for repeated alternating A/B. For the two fixed-size
arithmetic specializations use `--assert-control-equal`: solver counters and
complete output hashes should match exactly. Run the full-reference test before
accepting any solver change.

### Codex additions in `gainstagefx(8)`

- fixed-13 trial-residual gather/evaluation and cancellation/mapping regression
- complete-output hash in `twin_realtime_recording_solver_trace`
- repeated A/B benchmark driver `tools/solver_ab.py`
- direct Twin selection Reverb/Intensity state synchronization + regression test
- isolated April 1991 MT-2 U2a/U2b middle-EQ reference netlist + AC/time tests
- updated DSP/model inventory/research documentation

The Rust changes have **not been validated in this handoff environment** because
Cargo/Rust is unavailable here. Treat compile/test execution on the development
machine as the next gate, not as optional cleanup.

### Circuit continuation

The production MT-2 still uses the old approximate swept gyrator. Validate the
isolated U2a/U2b reference first, then add headroom/state/loading tests and integrate
it with the real low/high stage and Level loading. Recalibrate and compare presets
and realtime cost after integration.

After MT-2, the current documented priorities are: resolve the 5150 revision/tone
stack/resonance; complete DR103 feedback/presence and IIC+ PI/power reconciliation;
finish DS-1 and Studio Preamp evidence; then modern and bass families. `MODELS.md`
and `docs/MODEL_INVENTORY.md` are the current device-status sources.

### Immediate commands

```bash
cargo test --release fixed_13_merit_and_trial_residual_preserve_mapped_cancellation_cases -- --nocapture
cargo test --release twin_power_four_backtracks_matches_full_reference -- --ignored --nocapture --test-threads=1
cargo test --release direct_twin_selection_uses_the_same_dry_defaults_as_settings -- --nocapture
cargo test --release --test metal_zone original_wien_mid -- --nocapture
```

Then run the two five-pair A/B jobs documented in `IMPLEMENTATION_PROGRESS.md`.
Only after those results should the next solver optimization be implemented.

---

## Historical handoff — 2026-09-15

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
