# British 47 (EMI REDD.47) research log

Engineering identity: **EMI Line Amplifier Type REDD.47**, the "cassette type" valve
amplifier of EMI's Record Engineering Development Department, used as the microphone
and line amplifier of the REDD.37 and REDD.51 consoles. Stable id
`pre_emi_redd47` (`circuit`). Display: **British 47**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::british_47`; microphone preamplifiers),
on EMI's own drawing and description, with R. Gorbutt's 2001 redraw as the cross-check
for the two readings the 150 ppi scan could not settle on its own.

## Eight-question checkpoint

1. **Revision.** The REDD.47 of EMI's drawing **REDD.47/C1, dated 1.10.59** ("LINE
   AMPLIFIER, CASSETTE TYPE", E.M.I. International, Hayes, Middlesex, Record Engineering
   Development), and EMI's description **Ref. REDD.M47**: two valve stages, an EF86 and
   an E88CC with its halves in parallel, resistance-capacitance coupled "with two loops
   of negative feedback", three gains, 0 dBm into 200 ohm matched or +6 dBm from a low
   source, +14 dBm on overload peaks.
2. **Original schematic found?** **Yes**: EMI's own drawing and its description, in
   the PDF Sowter keeps (`sowter.co.uk/schematics/EMI-REDD47.pdf`, which also carries
   pages about a modern R.47 -- ignored). The drawing is a rotated scan cut into strips
   and is legible once reassembled; a copy belongs in `docs/schematics/` (git-ignored).
3. **Best source.** That drawing for values; REDD.M47 for intent and the gains.
4. **Cross-check.** REDD.M47's prose -- R5 120 ohm across which "the overall negative
   feedback voltage is developed", R1 damping the input transformer's resonance with C13
   removing it at low frequencies -- against the drawing, which agrees; and the drawing's
   own voltages and currents (V1 plate 73 V, cathode 1.65 V, 1.35 mA from 205 V; V2 plate
   138 V, cathode 3.7 V, 18 mA; 290 V and 30 mA at 380 V from the power unit), which a
   model has to land on. Three selectable gains, 34, 40 and 46 dB (S1).
5. **Signal path and values** (EMI's designators).

```text
T1 REDD.A92, 1:7 step-up from the 200 ohm input; C14 0.75-1.8 uF chosen on test
   (EMI's table: 18 H with 1.8 uF, 45 H with 0.75 uF); R1 100k across the secondary
   with C13; R24 3k the rumble filter (strap 2-4 to remove it)
V1 EF86: plate R2 100k from the 205 V node; screen R3 (390k, PLAUSIBLE at this scan)
   with C3 .47; cathode R4 1k over R5 120 ohm -- the feedback resistor -- with VR1 500
   (fine gain, limits 77-120 ohm) and R6 240, C2 100 uF
V2 E88CC, both halves in parallel (R25, R26 1k grid stoppers): C4 .02 in, R10 1M; R8
   330k and R9 1.6M (the inner loop, to be traced at full resolution); plate R11 8.2k
   (5 W) from the 18 mA node; cathode R12 200 ohm with C6 100 uF; out through C5 to
   T2 REDD.A83, 7:1, with R23 150 ohm; 200 / 50 ohm outputs
S1, the gain: C8 680 pF, C7 330 pF, R15A 6.8k, R14 10k, R13 47k, R15B 120k -- the
   outer loop's network onto R5, for 34, 40 and 46 dB
supply: from power unit REDD.43/D23/2, 290 V; two OA2 regulators (V3, V4), R16 470k,
   R17 220k; R18-R21 1k and C9-C12 8-40 uF to the 205 V and 18 mA nodes
```

6. **To be modeled exactly.** Both stages, both loops, the gain switch's three
   networks, R1/C13 and C14 at their mid value, both transformers' ratios. The
   regulated supply as the stiff nodes it is.
7. **To be approximated, and why.**
   - **EF86 and E88CC**: no small-signal pentode and no E88CC triode fit exist in the
     catalogue. Each from its data sheet (Mullard EF86; Philips/Mullard E88CC), checked
     against the drawing's voltages. ESTIMATED until then.
   - **T1 and T2's losses and cores**: ratios printed, T1's inductance bracketed by
     EMI's own table (18-45 H), the rest not. ESTIMATED.
   - The values that are marginal at this scan (R3's first digit, C13) to be confirmed
     at full resolution before code; PLAUSIBLE until then.
8. **Why.** EMI's own drawing and its own description of it, agreeing, with voltages
   and the gains to measure against.

## What was settled before code, and how

- **The valves.** `PentodeSpec::EF86` (`tools/tube_fit/fit_ef86.py`, Philips 1970) and
  `TriodeSpec::E88CC` (`tools/tube_fit/fit_e88cc.py`, Philips 1968).
- **The inner loop.** EMI's own words settle it -- "negative feedback is therefore
  applied directly from anode to grid ... This feedback voltage is developed across the
  resistor R8, which also serves to provide an adequate load resistance for the first
  stage" -- and Gorbutt's redraw draws it so: **V1's anode -- R8 330 k -- J; J -- R9
  1.6 M -- V2's anode; J -- C4 .02 -- V2's grids, R10 1 M to ground.** C4 keeps V2's
  grid at ground, so the E88CC fit's question (does R9 hold the grid up?) is answered:
  it does not, and the pair runs at 17.3 mA against the drawing's 18 on the fit alone.
- **R1 and C13** are in series across T1's secondary, as REDD.M47 says ("R1 damping
  the input transformer's resonance with C13 removing it at low frequencies"); C13 is
  100 pF (Gorbutt agrees; the scan reads "1?0pF").
- **S1** as Gorbutt draws it and EMI lists it: R15A 6.8 k with R15B 120 k and C8 680 pF
  across them, then R14 10 k (C7 330 pF across it), then R13 47 k to T2's primary, the
  switch shorting the output to the 6.8 k's end (34 dB) or to the 10 k's (40 dB) or
  neither (46 dB). The check is EMI's: **steps of 6.1 and 5.9 dB measured, 33.9 /
  40.0 / 45.9 dB**, with only the middle fitted. Built as a `Taper::Steps` rheostat on the Drive
  knob's thirds; C7's 330 pF cannot follow a rheostat and is not built (48 kHz corner).
- **Where the gains are measured.** "Between terminals 2/5 and 2/6": the secondary
  itself. R23's 150 ohm build-out leads to the other output terminal (2/13) and is not
  in that path; built at 2/6 into 200 ohm.
- **The rumble filter** (C14 and R24 on T1's primary taps) is strapped out as EMI's
  note describes ("disconnect 2-4 and connect 3-4"): the tap wiring is not legible at
  this scan. A documented configuration, not an invention.

## Measured (`examples/british_47_op.rs`, held by `tests/british_47.rs`)

- Gains into 200 ohm, input terminals to output: **33.9 / 40.0 / 45.9 dB** against
  REDD.M47's 34 / 40 / 46, with the fine gain at 659 ohm across R5 -- R5 shunted to
  101.5 ohm, inside EMI's stated 77-120 ohm.
- Operating point: V2's pair 17.3 mA, anode 146 V (18 mA, 138 V); V1 1.54 mA, anode
  86 V (1.35 mA, 73 V) -- the EF86 fit's own 1.50 against 1.35 for this stage.
- Response at 40 dB, source to load: -0.3 dB at 20 Hz, -2.2 at 10 Hz (two-second
  tones at 48 kHz), -0.6 at 20 kHz (at 96 kHz).
  T2's primary is ESTIMATED at 100 H: it sees 200 ohm times 49, 9.8 k, so that keeps
  its own corner near 16 Hz.
- Distortion: 0.35 % at 0 dBu out, 1.4 % at +10 dBu, rising steeply past that
  -- EMI's "+14 dBm on peaks", 2.24 V in 200 ohm -- to the limit near +14 dBu.
- ESTIMATED: both transformers' inductance, copper and cores; the OA2 pair as 290 V
  behind 100 ohm.
