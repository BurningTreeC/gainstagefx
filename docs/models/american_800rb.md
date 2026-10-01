# American 800RB and American SS 800 (Gallien-Krueger 800RB) research log

Engineering identity: **Gallien-Krueger 800RB** bass amplifier (1983 on): a solid-state
preamplifier with voicing filters, a four-band active EQ, an effects loop, a balanced direct
out, a footswitchable boost and a tunable crossover into two power amplifiers, 100 W and
300 W. Proposed stable ids `amp_gk_800rb` (`circuit`) and `power_gk_800rb` (`power_amp`), as
the roadmap reserves them. Proposed display: **American 800RB** and **American SS 800**.

Status: **IMPLEMENTED 2026-10-01** (`src/circuits/american_800rb.rs`,
`src/circuits/american_ss800.rs`), preamp rev C and power amplifier rev B, on GK's own
service manual. Results below.

## Eight-question checkpoint

1. **Revision.** GK's service manual carries two preamp drawings, both titled "NEW 800RB
   PREAMP": **rev C** (406-0045-C, 8/13/91, LF353s, a TL604 switching the boost) and **rev
   D/E** (406-0045-D/E, 5/25/95, a DG419 for the boost, different op-amps, and changed
   values around the effects loop -- R63 220 R against 4.7 k, R73 470 R against 12 k,
   C61 / C70 3.3 uF). **Two power amplifier drawings**, both "100W + 300W POWER AMPS":
   **rev B** (406-0044-B, 9/3/91, page 13) and **rev C** (406-0044-C, 4/1/92, PCO 16 of
   8/95, page 17), which changes U1's input and output resistors (R29 5.6 k to 47 k, R34
   3.3 k to 2.2 k) and so its local gain from 179 to 21; the output stage and the
   printed voltages are the same. Built: **rev B**, the drawing of the turn-on
   procedure's boards and the preamp's year. One supply. The turn-on procedure (420-0045-C, 8/19/91) names boards
   206-0045-C, 206-0044-A and 206-0048-A, "800RB- all options". Engineering change orders
   occupy pages 24-33 of the original manual. **Chosen 2026-10-01 by the owner: rev C**, the drawing the turn-on procedure and its
   levels were written against;
   the D/E drawing is the later and probably the most produced, the C drawing the one the
   turn-on procedure was written against. Not the 1983 original, whose preamp is not in
   this manual.
2. **Original schematic found?** **Yes.** GK's service manual (Acrobat compilation of 2000,
   21 pages: operating instructions, turn-on and calibration procedures, scope photographs,
   schematics, layouts, bill of materials), scanned at **635 ppi**, from audiocircuit.dk.
   A copy is in `docs/schematics/` (git-ignored).
3. **Best source.** The manual itself: drawings for the circuit, the procedures and the
   operator's pages for the figures to meet.
4. **Cross-check.** Unusually rich, all GK's own:
   - **RMS voltages on the preamp drawing**, stage by stage, at 2 mV / 200 Hz in with
     every tone control and the volume on 10, boost on 10, filters out: 19 mV after the
     input stage, 18 mV into the EQ, 107 / 115 / 115 / 150 mV through it, 93 mV at the
     send, 10 mV and 1.1 V through the boost; and on the power amplifier drawing (41 V no
     load, the drivers' and bias points' DC).
   - **The turn-on procedure**: 200 Hz at -46 dBV in, at slight clipping 36 V rms from
     the 300 W amplifier into 4 ohm and 28 V rms from the 100 W into 8 ohm; the -10 dB
     switch then gives 6.0 V; boost at 0 gives 3.1 V; direct out 0.6 V rms at full power.
   - **The specifications**: sensitivity 2 mV rms (-10 dB: 6 mV), maximum input 1 V rms
     (3 V), effects send 1.5 V rms, direct out 500 mV rms into 600 ohm, boost 15 dB, 100 W
     into 8 ohm and 300 W into 4 ohm (200 W into 8).
   - **The operator's pages**: Treble 4 kHz, High Mid 1 kHz, Low Mid 250 Hz, Bass 60 Hz;
     Lo Cut a bass roll-off, Mid Contour "a notch at about 500Hz", Hi Boost "edge".
   - **Sixteen scope photographs** of a 200 Hz square wave through every tone control at
     both ends, each filter, and the crossover -- a shape to compare each against.
5. **Signal path and values** (rev C as read at 30 %; to be traced in full at 635 ppi
   before code, with `tools/schematic/trace.py` where the junctions are dense).

