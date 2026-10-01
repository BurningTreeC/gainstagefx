# Brit Drive (Marshall Guv'nor) research log

Engineering identity: **Marshall The Guv'nor**, the original (1988-1992, made in England,
then Korea and Taiwan). Stable ids `pedal_marshall_guvnor` (pedal slot) and
`pedal_marshall_guvnor_circuit`. Display: **Brit Drive**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::brit_drive`; pedal slot and circuit list),
on the Rodent's standing (a published analysis whose values agree with its own
arithmetic).

**Measured** (`examples/drive_pedals_op.rs`, held by `tests/drive_pedals.rs`): the first
stage 0 / +32.6 dB at 5 kHz against 0 / 33.3 (32.6 with C2 in the arithmetic); the second
stage +13.1 dB with the gain down and +33.7 at full, the one track doing both; the clip
node peaking at 1.58 V against ElectroSmash's "around 1.8 to 2 V" -- `DiodeSpec::LED`
is the catalogue's red LED, whose knee sits a little lower than that. **The bass pot's direction**: wired with its wiper going to ground as
the knob comes up -- the other way round cut the bottom as BASS turned up. Each of the
three tone knobs now raises its band (80 Hz, 1 kHz, 8 kHz). The LEVEL pot rests at 0.714
for unity (`examples/drive_pedals_level.rs`).

## Eight-question checkpoint

1. **Revision.** The original Guv'nor, not the Mk II or the Guv'nor Plus. The analysis
   draws one circuit for all three countries of manufacture.
2. **Original schematic found?** No Marshall drawing.
   - Used: [ElectroSmash's "Marshall The Guvnor Analysis"](https://electrosmash.mas-effects.com/marshall-guvnor-analysis),
     whose schematic gives every value with designators.
3. **Best source.** That analysis.
4. **Cross-check.** Its arithmetic: input high-pass "16 Hz" (C1 9.6 nF, R2 1 M); the
   first stage "Gv(max) = 1 + (100 K / 2.2 K) = 46.45 (33.3 dB)" with its 723 Hz shelf
   (C3, R3) and 13.2 kHz top (C2 120 pF, VR1); the second "Gv = -R5 / R4 = -68 (36 dB)";
   "the clipping point is around 1.8 to 2 V" (red LEDs); the output's "fc = 15.3 kHz"
   (R11 22 k, C13 470 pF).
5. **Signal path and values.**

```text
power: 9 V; R13 / R14 47k with C14 10u -> 4.5 V
input: R1 2.2M to ground; C1 9.6n; R2 1M to 4.5 V -> U1A +in (TL072)
U1A: R3 2.2k + C3 100n from -in to ground; C2 120p from -in to out; VR1 GAIN 100k, one
   track: from -in to its wiper (on U1A's output) is the feedback, and from the wiper to
   its other end is in series with the next stage's input
U2B, inverting: C4 220n, R4 10k, C5 100n -> -in; R5 680k || C6 220p; +in on 4.5 V
clip: C7 220n, R6 1k, D1 / D2 red LEDs back to back to ground
tone, from the clip node: R7 1.5k down to node A; C9 4.7n across the top to node T;
   from T, C11 10n + R10 100 to the top of VR3 MID 10k (to ground), its wiper through
   C10 220n from A; VR4 TREBLE 10k from T to node B, its wiper the output; R9 680 from A
   to B; C12 68n from B to ground; VR2 BASS 10k from B to ground, its wiper on the node
   between C8 100n (from A) and R8 680 (to ground)
output: VR5 LEVEL 100k to ground, its wiper through R11 22k, C13 470p to ground
```

   **The gain pot is one track doing two jobs**: turned up, more of it is U1A's feedback
   (up to 33 dB) and less of it is in series with R4, so U2B's gain rises from
   680 k / 110 k (16 dB) to 680 k / 10 k (36 dB) at the same time. The analysis gives the
   two stages' gains separately; the drawing shows one track.
6. **To be modeled exactly.** Every part above.
7. **To be approximated, and why.**
   - **TL072** rail-limited at 3 V either way of 4.5 V, as the TS808. APPROXIMATED.
   - **Red LEDs**: the catalogue's LED diode. APPROXIMATED.
   - **Pot laws and directions**: not printed. Gain and level audio, the three tone
     controls linear (PLAUSIBLE); each tone control's direction chosen so the knob raises
     the band it is named for -- the drawing does not mark clockwise.
   - The footswitch's grounding of U2B's input when off, and the LED, are not in the path
     when the pedal is on; not built.
8. **Why.** A complete drawing with its arithmetic beside it. The interactive tone stack
   is the pedal and is built as drawn rather than as a generic three-band.
