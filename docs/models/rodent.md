# Rodent (Pro Co RAT, LM308) research log

Engineering identity: **Pro Co RAT**, LM308 version (the op-amp the pedal is known for).
Stable id `pedal_rat` (pedal slot). Display: Rodent.

Status: **IMPLEMENTED 2026-09-15** (`src/circuits/rodent.rs`), tested in `tests/rodent.rs`
and `tests/pedal_slot.rs`.

## Eight-question checkpoint

1. **Revision.** A RAT with the National LM308N and 30 pF compensation, 1N914 clipping
   diodes to ground and a 2N5458 JFET output follower. Not the RAT2 with the OP07 op-amp,
   the Turbo RAT (LEDs) or the You Dirty RAT.
   - Coda Effects confirms the LM308 in the 1985 "White Face" units.
2. **Original schematic found?** No Pro Co drawing.
   - Used: ElectroSmash's "Pro Co Rat Analysis" schematic and parts list (the site was
     unreachable; retrieved from the backup repository
     [mstratman/electrosmash-pdf-backups](https://github.com/mstratman/electrosmash-pdf-backups)
     via `gh api`).
3. **Best source.** The ElectroSmash analysis. It carries the LM308 data sheet curves it
   reasons from: open-loop frequency response and voltage-follower pulse response.
4. **Cross-check.**
   - National LM308 data sheet curves (as reproduced): 67 dB at 500 Hz with Cf = 30 pF,
     i.e. a gain-bandwidth near 1.1 MHz; slew about 0.3 V/µs; about 108 dB at DC.
   - The analysis: "At 10KHz ... no more than 30dB" at full Distortion. The GBW line gives
     about 41 dB; the model gives 38 dB.
5. **Signal path and values.**

```text
supply: R10 47R from the battery, C11 100uF, C12 .1uF; R11/R12 100k bias divider, C13 1uF
input:  R1 1M to gnd; C1 22nF; R2 1M to Vref; R3 1k; C2 1nF to gnd -> LM308 +in
gain:   R DISTORTION 100k audio (rheostat) out -> -in, C4 100pF across it
        R4 47R + C5 2.2uF and R5 560R + C6 4.7uF from -in to gnd
        gain 1 + Rdist / (R4 || R5), 0 to 67 dB
clip:   C7 4.7uF; R6 1k; D1/D2 1N914 to gnd
filter: R TONE 100k audio (rheostat); R7 1.5k; C8 3.3nF to gnd (32 kHz down to 475 Hz)
output: C9 22nF; R8 1M; Q1 2N5458 follower, R9 10k; C10 1uF; R VOLUME 100k audio
```

6. **Exactly modeled.** Every part above, with the two gain-leg high-passes and C4.
7. **Approximated and estimated.**
   - **LM308.** A saturating transconductance into an integrator (GBW and slew from the
     curves), a resistor for the finite open-loop gain, diode clamps for the output swing
     on a 9 V battery (about ±3.5 V around the bias point), and an ideal follower.
     Input bias current, offset and output impedance are not modelled.
   - 2N5458: IDSS 4 mA, pinch-off −2.5 V, ESTIMATED inside the data sheet's ranges (a
     follower hardly depends on either).
   - The panel's Tone knob turns FILTER the other way, so the knob brightens clockwise
     like every other tone control.
   - Battery resistance: R10's 47 ohm stands in for it.
8. **Why.** The LM308 has no transistor-level model in the solver, and its character
   is its speed. An ideal op-amp would make a different, fizzier pedal.

## Measured

- Operating point: bias 4.49 V, op-amp output 4.49 V, JFET source 1.95 V.
- Small signal at the op-amp output, full Distortion: 51.5 dB at 200 Hz (R5/C6 region),
  63.3 dB at 1 kHz, 50.8 dB at 3 kHz, 38.1 dB at 10 kHz (ideal: 67 dB).
- Whole pedal at 0.122 V, 220 Hz: THD 0.1 % at Distortion 0, 39 % at 0.5, 45 % at 1.

## Correction 2026-09-15: the op-amp froze when Distortion came down

**Owner report.** The Rodent in front of a heavily distorted Cali IIC+ "crackles and
dropouts heavily".

**Reproduced** (`tests/rodent.rs::distortion_can_be_turned_down_while_playing`).
- Distortion was pulled from high to near zero while a note rang, and every sample then
  failed to converge (64 passes, thousands of fallbacks per second).
- The solver keeps the reactances on the last good answer when a solve fails. The pedal
  froze, went silent or crackled, and cost about twenty times its CPU until the knob
  passed 0.02.
- At rest nothing showed, which is why the first probes were clean.

**Cause.**
- At low Distortion the op-amp runs near unity gain, and its loop pole is near 1 MHz,
  far above the sample rate. A disturbance drives the transconductor into saturation.
- The transconductor was linearised on its tangent. A saturated tanh's tangent is flat, so
  each Newton pass asked for volts of correction and landed saturated the other way.
- Two smaller faults fed it:
  - The integrator and clamps were referenced to the 4.5 V bias divider, so every clipped
    edge moved Vref by a few hundred millivolts, and R2 carried that onto the input.
  - The output swing was a two-state rail switch inside the same loop.

**Fix.**
- `Transconductor::stamp` uses the chord through the origin once saturated
  (`|gm·vd/limit| ≥ 1`). The chord cannot carry the input past the point where the current
  changes sign. The converged answer is unchanged.
- `lm308` is ground-referenced, with diode clamps at the swing limits and a linear
  follower.

**Result.**
- The block-by-block sweep went from 2,943 unsettled solves and 29,095 fallbacks to 2
  and 0.
- The plugin-style chain with Distortion swept in front of Puppet Master '86: 1 unsettled
  solve, worst block 76 % of its deadline (was 394 %).
- The slew and gain-bandwidth test still passes.