```text
J1 -- R1 12 k -- C2 0.1 uF -- U1A LF353, non-inverting: R4 1 M to 0 V, C6 100 pF, 1N759
   zener pair; feedback R5 33 k / C7 560 pF, with S1 (-10 dB) switching R3 4.7 k;
   Lo Cut (S2, C25 0.22 uF), C11 10 uF, R24 3.3 k; Hi Boost (S4, C26 10 nF, R23 470 k)
VOLUME R17 50 k A -- C21 3.3 uF -- Mid Contour (S3): U1B with R12 12 k, R13 120 k,
   C14 / C15 4.7 nF, R16 33 k, C20 560 pF, R19 220 k, C23 0.22 uF, R22 220 k
U2A, gain 1 + 22 k / 4.7 k with C31 100 pF
EQ: U2B High Mid (C37 / C40 2.2 nF, R53 50 k B), U3A Low Mid (C46 / C54 10 nF, R64 50 k B),
   U3B Bass (C62 0.1 uF, R68 50 k B) and Treble (C56 2.2 nF, R36 50 k B)
C61 -- R63 -- SEND / RETURN -- direct out: U4A / U4B balanced driver, 1 k build-outs
   -- R73 / C70 -- BOOST: R82 50 k B, U5 analogue switch on the footswitch, Q89 J113
   -- crossover: U6 / U7 state-variable, FREQ dual 50 k B, S5 FULL / BIAMP
   -- HI master R115 50 k B to the 100 W, LO master R118 50 k B to the 300 W
300 W: U1 LF353, 56 k / 5.6 k, a MJE15023 driver, A06 / A56 bias spreader, three pairs
   MJ15022 / MJ15023 with 0.33 ohm emitter resistors, +-85 V
100 W: the same arrangement, MJE15030 / MJE15031 drivers, one pair, +-60 V
```

6. **To be modelled exactly.** The preamp from the jack to the masters (input stage, its
   three switches, volume, contour, the four bands, the boost), the direct-out driver,
   and the 300 W amplifier as the power stage, loaded by the speaker as every power
   stage here is. Full-range mode first: S5 FULL feeds both amplifiers the whole band.
7. **To be approximated, and why.**
   - **The op-amps** as the solver's transconductor op-amp with the part's GBW and slew
     (LF353: 4 MHz, 13 V/us) on the drawing's +-14.5 V rails.
   - **The output transistors** as the Ebers-Moll pairs the JC-120's amplifier already
     uses; the bias set by the drawing's own procedure (5 mV across the emitter
     resistors).
   - **The bi-amp mode and the 100 W amplifier** later: the panel has one power stage.
   - **The cabinet**: GK's 410RBH uses Eminence-built P10/200 drivers whose parameters are
     not published; not cleared here (see the bass cabinet entry in the roadmap).
8. **Why.** GK's own drawings, procedures and specifications for the whole amplifier, with
   voltages at every stage.

## Results (2026-10-01)

Measured by `tests/american_800rb.rs`; `examples/american_800rb_op.rs` and
`examples/american_ss800_op.rs` print the same and the fits.

**The preamp against the drawing's stage voltages** (2 mV, 200 Hz, everything on 10):

| Node | Drawing | Solved | |
|---|---|---|---|
| U1A out | 19 mV | 21.7 mV | +1.2 dB |
| U2A in | 18 mV | 21.7 mV | +1.6 dB |
| U2A out | 107 mV | 123 mV | +1.2 dB |
| HIGH MID out | 115 mV | 125 mV | +0.7 dB |
| LOW MID out | 115 mV | 114 mV | -0.1 dB |
| BASS / TREBLE out | 150 mV | 148 mV | -0.1 dB |
| boost input (SEND) | 93 mV | 105 mV | +1.1 dB |
| boost wiper | 10 mV | 10.6 mV | +0.5 dB |
| J113 drain | 1.1 V | 1.14 V | +0.4 dB |

The first three sit 1.2-1.6 dB high together and the bands then land on the drawing, so
the difference is in U1A / U2A -- inside what a meter reading of 2 mV in can be trusted to.

**The bands against the operator's pages**: LOW MID +5.9 / -9.3 dB at 250 Hz, HIGH MID
+6.2 / -9.5 dB at 1 kHz, each peaked against an octave either side by more than 2.5 dB;
BASS +9.1 dB at 60 Hz, TREBLE +14.1 dB at 4 kHz, each flat at the other end. **Mid
Contour**: -9.7 dB at 500 Hz ("a notch at about 500Hz"), within 1.5 dB at 100 Hz and
3 kHz. **Lo Cut**: -13.9 dB at 40 Hz, flat at 1 kHz. **Hi Boost**: +15 dB at 6 kHz with
the volume at 3, nothing with it at 10 -- it bridges the volume, as the drawing has it.

**-10 dB**: R3 across R5 gives **13.7 dB**. GK's own sensitivities (2 mV, 6 mV with the
switch in) would be 9.5 dB, and the turn-on procedure's 36 V to 6.0 V would be 15.6 dB
(though that one is from clipping to below it). The drawn values are built; the
disagreement is GK's, recorded here.

