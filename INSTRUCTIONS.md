# When a circuit changes or is added

A checklist for any change to a netlist in `src/circuits/` — a new circuit, a
corrected value, a rewired stage — and for a new pedal, preamplifier or power
stage. The reasons behind each rule are in `CLAUDE.md`; this file is the order
to do things in.

Build and run everything with `--release`. A debug solver is slow enough that
tests look hung.

## 1. Before any code (a new model, or a correction to one)

- Write or update the research log `docs/models/<model>.md`. It needs the
  eight-question checkpoint: revision; whether the original schematic was
  found; best source; cross-check; signal path and values; what is modelled
  exactly; what is approximated and why; why. If question 2 fails, stop
  there, as `docs/models/studio_pre.md` does. Do not build from a near
  relative.
- Label every value's evidence, in the log and in the code comment that
  carries it: DOCUMENTED, WIDELY REPORTED, PLAUSIBLE, APPROXIMATED or
  ESTIMATED.
- Manufacturer drawings go in `docs/schematics/`, which is git-ignored. Never
  commit them.
- For junctions that are not obvious on a scan, run
  `uv run tools/schematic/trace.py SHEET.png X Y W H --overlay` before writing
  the netlist.

## 2. Build the circuit

- One function in `src/circuits/<name>.rs` returning a `Netlist`, with the
  pot positions exported as `pub const` indices.
- Probe it with `tap()` node by node before writing tests. A stage that reads
  40 dB below its neighbour localises a wiring error at once.
- Check the known failure modes in `CLAUDE.md` → *Working on a circuit*:
  shunt-feedback transistor stages, gyrators, op-amp sections on a 4.5 V bias,
  pot direction, and filter centres derived from the values.
- A circuit with a chorus exposes it as a Tone-section knob.

## 3. Register it (new circuits only)

- Append the variant to the voice enum in `src/voice.rs` (`Gain`, `Pedal`,
  `PowerModel`, …) and to its `ALL` array. Append it to the matching host
  enum in `src/params.rs` (`Circuit`, `PedalModel`, `PowerAmp`, …) and extend
  that enum's `voice()`.
- **Append only.** Enum ids are never renamed, and their declaration order
  is load-bearing: saved presets and automation store normalised values.
  Give anything new a default in `migrate()` in `src/presets.rs`.
- Keep the display name generic. The mapping to real hardware goes in the
  README's *What each name is* table, `docs/MODEL_INVENTORY.md` and the
  research log.
- For a pedal, decide `Pedal::is_expensive`. An expensive pedal forces the
  chain to 1x. Modelled voices are capped at `MODELLED_MAX_OVERSAMPLING`.

## 4. Regenerate the generated tables, in this order

`src/calibration.rs`, `src/power_trim.rs`, `src/direct_out.rs` and
`src/dsp/partition/kernels.rs` are generated. Edit the generator, never the
table. The order matters: power
trim is measured at the calibrated drive, and the kernels are counted on the
presets as they then play.

1. **Calibration**, when a gain circuit changed or was added. Generate into a
   temporary file first: the shell truncates the target before Cargo compiles
   the example.
   ```
   cargo run --release --example calibrate > /tmp/calibration.rs
   mv /tmp/calibration.rs src/calibration.rs
   ```
   `tests/voice.rs` → `the_calibration_table_still_describes_the_circuits`
   fails while this is stale.
2. **Power trim**, when a power stage or a gain voice changed, after step 1:
   ```
   cargo run --release --example powertrim > /tmp/power_trim.rs
   mv /tmp/power_trim.rs src/power_trim.rs
   ```
