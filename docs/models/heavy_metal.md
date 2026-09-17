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
INPUT -- R1 10k -- C1 .047 -- Q1 JFET input buffer, R8 1M to 4.5 V, R7 10k
  -- C6 .047 -- R22 22k -- Q6 2SC2240-GR NPN
       R17 100k to ground, R19 10k collector load, R18 22 emitter,
       R16 470k // C10 100p collector-to-base feedback
  -- C11 .047 -- R27 22k -- Q7 2SA970-GR PNP
       R26 100k to +9 V, R28 120 emitter resistor, R31 10k collector load,
       R29 470k // C14 100p collector-to-base feedback
  -- Q7 collector -> IC1B non-inverting input (68k return to the 4.5 V reference)
  -- IC1B feedback: R20 220k // C9 100p with D3 versus D4+D5 asymmetric soft clipping
  -- VR4 250kD DIST has its WIPER GROUNDED:
       Q6 collector -- C31 10uF -- R49 150 -- one end of VR4
       other end of VR4 -- R25 47k -- C22 .047 -- IC1B inverting input
       (one knob therefore changes both Q6 attenuation and IC1B closed-loop gain)
  -- C12 1uF -- R23 10k -- D6/D7 anti-parallel series coring element -- R30 10k
  -- D8/D9 hard clip to ground // C16 .001uF
  -- C15 1uF -- R24 68k bias return -- IC1A unity buffer
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
   - Op-amps: the parts list identifies the M5218L family; the existing ideal op-amp-with-rail
     model captures the topology and clipping limit but not that IC's finite bandwidth/slew.
   - Transistors: Boss's parts list identifies **Q6 as 2SC2240-GR (NPN) and Q7 as
     2SA970-GR (PNP)**. The generic Ebers-Moll magnitude is still an approximation because
     the plugin does not yet carry dedicated 2SC2240/2SA970 SPICE parameters, but the
     complementary polarity is now modelled explicitly.
   - Germanium D6/D7: modelled to the **0.3 V** the second source measures, as with the
     Yellow Dist's 1N270s, rather than the catalogue's softer generic germanium.
   - The Q8/Q9 bypass flip-flop is switching and is not built.
8. **Why any of it is approximated.** The service notes do provide the semiconductor
   part numbers, but not full manufacturer SPICE parameters for those devices. The topology and
   polarity come from Boss; the detailed device constants remain approximations.

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

**The Dist control is not a simple pre-clipper attenuator.** Boss's 250kD VR4 has its
**wiper grounded**. One end is AC-coupled to Q6's collector through C31 10uF and R49 150;
the other reaches IC1B's inverting input through R25 47k and C22 .047uF. Turning DIST up
therefore simultaneously removes the shunt from the transistor pre-gain and lowers the
op-amp's feedback ground-leg resistance. That dual action is why the real HM-2 control can
feel abrupt, but it still has a usable full travel. The previous implementation instead
used a linear 250k divider feeding a fixed-gain clipper, which compressed the useful range
into the first few percent and drove the nonlinear solve into an unrealistic operating
region above it.

**Q7 is PNP, not a second NPN.** The service parts list and schematic identify Q6 as
2SC2240-GR and Q7 as 2SA970-GR. Treating Q7 as NPN put the second pre-gain stage at the
wrong operating point exactly where DIST increases both signal and loop gain.

**D8/D9 are a separate hard clipper before IC1A.** After the germanium D6/D7 coring element,
R30 feeds D8/D9 anti-parallel to ground with C16 across the node; C15 then couples that
bounded signal into IC1A, which is a follower. The earlier model incorrectly put D8/D9 in
IC1A's feedback loop and used R23/R30 as bias returns. That removed the intended amplitude
limit and made high-DIST internal excursions much larger than the pedal's actual circuit.

**Both gain transistors use collector-to-base shunt feedback** -- R16 around Q6 and R29
around Q7 -- but their DC networks are complementary rather than identical. Q6 is the NPN
stage with its emitter toward ground; Q7 is the PNP stage with its emitter toward the
positive rail. The feedback resistors are part of the bias and linearisation, not a reason
to instantiate both devices with the same polarity.

## Measured (`tests/heavy_metal.rs`)

- It solves, and silence stays silent.
- The historical monotonic-DIST measurement predates the 2026-09-17 topology correction and
  must be re-measured. The new regression instead requires every DIST position to remain finite
  and settled under a hot low-guitar stress signal with both Colour Mix controls at maximum.
- Colour Mix Low lifts 80 Hz and Colour Mix High lifts 1.1 kHz, each by more than three
  decibels, and each does more where it lives than where the other does.
- The coring gate passes a louder signal more readily than a tiny one. From outside that is
  **gain that rises with level**, the opposite of an ordinary clipper as it runs out of room.
  After the topology correction the regression measures **+12.5 dB at 1 uV and +30.4 dB at
  1 mV**: about 17.9 dB of level-dependent opening before the circuit reaches its clipping
  region. The exact amount is deliberately not treated as a hardware specification because
  it depends strongly on the germanium pair.
- After the 2026-09-17 topology/rail correction, broadband calibration puts unity with
  DIST/LOW/HIGH centred at physical **Level 0.515** (`LEVEL_REST`). The old 0.60 rest was
  measured at **+6.88 dB**, while 0.51 measured **-0.61 dB**; interpolation through the
  audio-taper divider gives about 0.5148. This matters especially when the HM-2 feeds
  another pedal, because final preset trim cannot undo excessive inter-stage drive.
- The older slot-level figure predates this `LEVEL_REST` correction and must be re-measured
  with `examples/pedallevel.rs`; do not preserve it by adding preset trim. With Dist shut the
  coring/gain structure still closes the pedal strongly, which is pedal behaviour rather than
  a solver fault.

## Still to do

- The collectors idle at about 1.1 V, which is low; that follows from a beta-320 stand-in
  in a shunt-feedback stage, and a lower-beta part would sit higher. Worth revisiting when
  the transistor types are pinned down (question 7).
