# Implementation progress

## 2026-09-20 — follow-up rescue research and optional input expander

After the instrumentation task passed all 439 tests (eight skipped), the user
authorized further targeted solver research and selected a gentle gate/expander
for noise between notes. The two efforts remain independent: the expander is a
user-controlled effect, **not** a way to improve the solver benchmark.

### Failed-ray rescue experiments — REJECT / removed

Hypothesis: a small amount of residual-only work at the unchanged iterate can
prevent the finite fallback excursions exposed by sample 59930, avoiding many
restamps/factorizations. Read local SUNDIALS KINSOL at `312fc0f`, local Ceres at
`fe351d5`, and upstream Rust-CV LM/QR/trust-region source. Exact links and transfer
decisions are in DSP.md. No library dependency was added.

Only `src/dsp/time.rs` was temporarily changed for these experiments. Under a
test-only `GAINSTAGEFX_TEST_FAILED_RAY_RESCUE=1`, a complete failed Twin reduced
search could try half and quarter of its existing minimum lambda. Existing
acceptance plus strict current-merit decrease was required; rejected attempts
restored device checkpoints/caches and retained the original fallback. Neither
equations nor convergence tolerances changed. This changes candidate policy and
can change subsequent solver trajectories; it is not an exact arithmetic speedup.

| Variant | Newton passes | Residual trials | Fallbacks | Continuation attempts | Restart passes | Unsettled | Decision |
|---|---:|---:|---:|---:|---:|---:|---|
| V3.5 | 1944584 | 613429 | 14107 | 840 | 245 | 0 | KEEP |
| Two probes after failed worsening ray | 1956155 | 660835 | 5688 | 73 | 315 | 1 | REJECT |
| Only if best merit > current merit × 1e6 | 1944584 | 613821 | 14107 | 840 | 245 | 0 | REJECT |

The broad version solved selected sample 59930 in **18 passes / 27 trials**, but
the complete recording failed its unsettled gate and increased global trial work
7.73%. The new failure is relative sample 48090 (block 751/frame 26): 161 Newton
passes, 247 trials, 32 restart passes and final normalized correction 4.12e5.
Its unmodified V3.5 counterpart converges in 66 passes / 67 trials with two
fallbacks, one successful continuation and no restart; the complete counterpart
trace is `/tmp/twin-48090-v35-full-trace.log`.
A local success cannot justify that regression. The narrower version
accepted none of its 392 additional probes and retained the original trajectory.
The 1e6 cutoff was an isolated severe-excursion hypothesis, not a fitted or accepted
production threshold. Neither variant advanced to realtime timing A/B: correctness
failed in the first; the second only added work with no accepted rescue. Both
implementations and the switch were removed, preserving all tracing changes.

Commands/evidence (historical switch is now absent):

```bash
GAINSTAGEFX_TEST_FAILED_RAY_RESCUE=1 cargo test --release --lib ceres_lm13 -- --nocapture --test-threads=1
env -u GAINSTAGEFX_ATTACK_PRESET GAINSTAGEFX_TEST_FAILED_RAY_RESCUE=1 \
  GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1 \
  GAINSTAGEFX_TRACE_POWER_SAMPLE=59930 cargo test --release --lib \
  twin_realtime_recording_solver_trace -- --ignored --nocapture --test-threads=1
# Narrow variant: same recording, PROFILE_SOLVER_CONTROL_TAIL=1 instead of full trace.
```

Logs: `/tmp/twin-ray-rescue-{unit,trace,severe}.log`. No solver performance win is
claimed or enabled. The next useful experiment needs to distinguish useful early
nonmonotone progress from the precursor to a large finite excursion, rather than
simply spending more probes after the direction is already unusable.

### Noise reduction — implemented, default OFF

Added `src/dsp/noise_reduction.rs` with a bounded, allocation-free, stereo-linked
2:1 downward expander. Controls are **Noise Reduction Off/On** and **Threshold**
(-90 to -30 dBFS; default -60). The cubic knee, 25 ms hold, slow release and 5 ms
enable/disable fade avoid hard gating. Maximum attenuation is 40 dB. The shared
input detector tracks the hotter channel, and the worker consumes preallocated
gains calculated on the host thread. There is no added latency or skipped circuit
work. Factory presets and absent legacy-state fields initialize it Off; explicit
saved/automated values persist. Meter readings remain before the expander.

Validation covers curve/attenuation, hold/attack/release, exact off unity, reset,
five rates, mono/dual-mono, linked stereo, parallel/sequential equivalence, plugin
bypass, migration, preset ids, and no callback allocations. Six focused tests pass.
The actual host path applies it before dry/wet branching and before pedal/amp;
reverb/cabinet decay state continues independently. It is not a hardware emulation
or continuous denoiser. UI adds controls within the existing Input section.

Final-state formatting and strict release/workspace Clippy pass; four LM tests,
Twin reference and LM-enabled Blackface attacks pass. Clean and exact Standard
Twin recordings retain **all baseline counters and `b2fc6ff9fd0296df` audio hash**.
The exact artifact is `/tmp/twin-59930-full-trace.log` (166 passes, 173 trials,
sample 59930 / solve 60955 / block 936 / frame 26, unsettled=0). Final validation
logs are `/tmp/gainstagefx-final-*.log`. Full workspace result (completed
2026-09-21): **445 passed, 8 skipped**, 163.7 s test time. No failing tests remain.

### Final profiling-free baseline — 2026-09-21

After compilation/tests finished, measured the unchanged Standard Twin fixture
on the reported **Ryzen 7 5700G**, Linux, performance governor, pinned CPU 2,
48 kHz / 64 frames. Noise reduction OFF; both profilers and LM OFF; dogleg
explicitly disabled. Hyperfine ran one warm-up plus three measured repetitions
of the prebuilt library test. Each fixture also performs its normal circuit
warm-up. Eight seconds of input, 6000 measured blocks per repetition.

| Status | CPU mean µs | CPU p99 µs | CPU max µs | CPU budget overruns | Unsettled |
|---|---:|---:|---:|---:|---:|
| Current V3.5 baseline, medians of three | 765.14 | 1667.56 | 1995.76 | 397 | 0 |

Individual CPU maxima: 2321.90 / 1992.41 / 1995.76 µs. Report the **2321.90 µs
worst observation** as well as the median maximum; neither satisfies 1333.33 µs.
The unpaced fixture reports zero scheduler deadline misses, which does **not**
erase the measured CPU-budget overruns. All four runs retain output hash
`b2fc6ff9fd0296df`. These numbers establish a current baseline, not an A/B speed
improvement. Mean fits the budget; p99/max and zero overruns remain open goals.

Reproduction (use a fresh output log; executable path is recorded in metadata):

```bash
hyperfine --runs 3 --warmup 1 --export-json /tmp/gainstagefx-final-hyperfine.json \
  'env -u GAINSTAGEFX_ATTACK_PRESET -u GAINSTAGEFX_TRACE_POWER_SAMPLE -u GAINSTAGEFX_PROFILE_SOLVER_CONTROL_TAIL -u GAINSTAGEFX_PROFILE_TWIN_POWER_PHASES -u GAINSTAGEFX_TEST_CERES_LM13 GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1 GAINSTAGEFX_REALTIME_SECONDS=8 taskset -c 2 target/release/deps/gainstagefx-a6d9777ce2f4cd13 twin_realtime_recording_solver_trace --ignored --nocapture --test-threads=1 >> /tmp/gainstagefx-final-timing.log 2>&1'
```

