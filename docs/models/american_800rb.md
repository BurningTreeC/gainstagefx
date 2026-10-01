# American 800RB and American SS 800 (Gallien-Krueger 800RB) research log

Engineering identity: **Gallien-Krueger 800RB** bass amplifier (1983 on): a solid-state
preamplifier with voicing filters, a four-band active EQ, an effects loop, a balanced direct
out, a footswitchable boost and a tunable crossover into two power amplifiers, 100 W and
300 W. Proposed stable ids `amp_gk_800rb` (`circuit`) and `power_gk_800rb` (`power_amp`), as
the roadmap reserves them. Proposed display: **American 800RB** and **American SS 800**.

Status: **CLEARED 2026-10-01** on GK's own service manual. Not built; preamp rev C chosen.

## Eight-question checkpoint

1. **Revision.** GK's service manual carries two preamp drawings, both titled "NEW 800RB
   PREAMP": **rev C** (406-0045-C, 8/13/91, LF353s, a TL604 switching the boost) and **rev
   D/E** (406-0045-D/E, 5/25/95, a DG419 for the boost, different op-amps, and changed
   values around the effects loop -- R63 220 R against 4.7 k, R73 470 R against 12 k,
   C61 / C70 3.3 uF). One power amplifier drawing (406-0044-B, 9/3/91, "100W + 300W POWER
   AMPS") and one supply. The turn-on procedure (420-0045-C, 8/19/91) names boards
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

## The direct out

"An electronically balanced output that will put 500 mv rms into a 600 ohm load. It comes
after the effects loop" -- and, on the block diagram and the drawing, **before the boost,
the masters and the crossover**. It is this amplifier's DI: the preamplifier's output,
EQ and all, with nothing of the boost or the power amplifier in it. See `docs/DI.md`.
