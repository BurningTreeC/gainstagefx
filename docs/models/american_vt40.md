# American VT-40 and American V-4B (Ampeg VT-40, V-4B) research log

Engineering identity: two Ampeg valve amplifiers of 1971 from one family -- the
**VT-40**, a 60 W 4x10 combo with reverb, and the **V-4B**, the 100 W bass head -- both
with a 12AX7 input per channel, the passive bass and treble of a packaged network into a
6K11 compactron, the midrange selector's tapped inductor (the SVT's idea), a 12DW7, a
12AU7 inverter and four 7027As. Proposed stable ids `amp_ampeg_vt40`,
`amp_ampeg_v4b`, `power_ampeg_7027a`. Proposed display **American VT-40** and
**American V-4B**, after the American SVT.

Status: **VT-40 IMPLEMENTED 2026-10-02** (`circuits/american_vt40.rs`,
`power::PowerSpec::VT40_7027A` with `power::ParaphaseFront`, `PentodeSpec::T7027A`), on
Ampeg's own drawing and service data, **with its reverb** (same day, later); **V-4B
IMPLEMENTED 2026-10-02** (`circuits/american_v4b.rs`, `power::PowerSpec::V4B_7027A`) on
its own drawing. Results below.

## Eight-question checkpoint

1. **Revision.** The 1971 drawings, both: the VT-40 service section (12/71-195 to 202:
   specification, parts lists, transformer data, a.c. and tube-pin voltage tables,
   chassis layout, schematics) and the V-4B schematic 12/71-223 with its power and tone
   board pictorials (12/71-222, -221). The album rig accounts for *Songs for the Deaf*
   (2002) put a VT-40 at the centre and V-4Bs beside it (`brit_200.md`); their years are
   not given, and the 1971 circuit is what Ampeg documents.
2. **Original schematic found?** **Yes: Ampeg's own**, published on its support page
   (ampeg.com/support/schematics.html), with DC voltages on the drawings and, for the
   VT-40, a pin-by-pin tube voltage table. In `docs/schematics/ampeg_v4b/` (not
   committed).
3. **Best source.** Those drawings. Ampeg's page also carries the V-4, VT-22 (the 2x12
   VT-40) and the V-4B's later revision (V-4BH).
4. **Cross-check.** The voltages on the drawings: the V-4B's 7027A plates 540 V and
   screens 527 V, bias -64 V, the 12AU7 inverter's plates 200 V and cathodes 9.7 V, the
   12DW7 at 325 / 205 V, the 6K11's sections at 192 / 178 / 325 V; the VT-40's at
   354 V on the preamp rail, V1 205 / 2.2 V, the 6K11's 209 / 194 V. The VT-40's
   specification: 60 W RMS into 2, 4 or 8 ohm. The SVT's own midrange inductor and
   switching (built, `american_svt.md`), which these drawings repeat.
5. **Signal path and values** (VT-40, from the drawing; the V-4B the same family):

```text
V1 12AX7 (channel 1, "bright"): C1 .005 in, R1 220 k, R5 390 k plate, R3 33 k / R4 6.8 k
   cathode with C2 10 uF and the SENSITIVITY switch (R2 6.8 k), C3 .01 to VOL 1M lin
   (C102 120 pF, C101 .001 the bright and ultra hi parts)
V1b: R6 68 k plate, R7 3.3 k cathode, C4 .01 to the P.E.C. 6470000 (bass and treble),
   BASS 1 M log, TREBLE 1 M log, R101 120 k
V201 6K11: the tone network's recovery and the midrange (VR105 50 k lin with the
   inductor), R204 220 k, R207 470 k, R201 1 M, R202 560, R203 7.5 k, R208 56 k,
   R211 47 k, R210 6.8 k, R212 620, C203 / C204 .33 uF
V3 12DW7, V4 12AU7, the inverter, and V5-V8 7027A -- values here from the V-4B's
   drawing (12/71-223), the VT-40's still to be read from its own: R30 / R31 47 k
   plates, R28 1.5 k, R29 / R27 1 M, R33 820 k; 47 k grid leaks, 470 R 2 W screens; the
   output transformer's 2 / 4 / 8 ohm taps; R34 4.7 k // C14 .001 in the feedback
VT-40 only: V202 6CG7 and V203 12AX7, the reverb's driver and recovery
```

