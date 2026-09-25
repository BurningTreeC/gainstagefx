# American 5150 (Peavey EVH 5150) research log

Engineering identity: **Peavey EVH 5150**, the original (1992), lead channel. Stable ids
`evh5150` (`circuit`, display American 5150) and `power_5150` (`power_amp`, American 6L6
High-Gain).

Status: **the revision is settled (2026-09-25), and the tone stack, resonance and presence
are the drawing's.** Until then this log called the revision unresolved and pointed at the
5150 II; the preamp's tone stack was a 33 k resistor with the plugin's generic stack after
the power stage; and the power stage had no resonance and a Marshall-style presence.

## Eight-question checkpoint (rewritten 2026-09-25)

1. **Revision.** The original 5150: Peavey "VHALEN 120 PREAMP BD.", CB #98802910, drawing
   81501510, sheets 1-3, dated 5-FEB-92, and its power board. **Not** the 5150 II.
2. **Original schematic found?** Yes: the three factory sheets,
   [el34world copy](https://el34world.com/charts/Schematics/files/Peavey/5150_evh.pdf)
   (`docs/schematics/peavey_5150_evh.pdf`, 300 ppi scans), and the 5150 II's factory set
   with its bill of materials for comparison
   (`docs/schematics/peavey_5150_II.pdf`, [The Tube Store](https://www.thetubestore.com/lib/thetubestore/schematics/Peavey/Peavey-5150-II-Schematic.pdf)).
3. **Best source.** The original's sheets, read at their native 300 ppi.
4. **Cross-check, and why the old log was wrong.** The model's every stage was already the
   original's, designator for designator: R22 68 k and R12 1.82 k at V1A, R82 470 k, R15 39 k
   unbypassed at V2A, R11 330 k with C48, R32/R101, R87/R91, R88 1 M feedback round V5A, C58
   and R89 470 k into R24 33 k. The pots on this sheet are labelled **ULTRA PRE** (VR2) and
   **ULTRA POST** (VR7) themselves -- the labels that had made the model look like a 5150 II
   are on the original.
5. **Signal path and values** added 2026-09-25 (sheet 1 unless stated):

```text
tone stack, from R89's end ("stack"):
  R24 33k to ground; C59 .001 with R94 47k to ground
  C14 470pF and C60 100pF, side by side, to VR5 HIGH 250kB (pin 2; wiper = output)
  R28 47k to the slope node; C18 .022 from it to VR5 pin 1 and VR3 LOW 1MA (pin 2)
  VR3 wiper tied to its pin 2 (a variable resistor); pin 1 to VR4 MID 50kB pin 2
  C21 .022 from the slope node to VR4's wiper; VR4 pin 1 to ground; R95 47k from VR4 pin 2
    to ground
  output: VR5's wiper, loaded by VR6 CLEAN POST and VR7 ULTRA POST (1MA each); K2B picks
    VR7's wiper for the lead channel
feedback (sheets 1 and 2):
  FDBK -- R56 39k -- (NEXT3 = J34) -- VR8 RESONANCE 1M A, wiper tied to pin 2, with C12
    .0068 across it -- (J35 = NEXT2) -- C30 22uF -- the inverter's tail (R57 4.7k to ground,
    R51 10k to the grid-return junction, R52 470 to the cathodes)
  at NEXT2: C8 "1P10" to VR9 PRESENCE 10kA pin 1, C62 .033 to its wiper, pin 2 to ground
```

6. **Built exactly.** Every part above: the stack in `evh5150.rs` (its three controls are
   the amplifier's own and take the panel's Bass/Middle/Treble), and the feedback path as
   `power::FeedbackNetwork::PEAVEY_5150`, with VR7 as the power stage's master.
7. **Approximated, and why.**
   - C14 and C60 are drawn side by side in one box, read as **in parallel** (570 pF);
     PLAUSIBLE.
   - C59's value is partly obscured; ".001" is the reading. PLAUSIBLE.
   - C8's "1P10" is read as **1 uF**. PLAUSIBLE.
   - The **resonance control has no panel knob**: it rests at noon. ESTIMATED.
   - Rails, bias and transformer as before (`power.rs`, `evh5150::SUPPLY`): not printed on
     the drawing.
8. **Why.** One factory sheet carries the preamp, the stack and the feedback network; the
   old gaps were reading gaps, not missing evidence.

## Measured (`tests/evh5150.rs`)

- The stack, stack input to treble wiper: Bass 9.3 dB of range at 80 Hz, Middle 7.6 dB at
  600 Hz, Treble 9.8 dB at 5 kHz; at noon a scoop, -9.9 dB at 500 Hz against -7.1 at 100 Hz
  and -5.3 at 3 kHz.
- R89 into the built stack loses 27.6 dB at 1 kHz (R24 alone: 23.7).
- The power stage: resonance up lifts 80 Hz against 1 kHz by 4.6 dB; presence up lifts
  5 kHz against 1 kHz by 4.7 dB.

## Consequences

- `tests/legacy_baseline.rs` compared only the 5150 sample for sample; the correction
  removes it too (the fourth recorded exception). The file now guards the solver's
  stability budget and finite output only. A fresh capture of the corrected circuits is
  the owner's call.
- The two 5150 presets (Ultra Lead, Ultra Rhythm) now turn the amplifier's own stack with
  the plugin's generic stack off, and are held at their levels.
