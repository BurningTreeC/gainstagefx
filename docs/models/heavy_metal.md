# Heavy Metal (Boss HM-2) research log

Engineering identity: **Boss HM-2 Heavy Metal**, the Japanese original (1983-88). Stable id
`pedal_boss_hm2` (pedal slot). Display: **Heavy Metal**.

Status: **IMPLEMENTED 2026-09-16** (`src/circuits/heavy_metal.rs`), tested in
`tests/heavy_metal.rs`, and in the pedal slot as **Heavy Metal** (`pedal_boss_hm2`) with
all four of its knobs -- the first pedal here with two tone controls, which is what the
slot was widened to four for.

## Eight-question checkpoint

1. **Revision.** The MIJ HM-2 as Boss drew it: four knobs (Level, Dist, Colour Mix Low and
   High), a JFET input buffer, a two-transistor gain block, a hard-clipping op-amp stage, a
   germanium coring gate and three gyrators. Not the 2021 HM-2W Waza reissue (which adds a
   Custom mode), and not the Taiwanese later runs.
2. **Original schematic found?** **Yes** -- Boss's own service-notes drawing, read directly
   (project-local `docs/schematics/boss-hm2.png`, git-ignored). Every reference designator
   and value below is off that sheet.
3. **Best source.** That drawing. It carries the complete part list including the two
   pot boards (the `VOLUME BOARD` with VR4 250KD Dist, VR2/VR3 10KG Colour Mix L and H, and
   VR1 10KA Level).