3. **Direct outs**, when a circuit with its own direct out ahead of its last
   stage (`Gain::direct_out`, the 800RB's) changed or was added: the circuit's
   gain to that node at each drive knot, which levels its Preamp DI.
   ```
   cargo run --release --example directout > /tmp/direct_out.rs
   mv /tmp/direct_out.rs src/direct_out.rs
   ```
   The circuit's own test re-measures three knots and fails while it is stale
   (`tests/american_800rb.rs`).
4. **Compiled solver kernels**, last, once the circuit is final:
   ```
   cargo run --release --example kernels            # writes the file itself, ~8 min
   cargo run --release --example kernels -- --check # preview only
   ```
   A stale kernel table is never wrong, only slower: a kernel that no longer
   matches a circuit's pattern or pivot plan is not used. So skipping this
   step silently costs CPU. Rerun it after any netlist change.

## 5. Levels

- **Pedal:** `cargo run --release --example pedallevel`. A pedal's `.rest()` is
  unity through that pedal, so its Level knob at noon is unity.
- **Presets that use the circuit:** run
  `cargo run --release --example presetlevel -- "<name fragment>"` before and
  after, and move each preset's `output_trim` by the difference.
  `tests/presets.rs` holds the catalogue's spread.

## 6. The frozen baselines

- `tests/corrected_baseline.rs` (Mark IIC+, 5150, Twin, 73P) and
  `tests/legacy_baseline.rs` are frozen. **Never regenerate either to make a
  change pass.**
- If one of those voices' output must change, record the exception in that
  file's header. A new capture is a new file, and the owner's decision.
- The part-creation order inside `src/circuits/power.rs` is load-bearing for
  both.

## 7. Tests and measurements

- A test file for the circuit (`tests/<model>.rs`) checking what the research
  log documents: published gains, levels and filter centres, derived from the
  values rather than read back from the model.
- Anything touching the solver is tested at **44.1, 48, 88.2, 96 and
  192 kHz**.
- New routing on the audio path is covered by `assert_no_heap`
  (`tests/support/allocations.rs`).
- The catalogue-wide targets pick up a new voice automatically. Run them
  targeted, not the whole suite:
  ```
  cargo nextest run --release --lib --test voice --test pipeline --test knobs \
      --test devices --test runtime_state --test power_trim --test pedal_slot \
      --test presets --test corrected_baseline --test legacy_baseline
  ```
  `--lib` includes the check that every reduced matrix stays inside the
  pattern the kernels rely on (`the_guaranteed_pattern_holds_across_the_catalogue`).
  `tests/pipeline.rs` checks that the pipelined chain is still bit-identical
  to the serial one for every voice, and that the power stage's speculation
  (`SpecBlock` in `voice.rs`) gives the same bits on every block path. A new
  power stage gets a shadow automatically; nothing to add. Whether a chain
  speculates depends on its preamp's cost next to its power stage's
  (`SPECULATE_BELOW`), estimated from circuit size; `rt_scenario --block`
  prints the estimate (`front/power work`). A new circuit far off the fit
  (`hard_samples::pass_cost_by_size`) can land on the wrong side of it:
  `uv run tools/preset_ab.py --flag=--no-speculation --swap --preset "<preset>"`
  times it both ways, A-B-B-A, and reports that preset's own change.
- The per-sample `Chain::process` does not speculate, so on a block that did,
  `process_block` agrees with it to the solver's tolerance rather than the bit.
  A test comparing the two to the bit either uses blocks that never speculate
  or calls `Chain::set_speculation(false)`, as the plugin's dual-mono wake test
  does.
- **Solver health and cost** on the real take:
  ```
  cargo run --release --example rt_scenario -- --preset "<preset>" --events
  cargo run --release --example rt_scenario -- --preset "<preset>" --pipeline --stages \
      --callbacks /tmp/cb.csv && python3 tools/callback_tail.py /tmp/cb.csv
  ```
  Look at unsettled samples, fallbacks, half-step rescues, passes per solve,
  and the pipelined p99/p99.9 against the ~930 µs live budget. For a change
  meant to make things faster, `tools/preset_ab.py` gives every preset's own
  before-and-after; a median over the catalogue can hide one that got worse.
  `presetcost` and `modelcost` give the catalogue view.
- For a change meant to be exact (a refactor or an optimisation), compare
  output hashes across every preset:
  `cargo run --release --example rt_scenario -- --all --seconds 19`, and
  again with `--pipeline`, which is the path that speculates.

## 8. Before calling it done

- `cargo fmt --check`
- `cargo clippy --release --workspace --all-targets -- -D warnings` (CI treats
  warnings as errors)
- Update the documentation in the same change: README (the circuit list and
  *What each name is*), the research log, `docs/MODEL_INVENTORY.md`, and a
  session entry in `IMPLEMENTATION_PROGRESS.md`.
- CI runs `cargo test --release`, then clippy, then
  `python3 tools/third-party-notices.py` (which only changes when
  dependencies do).
