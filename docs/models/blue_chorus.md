# Blue Chorus (Boss CE-2) research log

Engineering identity: **Boss CE-2 Chorus** (1979): an input buffer, pre-emphasis, a
Sallen-Key anti-aliasing filter, an MN3007 bucket brigade on an MN3101 clock swept by a
triangle LFO, a matching reconstruction filter and de-emphasis, and the delayed signal
mixed half and half with the dry one. Two knobs, Rate and Depth. Proposed stable ids
`pedal_boss_ce2` (`pedal`) and `pedal_boss_ce2_circuit` (`circuit`); proposed display
**Blue Chorus**.

Status: **IMPLEMENTED 2026-10-02** (`src/circuits/blue_chorus.rs`), pedal and circuit,
on Boss's own service notes, obtained by the owner. Results below.

## Eight-question checkpoint

1. **Revision.** The CE-2 of 1979, made in Japan, until 1991, and its bass sibling the
   CE-2B (the same service notes cover both). To be confirmed on the notes' own revision
   letters.
2. **Original schematic found?** **Yes.** Boss's **CE-2/CE-2B SERVICE NOTES, First
   Edition**, dated FEB.1987 ("This notes includes the contents of the CE-2 First Edition
   and makes it obsolete"): two A3 pages scanned at 400 ppi -- the CE-2's specifications,
   parts list with Roland part numbers, board layout ET-50B (PCB 052-516B), adjustment
   procedure and circuit diagram, and the CE-2B's on the second page. Local copy
   `docs/schematics/boss_ce2/bossCE-2.pdf` (git-ignored). The redrawn schematics and the
   Tonepad layout that circulate are not used.
3. **Best source.** The service notes: every value, the semiconductors (IC1 uPC4558C,
   IC2 TL022CP, IC3 MN3007, IC4 MN3101, Q1-Q3 2SC732TM-GR, Q4-Q8 2SC945-P, Q9 2SK30A-Y),
   the specifications (input impedance 470 k, output load over 10 k, S/N 90 dB IHF-A,
   maximum input 0 dBm at 100 Hz and -10 dBm at 1 kHz) and the BBD-bias adjustment (VR-3
   for a centred operating point).
4. **Cross-check.** ElectroSmash's analysis of the same drawing, block by block, with
   its figures: input impedance 407 k; pre-emphasis +15 dB above 2.3 kHz (corners 410 Hz,
   2.341 kHz) and the de-emphasis its mirror; anti-aliasing and reconstruction each a
   Sallen-Key pair at 6.6 and 6.9 kHz, behind high-passes at 48 Hz and 14.6 Hz; MN3007
   1024 stages, delay 5.12 to 51.2 ms across the MN3101's range; LFO a TL022 to 3.5 Hz,
   smoothed by a 72 Hz low-pass; dry and wet at 50 % (R21 = R22 = R24 = 47 k). To be
   checked against the notes, not used in their place.
5. **Signal path and values** (read off the notes at 400 ppi; the clock section still
   to be traced in full before code):

```text
INPUT -- C1 .047 -- R1 1 k -- Q1 2SC732TM-GR follower (R2 470 k to the bias line,
   R3 10 k emitter load) -- C2 .47
IC1a pre-emphasis, inverting: R4 47 k in parallel with C3 .0068 + R5 10 k, feedback
   R6 47 k // C4 100 p, +in through R7 10 k to the bias line
   (unity below 410 Hz, x5.7 / +15 dB above 2.34 kHz: ElectroSmash's figures, DERIVED)
   -- the dry signal leaves here for the mixer (R21 47 k)
anti-aliasing: C5 .033, R8 100 k to the bias line, R9 / R10 / R11 10 k, C6 .0033 to
   ground, C7 .0082 to Q2's emitter, C8 470 p at Q2's base; Q2 2SC732 follower,
   R12 10 k -- R13 4.7 k -- IC3 MN3007 IN (biased by VR3: "a centered operating point")
IC3 OUT1 + OUT2 together, R14 56 k -- C10 .033, R15 330 k to the bias line
reconstruction: R16 / R17 / R18 10 k, C11 .0033 to ground, C12 .0082 to Q3's emitter,
   C13 470 p; Q3 2SC732 follower, R19 10 k -- C14 .033, R20 1 M to the bias line
   -- R22 47 k -- Q9 2SK30A-Y (the effect switch, on) -- IC1b -in
IC1b mixer and de-emphasis: dry through R21 47 k, wet through R22; feedback R24 47 k //
   C16 100 p // (C15 .0068 + R23 10 k); +in on the bias line -- R25 470 -- C17 1 uF --
   OUTPUT, R26 100 k
LFO: IC2 TL022, a Schmitt (R29 33 k, R30 47 k, both to pin 5, its non-inverting input;
   pin 6 on the bias -- read at full resolution, after a first read at a smaller scale
   had them on pin 6, which could not oscillate) and an inverting integrator (R32 1 M,
   C19 .1): a triangle of +-33/47 of the swing, period 4 R32 C19 (0.70) / k with k
   RATE's share, 1 to 10/110 -- 0.32 Hz to 3.6 Hz, DERIVED; ElectroSmash: "3.5 Hz";
   RATE VR1 100 kB with R31 10 k, DEPTH VR2 100 kB from the integrator's output;
   R35 220 k / C21 .01 into Q4 2SC945, R36 / R37 4.7 k, D1, R38 150 k, R39 33 k,
   R40 2.7 k, Q5 and D2 setting IC4 MN3101's clock with C22 47 p
bias line: VR3 10 kB between R27 4.7 k and R28 4.7 k, C18 47 uF
```

   **The clock**, which the notes give as parts and not as figures: a DIYstompboxes
   measurement of a real CE-2 ("BOSS CE-2: clock frequencies") reads about 110 kHz at
   minimum depth and 100-128 kHz at middle depth -- 4.0 to 5.1 ms through 1024 stages
   (512 / f). SECONDARY; the clock's law against Q5's control is the MN3101's internal
   oscillator, which is not published, so the model will take the measured range.
6. **To be modelled exactly.** The analogue path -- buffer, emphasis, both filters, the
   mixer -- as netlists, and the LFO's waveform and range.
7. **To be approximated, and why.** The bucket brigade itself, as the JC-120's already
   is (`dsp::bbd`): a fractional delay whose length the clock range gives, because
   solving 1024 stages at the clock rate is not a trade this plugin makes; the CE-2's
   own clock range and LFO in place of the JC-120's. ESTIMATED where the notes are
   silent (the MN3101's clock against its control voltage).
8. **Why.** The chorus asked for by the owner, and the pedal-slot partner of the
   JC-120's chorus already built.

## Results (2026-10-02)

Measured by `tests/blue_chorus.rs`; `examples/blue_chorus_op.rs` prints the same and fits
the clock.

- **The emphasis pair**: the dry signal, which takes both, is flat to 0.3 dB from 100 Hz
  to 8 kHz; the pre-emphasis alone lifts 14.3 dB from 100 Hz to 10 kHz (the arithmetic's
  15 dB, less the C4 / R6 corner at 34 kHz).
- **The anti-aliasing filter** into the brigade: -5.1 dB at 6.8 kHz re 1 kHz and -32 dB
  at 15 kHz -- ElectroSmash's poles at 6.6 and 6.9 kHz; the reconstruction filter is the
  same network.
- **The oscillator**: 0.35 Hz at RATE down, 3.8 Hz up (0.32 to 3.6 Hz from the values;
  ElectroSmash's "3.5 Hz"). Its triangle swings +-1.87 V about the bias.
- **The delay**: Q4's emitter rests at -0.895 V re the bias, where the clock law puts
  110 kHz, **4.65 ms**; at middle depth the sweep is 28 kHz wide, **4.15 to 5.35 ms**.
- **The comb**, depth down: the notches where the delay is an odd number of half
  periods, -8.9 dB near 340 Hz and +4.2 dB at the peak at 430 Hz -- but **not in the
  bass**: the wet signal is high-passed at 102 Hz on its way into the mixer (C14 into
  R22's 47 k on IC1b's virtual earth), and at 48 Hz (C5, R8) and 15 Hz (C10, R15)
  before that, so at 107.5 Hz it is weaker than the dry and turned, and the first notch
  is -1 dB. A test of mine expected a deep notch there; the drawing says otherwise, and
  Boss evidently kept the chorus out of the bass.

**Approximated**:
- **The bucket brigade itself** (`dsp::bbd::Brigade`, the JC-120's Hermite delay line):
  no clocking, no sampling images (above every rate the plugin runs at), no charge-
  transfer loss, no clipping. The service notes' "maximum allowable input" (0 dBm at
  100 Hz, -10 dBm at 1 kHz) comes to about 1 V peak at the brigade's input after the
  pre-emphasis -- the MN3007's headroom -- and is not modelled.
- **The clock law**: linear in Q4's emitter, through the measured 110 kHz at rest with
  the measured 28 kHz across the middle-depth sweep. The measurement's 100-128 kHz is
  centred on 114 kHz, which no plausible monotonic law reconciles with 110 at rest; the
  linear law sweeps 96-124 kHz. The MN3101's oscillator and D1 / R38 / Q5 / D2 / C22 are
  not built: the law reads Q4's emitter. ESTIMATED, on SECONDARY figures.
- **IC2a's bandwidth**, 5 kHz rather than the TL022's 0.5 MHz: the comparator's flip is
  an instability far faster than a sample, and the solver, stepping over it, found the
  balance between the thresholds a stable point and stopped there (64 Newton passes a
  sample, the triangle parked at the threshold). At 5 kHz the flip takes a millisecond;
  the triangle's period is longer by a fraction of a per cent. The integrator keeps the
  part's figures.
- **C19 starts charged** past the lower threshold (`Netlist::charged`), as the Orange
  Phase's C10 does, so the oscillator starts at once.
- **The effect switch** held on: Q9 a 400 ohm resistor, the flip-flop and bypass not
  built. The transistors' betas at the middle of their ranks; the TL022's and 4558's
  swing 1.5 V from each rail (ESTIMATED).
- **Referenced to the bias line** (VR3 at its middle, 4.5 V), which C18 holds still.

## What building it needed

One netlist after all, not two: the brigade's input is a node of it and its output an
auxiliary input, the way the Twin's reverb tank is sent and returned. A circuit says so
in `Gain::bucket_brigade` (send node, return input, clock-control node, delay law); the
chain holds a preallocated `dsp::bbd::Brigade` for the pedal and one for the circuit,
sized for 9 ms at 192 kHz times the deepest oversampling, and in the first half sets
the return before each solve and pushes the send after it -- one sample of latency in a
loop of milliseconds. The delay lines reset when the pedal or the circuit changes.