6. **To be modelled exactly.** Each stage as drawn, the midrange inductor and its
   selector, Ultra Hi, the sensitivity switch, the 7027A push-pull with fixed bias.
7. **To be approximated, and why.**
   - **The P.E.C. 6470000** (Centralab), the bass and treble network in a seven-pin
     package: its parts are WIDELY REPORTED -- two resistors, 220 k and 22 k, and four
     capacitors, 470 pF, 4.7 nF, 1 nF and 10 nF (Music Electronics Forum, "71 Ampeg
     V4B Questions"); the drawing shows its topology between pins 1 to 7 but not which
     part sits where. **Open**: a drawing with it discrete (the thread cites Ampeg's
     B-12XT; the file Ampeg's page links under that name is a B-15T owner's guide) or a
     measurement, before the network is built. Placing the six parts by a Baxandall's
     own logic would be PLAUSIBLE, not documented.
   - **The 6K11** (a compactron triple triode) and **the 6CG7** need fits from their
     data sheets; the **7027A** is a 6L6GC in another base with higher ratings, the
     solver's 6L6GC fit a first approximation (PLAUSIBLE).
   - **The reverb** (VT-40): the plugin's spring tank, as the Twin's.
8. **Why.** The album preset *Desert Deaf '02* is blocked on them (`docs/ROADMAP.md`,
   section C).

## Settled since the checkpoint (2026-10-02)

- **The P.E.C. 6470000 is the SVT's James stack.** Ampeg's own SVT drawing (D 591719
  rev D, built here as `american_svt::james_stack`) draws its tone module, P.E.C.
  250762-1, with every part placed: 220 k / 1 M log BASS / 22 k, .001 across the bass
  pot's upper half and .01 across its lower, .00047 / 1 M log TREBLE / .0047, 120 k from
  the bass wiper to the treble's. Those six parts are exactly the six reported for the
  6470000 (220 k, 22 k; 470 pF, 4.7 nF, 1 nF, 10 nF), around the same 1 M log pots, and
  the 120 k sits outside the package on both 1971 drawings (R101 on the VT-40, R109 on
  the V-4B). DOCUMENTED by Ampeg for the SVT's part; the same network in the 6470000 is
  PLAUSIBLE to the point of near certainty (six of six parts and the topology).
- **The 6K11 needs no fit.** RCA's and GE's sheets give its medium-mu unit the 12AU7's
  published characteristics (mu 17, 7.7 kohm, 2.2 mA/V, 10.5 mA at 250 V / -8.5 V) and
  its two high-mu units the 12AX7's (mu 100, 62.5 kohm, 1.6 mA/V, 1.2 mA at 250 V /
  -2 V): it is built from the solver's ECC82 and ECC83. Pins (RCA): unit 1 (medium-mu)
  4 / 9 / 10, unit 2 5 / 6 / 7, unit 3 2 / 3 / 11.
- **The 6CG7 is fitted** (`TriodeSpec::T6CG7`, `tools/tube_fit/fit_6cg7.py`), to
  Tung-Sol's sheet of 1 December 1959: six targets within 1.1 %.