**The J113** (Q89): no single data-sheet part reaches the drawing's 6 V at the drain
(it would need VGS -3.4 V behind R88, outside VGS(off) -0.5 to -3 V). Fitted inside the
sheet to the drawing's gain instead, IDSS 5 mA and VGS(off) -1.0 V: x11.9 against x11.8,
drain at 7.4 V. ESTIMATED.

**The SS 800 at idle**, against the drawing: the output -0.002 V; U1 -8.3 V (-8.8); the
VAS +1.25 V (+1.1) and the lower driver's base -1.25 V (-1.1); the driver emitters
+-0.63 V (+-0.5); Q11's emitter -0.66 V (-0.6). The bias pot rests at **0.242**, fitted
to GK's procedure -- 5 mV across each 0.33 ohm, 15 mA a device.

**The bootstrap disagrees with its own drawing, on both revisions.** R40 and R41 are
2.7 k each, from the lower driver's base to -85 V, with C14 to the output; DC can only
divide, so their junction sits at -43 V and the VAS runs at (86 V / 5.4 k) **15.6 mA**,
1.56 V across R37 and 2.2 V across R36. The drawing prints -29 V, 1.0 V and 1.6 V:
consistent with each other -- a 10 mA VAS -- and with R41 at about 5.6 k, but not with the
2.7 k both drawings print. The printed values are built. What that costs is not
measured: the VAS's standing current, and with it the current the bootstrap can pull
the lower half of the drive with.

**The gain**: 35.2 dB, unloaded and into 4 ohm (the drawing: 0.65 V to 41 V, 36 dB), THD
0.02 % at 0.1 V in.

**U1 has to be a real op-amp.** U1's local loop -- R30 over R29, 179 times -- sits
inside the global one; a real LF353 has run out of gain-bandwidth for it by 22 kHz, an
ideal op-amp never does. Built with the solver's ideal op-amp, as every preamp stage here
is, the amplifier latched U1 to a rail within a tenth of a second of silence and
distorted 1 % at a tenth of a volt. Built as the transconductor op-amp with the LF353's
GBW, slew and open-loop gain together (as `rodent` builds its LM308; which of the three
matters was not separated), it idles, moves 2 mV in a second of silence, and measures
the figures above. A test holds the silence.

**The rails**: the supply is a bridge and 3 x 1000 uF a side, unregulated, 85 V at idle.
Under a sustained tone the transformer's regulation and the ripple take them down, and
GK's clipping figure is where they end up: 36 V rms into 4 ohm at slight clipping. The
solver's supply is an ideal source behind a resistance, so that resistance is fitted to
the figure: **7.52 ohm** (ESTIMATED). Into 8 ohm the amplifier then clips at **44.9 V rms**
(250 W), above the rated 200 W (40 V); the rating is the conservative figure, and a
transient into either load gets the full 85 V until the reservoir sags.

**What else is approximated**: the preamp's op-amps are the solver's ideal op-amp on the
+-14.5 V rails with a 13.5 V swing (none of them sits inside a second loop); the
MJ15022 / MJ15023 betas are one figure per place they work (60 for the outputs, 100 for
the drivers, 80 for the VAS; ESTIMATED from the class of part); the three output devices
a side are folded into one (exact for identical parts); the input zeners, the crossover's
outputs, the 100 W amplifier and bi-amp mode are not built. The crossover's input loads
the bus as it does in FULL mode.

**On the panel**: VOLUME on Drive, BOOST on the level knob (the masters on 10, as the
manual recommends for the cleanest sound), BASS / LOW MID / TREBLE as the Tone section's
three and HIGH MID beside them, Lo Cut and Mid Contour on the low and mid switches, Hi
Boost on the bright switch, -10 dB as the input switch. **American SS 800** is also
selectable behind any circuit, where the Master knob turns its LO master.

## The direct out

"An electronically balanced output that will put 500 mv rms into a 600 ohm load. It comes
after the effects loop" -- and, on the block diagram and the drawing, **before the boost,
the masters and the crossover**. It is this amplifier's DI: the preamplifier's output,
EQ and all, with nothing of the boost or the power amplifier in it. See `docs/DI.md`.

**As built**: the Preamp DI reads the return node (`american_800rb::DIRECT_OUT`), so the
boost and the masters move the amplifier and not the DI, as they do on the amplifier. It
is levelled by the circuit's own small-signal gain from the input to that node at the
drive (`src/direct_out.rs`, `examples/directout.rs`), not by the make-up, because the
make-up is measured through the boost, which clips at the calibration level: -0.3 dB re
the input; the boost from 0.2 to 0.9 moves the amplifier 8 dB and the DI 0.1 dB
(`tests/di.rs`). U4's balanced driver is not built; its gain is levelled out.