`/tmp/gainstagefx-final-timing-summary.json` records each callback metric, medians,
binary SHA256, compiler, CPU/governor and Git state; Hyperfine's process elapsed
time is separate from the harness's per-callback thread CPU measurements.

## 2026-09-20 — exact-sample full-pass trace (instrumentation only)

Fresh discovery found a **clean `e2297db`** checkout, following `a075f71`. Its
`fix clippy errors` commit had already replaced the fixed-13 test's indexed loop
with `iter_mut().take(13).enumerate()` and replaced five constant release assertions
with debug-only panics (two attack tests, two recording tests, one Twin reference).
The exact requested release/workspace Clippy command was already clean on entry.
No further lint suppressions, dependency edits, or DSP refactors were needed.

Added `src/dsp/time/full_trace.rs`, small `cfg(test)` hooks in `time.rs`, a read-only
limiter-state accessor in `device.rs`, forwarding in `voice.rs`, and setup/reporting
in `tests/support/plugin_mono.rs`. `GAINSTAGEFX_TRACE_POWER_SAMPLE` is read once by
the harness after warm-up. Buffers are preallocated and reporting occurs outside
the audio callbacks. Every pass and candidate is retained for the selected solve;
overflow/missing selection is an explicit test failure. Production has no new
trace state, allocation, environment lookup, or call wrapper. LM and dogleg remain
opt-in. **No performance optimization or numerical-policy change was made.**

Sample mapping is zero-based after warm-up: 1024 + 59930 + 1 = solve 60955,
block 936/frame 26. The mapping test includes relative zero and integer overflow.
A second regression checks identical output bits, solver health and device cache
state with/without tracing at 44.1/48/88.2/96/192 kHz. The recording also runs under
the existing no-heap callback guard.

### Validation and deterministic baseline

`cargo fmt --check` and
`cargo clippy --release --workspace --all-targets -- -D warnings` pass with zero
project warnings/errors. Cargo still emits the upstream `xcb 0.9.0`
future-incompatibility notice, which is not a project Clippy diagnostic.
Four LM tests pass. The ignored Twin reference passes with nulls
-202.9 / -166.9 / -184.0 dB at drive 0.2 / 0.6 / 1.0. Blackface high-pick attacks
pass with LM explicitly enabled and dogleg disabled.

Clean Standard Twin, **before and after instrumentation, and with full tracing
active**, has identical complete control counters and output hash:

```text
solves=384000
newton_passes=1944584  searched_passes=552633  search_trials=613429
backtracks=109464  fallbacks=14107  cont_attempts=840  restart_passes=245
unsettled=0
fnv1a64=b2fc6ff9fd0296df
```

Generated `/tmp/twin-59930-full-trace.log` with the requested command. Parsed and
verified exactly 166 consecutive pass records (1–166), 173 trial records, only
relative sample 59930, correct solve/block/frame, no overflow, and unsettled=0.
The sample has 160 searched passes, 19 backtracks, five fallbacks, one midpoint,
and one 26-pass restart. Its phase counts are 59 normal, one midpoint, four deep
continuation, 76 exhausted-tail, and 26 restart passes.

### What the complete trajectory reveals (no policy change)

The previous source step is -2.551193 V and the new step -38.411718 V: their ratio
is **+15.0564**, above the existing 2x gate. The predictor is available but
suppressed; all after-predictor state equals the last settled state. This source
history is a large same-sign derivative increase, not itself a derivative sign
reversal (the voltage crosses zero).

Five failed searches choose the 1/8 fallback, at passes 5, 8, 16, 29 and 30.
The diagnostic exact reduced merit changes as follows:

| Pass | Before | After fallback |
|---|---:|---:|
| 5 | 0.2593 | 0.5241 |
| 8 | 0.1702 | 7.972e27 |
| 16 | 1.302e19 | 1.357e36 |
| 29 | 9.028e25 | 3.211e64 |
| 30 | 3.211e64 | 7.995e107 |

Pass 31 is the continuation midpoint. Thereafter many accepted full steps reduce
an enormous finite residual. Passes 65–140 occupy the exhausted-tail extension;
at pass 140 the merit is still about 0.529 and normalized correction about 2.47e6.
Pass 141 restarts from the previous settled voltage, with initial merit again
0.115, and converges at pass 166. `pl_a` dominates the final corrections. Final
normalized correction is 0.00229062 and diagnostic residual about 6.73e-25.
Thus the last-eight trace hid the early finite excursions and the long return
from them. This is evidence for the next investigation, not authorization here
to change globalization, predictors, budgets, or convergence criteria.

Logs: `/tmp/twin-full-trace-{clippy,lm-tests,reference,blackface,clean,nextest}.log`,
`/tmp/twin-full-trace-unit.log`, `/tmp/twin-59930-full-trace.log`.
`cargo nextest run --release --workspace --no-fail-fast --test-threads 4`:
**439 passed, 8 skipped**, including both new trace regressions (163.1 s test time).

## 2026-09-20 — fresh V3.5 / Ceres LM13 audit and rescue safeguards

Authoritative checkout: `8e09bb2`, version 0.19.0, with pre-existing user changes
in `time.rs`, `partition.rs`, the trace harness and Ceres notices. Inspected the
current diffs, full solver/control/device paths and local Ceres at
`/home/simon/Work/GitHub/ceres-solver` commit
`fe351d5ab8cbc574f33456376bf5aa90d3162d5d`. Detailed algorithm findings and pinned
source links are in DSP.md. No Ceres/Eigen/C++ dependency was added.

**Fixture correction:** requesting `Ultra Lead` in the Twin-specific test does
NOT apply that preset. It belongs to Peavey; `apply_preset` returns on circuit
mismatch. Historical "Ultra Lead Twin" rows actually measure the standard Twin
fixture. This session preserves that input/configuration for comparison, but now
prints `preset_configuration` with both requested and applied values. Blackface
Throb does apply. Do not silently change these fixtures and compare their numbers.

### Changes and decisions

| Change / hypothesis | Result | Decision |
|---|---|---|
| A rejected lazy LM restamp changes device correction history and evicts nonlinear caches; restoring only `exact` is insufficient. | Added preallocated test-only device checkpoints, rollback after capture/rejected probes, five-rate rollback/budget tests and triode/pentode/core cache tests. Both recording control profiles remain identical to unfixed v2. | **KEEP**, correctness safeguard; no speed claim. |
| Best Newton-ray merit zero must not be ignored by LM's 2% improvement guard. | Changed `> 0` to `>= 0`, added regression. | **KEEP**, acceptance edge-case fix. |
| Tests should measure the accepted baseline unless an experiment is requested. | Dogleg was on by default in test builds. Made it opt-in via `GAINSTAGEFX_TEST_NLSOLVE_DOGLEG_TRUST_REGION`; existing disable switch overrides it. Production unchanged. | **KEEP**, test isolation. |
| LM13-v2 behind every failed ray should provide bounded useful rescues. | Standard Twin: 12,519 entries, 14,313 probes, 10,740 accepts, one unsettled. Blackface: 2,336 entries, 3,125 probes, 1,548 accepts, one unsettled. Both clean baselines have zero unsettled. | **REJECT for production**. User's opt-in research code retained, with safeguards repaired. |
| Require an existing cycle rejection AND complete ray failure before LM. | Zero entries on either fixture: the subsequent shorter V3.5 ray already succeeds. Exactly baseline counters. | **REJECT**, no exercised benefit; code/switch removed. |
| Permit LM only inside the existing exhausted-solve restart, after that restart's ray fails. | Standard Twin: one LM entry/probe/accept, Newton -237, trials -198, restart passes 245 -> 165, zero unsettled. Blackface identical to baseline. Repeated maximum timing did not improve. | **REJECT**, insufficient tail benefit; code/switch removed. |
| Clean reproducible opt-in A/B and honest preset reporting. | `solver_ab.py` now supports mutually exclusive `--enable-switch` / `--disable-switch`, `--clean-v35`, and records applied-preset diagnostics. | **KEEP**, benchmark tooling. |