- **The 6K11 is the SVT's midrange loop in one bulb**: unit 3 (220 k plate, 560 R +
  7.5 k cathode, 1 M grid return), unit 2 (470 k, 3.3 k), unit 1 a follower at 196 V on
  47 k + 6.8 k with 56 k back to unit 3's cathode, and the MIDRANGE pot, toroid and
  selector between them -- the SVT's V3b, V4a and V4b with the same values but .33 uF
  (C203, C204) where the SVT has .68, and a 12AU7-like follower where the SVT's is a
  12AX7. The toroid is a different part (8910001; the SVT's is 320821-1), and its
  selector frequencies are not on the VT-40's drawings: to find, or ESTIMATED.
- **Two 7027As or four**: the VT-40's specification page prints "(4) 7027A", its
  schematic (DWG 06500, rev B, 4-71) draws two, V5-6, at 586 V, and its heater string
  and valve voltage table carry V5 and V6 only. The schematic is followed: two.


## What was settled in building it (2026-10-02)

Read part by part off DWG 06500 B at 400 dpi, with the service section's parts lists
(12/71-196, -199), transformer data (-197) and A.C. voltage readings (-198):

- **The input.** C1 .005 in series from J1, R1 220 k to ground: a 145 Hz corner against
  the leak -- channel one is the BRIGHT input. V1a: R5 390 k, R4 6.8 k to ground (the
  bias), R3 33 k with C2 10 uF and the **SENSITIVITY** rocker (a DP3T, the parts list):
  High shorts R3, Middle puts R2 6.8 k across it, Low leaves R3 alone (drawn). **Built
  at Middle**: with it V1a turns the A.C. table's 0.3 V at 400 Hz into 9.4 V against the
  table's 10; Low gives 7.3 and High 15.6. The table, not the drawn position, decides.
- **ULTRA HI** is a DP3T too: up bridges C102 120 pF across VOL 1's upper half; the
  centre leaves C102 and C101 in series from the wiper back to the wiper, nothing;
  down puts C101 .001 from the wiper to ground, a treble cut (drawn). The panel's Bright
  switch offers up and centre; the cut is not built. APPROXIMATED.
- **The mixer.** V1b and V2b share R6 68 k (cathodes R7 / R13 3.3 k, unbypassed): the two
  inputs are mixed on one plate. Channel two's volume at zero, V2b is built with its
  grid at ground, as the load it is.
- **The 6K11 loop** is the SVT's exactly, R209 / C203 in the other order: unit 3 grid
  pin 11, plate 2, cathode 3; unit 2 grid 7, plate 5, cathode 6 (R206 3.3 k
  unbypassed); unit 1 grid 9, plate 10, cathode 4. R208 56 k from the follower to unit
  3's cathode, R211 47 k / R210 6.8 k, C205 .1 off their junction. MIDRANGE SELECT
  (SW5, DP3T): the wiper on L101's top, tap 1 to ground, tap 2 through C107 .15, tap 3
  through C106 .033, R103 / R102 220 k from taps 2 and 3. **The toroid's sections**
  (8910001) are ESTIMATED from the VT-40's WIDELY REPORTED "+/-20 dB at 300, 800 or
  3,000 Hz" (dealer listings quoting Ampeg's specification) and the drawn capacitors:
  0.853, 0.384 and 0.0938 H.
- **The reverb send** is the treble wiper itself: V202's grid hangs on the P.E.C.'s
  output beside C201, so the send loads the dry path only by a grid. **The return**:
  V203's plate, C212 .005 into VR106's wiper (500 k linear REVERB), C105 .0022 off its
  top, the bottom grounded, R104 150 k from C105's far side to ground -- and **R15
  270 k from there to the mix**. The dry signal reaches the mix through C205 and R14
  180 k and returns through R15 and the return network: a horizontal line across the
  whole sheet (a fold or rule in the scan) runs through C205, C7 and R19 and looks like
  a wire; it is not one, and R14's top reaches C205 round the top of the board.
- **V3**, the 12DW7: Tung-Sol's sheet (1 July 1960) gives section 1 (pins 6 / 7 / 8)
  the 12AX7's figures and section 2 (1 / 2 / 3) the 12AU7's. V3a (pins 1-3) is the
  bootstrapped follower (C7 .01 in, R16 1 M to the R17 1 k / R18 47 k junction, cathode
  224 V), the drawing's 4.7 mA asking for the 12AU7 half; V3b (6-8) the gain stage
  (R22 220 k, R23 3.3 k, R24 33), 0.59 mA. C8 .47, R19 10 k, the EXT AMP jacks J3 / J4
  on R20 100 k, R21 10 k to V3b's grid.
- **The inverter is a floating paraphase with a shared cathode.** V3b's plate through C9
  .01 to V4's pin 2 (R27 1 M); both cathodes on R26 1.5 k, unbypassed, to ground;
  R29 820 k from the driven plate and R30 1 M from the other to a junction, C11 0.1 from
  it to pin 7, R25 1 M leak. R28 / R31 47 k 1 W from the 393 V node.
- **The output stage**: C12 / C10 .33, R32 / R33 100 k 5 % to -65 V, R35 / R34 47 k
  stoppers, 7027A pin 4 screens through R36 / R37 470 2 W, pins 8 to ground. D7-D10
  from the plates to the supply are protection and are not built. The loop: R48 4.7 k
  with C13 500 pF from the ORANGE (8 ohm) secondary to the R23 / R24 junction. The
  four 10" CTS speakers (8 ohm each) make 8 ohm on that tap; J5 moves them to the GRN
  (4 ohm) tap with an extension in.
- **Ampeg's output transformer data** (89500300): 6 k plate to plate, 2-4-8 ohm,
  **300 ohm of primary copper**, 0.418 ohm on the 8 ohm secondary. DOCUMENTED; the
  ratio and copper are built (the secondary's copper is not), the inductance, leakage
  and core ESTIMATED. The power transformer: 594 V at 150 mA, 6.3 V at 4 A.
- **The supply chain does not add up on the drawing.** 594 V -- R42 3 k -- 436 V -- R41
  3 k -- 393 V -- R40 2.2 k -- 354 V, the resistors confirmed by the parts list (3 k
  10 W wire-wound, 2.2 k). R41's 43 V is 14 mA, less than the 17.7 mA R40's 39 V passes
  on from the same node, with the inverter's 7.4 mA still to come off it. The model keeps
  the node voltages: the preamplifier hangs from a stiff 393 V through R40 (with the
  valves not built as one 102 k draw, DERIVED from the drawing's plate voltages), which
  puts its rail at 356.7 V against 354; the power stage's 393 V node is an open-circuit
  voltage behind R41, fitted.
- **The 7027A** got a fit of its own (`tools/tube_fit/fit_7027a.py`) from RCA's 7027-A
  sheet (8-59), whose class A row is the 6L6GC's to the figure: that row within 2.5 %,
  6.04 mA/V and 22.5 kohm, the three fixed-bias rows within 13 % (they disagree with
  each other), the 300 V / 300 V curve held out within 10 % from -30 to -10 V. The
  solver's `T6L6GC`, made for the amplifiers before this one and held by the frozen
  baselines, gives 23 mA where the sheet gives 50 at 540 / 400 / -38 and barely conducts
  at -65 V with 589 V of screen. **Two 7027As**, as the schematic draws them.

## Results (`tests/american_vt40.rs`, `examples/vt40_op.rs`)

No-signal DC, model against the drawing:

| | model | drawing |
|---|---|---|
| preamp rail | 356.7 V | 354 |
| V1a plate, cathode | 229.0 V, 2.23 V | 205, 2.2 |
| V1b plate, cathode | 262.4 V, 2.29 V | 250, 2.9 |
| 6K11 unit 3 plate, cathode | 231.3 V, 25.3 V | 209, 24 |
| unit 2 plate, cathode | 162.1 V, 1.37 V | 194, 1.37 |
| unit 1 (follower) cathode | 169.2 V | 196 |
| V3a cathode | 221.9 V | 224 |
| V3b plate, cathode | 225.4 V, 1.95 V | 223, 1.96 |
| V4 plates, cathodes | 230.0 V, 10.40 V | 218, 10.5 |
| 7027A grids, screens | -65.0 V, 587.7 V | -65, 589 |

The 12AX7 plates run 5-12 % high: the solver's 12AX7 (Koren's) is a little under the
published current, as everywhere in the catalogue. **Unit 2's plate is the drawing's
inconsistency, not the model's**: the drawing's own 1.37 V on its 3.3 k cathode is
0.42 mA, which through R207's 470 k puts the plate near 160 V -- where the model has it
-- not at the 194 V printed; the follower's cathode follows. Each 7027A idles at 42 mA,
25 W, under the 35 W rating; the drawing's 8 V between the rail and the plates is 53 mA
through 150 ohm of half primary, so the real valves idle hot too.

Ampeg's A.C. voltage readings, 0.3 V at 400 Hz in, treble, bass, midrange and reverb at
the middle, channel one's volume set (0.049) to put the table's 0.5 V on V1b's grid:

| pin | model | table |
|---|---|---|
| V1a plate (volume full) | 9.44 V | 10 |
| V1b plate | 5.54 V | 5.8 |
| 6K11 pin 11 / pin 3 / pin 2 | 0.477 / 0.470 / 0.102 V | .47 / .5 / .1 |
| 6K11 pin 5 / pin 4 | 5.06 / 4.71 V | 5.2 / 5 |
| V3 pin 2 / pin 3 | 0.324 / 0.302 V | .32 / .3 |
| V3 pin 7 / pin 6 | 0.27 / 3.15 V | .28 / 3.5 |
| V4 grids | 3.15 / 2.87 V | 3.4 / 3.4 |
| V4 plates | 34.1 / 32.4 V | 38 / 38 |
| V5 grid / plate | 32.4 / 314 V | 38 / 250 |
| 8 ohm | 22.9 V | 17.9 (three-quarter power) |

**Every stage from V1b's grid to V3a's cathode within a decibel of Ampeg's table** --
the tone network, the 6K11's loop, the mix through the reverb return, the follower --
**and V3b and the paraphase within 1.5 dB**, low, because the loop hands them back
more of a hotter output. **The output valves are hotter than the table**: 314 V on a plate for
32 V on its grid, where the table has 250 V for 38 V, so the loop gives the inverter a
little less and the speaker ends 2.1 dB above the table's 17.9 V. The transformer is not
the difference: the table's own plate-to-speaker ratio is Ampeg's 6 k into 8 ohm. Not
settled; candidates are the 7027A's knee, which in Koren's law does not rise with the
screen voltage as a real beam tetrode's does at 589 V, and the screen supply under
signal. Recorded, not tuned. The midrange lifts 21-22 dB at 300 Hz, 800 Hz and 3 kHz
(Ampeg: +/-20 dB). The same at 44.1 to 192 kHz; realtime-safe; presets *American
VT-40 Crunch* and, with its evidence table in PRESETS.md, *Desert Deaf '02*.

**Approximated, beyond the toroid and the output valves' gain**: channel two's input
stage; ULTRA HI's cut position; the stiff 393 V and 436 V nodes; the transformer's
inductance, leakage and core and the secondary's copper; the supply's source
resistance (ESTIMATED, 100 ohm); D7-D10; the tank (below). The cabinet: the plugin has
no 4x10 combo of its own; the presets use the American 4x10 (a bass cabinet) with its
horn off and a 10" guitar speaker, the American Vintage 10 (Jensen P10R).

## The reverb (built 2026-10-02)

The first build left the tank silent, with V203 as a fixed impedance behind the REVERB
pot. Now the whole echo board is in the netlist and the spring between its two halves
is `dsp::spring::Tank`, as the Twin's is:

- **The send** is the treble wiper: V202's first unit (6CG7, R213 120 k, R214 2.2 k
  unbypassed) has its grid on it, beside C201. C206 .01 into the second unit (R215 22 k
  to ground, R216 330 with C207 10 uF), its plate on R217 10 k 5 W from the 436 V node,
  C208 .022 to ground, and **C209 .47 straight into the tank's coil** -- no transformer.
- **The tank's input**, from Ampeg's own table. The parts list calls the unit "Reverb
  Unit Type 4C" (63100060) and gives no impedance. Read as the Accutronics code's "C"
  it would be a 200 ohm coil, and with it V202's plate reads 4.3 V for the table's
  2.3 V. A gain of 1.2 from a valve on a 10 k load needs a coil that resonates with
  C209 near the 400 Hz the table was taken at: a high-impedance input, the code's
  "F", 1475 ohm at 1 kHz -- 0.235 H with a tenth of that in copper, as the 4AB3C1B's
  8 ohm coil carries 0.8 -- resonating near 480 Hz. With it the plate reads 1.7 V
  (-2.6 dB). ESTIMATED; the table chose it.
- **The drive**: `Tank` is calibrated in volts across the 4AB3C1B's 8 ohm coil (Elliott
  Sound Products' measured transfer). The same power in a 1475 ohm coil moves the same
  spring, so the coil's current is taken to that tank's volts as `I sqrt(8 x 1475)`
  (`american_vt40::TANK_DRIVE_SCALE`). APPROXIMATED.
- **The return**: the pickup through the Twin's 2.25 k transducer (the Type 4C's output
  is not given; ESTIMATED) onto R219 22 k, C210 .005 from there to V203's bypassed
  cathode -- a treble shunt -- and R218 10 k into the grid; V203 a 12AX7 half on R221
  270 k and R220 1 k with C211 10 uF; C212 .005 from its plate onto the REVERB pot's
  wiper (500 k linear, the bottom grounded), C105 .0022 off its top, R104 150 k and
  R15 270 k to the mix. **REVERB is the panel's Reverb knob** for this voice; it rests
  at zero, where the panel starts, so calibration measures the amplifier with it down.
