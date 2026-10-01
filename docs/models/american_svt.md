# American SVT (Ampeg SVT, 6550) research log

Engineering identity: **Ampeg SVT**, the 300 W valve bass head with six 6550s, in the
revision Ampeg (then a Magnavox company) drew in 1974 and revised to the end of 1975.
Stable ids `amp_ampeg_svt` (`circuit`) and `power_svt_6550` (`power_amp`), as
[ROADMAP.md](../ROADMAP.md) section B reserved them. Display: **American SVT** and
**American 6550**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::american_svt`, power stage
`power::PowerSpec::SVT_6550` with `power::FollowerFront::SVT`).

## Eight-question checkpoint

1. **Revision.** The 6550 SVT, from the two factory drawings: **D 591719 "SVT PREAMP"**,
   revision D (ECN-P9517, 12/5/75: redrawn, pilot light revised, speaker enclosure
   added), and **D 591720 "SVT POWER AMP SCHEMATIC"**, revision H (patch jack removed,
   350 V supply on pin 2). Drawn by P. Reisher, May 1974. The 1969-71 SVT with 6146Bs
   (`svt1969` in the same Ampeg set) is a different power amplifier and is out of scope,
   as are the SVT-VR / SVT-CL reissues, which are only used as cross-checks below.
2. **Original schematic found?** **Yes**, both sheets, from Ampeg's own support page
   ([ampeg.com/support/schematics.html](https://ampeg.com/support/schematics.html)); copies
   are in `docs/schematics/` (git-ignored). Scanned at about 300 ppi and legible value
   by value except where noted under 7. Both sheets carry DC voltages ("no signal applied
   using a 20,000 ohm per volt voltmeter" on the preamp; "electronic volt meter" on the
   power amp), and the power amp sheet carries its **AC test voltages in boxes** and a
   **calibration procedure**.
3. **Best source.** Those two drawings.
4. **Cross-check.**
   - The power amp sheet's boxed AC voltages at full output: **0.257 V** at V1's grid,
     5.14 V and 4.52 V at the inverter's outputs, 37.5 V at a follower's grid, 33.2 V at
     the output grids, 372 V on each half primary, and **34.6 V across the 4 ohm load**
     -- 299 W, the SVT's 300. That is the power amplifier's closed-loop gain (42.6 dB)
     and its transformer ratio, both DOCUMENTED by the drawing itself.
   - The calibration procedure: each bank of three 6550s set to **0.072 V across its
     1 ohm** cathode resistor (R35, R36), 24 mA a valve, and the two banks equal to
     0.01 V. That is the idle current, DOCUMENTED.
   - Ampeg's own **SVT-VR owner's manual** (the reissue, the same channel 1 networks):
     BASS +/-12 dB at 40 Hz, TREBLE +/-12 dB at 4 kHz, MIDRANGE +/-20 dB at the selected
     frequency, all flat at centre; the 1-2-3 switch at **220 Hz, 800 Hz, 3 kHz**
     (left, centre, right); ULTRA HI "dependent on the setting of the volume control";
     BASS CUT / OFF / ULTRA LO "inactive in the center position". The Heritage SVT-CL's
     manual puts ULTRA LO at +2 dB at 40 Hz and -10 dB at 500 Hz. WIDELY REPORTED for
     this revision (the manuals are for later amplifiers built to the same networks).
5. **Signal path and values** (Ampeg's designators; channel 1, NORMAL input).

```text
PREAMP D 591719 (B+ "+300 V": 430 V at P1 pin 5 through R47 8.2k 5 W, C23 40 uF)
J2 NORMAL -> R2 47k -> V1 12AX7 pin 7; R4 5.6M grid leak (J1 BRIGHT: R1 100k || C1 .0047)
V1a: R3 220k plate (170 V), R5 3.3k cathode (1.75 V), unbypassed
V1b: grid on V1a's plate, plate on B+, R6 220k cathode (190 V) -- a follower
C2 .1 -> SW1 BASS SELECT (slide, 3 positions):
   R8 150k, R9 150k with C3 .002 from their junction to ground;
   C4 .002, C5 .002 with R10 820k + R11 68k from their junction to ground
   BASS CUT: through C4 and C5 only; OFF: straight through; ULTRA LO: R8-R9 / C4-C5
   as a twin-T with R11 (R10 shorted), centred about 530 Hz
