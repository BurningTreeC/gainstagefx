# Orange Dist (Boss DS-1) research log

Engineering identity: **Boss DS-1 Distortion**, the Japanese original with the Toshiba
**TA7136P**, built until November 1994. Stable ids `pedal_boss_ds1` (pedal slot) and
`pedal_boss_ds1_circuit` (circuit list). Display: **Orange Dist**.

Status: **IMPLEMENTED 2026-09-24** (`src/circuits/orange_dist.rs`). The first checkpoint
(2026-09-16) was built on ElectroSmash's drawing, which turned out to be the later DS-1A;
this one is built on Boss's.

## Eight-question checkpoint

1. **Revision.** The **DS-1 with the TA7136P**, the circuit on the records the preset list
   is waiting for (the 1991 sessions in [studio_pre.md](studio_pre.md)). Boss's own service
   notes say when it ended: "Because of IC modifications, the part name will be changed
   from DS-1 to DS-1A ... beginning from SN JG8100 of 1031 Lot (production in November,
   1994)". The two are **not** the same passive circuit with a different chip, which the
   first version of this log said. Read off Boss's two drawings side by side:

   | | DS-1 (TA7136P) | DS-1A (BA728N) |
   |---|---|---|
   | IC | TA7136P, one amplifier | BA728N, two: a buffer and the gain stage |
   | C5, into the IC | .47 uF | .068 uF |
   | R39 47 k and D8 on the IC input | absent | present |
   | C7, across the Dist control | 250 pF | 100 pF |
   | C8, in the gain stage's ground leg | 1 uF | .47 uF |
   | booster, clipper, tone, level, buffers | the same parts | the same parts |

   The later M5223AL and NJM2904L versions are revisions of the DS-1A and are not
   considered here.
2. **Original schematic found?** **Yes.** Boss, *DS-1 (DS-1A) Service Notes*, second
   edition, December 1994, pp. 4-5: "CIRCUIT DIAGRAM (DS-1)" (main board ASSY 7520551007,
   PCB 2291032607) and "CIRCUIT DIAGRAM (DS-1A)" (ASSY 70562634), with the parts legend on
   each. Read on ManualsLib (manual 1461407). Every value below is from the DS-1 page;
   `docs/schematics/` holds the page images and they are not committed.
3. **Best source.** That drawing, plus Toshiba's **TA7136AP technical data** (pp. 44-48 of
   a Toshiba data book: equivalent circuit, test circuits, application circuits,
   open-loop gain against frequency, maximum output against supply). The TA7136P is the
   same die; Boss's legend names it "TA7136P".