### Deterministic recording work (384,000 samples each)

| Fixture / path | Newton passes | Searched | Trials | Backtracks | Fallbacks | Continuation attempts | Restart passes | Unsettled |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Standard Twin / V3.5 | 1,944,584 | 552,633 | 613,429 | 109,464 | 14,107 | 840 | 245 | 0 |
| Standard Twin / v2 | 1,948,831 | 556,006 | 638,178 | 119,564 | 6,318 | 1,129 | 32 | 1 |
| Standard Twin / rejected restart gate | 1,944,347 | 552,405 | 613,231 | 109,451 | 14,068 | 835 | 165 | 0 |
| Blackface / V3.5 | 1,434,456 | 177,237 | 191,287 | 23,807 | 2,666 | 247 | 17 | 0 |
| Blackface / v2 | 1,435,390 | 178,016 | 196,524 | 25,788 | 1,773 | 315 | 57 | 1 |

All these profiles have zero full-MNA solves/replans. Lower fallback counts alone
are not a win: v2 displaced recovery and increased continuation and nonlinear
trials. Rollback alone changed none of the v2 control counters/audio hashes.

### Repeated timing, profiling OFF

Linux, CPU reported by `lscpu`: AMD Ryzen 7 **5700G**, pinned logical CPU 2,
48 kHz / 64 samples, eight-second excerpt, five alternating A/B pairs per fixture.
No build or test suite ran concurrently. Governors/affinity/compiler/binary SHA
are recorded in the run summaries. CPU is thread CPU time; pinning is not core
isolation. Figures are medians across repetitions, including median per-run max.

| Fixture / optimization | CPU mean µs | CPU p99 µs | CPU max µs | CPU budget overruns / 6000 | Decision |
|---|---:|---:|---:|---:|---|
| Standard Twin / clean V3.5 | 773.91 | 1681.18 | 2054.95 | 407 | Baseline; target unmet |
| Standard Twin / restart-only LM | 774.15 | 1673.44 | 2170.44 | 405 | REJECT |
| Blackface / clean V3.5 | 638.36 | 1447.87 | 1941.31 | 88 | Baseline; target unmet |
| Blackface / restart-only LM | 638.78 | 1450.38 | 2070.48 | 93 | REJECT |

Worst observed maxima across the five runs: standard Twin baseline 2577.76 µs,
candidate 2272.53 µs; Blackface baseline 2028.53 µs, candidate 2871.62 µs.
No robust tail win; different summaries of noisy maxima point in different
directions. Blackface had identical counters/hashes and zero LM entries, exposing
the timing noise directly. Unpaced `deadline_misses=0` is not a live deadline
claim: use CPU budget overruns above and the separately paced realtime test.
The 1333.33 µs p99/max requirement remains unmet.

Raw session evidence: `/tmp/lm13-before-*.log`, `/tmp/lm13-rollback-*.log`,
`/tmp/lm13-cycle-*.log`, `/tmp/lm13-restart-*.log`,
`/tmp/lm13-restart-ab-{ultra,blackface}/summary.json`. Raw /tmp files are ephemeral;
the decision, counters and timing tables above are the durable record.

### Tail evidence and ranked next experiments

Standard Twin's worst-work sample (relative sample 59930, block 936/frame 26)
uses 166 Newton passes, 160 searched passes and 173 nonlinear trials. It follows
a source reversal: -38.41 V step after -2.55 V, predictor suppressed. The last
lambdas are 1/32, 1/16, 1/8, 1/4, 1/2, 1, 1, 1; final correction is 0.00229.
Expensive blocks also contain many difficult samples without any restart:
block 5339 in the restart-gate profile has 877 Newton passes / 742 trials,
zero LM entries and zero restart attempts. Rescue-only LM cannot fix most of
that block's work. The v2 Blackface failure at relative sample 341494 uses
144 passes / 177 trials and alternating ~0.635 / ~0.317 steps; its two LM
probes are rejected, so the sample failure is not a locally accepted LM step.
Earlier accepted rescues still change its incoming trajectory.

1. Record full per-pass correction, merit and dominant unknown for a targeted
   worst sample, including pre/post predictor state. The current last-eight
   history loses the transition that explains 100+ passes. Keep trace test-only.
2. Measure a source-aware predictor confidence strategy at those reversals,
   independently of LM. Do not resurrect V3.6–V3.8.1 without new evidence.
3. Ceres's cheap positive model-reduction screen can avoid invalid nonlinear
   probes; all current traces have `lm_invalid=0`, so this is a safeguard, not
   an evidenced performance opportunity yet. Preserve arithmetic ordering when
   comparing predicted cost formulations.
4. Separate Jacobian column scaling experiment only after an activation policy
   earns correctness. Tiny dense factorization is not the measured bottleneck;
   no observed factorization failures justify mixed precision or refinement.
5. Nonmonotone-history and dogleg policy remain research-only. Neither should
   be stacked onto an unvalidated LM path.

### Validation

LM kernel, material-progress, rollback and cache regressions pass. The new library
reference test really compiles the experimental LM path (`cfg(test)`); setting an
environment flag on an integration-test dependency alone does not enable it.
Across 44.1/48/88.2/96/192 kHz and drive 0.2/0.6/1.0: zero unsettled/nonfinite,
nulls -164.0 to -210.1 dB. At 48 kHz: -202.9 / -166.9 / -184.2 dB.
Blackface high-pick test passes with LM enabled. These short-fixture passes do
**not** override the failed recording gate. Full suite/static checks follow below.

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

## Session 12 (2026-09-16): four research logs, a wider pedal slot, and the Heavy Metal

**A name in this repository was wrong, and it mattered.** *Never Mind '91* was listed as
blocked on "a Mesa Studio .22 preamp". There is no such product: the **Studio .22** is a
20 W EL84 combo, and the record used the rackmount **Studio Preamp** into a Crown Power
Base 2 and 4x12s. Modelling the .22 would have produced the wrong sound under the right
name. PRESETS.md is corrected, and the rig turns out to be a shape the plugin has never
had: a valve preamplifier into a **solid-state** power amplifier.

**Four research logs**, each with the eight-question checkpoint:

| Model | Source | Verdict |
|---|---|---|
| [heavy_metal.md](docs/models/heavy_metal.md) | Boss's own drawing | cleared, and **built** |
| [metal_zone.md](docs/models/metal_zone.md) | Boss's drawing + Electric Druid | cleared for code |
| [orange_dist.md](docs/models/orange_dist.md) | ElectroSmash, value by value | cleared but for the op-amp |
| [studio_pre.md](docs/models/studio_pre.md) | Mesa's manual; drawing illegible | **not cleared** |

