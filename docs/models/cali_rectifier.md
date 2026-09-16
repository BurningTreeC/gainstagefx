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
RED tone stack: C4 680 pF to TRBL 250k, R273 47k, C23 .02 to BASS 1M, C24 .02 to MID 25k,
     R215 1M between the two channels' stacks; RED MSTR 1M
power amp: V5A/V5B 12AX7 long-tailed pair, R104 90k and R281 82k plates with 120 pF each,
     R213/R214 1M grid leaks, R341 470 cathode, R353 10k tail, C40 .1 cross, C3 75 pF
     C31/C32 .047 to 1.5k grid stoppers, R222/R223 220k leaks to a -51 V bias (-39 V with
     EL34s), 1k 2 W screen stoppers, four 6L6, transformer #562105, 4 and 8-16 ohm taps
     feedback from the 8-16 ohm tap through the presence network (25k pot, .1 uF, 47k/82k)
```

6. **To be modeled exactly.** The RED channel from the jack to the master, with the
   interstage network in its Modern setting, the cold 39 k stage, the cathode follower and
   the RED stack; and in the power stage the inverter, the couplings, the leaks, the
   stoppers, the screens, the four 6L6s and the feedback with its presence control.
7. **Approximated and estimated.**
   - **The rectifier itself is not modelled as valves.** The amplifier's name is its
     switchable silicon/valve rectifiers; the supply here is a voltage behind a resistance,
     so the model is the silicon setting. The valve setting's sag would need a rectifier
     device. This is the one thing the name promises that the model does not do, and it is
     stated on the panel description as well as here.
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
- At the calibration point the whole amplifier makes 42 % distortion, third-harmonic led.
- Tone stack: treble 11.9 dB of range at 4 kHz, bass 9.4 dB at 80 Hz, middle 6.5 dB at
  600 Hz.
