# Yellow Dist (MXR Distortion+) research log

Engineering identity: **MXR Distortion+**, the 741 version. Stable id `pedal_mxr_dist_plus`
(pedal slot). Display: Yellow Dist.

Status: **IMPLEMENTED 2026-09-16** (`src/circuits/distortion_plus.rs`), tested in
`tests/distortion_plus.rs`.

## Eight-question checkpoint

1. **Revision.** The 741 pedal with germanium clipping diodes to ground, two knobs
   (Distortion and Output). Not the later reissues with silicon diodes, and not the
   Distortion III.
2. **Original schematic found?** No MXR drawing.
   - Used: [ElectroSmash's "MXR Distortion+ Circuit Analysis"](https://www.electrosmash.com/mxr-distortion-plus-analysis)
     (mirror: `electrosmash.mas-effects.com`), which gives every value with reference
     designators and works the gain and clipping out from them.
3. **Best source.** That analysis.
4. **Cross-check.** Its own arithmetic, which the model is measured against:
   - "Gv(max) = 1 + (1M / (4.7K + 0)) = 213 (46.5 dB)";
   - "Gv(min) = 1 + (1M / (4.7K + 1M)) = 1.5 (3.5 dB)";
   - the diodes clip "any signal bigger than 350mV";
   - corners: 23.5 Hz at the input, 720 Hz in the feedback leg at full gain, 15.9 kHz
     across the diodes, and a "mid hump around 1.5KHz".
5. **Signal path and values.**

```text
in -- R1 10k -- C1 1nF to ground -- C2 10nF -- op-amp +in, R2 1M to the 4.5 V bias
741 non-inverting: R4 1M from out to -in; -in through R3 4k7 and RV1 1M (Distortion,
     a rheostat: turned up it takes resistance *out*) into C3 47nF to ground
out -- C4 1uF -- R5 10k -- D1/D2 1N270 germanium back to back to ground, C5 1nF across
     -- RV2 10k (Output) -- out
supply: R6/R7 1M each and C6 1uF make the 4.5 V bias point
```

6. **Exactly modeled.** Every part above.
7. **Approximated and estimated.**
   - The **741** is built the way the Rodent's LM308 is: a saturating transconductance into
     an integrator, so the pedal has the chip's gain-bandwidth (1 MHz) and slew rate
     (0.5 V/us) rather than an ideal amplifier's. Input bias current and offset are not
     modelled.
   - Germanium diodes use the catalogue's `DiodeSpec::GERMANIUM`; 1N270 constants of their
     own were not located.
   - Battery resistance is the supply's 47 ohm, as with the other pedals.
   - No tone control: the panel greys that knob.
8. **Why.** No manufacturer drawing, and no published 1N270 model.

## Measured (`tests/distortion_plus.rs`)

- Small-signal gain: 3.6 dB with Distortion down and 44.6 dB with it up, against the
  analysis's 3.5 dB and 46.5 dB. The two decibels at the top are the op-amp running out of
  gain-bandwidth at 1 kHz, which is the 741 rather than the arithmetic.
- The clipping threshold is where the analysis puts it: the output stops following the
  input above a few hundred millivolts.
- At a guitar's level with the knob up it makes the fuzzy, mid-forward distortion the
  pedal is known for.