The Studio Preamp is honestly blocked: the only sheet located is hand-drawn and cannot be
read value-by-value, and the legible sheet in the same file is a different product. It was
not built from the near relative, because that is the "approximate alias" PRESETS.md
refuses.

**The pedal slot carries four tone controls** (`voice::PEDAL_TONES`). It used to carry one,
which was fine while every pedal in it had one tone knob or none; it is not fine for a
Metal Zone, whose entire point is its equaliser. Each pedal now declares its controls with
their own labels -- tone, filter, colour lo/hi -- and the panel draws the knobs that pedal
has and names them the way the box does. Three parameters appended (`pedal_tone_b/c/d`),
`pedal_tone` untouched, and presets saved before this leave them at their middles.

**The Heavy Metal (Boss HM-2)** is the first pedal with two of them. Three numbers derived
from the drawing -- gyrator resonances at **87, 958 and 1278 Hz** -- land on what the
published analyses hear from outside ("around 80Hz", "between about 900Hz and 1.3kHz"),
which is the cross-check the model rests on. What the build cost, all recorded in the log:
both gain stages are shunt-feedback stages; the drawing's R10/C5 decoupling matters (a
-40 dB floor without it); the equaliser has to be ground-referenced, as the Rodent's op-amp
did; and **the Dist control sits before the clipper**, coupled in, with its cold end on the
4.5 V bias -- built after the clipper it is a volume control, and taken to ground it drives
the clipping amplifier into its rail and leaves it there.

Its coring gate is the only circuit in the catalogue whose **gain rises with level**:
+18.5 dB at 1 uV, +53.2 dB at 1 mV, then falling into the clipping. Unity is at level
0.719. In the slot it is the hottest pedal, +5.3 dB against no pedal, and with Dist shut it
falls to -81 dB because the gate closes.

**The CI failure.** The oversampling cap shipped without the fixture change that belongs
with it: the four baseline voices are all modelled circuits, so they had been ignoring
`Settings::default()`'s request for 2x and now honour it, the oversampler's latency differs
between factors, and the render shifts in time. `tests/legacy_baseline.rs` renders at 1x
explicitly, with a note saying why that is not cheating: at 1x the code path through those
circuits is unchanged, so the comparison stays sample-for-sample honest, and the cap has
its own test.

**Still open.** The Metal Zone netlist; the DS-1's op-amp question; a legible Studio Preamp
drawing.

## Session 13 (2026-09-16): the Metal Zone's sweep, a CPU policy, and the pedals as circuits

**The Heavy Metal crackled, and it was not the solver.** No unsettled solves and no
non-finite samples anywhere; the pedal simply costs **41.6 %** of one channel's realtime
budget on its own -- four times any older pedal -- and in front of a Cali IIC+ at twice the
host rate it was **132 %**, which is a DAW missing its deadline. Session 12's oversampling
cap caused it: it looked at the voice, and the pedal runs inside the same oversampler.
`Pedal::is_expensive` now holds the chain at host rate when one of the two fifty-unknown
pedals is in the slot (75 % instead of 132 %, tested). Along the way the HM-2's Q10 turned
out to be one of the two JFETs that make its bypass -- it sits in the switching corner with
D12 -- and had been built as a bipolar gain stage.

**The Metal Zone's Mid Freq control works.** Three arrangements were built and measured:
a gyrator with one swept resistance (centre goes as `1/sqrt(R)`, a fifth of the span, and
the Q slides with it); the Wien network bootstrapped by its buffer (faithful, but the depth
goes as `1/(1 - G/3)` and it oscillates at `G = 3`, so a usable margin left a band a decibel
deep); and the one that ships, **both gang sections sweeping one gyrator**. That gives
`L = R^2 C`, so the centre goes as `1/R` -- the Wien bridge's own law -- and
`Q = sqrt(C_gyr / Cs)`, a constant. Measured 4877 Hz to 206 Hz against a published 4.7 kHz
to 240 Hz. What it does not hold is the depth, which is recorded in the log.

**Every pedal is now also a circuit** (`Circuit::Green9`, `Rat`, `FuzzFace`, `DistPlus`,
`Hm2`, `Mt2`), appended so no saved automation lane moves, with `CALIBRATION` regenerated
for 35 voices. This found a **latent crash**: `POWER_TRIM_DB` was declared `[[f64; 10]; 21]`
but indexed by position in `Gain::ALL`, so selecting a power override on any new circuit
would have indexed past the end of the table. Its length is now tied to `voice::GAINS`, and
adding a circuit fails to compile rather than panicking.

**The modelled list is grouped** under PEDALS, AMPLIFIERS and MICROPHONE PREAMPS. The
grouping is done in what the menu draws; the parameter's order is untouched, because that
is what a saved automation lane was recorded against.

**Two album presets**, both documented:
- *Swedish Death '90* (Entombed, **Left Hand Path**): a Boss HM-2 with all four knobs at
  ten into a clean solid-state combo. The pedal is the whole sound.
- *Slaughter '95* (At the Gates): an HM-2 fed **into** an MT-2, which is a chain the plugin
  could not express until the pedals became circuits -- one in the slot, the other behind
  it.

**Puppet Master '86** gained a Green 808 at the owner's request, marked in PRESETS.md as a
deliberate departure from the evidence: the producer says the rhythm tracks went straight
in. Measured, it tightens rather than distorts -- 18.9 % to 19.3 % distortion and the
low-to-high balance 7.2 to 6.8 dB -- for the reason he gave, that the amplifier is already
saturated.

Seventy-three presets.

## Next: two addenda, written up rather than started

Requested 2026-09-16: a **modern high-gain chain** (Revv G3 as Modern Purple, Fortin 33 as
Modern 33, Revv Generator 120 as Modern Generator with its own Modern 6L6 power stage, and
a Modern Oversized 4x12 reusing the Brit V30) and **bass** (Ampeg SVT, GK 800RB and
Darkglass Microtubes 900 with a power stage each; SansAmp Bass Driver DI, Darkglass B3K and
the Bass Big Muff; bass cabinets including an 8x10 array; and a DI tap).

Eleven models, every one of which needs its schematic-first checkpoint before any code, so
what exists today is the write-up: **[docs/ROADMAP.md](docs/ROADMAP.md)**, with the
requirements, the research each needs, the traps named in the requests (the Aggression
switch, Depth belonging in the feedback loop, Blend's phase, an 8x10 not being one speaker
times eight, "Microtubes" not meaning valve equations), the cross-combination tests, and
the eight devices the album presets are still waiting on. `docs/MODEL_INVENTORY.md`
carries them as PLANNED rows with their stable ids.

Two overlaps worth taking together: the **Ampeg VT** that *Desert Deaf '02* and
*Californicated '99* want is a close relation of the SVT work, and the **Sunn Model T**
that *Unknown Garden '94* wants is a large valve amplifier of the same era.

## Session 14 (2026-09-16): the level matching was measured on one tone

Reported as "everything using the HM-2 is way too loud", and it was -- but the HM-2 was
only the sharpest instance of a fault in the whole catalogue.

**The make-up was built from `gain_db()`: the gain of the *fundamental* of a single 220 Hz
sine.** That is not loudness, and the two part company as distortion rises -- at the HM-2's
88 % the fundamental is a minority of what comes out, so normalising it left the output far
too loud. Measured broadband, a catalogue that was nominally level matched spanned
**14.2 dB**, with the most distorted circuits ten decibels above the cleanest:

| | before | after |
|---|---|---|
| Boss MT-2 | +3.1 | -28.8 |
| Boss HM-2 | +1.0 | -29.5 |
| Twin Reverb | -1.6 | -26.9 |
| JCM800 2203 | -11.1 | -27.1 |
| **spread** | **14.2 dB** | **~4 dB** |

The make-up now normalises the **whole output**, measured on a low chord -- a root, a fifth
and an octave -- rather than one sine.

**Anchored, deliberately.** Correcting the metric fixes how the circuits sit against each
other, which is what level matching is for; on its own it would also move the catalogue's
absolute loudness and every session saved with it. One constant offset keeps the average
where it was: **+0.48 dB**, computed in the generator from the table compiled in at the
time, so it is exact and running the generator twice changes nothing the second time.

**Everything that measures level now measures the same thing**, which is how the fault hid
for so long: `examples/calibrate.rs`, `examples/powertrim.rs`, `examples/presetlevel.rs`,
`examples/loudness.rs` (now walking `Gain::ALL`, so a new voice cannot be added without its
level being seen), `tests/presets.rs`, `tests/power_trim.rs` and `tests/voice.rs` all use
the one chord. Two of those had been measuring against a different signal than the table
they were checking, which is why `power_trim` failed by a decibel with nothing wrong.

**Preset trims re-derived.** Five had been set against the old metric and were badly wrong
-- *Slaughter '95* had been trimmed **+16 dB** from a reading that was 23 dB out. The
catalogue now spans 13.8 dB broadband and the HM-2 presets sit in the middle of it rather
than at the top.

**The frozen fixture was re-baselined once**, deliberately, with the reason written into
its header beside the exception already recorded there. Every voice's level changed, so the
old capture could not survive the correction; it guards these four voices from here on
exactly as before. The rule is unchanged: if it stops matching and you did not mean to
change what the plugin does, the plugin is what is wrong.

**One test was wrong, not the plugin.** `the_make_up_holds_the_level_between_the_measured_points`
compared an output RMS against `NOMINAL_DBFS`, the chord's *peak* constant, and read every
voice seven decibels low -- the chord's crest factor, nothing to do with the make-up. It
compares against the input's own broadband level now. Measured properly, the make-up holds
the Clean voice within **0.13 dB** across the top five knots.

## 2026-09-20 Codex handoff audit: current solver and circuit checkpoint

This checkpoint supersedes the older handoff text for current work. The working
source arrived as `gainstagefx(8).zip`, based on git `aab1277` (`v0.18.0`) with
all recent solver experiments still uncommitted. The codebase was compared against
`gainstagefx(7)` plus the previously accepted/provisional solver patches so the
new Codex work could be separated from earlier work.

### Solver work present before Codex and retained

- accepted-trial nonlinear evaluation cache (current V3 form)
- reciprocal-pivot reduced LU
- boundary-major internal recovery
- fixed-size 13x13 reduced LU with the stale-pivot-plan regression fix
- precondensed 13x13 Schur stamp base
- Twin power phase profiler
- solver-control tail profiler
- provisional convergent-search release
- fixed-13 stamped-current-merit specialization, with a bit-for-bit helper test

The earlier accepted-search-merit-reuse experiment is **not present**: it was
replaced because direct physical trial merit and stamped current-point merit are
algebraically equal but can differ in floating-point operation order and change
search decisions.

### New Codex solver work

Codex added a second fixed-13 specialization for the *linear part of line-search
trial residuals*. `ReducedNonlinear::begin_residual()` now gathers the 13 boundary
voltages once and evaluates the cached 13x13 Schur rows without repeated global
boundary indexing. It preserves row/column accumulation order and has an A/B
switch:

`GAINSTAGEFX_TEST_DISABLE_FIXED_13_TRIAL_RESIDUAL=1`

The new `fixed_13_merit_and_trial_residual_preserve_mapped_cancellation_cases`
test exercises non-contiguous boundary mapping, mixed scales and cancellation and
requires bit-identical generic/fixed results. This optimization has **not yet been
benchmarked or accepted**. Do not stack another numerical-policy change on top of
it until the A/B below is complete.

Codex also added an FNV-1a hash of the complete Twin trace output outside the timed
callbacks and `tools/solver_ab.py`. The script builds or reuses one release test
binary, alternates A/B ordering, records machine/compiler/git metadata, rejects
unsettled solves, reports medians, and can require exact control-counter and output
hash equality. The Python script passes `py_compile`; Rust execution still needs a
machine with Cargo.

### Current performance objective and next validation

Primary fixture: 48 kHz, block 64, callback budget **1333.33 us**. Required target:

- CPU p99 < 1333 us
- CPU max < 1333 us
- zero CPU callback-budget misses
- zero unsettled solves

The fixed-13 stamped-merit specialization and the new fixed-13 trial-residual
specialization must be validated independently with repeated, unprofiled A/B runs.
For either fixed-size arithmetic specialization, control counters and output hash
should be identical between A and B. If they differ, investigate before acceptance.

Recommended sequence:

```bash
cargo test --release fixed_13_merit_and_trial_residual_preserve_mapped_cancellation_cases -- --nocapture
cargo test --release twin_power_four_backtracks_matches_full_reference -- --ignored --nocapture --test-threads=1

python3 tools/solver_ab.py \
  --output /tmp/gainstagefx-ab-stamped-merit \
  --repetitions 5 \
  --cpu <isolated-or-idle-cpu> \
  --disable-switch GAINSTAGEFX_TEST_DISABLE_FIXED_13_STAMPED_MERIT \
  --assert-control-equal

python3 tools/solver_ab.py \
  --output /tmp/gainstagefx-ab-trial-residual \
  --repetitions 5 \
  --cpu <same-cpu> \
  --disable-switch GAINSTAGEFX_TEST_DISABLE_FIXED_13_TRIAL_RESIDUAL \
  --assert-control-equal
```

If `--cpu` is not used, run on an otherwise idle machine and keep the same system
conditions for both experiments. Profiler runs are diagnostic only and are excluded
from timing medians by the script.

The convergent-search release remains **provisional**. After the two arithmetic
specializations are decided, repeat its A/B with:

`GAINSTAGEFX_TEST_DISABLE_CONVERGENT_SEARCH_RELEASE=1`

and at least five alternating timing pairs before final KEEP/REJECT.

### New Codex circuit/model work

Codex corrected direct Twin voice selection so the chain's stored Reverb and
Intensity defaults are synchronized into the unified Twin electrical netlist.
The regression `direct_twin_selection_uses_the_same_dry_defaults_as_settings`
compares direct selection with explicit dry `Settings` beyond the first spring
return. This fix is sensible but still needs Rust test execution in this handoff.

Codex also researched and implemented the April 1991 Boss MT-2 U2a/U2b middle EQ
as **an isolated reference circuit only** (`metal_zone::mid_eq_reference`). New
tests cover:

- boost/cut depth across the Mid Freq sweep
- near-flat center setting
- small-signal AC/time-domain agreement at 44.1/48/88.2/96/192 kHz