4. **Cross-check.** [ElectroSmash's "Boss DS-1 Distortion Analysis"](https://www.electrosmash.com/boss-ds1-analysis)
   (mirror: [electrosmash.mas-effects.com](https://electrosmash.mas-effects.com/boss-ds1-analysis)),
   which analyses the DS-1A (its drawing has the BA728N's buffer and C7 100 pF) and whose
   booster, clipper and tone therefore apply to both. Its figures against this model,
   measured by `examples/ds1_op.rs` and held by `tests/orange_dist.rs`:
   - booster: "the effect of the feedback resistor and capacitor (R7 and C4) will lower
     this gain to 35dB". **Model: 34.8 dB at 1 kHz, 36.0 dB at 3.3 kHz. Agrees.**
   - booster low end: "the freqs below 33Hz are attenuated". **Model: 28.9 dB at 300 Hz,
     20.1 dB at 100 Hz -- a corner near 800 Hz. Disagrees, and the model is right**: 33 Hz is
     C3 against R6 alone, but R7's shunt feedback divided by the stage's own gain puts
     about 3.5 k at the base, and 47 nF against 3.5 k is about a kilohertz. Boss's own
     output check (service notes p. 6: a 200 Hz, 100 mVpp square wave with three control
     settings, drawn at 1 ms and 0.2 V per division) shows the output falling back during
     the flat half of each cycle, which a 33 Hz corner could not do.
   - gain stage: "Gv max = 1 + (100K/4.7K) = 22.3 (26.5 dB)", minimum 1. **Model: 26.8 dB
     and 0.0 dB.** Its "fc = 72 Hz" is the DS-1A's .47 uF; the DS-1's 1 uF gives 34 Hz.
   - tone: "a scoop around 500Hz ... an overall -12dB (4 times) loss and at the notch the
     loss is about -20db". **Model, Tone at noon: -14.6 dB at 700 Hz, about -10 dB either
     side.** ElectroSmash's tone drawing leaves out **R15**, 2.2 k in series with C11, which
     is on both of Boss's drawings; without it the high-pass arm is its 1,063 Hz and the
     notch is deeper and lower. With it the high-pass is 800 Hz and R17 is tapped at
     6.8 of 9 k.
5. **Signal path and values.** From the DS-1 page, Boss's designators.

```text
INPUT -- R1 1k -- C1 .047 -- Q1 2SC2240GR emitter follower, R2 470k to VB, R3 10k emitter
  -- C2 .47/50, R4 100k to VB -- Q6 2SK30ATM GR (bypass switch, on) -- R5 1M to VB
  -- C3 .047 -- Q2 2SC2240GR common emitter: R8 10k collector, R9 22 emitter,
     R7 470k and C4 250p collector to base, R6 100k base to ground
  -- C5 .47/50, R10 100k to VB -- IC TA7136P pin 2 (+)
     feedback: VR1 100KB (Dist) from pin 6 (lug 1) to R13 4.7k -- C8 1/50 -- ground (lug 3),
     wiper through R11 100k to pin 3 (-), C7 250p pin 6 to wiper;
     C6 150p pin 1 to pin 6, C24 150p pin 1 to ground, R12 27k V+ to pin 5
  -- R14 2.2k -- C9 .47/50 NP -- D4, D5 1S2473 or 1S1588 back to back to VB, C10 .01
  -- tone: R16 6.8k to lug 1 of VR3 20KB with C12 .1 to VB;
           C11 .022 -- R15 2.2k to lug 3 with R17 6.8k to VB; wiper to VR2
  -- VR2 100KB (Level), lug 1 to VB -- R18 10k -- Q7 2SK30ATM Y (switch, on), R19 1M to VB
  -- C13 .047 -- Q3 2SC732TM GR emitter follower, R20 1M to VB, R21 10k emitter
  -- R22 1k -- C14 1/50 -- R23 100k -- OUTPUT
supply: 9 V, D1 shunt reverse-polarity clamp, C23 100/16;
        VB from R24/R25 10k with C15 47/6.3;
        bypass flip-flop Q4/Q5 2SC2458GR, R26-R33, C16-C21, D6/D7, LED D10 with R35 3.9k
```

   TA7136P pins, from Toshiba's equivalent circuit and application circuits: **2 is the
   non-inverting input and 3 the inverting** (the application circuits feed the signal to
   2 and the feedback to 3, as Boss does), 1 is the first stage's output -- the
   compensation node -- 5 sets the bias current, 6 is the output, 7 and 4 the supply. The
   bias equation is Toshiba's: `I5 = (Vcc - Vee - 1.4) / R`, 300 uA intended; R12 27 k on
   9 V gives 281 uA.

   Derived corners: C1 against R2, 7 Hz; C2 against R4, 3.4 Hz; C3 against the booster's
   own input, near 800 Hz; C5 against R10, 3.4 Hz; C8 against R13, 34 Hz (Dist up) to
   1.5 Hz (down); C7 against 100 k, 6.4 kHz (Dist up); R14 into C10, 7.2 kHz before the
   diodes conduct; tone 234 Hz low-pass and 800 Hz high-pass; C13 into R20, 3.4 Hz; C14
   into R23 and the load, under 5 Hz.
6. **To be modeled exactly.** Every part in the audio path above, by value: both
   followers, the booster with its shunt feedback and its own asymmetric clipping, the
   gain stage with its frequency-selective feedback and C7, the diode pair with C10
   across it, the tone blend with R15, and the level control. All three pots are
   **linear** ("100KB", "20KB", "100KB"), and are modelled as linear: the Dist control
   does little in its first half and most of its work in its last quarter, as on the box.
7. **To be approximated, and why.**
   - **TA7136P.** The rodent/741 construction (transconductor into an integrator, open-loop
     resistance, swing clamps, ideal follower) with its own numbers. Open-loop gain 92 dB
     (DOCUMENTED, typical). Output swing +/-4.0 V about the bias point (DOCUMENTED: the
     maximum-output curve reads 2.8 V rms at 0.1 % THD on +/-4.5 V with the bias resistor
     chosen for 300 uA). **Gain-bandwidth 2 MHz and slew 0.6 V/us are ESTIMATED**: the
     sheet's uncompensated open-loop curve (84 dB at 10 kHz, 72 dB at 30 kHz) is about
     140 MHz against the chip's internal capacitance; C6's 150 pF across the second stage
     divides that by `(150 pF + C_int) / C_int`, 1-3 MHz for an internal 1-3 pF. Nothing
     here depends on it: at 27 dB of closed-loop gain the loop still reaches 90 kHz, and
     C7 closes it at 6.4 kHz. C6, C24 and R12 are therefore inside the model rather than
     parts of it.
   - **2SC2240GR, 2SC732TM GR**: the catalogue's 2SC1815 GR, the same Toshiba family and
     rank. APPROXIMATED.
   - **1S2473 / 1S1588**: the catalogue's silicon switching diode. APPROXIMATED.
   - **Q6, Q7 (2SK30ATM)**: the electronic bypass's switches, on, as their channel
     resistance: 300 ohms, ESTIMATED from `1 / gm0` (Toshiba guarantees |Yfs| >= 1.2 mS;
     the GR and Y ranks sit above that). Q7 is in series with R18's 10 k and does not
     matter. Q6 is in series with C3 into the booster's 3.5 k; measured with 100 and 800
     ohms in its place, the booster moves by under a decibel below 3.3 kHz and by three
     at 10 kHz, where C7 and C10 are already cutting.
   - The flip-flop, the LED and the bypass path are not audio when the pedal is on and
     are not built. Battery resistance is the supply's 47 ohms, as for every pedal.
8. **Why.** Boss's drawing gives every value; the chip's data sheet gives its gain, pins,
   bias and swing; one figure (its compensated bandwidth) has to be estimated, and the
   circuit is built so that it does not matter.

## What it is for

The plugin's other distortions clip in a loop (Green 808) or straight after one op-amp
(Rodent, Yellow Dist). The DS-1 is the one where **a transistor stage clips first,
asymmetrically, and a diode stage clips what is left** -- two clippers in series, with the
bass cut *before* them rather than after, and a passive scoop at the end. It is what the
1991 sessions in the preset list used in front of their preamplifier.

## Level

`LEVEL_REST` is unity through the pedal with Dist and Tone centred, measured with
`examples/pedallevel.rs` the way every pedal's is.