4. **Cross-check.** Published analyses, used only to confirm what the drawing shows and to
   label intent:
   - [David Ross: HM-2 circuit analysis](https://davidrossmusicalinstruments.com/boss-hm-2-circuit-analysis/)
     -- "the tone controls boost or cut the lows at around 80Hz, and also boost or cut the
     mids between about 900Hz and 1.3kHz", and names the stack as gyrator-based.
   - [Electronic States: Boss HM-2 analysis](https://atomiumamps.tumblr.com/post/139197356031/boss-hm-2-analysis)
     -- asymmetric soft clipping, hard clipping, and a germanium pair in series with the
     signal as a "coring" gate; D6/D7 measured at **0.3 V** forward.
   - [hobby-hour mirror of the schematic](https://www.hobby-hour.com/electronics/s/hm2-heavy-metal.php)
     for a second reading of the same sheet.
5. **Signal path and values** (Boss's own designators, off the sheet).

```text
INPUT -- R1 10k -- C1 .047 -- Q1 JFET buffer, R8 1M to 4.5 V, R7 10k, R2 330
  -- C4 1uF -- Q4 (D11 at its gate) R11 1M
  -- C6 .047 -- R16 470k / R21 100k / R18 22 -- Q6, R19 22k, C10 100p, R17 10k
  -- R26 100k / R28 120 -- Q7, R29 470k, C11 .047, R22 22k, C14 100p, C31 10uF, R49 150
  -- R25 68k, C22 .047 -- op-amp 1b: R20 220k and C9 100p in the loop
     clipping: D3, and D4/D5 back to back
  -- C12 1uF -- R23 10k -- D6, D7 in *series with the signal*: the germanium coring gate
  -- R30 10k -- C15 1uF -- op-amp 1a, R24 68k, D8/D9 to ground, C16 .001, R42 47k
  -- VR4 250KD (Dist) on the volume board
tone: three gyrators off op-amps 3b, 2a, 2b --
     R47 330 / C30 .068 / R51 100k    (the low one)
     R41 330 / C27 .0068 / R43 82k
     R45 330 / C26 .0047 / R44 100k   (the high one)
     with C35 1.5uF, C28 .15uF, C29 .1uF into VR2 10KG (L) and VR3 10KG (H), Colour Mix
  -- op-amp 3a: R52 3.3k, R54 3.3k, C33 470p, C34 10uF, R53 10k
  -- VR1 10KA (Level) -- C32 1uF -- Q10 -- C7 .047 -- Q3 output buffer, R12 470k, R13 10k,
     R4 1k, C3 1uF, R3 10k
supply: 9 V, R5/R6 10k each make the 4.5 V bias, C2 100uF, C25 47uF
bypass: Q8/Q9 flip-flop with R34-R40 and C17/C18/C20/C21 -- switching, not audio
```

6. **To be modeled exactly.** Everything on the audio path above: the JFET buffer, both
   transistor stages, the op-amp gain stage with its asymmetric clipping (D3 against the
   D4/D5 pair), the **germanium coring gate in series** -- which is the part that gives this
   pedal its gated, chainsaw decay and which nothing else in the plugin has -- the three
   gyrators and the two Colour Mix controls, the Dist and Level pots and the output buffer.
7. **To be approximated, and why.**
   - Op-amps: the sheet numbers them 1a/1b/2a/2b/3a/3b without naming the part. The pedal is
     normally documented as carrying a **4558-family dual**, so the existing `OpAmp` with a
     rail limit is the starting point, as with the Green 808. ESTIMATED.
   - Transistors: the sheet does not letter Q1/Q3/Q4/Q6/Q7/Q10 with part numbers on this
     scan. The Boss standard of the period is 2SK30A-class JFETs and 2SC2240/2SC1815-class
     NPNs; the catalogue's existing devices stand in. ESTIMATED.
   - Germanium D6/D7: modelled to the **0.3 V** the second source measures, as with the
     Yellow Dist's 1N270s, rather than the catalogue's softer generic germanium.
   - The Q8/Q9 bypass flip-flop is switching and is not built.
8. **Why any of it is approximated.** No Boss part-number list on the scan, and no
   manufacturer device models.

## What it is for

The plugin has no gated distortion and no gyrator stack. The coring gate is a different
mechanism from anything already built -- diodes *in series* rather than to ground, so small
signals are held back entirely and the pedal cuts off rather than fading -- and the Colour
Mix is a boost/cut pair rather than a passive tone control. It is also the documented sound
of an entire genre, which makes it a preset target rather than only a circuit.

## What the build found

**The gyrators are inductors, and the arithmetic confirms the reading.** A gyrator
simulates **L = R1 x R2 x C**, so the three are built as the inductances they stand for --
which is what the Mark IIC+'s graphic already does with five real ones. Working them out
from the drawing's values and pairing each with its series capacitor:

| | from the drawing | L | with | resonance |
|---|---|---|---|---|
| low | R51 100k, R47 330, C30 .068 | 2.244 H | C35 1.5 uF | **87 Hz** |
| mid | R43 82k, R41 330, C27 .0068 | 0.184 H | C28 .15 uF | **958 Hz** |
| high | R44 100k, R45 330, C26 .0047 | 0.155 H | C29 .1 uF | **1278 Hz** |

Which is exactly what the published analysis hears from the outside: the Low control works
"at around 80Hz" and the High one "between about 900Hz and 1.3kHz" -- that is these two
upper bands, both on the one knob. Three numbers derived from the sheet landing on an
independently published measurement is the cross-check this model rests on.

**The Colour Mix is a feedback equaliser**, the same shape as the Mark IIC+'s graphic: each
track runs between the two inputs of the mixing amplifier with its resonant leg on the
wiper, so it boosts toward one end and cuts toward the other and is flat in the middle.
That is why both knobs on full is a real setting rather than simply the loudest one.

**Two things had to be right that were not, at first, and both are on the sheet:**

- **R10 1k and C5 47 uF** decouple the two gain stages' rail. Left out, the pedal had a
  floor forty decibels down that did not track the signal at all -- the gain stages pulling
  the supply about and the output followers passing it on.
- **The Dist track is 250 k and the equaliser's input is 3.3 k**, so the wiper has to be
  buffered into it, as the Mark's graphic buffers its own input with Q1. Without that the
  control lost sixteen decibels into the stack.

**And one that is the solver, not the circuit.** The equaliser is built ground-referenced,
with the Dist wiper coupled into it, rather than riding the 4.5 V bias: biased, the buffer
would not follow below about a tenth of a volt. That is the same construction the Rodent's
LM308 needed, and for the same reason.

**The Dist control sits *before* the clipper, not after it.** Its three pins on the volume
board all land in the gain-stage region, between Q6/Q7 and op-amp 1b, and that is what
makes it a distortion control rather than a volume: it decides how hard the clipper is
driven into its diodes, and the clipping then holds the level roughly where it was. Built
after the clipper instead, the knob was twenty decibels down at noon with the same
distortion, which is a volume control wearing the wrong label.

Moving it wanted two things that are easy to get wrong and that the sheet takes for
granted: the track is **coupled in**, because a 250 k path straight off Q7's collector
costs the stage forty decibels and most of its operating point; and its **cold end is on
the 4.5 V bias, not ground**, because the wiper feeds the clipping amplifier's inverting
input through R25. Taken to ground, R25 works against R20, drives the op-amp into its rail
and leaves it there, and the pedal passes about a millivolt of leakage and nothing else.

**Both gain stages are shunt-feedback stages** -- R16 470 k and R29 470 k from collector
back to base are the whole DC bias, with no other path to the base -- which is the Boss
house arrangement of the period and the same one the DS-1's booster uses. Read the other
way round (base biased to the 4.5 V rail) the stages draw enormous current and the pedal
passes nothing, which is how the mistake announced itself.

## Measured (`tests/heavy_metal.rs`)

- It solves, and silence stays silent.
- The Dist control raises the gain monotonically across its travel.
- Colour Mix Low lifts 80 Hz and Colour Mix High lifts 1.1 kHz, each by more than three
  decibels, and each does more where it lives than where the other does.
- The coring gate passes a loud signal and holds a quiet one. From outside that is **gain
  that rises with level**, which is the opposite of every other pedal here -- a clipper's
  gain always falls as it runs out of room. Measured with Dist at its middle: **+18.5 dB at
  1 uV, +53.2 dB at 1 mV**, then falling again into the clipping (+6.1 dB at a guitar's
  level). Thirty-five decibels of it, and it is the only circuit in the catalogue that
  behaves this way.
- Unity through the pedal is at **level 0.719**, which is its `LEVEL_REST` -- higher than
  the other pedals because the Dist control gives away most of the gain at noon.
- In the slot it is the hottest pedal in the list, **+5.3 dB** against no pedal with the
  knobs at their middles (`examples/pedallevel.rs`), and with Dist shut it falls to
  -81 dB: the gate closes completely, which is the pedal rather than a fault.

## Still to do

- The collectors idle at about 1.1 V, which is low; that follows from a beta-320 stand-in
  in a shunt-feedback stage, and a lower-beta part would sit higher. Worth revisiting when
  the transistor types are pinned down (question 7).