The production MT-2 still uses the approximate swept gyrator. The factory middle
stage has **not** been integrated into the shipping pedal, recalibrated, preset-
checked, or realtime-qualified. Next circuit work is to run the isolated-stage
tests, add headroom/state/loading validation, then integrate U2a/U2b with the real
low/high-EQ source impedance and Level load while preserving a deliberate legacy
compatibility strategy.

### Documentation/model audit from Codex

`DSP.md`, `MODELS.md`, `docs/MODEL_INVENTORY.md`, `docs/models/american_twin.md`
and `docs/models/metal_zone.md` were substantially updated. The inventory now
separates implemented from validated/performance-ready status and records the
major remaining model gaps. Important unfinished families include:

- MT-2 factory mid-EQ production integration; HM-2/MT-2 active-gyrator/device validation
- exact 5150 revision, loaded tone stack and resonance network
- DR103 feedback/presence coupling and IIC+ PI/power reconciliation
- DS-1 revision/device choice and Mesa Studio Preamp schematic acquisition
- Revv/Fortin/Generator modern chain
- SVT, GK800RB, Microtubes 900 and bass pedal/cabinet/DI families
- remaining speaker/microphone parameter/evidence gaps

No nonlinear family should be called performance-ready until complete-chain 48 kHz
/ 64-sample p99, max, misses and unsettled counts have been measured with profiling
disabled.

### Handoff quality / unfinished items

Codex stopped before completing the requested handoff:

- no KEEP/REJECT benchmark for fixed-13 stamped merit
- no KEEP/REJECT benchmark for fixed-13 trial residual
- no final KEEP/REJECT for convergent-search release
- no demonstration of p99/max below 1333.33 us
- no MT-2 production integration
- no 5150/DR103/IIC+/DS-1/Studio/modern/bass implementation from the updated roadmap
- `IMPLEMENTATION_PROGRESS.md` and `IMPLEMENTATION_RESUME.md` had not been updated
- all current changes remained uncommitted

This section records the actual continuation point. Preserve the generic/reference
paths and make one measured change at a time.

## Session 2026-09-22 -- solver measurement correction, Step 33, Windows preset path

Full detail in [`docs/SOLVER_OPTIMIZATION.md`](docs/SOLVER_OPTIMIZATION.md) and
[`docs/SOLVER_EXPERIMENTS.md`](docs/SOLVER_EXPERIMENTS.md); those two files are
now the place solver performance work is recorded.

**The phase profiler was measuring itself.** It timed with
`clock_gettime(CLOCK_THREAD_CPUTIME_ID)`, which is a real syscall on Linux and
costs 558 ns a call here, against phases of 100-400 ns. It now reads the
invariant TSC (8 ns). Every phase figure taken before this is unusable for
ranking work, including the `recovery_us ~1.30 s` that had made internal-node
recovery look like the next target: it is 0.22 s, 13 % of the profiled phases
and under 5 % of chain CPU.

**Where the time actually goes** (`perf`, whole Twin deterministic trace):
reduced dense linear algebra 43 %, libm transcendentals 19 %, device stamping
about 17 %. `examples/partition_survey` is new and prints the boundary/internal
dimensions, recovery-matrix density and elimination cost of every model.

**Step 33 accepted:** AVX (four-lane) instantiations of `solve_dense_planned`,
`solve_dense_planned_fixed_13_reciprocal` and `recover_boundary_major_in_place`,
selected by runtime CPU detection, with the scalar bodies kept for CPUs without
AVX. Bit-identical by construction and by test; the Twin hash
`ae73533fafbdafdb` and every solver counter are unchanged. Worth -1.61 % CPU
averaged over Cali IIC+, 5150 and Twin in five modes each, and -5.4 % deadline
misses on the Twin.

**Symbolic sparse reduced LU rejected:** exact, and 58-99 % fewer elimination
flops on paper, but 5-11 % slower in practice. These matrices are too small;
the dense kernel is overhead-bound, not flop-bound. Removed, with the
measurement kept.

**Wired up rather than left in a transcript:** the Twin hash is now an
assertion in `twin_realtime_recording_solver_trace` instead of a hex string a
human compares, and it arms itself only under the accepted configuration;
`tools/twin_trace.sh` carries that configuration so it cannot be mistyped;
`examples/partition_survey` prints the structural survey; and
`dsp::partition::tests` gained a bit-exactness test for the wide kernels and a
guard on the profiling clock's cost. All four were verified to fail when they
should.

**Open decision for the owner:** building the whole crate with
`-C target-cpu=x86-64-v3` is bit-exact (verified against the Twin hash) and
worth -2.8 % mean and -16 % deadline misses, at the cost of requiring AVX2 at
load time.

**Also fixed:** saved presets on Windows. `preset_dir()` used
`$XDG_CONFIG_HOME`/`$HOME`, which is a Unix convention and normally unset
there, so saving failed; a host started from a Unix-style shell was worse,
writing presets somewhere no Windows DAW session would look again. Windows now
uses `%APPDATA%\GainStageFx\Presets`, with `%USERPROFILE%` as a fallback. Linux
and macOS are untouched. Verified against the `x86_64-pc-windows-gnu` target.

## Session 2026-09-22 (cont.) -- the JC-120's CH-1, settled by a better scan

The previous entry left the Jazz 120 blocked on one question: whether CH-1's
tone network was the passive stack it was modelled as, or an active one inside
a feedback loop. The 200 ppi A3 scan of the Fifth Edition could not answer it,
and the fitting harness had proved -- correctly -- that the modelled topology
was wrong, because each of the three published control ranges could be reached
alone and none of them together.

**A search for better schematics found four documents, and the answer was not
the one on the table.** `Roland JC-120.zip` on schematicsforfree.com carries the
**JC-120UT/JT Service Notes of Sep. 2000** -- the current-production amplifier --
whose two schematic pages are **1334 and 1305 ppi**, against 200 for the sheet in
hand. Also recovered: the Third Edition at 600 ppi, and the factory sheet for
serials 380100-410599. All four are in the git-ignored `docs/schematics/`.

**CH-1 is a different circuit in every generation of this amplifier.** Four of
them, with four different tone networks. The 250 k / 250 k / 10 k pot values on
the Fifth Edition's *parts list* belong to the amplifier that edition covers;
the 17 / 13 / 14 dB control ranges on its *specification page* describe the
50 k / 100 k / 5 k network of the generation it supersedes, carried forward
wholesale as Roland's service notes do. Fitting one to the other could only ever
fail, which is exactly what it did. **A specification page and a schematic page
in the same service manual need not describe the same circuit.**

**`circuits::jazz120` is rebuilt against the 2000 sheet** and is now the
current-production JC-120's CH-1, jacks to CN3, 25 unknowns. J3 a 2SK184 in
common source with a 15 k drain load -- not the source follower the older sheet
seemed to show -- then the passive stack, the 1 M Volume with its bright
network, a second 2SK184 as the gain stage and a 2SC1815 follower out.

The stack is a Fender FMV with one structural difference that no amount of
value-fitting could have found: **the slope resistor feeds two caps into two
entry points**, C13 .082 into the Treble/Bass junction and C12 .056 into the
Middle pot's top, past R19. R19, the connection the whole question had come down
to, is a plain 1 k in the ladder between Bass and Middle. The stack's bottom
returns to ground; there is no feedback path anywhere near it.

**Nothing is fitted any more, and the sheet checks itself.** Its printed test
points are -30 dBm at the jack, -10 dBm at J3's drain and +13 dBm at CN3 under a
stated `VOL, EQ MAX / BRI OFF`, so the amplifier's own gain is documented:

| check | published | model |
|---|---|---|
| jack to J3's drain, 1 kHz | 20 dB | 19.0 dB |
| jack to CN3 | 43 dB | 43.7 dB |
| input impedance | 680 kohm | 5.7 dB into a 680 k source |
| +VA (MTZ-30B zener) | 30 V | +VO 27.2 V after R20's 680 ohm |

Both gains land within a decibel with the JFET model being nothing but the two
numbers on Toshiba's sheet. `JfetSpec::J2SK184` is new;
`JfetSpec::J2SK117_GR`'s comment no longer claims to be the JC-120's input JFET.

**Wired up rather than left in a transcript:** `tests/jazz120.rs`, nine tests --
the two test-point gains, the operating point and its headroom for the sheet's
9.79 Vpp, the published input impedance, the two jacks, that the tone network is
passive at every setting and every frequency (the test that would have caught
the original error), that each control moves its own band the right way, the
Bright switch, and rate independence at all five rates. `examples/jc120_fit` is
now `examples/jc120` and checks rather than searches.

Two orientation bugs fixed on the way: the Bass and Middle pots are rheostats
with the wiper strapped to terminal 3, so they need `ReverseAudio` and
`ReverseLinear` -- with a forward taper both controls ran backwards, which is
`CLAUDE.md`'s failure mode 5 exactly.

## Session 2026-09-22 (cont.) -- the Jazz 120 on the panel

`Circuit::Jazz120` (`amp_roland_jc120`, display **Jazz 120**) appended to the
circuit list, `Gain::Jazz120` appended to `Gain::ALL` behind it. Drive turns the
amplifier's own Volume and the panel says `VOLUME`, as it does on the two
Fenders; Bass, Middle and Treble drive CH-1's own stack through `own_tone`. One
factory preset, **Jazz Clean**.

**The first voice in the catalogue with no power stage.** `Matched` resolves to
nothing and the channel hands straight over to the tone section. That is the
honest state until the JC-120's two 60 W transistor amplifiers are built, rather
than borrowing a valve stage the amplifier has not got.

**The Bright and sensitivity selectors are no longer the Twin's alone.** They
were gated on `Circuit::Twin` and carried its labels as literals in the editor,
so the American Deluxe -- which has the same switched jacks -- never got them
either. `Gain::input_jacks` and `Gain::bright_switch` now carry the slots, the
values *and* the labels, and the panel reads whichever circuit is selected:
High/Low with Roland's own -30 and -20 dBm on the Jazz 120, the AB763's
1 MOhm / -6 dB on the two Fenders. The two are gated separately, because the
Deluxe has the jacks but its Bright capacitor is soldered in.

`src/calibration.rs` and `src/power_trim.rs` regenerated -- both are `Gain::ALL`
shaped, so a new voice is a new row in each. The JC-120 calibrates at a guitar's
0.122 V, like every other amplifier here, and makes 10.8 % at the Volume stop:
43 dB of gain with no valve compression behind it does run out of room, and it
does so abruptly, which is what the amplifier is known for.

**Also fixed, because `examples/presetlevel` named it:** the two American Deluxe
presets shipped with no `output_trim` and were the two loudest things in the
catalogue -- Deluxe Breakup at +2.4 dB against a -7.2 dB mean, which put the
spread at 16.2 dB and failed `the_presets_are_level_matched`. Both now carry a
measured trim (-7 and -10), and Jazz Clean carries +4 for the opposite reason:
no power stage behind it, so it loses what every valve amplifier here gains.
All three sit within half a decibel of the mean.

**And one test fixed rather than worked around:**
`every_control_is_either_reachable_or_given_a_resting_position` hard-coded
`twin::REVERB` and `twin::INTENSITY` as the reachable pair for every voice with
a tank and a tremolo. The Twin has a Middle control and the Deluxe has not, so
the Deluxe's numbers are one lower and its Reverb looked unreachable -- the test
had been failing on the Deluxe since that voice was added. It now asks each
voice for its own numbers through `Gain::ab763`, which is what keeps it true
when a third AB763 arrives.

`tests/tone_knobs.rs` needed the same kind of correction: its `OWN` table is the
panel's claim about which circuits carry their own tone controls, and the
American Deluxe had never been added to it, so a test asserting the Deluxe had
no tone control was failing against a Deluxe that has two. Both it and the
Jazz 120 are in the table now -- Deluxe `[true, false, true]`, Jazz 120 all
three.

## Session 2026-09-22 (cont.) -- the Deluxe's leftovers

Two items the American Deluxe's research log listed as unbuilt, both now done.

**The cabinet and speaker choice.** The box was already there; its
`default_speaker` was wrong. `CabinetProfile::AMERICAN_OPEN_112` resolved
`Matched` to the *alnico* Jensen P12R, which is not a blackface Deluxe part at
all -- the amplifier shipped with a ceramic C12N or an Oxford 12K5, and the
Twin's own AB763 cabinet in this repository already defaults to the C12N. The
1x12 now does the same, both Deluxe presets follow it, and the '65 reissue's
C12K stays excluded as the later part it is.

**The Normal channel, and a missing triode.** The drawing has two channels and
only one was built. The Normal channel's *second* stage was present -- it shares
the [A] cathode and loads the mixer -- but its first stage, its stack and its
Volume were not, so a triode's worth of standing current was missing from the
dropper chain that feeds every plate in the amplifier. Against the drawing's own
printed voltages:

| node | before | now | drawing |
|---|---:|---:|---:|
| `v1_p` | 193.4 V (+13.8 %) | **186.1 V (+9.5 %)** | 170 V |
| `v2_p` | 197.3 V (+9.6 %) | **189.9 V (+5.5 %)** | 180 V |

Nothing was tuned for that. It is the current of a stage that should always
have been there, and it is the second time on this amplifier that a plate
sitting high turned out to be a missing load rather than a wrong resistor --
the first was the phase inverter's, which became `PI_STANDING_LOAD`.

Both channels are now built by one function, `deluxe::front_end`, because on the
drawing they *are* one circuit twice: same 68 k jacks, same 100 kΩ plate, same
1.5 kΩ / 25 µF cathode, same stack, same 1 MΩ Volume. They differ in exactly two
things, and the sheet is unambiguous about both -- the Vibrato channel carries
the 47 pF across its Volume and the Normal channel does not, and the reverb and
the tremolo hang on the Vibrato channel's second stage, joining the Normal
channel only at the mixer, *after* the intensity tap.

So **the Normal channel has no reverb and no tremolo**, which is what the
amplifier does, and those two controls rest rather than appearing as greyed
knobs. `deluxe::Channel` says which channel the jacks feed; the other stays in
the network with its jacks open -- grid returned through the same 34 kΩ + 1 MΩ
the High jack presents, because with no plug in it that is the same network --
and its Volume resting at zero.

Selectable as `Circuit::DeluxeNormal` (`amp_fender_deluxe_ab763_normal`,
display **American Deluxe Normal**), matched to the same `PowerSpec::DELUXE_6V6`,
with one preset, *Blackface Normal*. The circuit now declares eight controls:
the five the panel reaches and the quiet channel's three, which `tests/deluxe.rs`
asserts so that a ninth would still mean a Middle pot had appeared.

