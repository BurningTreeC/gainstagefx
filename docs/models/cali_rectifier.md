# Cali Rectifier (Mesa/Boogie Dual Rectifier, Rev F) research log

Engineering identity: **Mesa/Boogie Dual Rectifier**, two-channel, the RED channel in
its Modern setting, into four 6L6s.
- Stable ids: `amp_dual_rectifier` (`circuit`), `power_recto_6l6` (`power_amp`).
- Display: Cali Rectifier / Recto 6L6.

## Eight-question checkpoint (2026-09-16, before code)

1. **Revision.** The two-channel amplifier as Mesa drew it on the **RF-1F preamp board**,
   sheet dated 6-93: that is the Revision F circuit (serials 0643-3180, 1992-94), the
   revision before the Rev G that ran to 2000. Not the three-channel Rectifier (1999 on),
   not the Rectoverb or the Single Rectifier, and not Rev C.
2. **Original schematic found?** Yes, two Mesa-drawn sheets:
   - "MESA-BOOGIE DUAL RECTIFIER PREAMP RF-1F", 6-93;
   - "MESA-BOOGIE DUAL RECTIFIER POWER AMP".
   Both in the Dual Rectifier schematic set at
   [schematicheaven](https://schematicheaven.net/boogieamps/boogie_dualrectifier.pdf).
   The same six-sheet set is at
   [el34world](https://el34world.com/charts/Schematics/Files/Mesa_boogie/Boogie_dualrectifier.pdf)
   (read 2026-09-30, kept in the git-ignored `docs/schematics/Boogie_dualrectifier.pdf`):
   preamp, power amp, effects loop, power supplies, and the two switching-matrix
   sheets, the second of which carries Mesa's **LDR mode table**.
3. **Best source.** Those two sheets, which carry their own node voltages.
4. **Cross-check.**
   - [The Rectifier Guide](https://www.rectifierguide.com/) for what each revision is:
     Rev F (1992-94, serials 0643-3180, output transformer 562100 then 562105, power
     transformer 561136 then 561140, STR-420 6L6s); Rev G (1994-2000) is "the classic
     Rectifier sound".
   - The power amp sheet names the same transformer the guide does for the later Rev F:
     "XFMR #562105".
   - Mesa's two-channel owner's manual for what the controls are called and what the
     Vintage/Modern switch does.
5. **Signal path and values (RED channel, Modern; the drawing's own designators).**

```text
INPUT -- R211 1M to ground -- V1A 12AX7: R221 220k plate (200 V), R291 1k8 cathode with
        C31 1 uF (R271 47k switches in through LDR3 on the clean channel only)
  plate -- C21 .02 -- the interstage network:
        C9 .002 with R110 680k across it (LDR4), C10 82 pF with R322 2.2M across it
        (LDR1/LDR2), R321 2.2M to ground
  -- RED GAIN 1M with .001 across its wiper (LDR5/LDR6 choose which channel's gain pot)
  -- R241 470k -- V2A grid
V2A: R201 100k plate (280 V), R292 1k8 cathode with C32 1 uF, C12 20 pF; C55 .005 on its rail
  plate -- C22 .02 -- R242 470k -- V2B grid, R212 1M to ground
V2B: R202 100k plate (384 V), **R103 39k cathode with nothing across it** -- the cold stage
     this amplifier is known for -- C6 .001 across the plate
  plate -- C27 .02 -- V3A grid: R225 220k stopper, R106 330k to ground
V3A: R224 220k plate (213 V), R293 1k8 cathode with C33 1 uF
V3B: cathode follower, R207 100k cathode load (216 V), plate on the 415 V rail
RED tone stack: C4 680 pF to TRBL 250k, R273 47k, C23 .02 to BASS 1M, C24 .02 to MID 25k;
     fed from the follower through LDR8. R215 1M joins its input to the ORANGE stack's
     input, whose LDR9 is off. RED PRSNC: R254 22k and C8 .003 from the treble wiper
     into a 25k rheostat to ground. RED MSTR 1M from the same wiper.
power amp: V5A/V5B 12AX7 long-tailed pair, R104 90k and R281 82k plates with 120 pF each,
     R213/R214 1M grid leaks, R341 470 cathode, R353 10k tail, R372 4.7k below it,
     C40 .1 from there to V5A's grid, C3 75 pF, C31/C32 .047 to 1.5k grid stoppers,
     R222/R223 220k leaks to a -51 V bias (-39 V with EL34s), 1k 2 W screen stoppers,
     four 6L6, transformer #562105, 4 and 8-16 ohm taps
feedback: 8-16 ohm tap -- C51 .1 -- R276 47k (a second 47k in parallel through LDR20,
     "MORE FEEDBACK") -- LDR19 ("FEEDBACK") -- the tail node (R353/R372/C40).
     ORANGE PRSNC: C52 .1 into a 25k rheostat, from the same node to ground, on the
     amplifier's side of LDR19.
```

   **Mode table (switching matrix, sheet 2), for the loop:**

   | LDR | OR NORM | OR CLN | OR MOD | RD NORM | RD VINT |
   |---|---|---|---|---|---|
   | 19 FEEDBACK | ON | ON | OFF | **OFF** | ON |
   | 20 MORE FEEDBACK | OFF | ON | OFF | **OFF** | OFF |
   | 7 TREBLE ROLLOFF | ON | ON | OFF | OFF | ON |
   | 8 RD TONE CAPS | OFF | OFF | OFF | ON | ON |
   | 14 RD MSTR POT | OFF | OFF | OFF | ON | ON |

   The red channel's toggle is NORM/VINT; RD NORM is its Modern setting, which is what
   this model is. In it **the power amplifier has no loop** (DOCUMENTED): LDR19 lifts it.
   Both modes with a loop use 47 k (23.5 k in OR CLN). The presence the red channel's
   player turns is RED PRSNC in the preamplifier; ORANGE PRSNC, still hanging on the
   tail, is the other channel's knob.

   **Resolved 2026-09-30.** The audit of that day found the log's "presence network (25k
   pot, .1 uF, 47k/82k)" and the model's one 100 k loop with the 25 k / .1 uF shunted off
   it -- the JCM800 arrangement -- and could not re-read the sheet. Read again: the 47 k
   are R276, the 82 k was never in the loop (the nearest are R281, the inverter's plate,
   and R263 in the orange channel's master), and in the mode modelled the loop is not
   there at all. Changed, all from the sheet:
   - `RECTO_6L6` and `RECTO_6L6_TUBE` have no feedback. ORANGE PRSNC stays on the tail
     node, resting at half (ESTIMATED; linear, ESTIMATED -- the sheet prints no law).
   - The panel's Presence knob turns RED PRSNC (`rectifier::PRESENCE`, through
     `Gain::own_presence`): topology and values DOCUMENTED; up is more resistance and
     less treble shunted (PLAUSIBLE, from the name); linear (ESTIMATED). In a custom
     chain the knob still turns it, and the power stage's own presence rests.
   - R215 hung from the treble wiper to ground, loading the stack's output with 1 M. It
     is now on the follower, where the sheet has it (the orange stack behind it left out:
     APPROXIMATED, a few hundredths of a decibel through 1 M).
   - Calibration, power trim and kernels regenerated. Recto Lead and Recto Rhythm put
     back to their levels by `output_trim` (+1.47 and +0.82 dB); neither sets its own
     Presence.

6. **To be modeled exactly.** The RED channel from the jack to the master, with the
   interstage network in its Modern setting, the cold 39 k stage, the cathode follower and
   the RED stack with its presence; and in the power stage the inverter, the couplings,
   the leaks, the stoppers, the screens and the four 6L6s, with the loop as the mode
   table has it in that mode: lifted.
7. **Approximated and estimated.**
   - **Both rectifier settings are modelled** (2026-09-16). SILICON DIODE is the supply
     as a voltage behind a resistance; VALVE is two 5U4GB as a `Part::Rectifier`, which
     follows Child's law, so the rail sits lower and sags under load. They are two power
     amplifiers in the list -- Recto 6L6 and Recto 6L6 Tube -- because the amplifier has a
     switch. Matched is the silicon one. Measured: 482 V idling and 59 V of sag on a loud
     low note against 471 V and 83 V.
   - Supplies: the sheet's own node voltages are used to set the rails (200 V at V1A's
     plate, 280 V at V2A's, 384 V at V2B's, 415 V at the follower's).
   - The LDR switching (channel and mode) is modelled as the RED channel in Modern; the
     other channel's stack still loads this one through R215 1 M.
   - Output transformer: 6L6-class data from the Cali 6L6 stage, APPROXIMATED; no winding
     data for #562105 was found.
8. **Why.** No rectifier-valve device in the solver; no transformer data; the LDR matrix
   is a switching arrangement rather than a sound.

## Measured (`tests/rectifier.rs`, `examples/recto_op.rs`)

Against the sheets' own marked voltages:

| Node | Sheet | Model |
|---|---|---|
| V1A plate | 200 V | 194.3 V |
| V2A plate | 280 V | 286.5 V |
| V2B plate | 384 V | 397.4 V |
| V3A plate | 213 V | 210.2 V |
| follower cathode | 216 V | 211.1 V |
| inverter plates | 280 V | 281 / 277 V |
| inverter cathodes | 48 V | 47.3 V |

- The cold stage sits at **4.9 V** of cathode bias, where an ordinary gain stage sits at
  1.5 V, and at a guitar's level with the gain at 0.8 it makes 32 % distortion with the
  second harmonic six times the third: it is clipping one side of the wave, which is what
  a stage biased that close to cutoff does.
- The output valves idle at 16.7 mA and 8 W each on the sheet's -51 V, which is the cold
  bias these amplifiers are known for.
- At the calibration point (gain full, 0.122 V) the whole amplifier makes **22.5 %**
  distortion, fifth-harmonic led. It made 42 %, third-harmonic led, before 2026-09-30,
  and both corrections are in the difference (`examples/recto_op.rs`, whole voice, the
  old 100 k loop put back for comparison):
  - RED PRSNC's fixed parts, R254 and C8, shunt the stack's output at every setting of
    the rheostat. At the preamplifier's output they take the third harmonic from 24 %
    to 16 % and the fifth from 16 % to 8 %: 42.9 % to 23.6 % in all, 2.6 dB of level.
    Moving R215 alone changed the level by +1.3 dB and the distortion by under two
    points.
  - Opening the loop then moves the voice from 25.6 % to 22.3 %, the third falling
    from 15.6 % to 5.2 %: the preamplifier hands the power stage 3.7 V, which without
    the loop is its clipping knee, and there its own third passes through a null and
    turns over against the preamplifier's.
  - The power stage alone without its loop is 3 dB louder and distorts more at every
    level (10.5 % against 6.2 % at 3 V in); the old loop was that shallow.
- Tone stack: treble 6.9 dB of range at 4 kHz, bass 8.6 dB at 80 Hz, middle 6.2 dB at
  600 Hz, with the presence at half. The treble had 11.9 dB before 2026-09-30: RED
  PRSNC shunts the same node the treble wiper drives, so it takes the top the treble
  control would otherwise hand on.
