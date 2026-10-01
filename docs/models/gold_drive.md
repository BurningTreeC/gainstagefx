# Gold Drive (Klon Centaur) research log

Engineering identity: **Klon Centaur**, Bill Finnegan's overdrive (1994-2009). Stable
ids `pedal_klon_centaur` (pedal slot) and `pedal_klon_centaur_circuit`. Display:
**Gold Drive**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::gold_drive`; pedal slot and circuit list),
on the same standing as the Rodent and the Yellow Dist (a published analysis whose values
agree with its own arithmetic).

**Measured** (`examples/drive_pedals_op.rs`, held by `tests/drive_pedals.rs`): the gain
stage alone 39.9 dB near 1 kHz against ElectroSmash's 40 (about 3 dB less read from the
buffer, which is C3 into feed-forward 1's 1.5 k), and 29.5 dB at 100 Hz, the mid hump;
the treble shelf +17.1 / -7.4 dB at 20 kHz against +18.2 / -8 (5 kHz reads +15.7 / -7.2,
short of the shelf); the second gang's path 29 dB further down at full gain than at
none; no measurable THD at no gain and 19.7 % at full on a guitar's 0.122 V. The OUTPUT pot rests at
0.116 for unity on the level probe (`examples/drive_pedals_level.rs`): the clean paths
alone give the pedal about +13 dB.

## Eight-question checkpoint

1. **Revision.** The production Centaur, gold or silver: "under the hood they're all
   basically the same" (Finnegan, Premier Guitar, quoted in the analysis). His 1995
   changes were an input protection resistor ("no sonic effect"), a ground plane, and "a
   resistor to give the circuit a very small amount of additional low-mid response" --
   which one is not said. The drawing below is the one in circulation; which side of
   1995 it is, is not established.
2. **Original schematic found?** No Klon drawing (the board was potted in epoxy).
   - Used: [ElectroSmash's "Klon Centaur Analysis"](https://electrosmash.mas-effects.com/klon-centaur-analysis)
     (the site's archive), whose schematic gives every value with designators and works
     each block out from them.
3. **Best source.** That analysis.
4. **Cross-check.** Its own arithmetic, which a model has to reproduce from the values:
   - feed-forward 1, R7 1.5 k and C16 1 uF: "fc = 106 Hz";
   - the summing amplifier's R20 392 k and C13 820 pF: "attenuating frequencies above
     495 Hz";
   - the treble shelf, R22 100 k and C14 3.9 nF: "fc = 408 Hz", "Gvmax = (RV2 + R23) / R21
     = 14.7 / 1.8 = 8.16 (18.24 dB)", "Gvmin = R23 / (RV2 + R21) = 4.7 / 11.8 = 0.4 (-8 dB)";
   - the gain stage: "the maximum gain is 40 dB (100 times) around 1 kHz".
5. **Signal path and values** (the analysis's designators).

```text
power: 9 V; MAX1044 charge pump to +16.2 V and -8.6 V; R29 / R30 27k with C18 47u -> 4.5 V
U1A TL072 (9 V) buffer: R1 10k, C1 0.1u, R2 1M to 4.5 V
gain stage, from the buffer: C3 0.1u, then R6 10k || C5 68n to U1B +in;
   RV-GAIN_a 100k B: its track from U1B +in to R10 2k, its wiper on 4.5 V;
   R10 -> R11 15k || C7 82n -> U1B -in; R12 422k || C8 390p from -in to out;
   C9 1u, R13 1k, D2 / D3 1N34A back to back to ground, C10 1u
feed-forward 1, from after C3: R7 1.5k, C16 1u to ground, R19 15k to the summing node
feed-forward 2, from the buffer: R5 5.1k || C4 68n to a node with R8 1.5k and C6 390n +
   R9 1k to 4.5 V; RV-GAIN_b 100k B across that node and 4.5 V, its wiper to
   R17 27k || (C12 27n + R18 12k) into the summing node
from after C10: R16 47k to the summing node, and C11 2.2n + R15 22k to RV-GAIN_b's wiper
U2A TL072 (+16.2 / -8.6 V) inverting summer: R20 392k || C13 820p
tone, U2B (+16.2 / -8.6 V): R22 100k in, R24 100k feedback; R21 1.8k from the input to
   RV2 TREBLE 10k B, R23 4.7k from its other end to the output, the wiper through C14 3.9n
   to -in
output: C15 4.7u, R25 560, RV3 LEVEL 10k B to ground, the wiper out (R28 100k at the jack)
```

6. **To be modeled exactly.** Every part above: both gangs of the gain pot on one
   control, the two feed-forward paths, the 1N34A pair to ground, the summer and the
   shelf on the charge pump's rails.
7. **To be approximated, and why.**
   - **TL072**: the catalogue's rail-limited op-amp, swinging about 1.5 V short of each
     rail as the TS808's 4558 does -- 3 V either way of 4.5 V on 9 V, and 10.2 V on the
     +16.2 / -8.6 V pair (the smaller side). Its phase inversion past the negative
     common-mode limit is not modelled.
   - **1N34A**: the catalogue's germanium diode, as for the Yellow Dist. APPROXIMATED.
   - **The charge pump** as the two stiff rails the drawing prints; its ripple and
     droop are not modelled.
   - Pot laws: "B" (linear) as printed.
   - The bypass network (C2, R3, R4) carries signal only when bypassed; not built.
8. **Why.** Every value is on the drawing and four of its consequences are worked out
   beside it. The two clean paths summed with the clipped one -- the reason the pedal
   is described as transparent -- are on the drawing, not inferred.
