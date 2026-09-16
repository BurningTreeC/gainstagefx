# Round Fuzz (Arbiter Fuzz Face, germanium) research log

Engineering identity: **Arbiter Fuzz Face**, germanium version (1966-68). Stable id
`pedal_fuzz_face` (pedal slot). Display: Round Fuzz.

Status: **IMPLEMENTED 2026-09-15** (`src/circuits/round_fuzz.rs`), tested in
`tests/round_fuzz.rs` and `tests/pedal_slot.rs`.

## Eight-question checkpoint

1. **Revision.** The positive-ground PNP germanium unit: two transistors, 1 k Fuzz,
   500 k Volume. Not the silicon BC108/BC183 versions (negative ground, different bias),
   and not modern reissues.
2. **Original schematic found?** No Arbiter drawing.
   - Used: ElectroSmash's "Fuzz Face Analysis" schematic and parts list, from the backup
     repository [mstratman/electrosmash-pdf-backups](https://github.com/mstratman/electrosmash-pdf-backups).
   - The same analysis publishes AC128 SPICE models (GERPNP_LOWGAIN β 85,
     GERPNP_HIGHGAIN β 120).
3. **Best source.** The ElectroSmash analysis, for the circuit and transistor models.
4. **Cross-check.** The fuzzboxes.org survey of surviving units.
   - **Conflicts recorded:**
     - fuzzboxes.org reports NKT275 transistors on the early units with C1 2.5 µF and C2
       20 µF;
     - ElectroSmash lists AC128 and C1 2.2 µF.
   - The model uses the surviving units' capacitors and the AC128 models, the only
     published germanium models with both gains.
5. **Signal path and values.**

```text
battery -9 V (positive ground)
in -- C1 2.5uF -- Q1 base; Q1 emitter gnd; R1 33k from -9 V to Q1 collector
Q2 base = Q1 collector; R2 470R -9 V -> tap; R3 8.2k tap -> Q2 collector
R4 100k Q2 emitter -> Q1 base (DC bias and AC feedback)
FUZZ 1k linear from Q2 emitter to gnd, wiper -> C2 20uF to gnd
tap -- C3 .01uF -- VOLUME 500k audio -> out
```

6. **Exactly modeled.** Every part above. Both transistors are PNP Ebers-Moll with the
   models' IS, BF, BR and VAF.
7. **Approximated and estimated.**
   - The SPICE models' leakage terms (ISE/ISC), base resistance and junction capacitances
     are not in the solver's bipolar device. Germanium leakage is part of why real units
     vary, so this is a "good one", not an average.
   - Battery internal resistance 20 ohm, ESTIMATED. The analysis says it matters but gives
     no figure.
   - No tone control; the panel greys the Tone knob.
8. **Why.** The solver's bipolar device has no leakage or charge terms.

## Solver work this needed

- `Part::Bipolar { pnp }`: a mirrored Ebers-Moll junction pair (stamps e→c and e→b).
  Before this there was no PNP device.

## Measured (`examples/pedals.rs`)

- Operating point, not adjusted: Q1 collector −0.708 V, Q2 collector −4.714 V, Q2 emitter
  −0.494 V, Q1 base −0.206 V. That is where a well-set Fuzz Face sits.
- Small signal at 1 kHz: +4.2 dB at Fuzz 0, +10.0 dB at 0.5, +26.8 dB at 1.
- At 0.122 V, 220 Hz: THD 29 % / 39 % / 49 %. Small signals clip one side first, so the
  2nd harmonic leads.
- Solver: at full Fuzz, 2-10 fallbacks in 4 seconds of hard playing, none unsettled.