VR1 1M LIN VOLUME; SW2 ULTRA HI puts C6 500 pF from VR1's top to its wiper
V3a 12AX7: R24 47k plate, R25 4.7k cathode, unbypassed
C7 .1 -> tone stack (P.E.C. 250762-1):
   220k / VR5 1M LOG BASS / 22k to ground, .001 across the bass pot's upper half,
   .01 across its lower half, R26 120k from its wiper to VR6's wiper;
   .00047 / VR6 1M LOG TREBLE / .0047 to ground; output from VR6's wiper
C14 .01 -> V3b 12AX7 grid; R27 1M to the R29/R30 junction
V3b: R28 220k plate (180 V), cathode R29 560 + R30 7.5k (20 V)
C15 .01 -> V4a 12AX7: R31 1M grid leak, R32 470k plate (130 V), R33 3.3k cathode
V4b: grid on V4a's plate, plate on B+, cathode (135 V) R37 47k + R36 6.8k to ground
R35 56k from V4b's cathode back to V3b's cathode   <- a two-stage feedback loop
MIDRANGE: V3b's cathode -> C16 .68 -> R34 470 -> one end of VR7 50k LIN;
   the R37/R36 junction -> C19 .68 -> R39 620 -> the other end, R38 220k to ground;
   VR7's wiper -> L1 toroid tap 4; SW5 grounds tap 1 (220 Hz), or tap 2 through
   C18 .15 (800 Hz), or tap 3 through C17 .033 (3 kHz); R40, R41 220k bleed C17, C18
OUTPUT: R37/R36 junction -> C20 .1 -> R42 100k -> mix; channel 2 -> R23 100k -> mix
   mix -> C21 .02 -> V5 6C4 follower: R43 1M to the R44 1k / R45 47k junction,
   plate on B+, cathode 170 V; C22 .1, R46 100k -> P1 pin 1 -> the power amp

POWER AMP D 591720
in -> R1 1k -> D1/D2 1N456 to ground -> V1 12AX7 pin 7, R2 470k leak
V1a: R3 220k plate from C (362 V), cathode R4 2.2k + R5 220 to ground, unbypassed;
   the feedback lands on the R4/R5 junction
C1 .1 -> V1b, a CATHODYNE: R6 15k plate from C (280 V), cathode R8 1k (95 V) then
   R9 10k and VR3 15k LIN BAL CONT to ground, R7 1M grid leak to the R8/R9 junction;
   20 pF from V1a's plate to V1b's plate, C2 120 pF from V1b's grid to ground
C3 / C4 .1 from the plate / cathode -> V2a / V3a 12BH7, R10 / R11 470k leaks
V2a, V3a: R15 / R14 47k 5 W plates from H (342 V, 159 V), R12 / R13 1.8k cathodes (7 V)
C5 / C6 .047 600 V -> R16 / R18 150k -> V2b / V3b 12BH7 grid; R17 / R19 150k from
   that point to the wiper of VR2 / VR1 15k LIN BIAS CONTROL, which sits in
   R20 / R23 39k from D (-150 V) and R21 / R22 22k to ground
V2b, V3b: CATHODE FOLLOWERS, plates on E (350 V), cathodes R24 / R25 47k 1 W to D;
   the cathode drives three 6550 grids directly through 47k each (R29, R34, R43 ...)
6 x 6550 (V4-V9): plates through 5.1 ohm 5 W each, screens from E through 22 ohm each
   with a diode (D15-D20) across each 22 ohm, each bank's cathodes R35 / R36 1 ohm 5 W
T3 320825-1: two paralleled secondaries, 4 ohm and 2 ohm taps; R46 47k || C7 120 pF
   from the 4 ohm tap to V1a's R4/R5 junction
supply: bridge (D3-D10) -> A 660 V on C10 100 uF + C12A 100 uF in series (R47, R48 220k);
   R49 4.7k 15 W -> B 462 V (C11 30 uF); R50 15k 1 W -> C 362 V (C12B 40 uF);
   second winding: D11, D14 -> E 350 V (C9A 100 uF); R52 1k -> H 342 V (C9B 40 uF);
   D12, D13 -> D -150 V (C8 90 uF)
