# Modern Purple (Revv G3) research log

Engineering identity: the **Revv G3**, the original (2018), Revv's pedal of the
Generator 120's Purple channel: an all-op-amp (TL07x) distortion -- a buffer stage, a
gain stage whose leg the Aggression switch changes, a diode-feedback clipper, a second
inverting stage into red LEDs to ground, a Baxandall bass and treble, an active mid, and
the volume. Stable id `pedal_revv_g3` (proposed in the roadmap). Display **Modern Purple**.

Status: **IMPLEMENTED 2026-10-02** (`circuits/modern_purple.rs`, pedal and circuit),
the original G3 on a trace of a genuine unit. The G3 V2 is not cleared (see 1).
Results below.

## Eight-question checkpoint

1. **Revision.** **The original G3**, the one traced in 2019. Revv's G3 V2 manual says the
   V2 "features circuit revisions from a decade of development", with "a thicker low end
   from the updated circuit": a different circuit, and no trace of it was found. The
   model is the G3, and says so; it is not the V2.
2. **Original schematic found?** **No factory drawing; a trace of a genuine unit, yes** --
   PedalPCB's "Tyrian Distortion", traced by PedalPCB's owner from a real G3 (credited
   so on the Dirtbox Layouts verified layout, 2019-11-04, and in PedalPCB's forum). The
   2019 and 2020 revisions of the document are the same circuit, the second re-laid out
   for TL072s. The Cry Baby's and the Bass Driver's standing here: a trace of the real
   pedal, not of a clone kit.
3. **Best source.** PedalPCB's Tyrian schematic (revision 10.14.19) and parts list.
4. **Cross-check.** PCB Guitar Mania's "Revolution III" carries the same set of values,
   renumbered -- consistent, but not independent proof, as it may be copied. Its text
   agrees with the trace on what the Aggression switch does: it "engages different
   settings of resistors and capacitors to change how the gain on the first stage
   behaves". Revv's own manual describes the switch as Off (least distortion), Blue and
   Red (most), and calls it "clipping" -- the trace shows it changing a coupling
   capacitor and a gain leg, which raises the distortion without switching diodes; the
   manual is marketing shorthand, the trace is the evidence. Supply 9 V, 14 mA (Revv;
   PCB Guitar Mania measures 22 mA on its build).
5. **Signal path and values** (Tyrian, v1 designators):

```text
IN -- R1 1M to ground -- C1 22n -- (R3 470K to VREF) -- R5 10K -- (C3 100p to ground)
   -- IC2.2 (+), non-inverting: R7 82K out to (-), R6 47K (-) to VREF_B: x2.74
IC2.2 out -- C6 2n2 -- node (R10 33K to VREF_B) -- R11 10K -- (C9 1n to ground)
   -- IC2.1 (+), non-inverting gain stage
   Aggression, SW1.2: C7 2n2 beside C6 in either engaged position (the coupling's
   corner into R10 from 2.2 kHz to 1.1 kHz)
IC2.1: feedback R17 56K + GAIN (B1M as a rheostat), C11 100p across;
   leg to C10 220n (to ground): R16 33K always, and by SW1.1 R14 33K (one way) or
   R15 10K (the other) in parallel: at full gain x33 (off), x65, x139
IC2.1 out -- C12 22n -- R18 22K -- IC2.4 (-), inverting: R19 56K // C13 100p //
   D2+D4 and D3+D5 (1N4148 pairs, antiparallel): x2.5, soft clipping at two diodes
IC2.4 out -- R20 4K7 -- IC2.3 (-), inverting: R21 82K // C14 100p: x17.4
IC2.3 out -- C15 22n -- R22 4K7 -- node: C16 10n to ground, D6/D7 red LEDs antiparallel
   to ground (hard clipping, and a 3.4 kHz low-pass with R22)
-- R23 56K -- IC1.2 Baxandall: BASS A100K (C18 47n, C19 100n, R24 82K, R25 2K, R26 4K7),
   TREBLE A50K (C20 4n7, C21 47n), C17 4n7, R27 100K, C22 47n, R28 470K to VREF_C,
   R29 22K, R30 150K + C23 220n, C24 2n2, R31 22K
-- IC1.3 active mid: MID B100K with C25/C26 10n, R32 10K, R33 1K5, R34 10K to VREF_C,
   R35 470K // C27 100p
-- C28 220n -- R36 2K -- VOLUME A50K -- OUT
Supply: 9 V -- D1 1N5817 -- R2 10R, C2 100u; VREF = 4.5 V (R8/R9 20K, C5 100n, C8 22u),
   buffered to VREF_B and VREF_C by IC1.1 and IC1.4 through R12/R13 1K5.
```

   The tone section as read part by part: R23 into T1 (C17 to ground); R24 from T1 to
   BASS's top, C18 across its upper half and C19 its lower, R25 2K from its bottom to
   ground; C20 from T1 to TREBLE's top, C21 from its bottom to ground; the two wipers
   joined through R26, into R27 / C22 and IC1.2's +in. IC1.2's feedback: C24 out to -in,
   R29 from -in to a node that R31 ties to the output and R30 + C23 to ground (unity in
   the bass, x1.15 above 5 Hz). MID across IC1.2's output and C26, C25 / C26 to R32 10K.
   IC1.3: R35 // C27 over R34 to VREF_C, x48.
6. **To be modelled exactly.** Every stage as drawn, the Aggression switch's two halves,
   the clippers, the tone section and volume, the 4.5 V references.
7. **To be approximated, and why.**
   - **TL072/TL074**: the solver's transconductor op-amp (GBW, slew, rails). The
     sections ride 4.5 V; per this repository's failure mode 4 they may be
     ground-referenced with coupling where the solver needs it, as `rodent` does.
   - **Red LEDs**: Shockley with a red LED's emission coefficient and saturation
     current (ESTIMATED from data sheets), as elsewhere in this repository.
8. **Why.** Asked for by the owner (2026-09-16), the standalone distortion of the modern
   chain. Without the Generator 120 (not cleared, `modern_generator.md`) this is also
   the only route to the Purple channel's voicing that the evidence allows.

## Results (`tests/modern_purple.rs`, `examples/modern_purple_op.rs`)

- **Every stage at its values' gain** (1 kHz, small signal): IC2.2 x2.65 against 2.74;
  the gain stage at full GAIN x27.4, x53.6 and x112.8 (off, Blue, Red) against x27.5,
  x54.1 and x115.0 computed from R17, the track, C11, R16, R14, R15 and C10.
- **Aggression's other half**: 200 Hz against 3 kHz into the gain stage -16.4 dB off,
  -12.2 engaged -- C7 halving the coupling's corner.
- **The clippers**: the LED node peaks at 1.5 V at a guitar's level whatever the toggle.
- **The tone controls** run the box's way: BASS +11.7 dB at 100 Hz end to end, TREBLE
  +28.9 at 5 kHz, MID +12.5 at 700 Hz (clockwise more mids).
- **VOLUME_REST 0.353**: unity for a guitar's 110 + 330 Hz with the other knobs at
  rest. **AGGRESSION_REST 0.5**: Blue, where the knob's default puts it.
- Same response at 44.1 to 192 kHz; realtime-safe in the slot.
- **Expensive**: 5.76 Newton passes a sample (the Metal Zone 4.56). With its compiled
  kernels 23.9 % of a channel alone and 47.1 % in front of the Cali IIC+ at 1x, the
  Metal Zone 20.2 and 45.0 beside it (on a loaded machine); before the kernels, 109.6 %
  at 2x. It holds the chain at the host rate, as the Metal Zone and the Heavy Metal do.