## Session 2026-09-22 (cont.) -- the JC-120's VAS, and three Windows reports

**The one node the power amplifier was waiting on is read.** Rendered at 260 %
of the native 1334 ppi scan, **there is no junction dot** where the R76/R75
vertical crosses the output line -- the vertical is the same width above and
below it, against four unmistakable filled junctions in the same crop. So the
string R78 1.5 k / R82 6.8 k / R76 3.9 k / R75 6.8 k runs from +VE down to the
`Vbe` multiplier *past* the output without touching it, and C43's 47 µF is the
only path between them. That makes it a **bootstrap**: the VAS's collector load
is held a constant voltage above the output and behaves as a current source. A
divider across the output would have done the opposite, and the two are not
worth confusing. `docs/models/jazz_120.md` carries the diagram; nothing else in
that amplifier is unread now.

### Three reports from a Windows user, and one cause behind two of them

**"There is no cursor in the preset-name box."** Two separate faults, and the
first guess at it was wrong -- recorded because the thing that corrected it was
one sentence from the person who reported it.

The dialog built its `Textbox` and never focused it, so the box came up not
editing at all. That was fixed with `.on_build(|cx| cx.focus())`, and it is a
real fix: on a host that does not hand the plugin window keyboard focus by
itself, there was otherwise nothing to type into. But it was not the reported
symptom, and the report said so: *"if I click somewhere in the middle of the
text and hit delete it deletes there where I clicked."* A box that deletes the
character you clicked in front of is editing, and its caret is tracking. The
caret was not missing. It was **invisible**.

`caret_color` is a style property, and an unset colour property in vizia reads
back as `rgba(0, 0, 0, 0)` -- so the caret was drawn on every frame in
transparent black, and the selection highlight with it. vizia's own themes set
both, but this editor is created with `ViziaTheming::None` precisely so that
nothing arrives looking like a default toolkit, and the one sheet it does add
was about the scrollbar, which had gone invisible for exactly the same reason.
The caret rules now live in that sheet beside it, rather than being set on the
view, because vizia blinks the caret by toggling a `caret` class and a rule is
what that class has to act on -- inline would have made it visible and left it
staring.

**"Presets do not save and no `GainStageFx\Presets` folder appears."** follows
from the first fault rather than from the path code: with nothing focused there
was no name, and `presets::save` rejects an empty name *before* it calls
`create_dir_all`, so nothing was written and no folder ever appeared.

The path itself was never wrong, and `tests/store.rs` now proves it rather than
asserting it: it checks the preset directory does **not** exist before the first
save and **is** a directory after it, so "saving creates the folder" is a test
rather than a reading of the source.

**Two Windows-only hazards found while there**, both now covered by unit tests
that run on any platform:

* `preset_dir`'s Windows rule was behind `#[cfg(windows)]`, so it was only ever
  compiled on the platform nobody here develops on. Both rules are now selected
  with `cfg!` and take their environment variables as arguments, which makes
  them pure functions with tests.
* `file_stem` mapped the forbidden *characters* but not the reserved *names*.
  `CON`, `AUX`, `NUL`, `COM1`..`LPT9` are devices on Windows, not files -- Win32
  resolves them before the filesystem sees them and the `.json` does not help --
  so a preset called `Aux` could not be saved there. Those stems now get an
  underscore. Windows' other rule, that a trailing dot or space is stripped
  silently, needs no code: `trim` takes the spaces and the character map has
  already turned the dot into an underscore. A trim added for it was removed
  once a test showed it could never fire.

**Two tests on the stylesheet**, because nothing else would ever report it.
`Context::add_stylesheet` returns `Ok(())` whatever happens and the parse
behind it is `if let Ok(..)`, so one missing semicolon does not break one rule:
it silently discards **the whole sheet**. Both things in that sheet are
invisible when they are missing rather than wrong -- a scrollbar with no width,
a caret drawn in transparent black -- and nobody would see which had gone.
vizia imports its own parser privately so the test cannot call it; it checks
the structure, which is what a typo breaks, and that some `caret-color` in the
sheet is not transparent. Both were verified to fail when the rule is removed
and when a semicolon is dropped.

**The CLAP install folder is correct and was not changed.** `%COMMONPROGRAMFILES%\CLAP`
and `%LOCALAPPDATA%\Programs\Common\CLAP` are the two locations `clap/entry.h`
names, and the VST3 pair matches Steinberg's. What was wrong was the **Windows
readme**: it documented a flat copy while `install.exe` uses a `BurningTreeC`
subfolder, which is what the Linux readme has always said about its own
installer. Someone checking where the plugin went would have looked in
`Common Files\CLAP`, found only a folder, and reasonably concluded it had gone
somewhere else. The readme now states both paths in full, says hosts search
subfolders, and notes that `install.exe` prints where it put each file.

**The JC-120's power amplifier: attempted, wrong, withdrawn.** With the
bootstrap settled the netlist was written -- and it took Q12 and Q14 for a
current mirror on the strength of them being a complementary pair drawn side by
side, which is an inference, not a reading. The DC solve rejected it in one
run: both differential-pair collectors within a volt of the negative rail.

A proper look says Q12 is not in the signal path at all. Its emitter is on the
bootstrapped node, its base on the R79/R76 divider and its collector on Q14's
base -- an overcurrent limiter that stays off until the drop across R79 reaches
a `Vbe`. Q14 is the voltage amplifier stage. The pair drives Q14's base and Q12
clamps it, which is the opposite of what was built.

`src/circuits/jc120_power.rs` was **removed rather than left in the tree**. A
power amplifier that solves to a wrong operating point is worse than none,
because the next thing that happens to it is being wired up. What remains
unread is one node -- where Q14's emitter returns, and how the `Vbe` multiplier
and the two drivers sit across it -- and it wants a dedicated pass rather than
a fourth inference. The device specifications read off the parts list
(`PNP_2SA970_GR`, `PNP_2SA1015_GR`, `NPN_2SD669_AC`, `PNP_2SB649_AC`,
`NPN_2SC4386`, `PNP_2SA1671`) are kept: those are readings, and they are what
the rebuild will use.

**So the junctions are measured now, not judged.** Two wrong netlists in a row
came from the same thing -- deciding by eye whether a crossing carried a
junction dot -- so `tools/schematic/trace.py` settles it in the pixels instead.
It measures the stroke width, separates dots from wires by morphology, joins
wires into electrical nodes with union-find, and draws the result back over the
scan in colour so the reconstruction can be checked rather than trusted.

The rule it encodes is the one that matters: a dot joins everything passing
through it, and a wire that *ends* on another is joined whether or not a dot is
drawn, because a corner and a T are connections by construction -- you cannot
cross and stop. Only a crossing where both wires continue needs the dot.

On the JC-120's power amplifier it finds 51 junction dots and resolves the
board into nine clean nodes, and the two things that were in doubt fall out of
it: **C43 joins the R78/R82 junction to the output and nothing else does** --
the bootstrap, measured -- and **R88 joins the output to the inverting base**,
which is where the gain calculation said the feedback resistor was.

It found its own limits too, and they are written down: connector boxes are
rectangles whose edges read as wires, and shorted every row of W8 together
until the crop excluded them. That is why the overlay exists.

What remains on that amplifier is no longer topology. It is which terminal of
each transistor sits on which of those nodes -- a reading against a known node
list rather than against a picture. The netlist gets written from the list.