```

6. **To be modeled exactly.** Channel 1 from the NORMAL jack: every value above, the
   James tone stack, the V3b-V4 feedback loop with the midrange network in it, the
   channel mix and the 6C4 follower. In the power amplifier: V1a with the loop on its
   cathode, the cathodyne, both 12BH7 gain stages, both direct-coupled cathode followers
   with their bias networks, six 6550s with their plate, screen and stopper resistors,
   and the transformer's ratio from the boxed voltages. This is a new front end for the
   power stage (`FollowerFront`): no stage here had a cathodyne, or followers between
   the inverter and the output valves.
7. **To be approximated, and why.**
   - **L1, the toroid.** Its inductance is not on the drawing (part 320821-1 only).
     ESTIMATED from the three published frequencies and the drawing's capacitors: each
     selected branch resonates with C16's .68 in series, so tap 1 is 0.770 H, tap 2
     0.322 H with C18, tap 3 0.0894 H with C17. A toroid's inductance goes as turns
     squared, and these are 100 / 65 / 34 % of the turns: a plausibly tapped winding.
     Its copper is not known and is not built; R34's 470 and R39's 620 set the Q.
   - **The switches** (on the panel since 2026-10-01, the second pass). ULTRA HI rides
     on the panel's Bright switch (a bright capacitor across the volume, as the Twin's
     and the JC-120's are). BASS SELECT and MIDRANGE SELECT are the panel's **low
     switch** and **mid switch**, three positions each, built as the contacts they
     close (adjustable resistors, 1 ohm closed, 1 G open): BASS SELECT's four
     contacts between the network's nodes, MIDRANGE SELECT's three, one in front of
     each toroid section and its capacitor. The three sections are three inductors,
     not one tapped winding: only the section in circuit carries current
     (APPROXIMATED). The netlist is built at OFF and 800 Hz, where it is calibrated.
   - **Channel 2** is not built as an amplifier, but it loads the mix node: its stack
     (the same P.E.C. part) is built with its knobs at noon, driven from V2b's plate as
     39 k to ground (R20's 100 k against a 12AX7's plate resistance near 65 k at its
     0.7 mA). APPROXIMATED.
   - **R47 reads "82K 5W"**: 82 k would pass 1.6 mA, and the sheet's own voltages ask
     for 16 mA (430 V to 300 V). **8.2 k**, which the 5 W rating also says. The rail is
     built as 430 V behind 8.2 k and C23, and channel 2's two triodes are a resistor
     drawing their 1.3 mA. PLAUSIBLE.
   - **Power amp R4 reads "22K"**; the sheet's 1.7 V on V1a's cathode with V1a's 0.7 mA
     (207 V against 362 V through 220 k) asks for **2.2 k + 220**. PLAUSIBLE.
   - **V3a's printed voltages** (200 V plate, 3.2 V cathode) do not agree with its
     parts (47 k, 4.7 k): 100 V across 47 k is 2.1 mA, 3.2 V across 4.7 k is 0.68 mA.
     The parts are built as drawn and the solver finds its own point.
   - **VR3, the balance**, is set where the cathodyne's two outputs are equal: R8 + R9 +
     VR3 = R6, so 4 k of VR3. The procedure sets it for least distortion at 25 V out,
     which is the same thing. ESTIMATED.
   - **The bias pots** rest where each bank idles at the procedure's 24 mA a valve
     (`examples/svt_op.rs`).
   - **D15-D20** across the 22 ohm screen resistors are not built: they conduct only
     past 27 mA through 22 ohm, so they move a 350 V screen by less than a volt.
     **D1/D2** at the input are not built: they conduct past 0.6 V and the full-power
     drive is 0.257 V. The 1 ohm cathode resistors are not built (72 mV).
   - **Supplies.** C (V1's) is 462 V behind R50 15k and C12B, B itself stiff (the
     Sunn's precedent: B feeds the preamp too, whose draw is in another netlist). A is
     660 V at idle behind an ESTIMATED 50 ohm with the 50 uF of C10 and C12A in series.
     E is 350 V at idle behind an ESTIMATED 150 ohm with C9A, and it feeds the screens,
     both followers' plates and, through R52 1k and C9B, both 12BH7 gain stages -- so
     the drivers sag with the screens, as they do. D is -150 V behind an ESTIMATED
     1 k with C8.
   - **T3**: the ratio is from the drawing (744 V plate to plate for 34.6 V on 4 ohm,
     21.5, so 1.85 k plate to plate, 5.5 k for each pair of valves). Inductance,
     leakage, copper and core are ESTIMATED as for the Sunn, sized to hold 300 W at
     40 Hz.
8. **Why.** Two factory drawings with DC voltages, AC test voltages and a calibration
   procedure; the only unpublished values are the toroid's inductance, which three
   published frequencies constrain, and the transformer's and supplies' internals.

## Measured

Preamplifier (`examples/american_svt_op.rs`, held by `tests/american_svt.rs`):

- Operating point against the sheet: V1a cathode 1.76 V (1.75), V3b cathode 21.3 V (20),
  V4a cathode 1.15 V (1.2), V4b cathode 138 V (135). The rail lands at 322 V where the
  sheet prints 300, and V1a's anode at 205 V against 170: this model draws less than
  the sheet's 12DW7 build did (the 1969 drawing, 669-0580, has 12DW7s in V1, V3 and V4,
  R25 1.5 k, and the very voltages the 1975 sheet still prints for V3a).
- MIDRANGE at 800 Hz: **+21.5 / -19.1 dB** around noon (Ampeg: +/-20), centred there --
  +8 / -7 dB an octave either side -- and leaving 100 Hz within a decibel.
- BASS at 40 Hz +15.5 / -10.9 dB, TREBLE at 4 kHz +17.5 / -16.9 dB around noon. Ampeg
  rates both +/-12 on the SVT-VR; the drawn James stack has more treble range than
  that, because a full treble boost recovers the stack's midband insertion loss. Built
  as drawn; recorded.
- ULTRA HI lifts 8 kHz by more with the volume low than high (tested), as Ampeg says.
- **Direction**: VR7 was first wired with turned-up at the follower's end, which cut
  the band; the sheet's arrow and Ampeg's manual put clockwise at V3b's cathode, where
  it boosts. Corrected.

Power amplifier (`examples/svt_op.rs`, `examples/svt_lf.rs`):

- Fitted to the idle: A 660.0, E 350.0, D -150.0 V, **24.0 mA a valve** (the
  procedure's 0.072 V across a bank's 1 ohm), with the bias controls at 0.8865 of their
  track and the open-circuit voltages 667.2 / 353.6 / -158.5 V. Not fitted, and landing
  near the sheet: H 342.1 V (342), the 12BH7 gain stages' cathodes 7.1 V (7) and plates
  156 V (159), the followers' grids -69.7 V (-77) and cathodes -43.4 V (-47). The
  inverter does not land: C at 415 V (362), V1b's plate and cathode 379 / 35 V (280 /
  95) -- a 12AX7 cannot pass the 5.5 mA the sheet's V1b figures imply, and Ampeg itself
  notes "DC voltages for V1 are approximate".
- The drawing's boxed AC voltages at the drawing's 0.257 V in: **31.8 V into 4 ohm
  (253 W)** against 34.6 (299 W), -0.7 dB. Stage by stage: the inverter's outputs 3.9 V
  (5.14 / 4.52), the followers' grids 29.7 V (37.5), the output grids 26.8 V (33.2), the
  half primary 342 V (372); the drivers' gain 7.6 (7.3) and the followers' 0.90 (0.885)
  land on the boxes, the shortfall is V1's.
- Small-signal, closed loop: -1.7 dB at 12 Hz, -6.7 at 8 Hz, flat above 20 Hz -- no
  low-frequency peak with the ESTIMATED 20 H primary.
- Driven to three times full output: the followers' grids stay at least 10 V below
  their cathodes (no grid current in the followers) while the output grids reach +1.25
  V -- class AB2, held by the 47 k stoppers. After such a burst the 12BH7 gain stages
  have blocked (C3 charged to -17 V; 0.1 uF into 470 k, 47 ms) and their recovery
  reaches the output grids through C5 as +27 V, then -17 V, gone in about 100 ms.

## Second pass, 2026-10-01: the switches and the cabinet

Measured (`examples/american_svt_op.rs`, held by `tests/american_svt.rs`):

- **MIDRANGE SELECT**: with MIDRANGE up, the boost peaks at 220 Hz, 800 Hz and 3 kHz
  for positions 1, 2 and 3, +21.1 / +21.5 / +21.5 dB -- Ampeg's frequencies, by the
  toroid sections' construction.
- **BASS SELECT** against OFF: BASS CUT -17.1 dB at 40 Hz, -8.6 at 100, -0.5 at 3 kHz
  (C4 and C5 alone into VR1, a second-order high-pass). ULTRA LO -18.2 dB at 500 Hz and
  -2.7 at 40 Hz: the drawn twin-T notches deeper than the Heritage SVT-CL's published
  "-10 dB at 500 Hz" and does not lift 40 Hz by its "+2 dB". The CL is a later amplifier
  with its own network; this one is the 1975 drawing's. Recorded, not tuned.
- Thrown from the panel through `apply`, the switches allocate nothing.

The cabinet: the **American 8x10** (Ampeg SVT-810E, four sealed chambers of two) with
**American Bass 10s** (Eminence Legend B810, 32 ohm). See `cabinets.md` and
`speakers.md`. The preset now plays through it: -12.2 dB untrimmed against the
catalogue's -12.8.