- The footswitch (SW4, which shorts the return) is not built.

The chain's tank plumbing, which belonged to the AB763s and their tremolo together, is
split: `Gain::spring_reverb` for any voice with a tank, `Gain::ab763` for the tremolo,
and the panel greys Reverb and Speed / Intensity separately.

## The V-4B (built 2026-10-02)

DWG 06700 rev A (5-71), 12/71-223, read at 600 dpi. **The same family, part for part
where it overlaps**: the P.E.C. 6470000 with R109 120 k, the 6K11's loop and midrange
with every value the VT-40's (C113 .033, C114 .15, R110 / R111 220 k on the selector),
V3 the 12DW7 follower and gain stage, the floating paraphase (R30 / R31 47 k, R28 1.5 k,
R33 820 k, R32 1 M, C12 0.1), .33 couplings, 4.7 k of feedback from the 8 ohm tap. Both
build from one `american_vt40::tone_section`. Its own:

- **No reverb**, no SENSITIVITY: J1 straight to V1a's grid on R2 5.6 M, R6 390 k, R5
  3.3 k unbypassed; C3 .1 (the VT-40's .01) and C7 .1 into the network.
- **ULTRA LO**, four poles a channel (the switch gangs both channels): off ("OFF
  (LEFT)", drawn) feeds VOL 1 straight; on feeds it through C101 .015 to ground and
  three .0047s in series (C102-C104) with R102 1.5 M and R103 1.2 M between them,
  bypasses V1b's cathode with C106 6.8 uF (R107 47 k keeps it charged), and hangs C22
  .03 (R59 5.6 M) from the mixer's plate. Measured (`examples/v4b_op.rs`): -2 dB at
  80 Hz, -1 dB at 150 Hz, -13 dB at 600 Hz, -34 dB at 2.5 kHz -- a band that keeps
  the bottom and takes the rest away. On the panel's low switch.
- **ULTRA HI**, C105 500 pF across VOL 1, on the Bright switch.
- **The power stage**: four 7027As, two a side, 47 k stoppers and 470 ohm 2 W screen
  resistors a valve, R35 / R36 100 k to -64 V; plates on the 545 V rail through the
  primary (540 V printed at the plates); screens from a 540 V node behind R55, 470 ohm
  7 W. The drawing prints 527 V at the screen pins, which through 470 ohm would be
  28 mA a screen; not used. The plates' "PR1 5W" resistors are a part code, PLAUSIBLY
  parasitic suppressors like the SVT's 5.1 ohm plate resistors, and are not built.
  C14 .001 across R34 4.7 k (the VT-40's 500 pF).
- **Its supply chain adds up** where the VT-40's does not: 545 V -- R53 4.7 k -- R52
  4.7 k -- 360 V (the inverter) -- R51 2.2 k -- 325 V (the preamplifier). The
  preamplifier hangs from a stiff 360 V through R51; the inverter's node is fitted
  behind R52 + R53.
- **The output transformer** (8950028, which Magnavox later numbered 320822-1) has no
  data located: ESTIMATED at 3 k plate to plate, half the VT-40's 6 k for twice its
  valves, with half its copper, sized for 100 W at 40 Hz.

Results, model against the drawing (`tests/american_v4b.rs`, `examples/v4b_op.rs`):

| | model | drawing |
|---|---|---|
| preamp rail | 331.0 V | 325 |
| V1a plate, cathode | 165.7 V, 1.40 V | 155, 1.48 |
| mixer plate, V1b cathode | 243.8 V, 2.11 V | 230, 2.1 |
| 6K11 unit 3 plate, cathode | 215.4 V, 23.5 V | 192, 22 |
| unit 2 plate, cathode | 150.8 V, 1.26 V | 178, 1.26 |
| follower cathode | 157.3 V | 140 |
| V3a cathode | 205.4 V | 200 |
| V3b plate, cathode | 207.4 V, 1.78 V | 205, 1.8 |
| inverter plates, cathodes | 211.1 V, 9.50 V | 200, 9.7 |

Unit 2's plate is the drawing's inconsistency again: its own 1.26 V cathode puts the
plate near 146 V. Each 7027A idles at 25.5 mA, 13.9 W. No A.C. table was published
for the V-4B. The same at 44.1 to 192 kHz; realtime-safe; one preset, *American V-4B
Growl*.
