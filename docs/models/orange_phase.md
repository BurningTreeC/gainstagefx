# Orange Phase (MXR Phase 90) research log

Engineering identity: **MXR Phase 90** (MX-101), Keith Barr's 1974 phaser: an input
buffer, four first-order all-pass stages whose resistance is a JFET each, an output mixer
summing the shifted signal with the dry one, and a one-op-amp LFO on the JFETs' gates. One
knob, Speed. Proposed stable ids `pedal_mxr_phase90` (`pedal`) and
`pedal_mxr_phase90_circuit` (`circuit`); proposed display **Orange Phase**.

Status: **CHECKPOINT 2026-10-02 -- clears on traces of genuine units; revision to choose.**

## Eight-question checkpoint

1. **Revision.** Three, by the pedal's own history (ElectroSmash; FX Doctor's board
   revisions):
   - **Script logo, 1974-77**: six single op-amps (741s by National or TI, per
     ElectroSmash; TL061 on its drawing), no feedback resistor.
   - **Early block logo, 1977 on**: the script board with one addition, **R28 24 k** from
     the last stage's output back into the shifting chain -- more resonance, a midrange
     lift, "pulsing" against the script's "swooshing".
   - **Late block logo, early 1980s**: three dual op-amps (TL062), R28 on the board, a
     reworked bias network for easier calibration, pre- and de-emphasis capacitors, a
     Miller capacitor C11 on the mixer transistor.
   Dunlop's reissues reproduce the first two (the '74 Vintage / CSP-026 the script, the
   M-101 the block; the Phase 95 switches between them).
   **To choose**: the script (the original), or the script board with R28 switchable --
   which is exactly the early block logo, and what the Phase 95's button does.
2. **Original schematic found?** **No factory drawing is public**; MXR's own schematic
   has never circulated (stated by ElectroSmash and by Dunlop's reissue material).
   **Traces of genuine units are**: ElectroSmash's script-logo schematic with a full parts
   list (local copy in `docs/schematics/mxr_phase90/`, git-ignored), and the
   freestompboxes.org threads tracing the script and its reissue. This is the standing of
   the Bass Driver, built from a trace of a real V2: traced from the hardware, not
   inferred from a relative.
3. **Best source.** ElectroSmash's script schematic and parts list: every value, the
   four stages, the LFO, the mixer, the supply, and the bias procedure.
4. **Cross-check.**
   - ElectroSmash's own arithmetic: the notches at 58.5 Hz and 340.8 Hz with the JFETs
     off (24 k, 47 nF; `f = tan(phi/2) / 2 pi R C` at 45 and 135 degrees); the input at
     480 k; the output high-pass at 22 Hz; the LFO "from tenths of Hz to some Hertz".
   - The bias procedure: the LFO's swing at the gates "from 5.1 V to 3.8 V / 1.6 V
     (depending on the FET batch cut-off voltage)", a triangle centred near 4 V.
   - Eichas, Fink, Holters and Zölzer, *Physical Modeling of the MXR Phase 90* (DAFx-14):
     a physical model of the same topology against a measured unit -- a kit, "a slightly
     modified version of the original circuit", so a check of the method and of the JFETs'
     working range (VGS -2.8 to -1.5 V, VDS within +-2 V), not of values.
5. **Signal path and values** (script, ElectroSmash):

```text
in -- R3 10 k -- C5 10 nF -- U1a TL061 follower, R14 470 k to Vref
   -- four stages, each: R 10 k to -in, 10 k feedback, C 47 nF to +in,
      24 k from +in to Vref with a 2N5952 drain-source across it, its gate on the LFO line
      (U2a: R1/R5/C1/R6/Q2, U2d: R11/R10/C3/R25/Q3, U2c: R12/R13/C6/R26/Q4,
       U2b: R17/R18/C9/R23/Q5)
   -- mixer: dry (U1a) through R16 150 k, shifted (U2b) through R8 150 k, into Q1 2N4125's
      base; R7 150 k collector to base, R4 56 k collector to ground, emitter at Vref;
      C2 47 nF, R2 150 k to the output
LFO: U1b TL061 Schmitt, R19 150 k from Vref and R21 470 k from its output to +in;
     R36 SPEED 470 k (rheostat) from its output charging C10 15 uF; R24 150 k from C10 to
     -in, C7 10 nF from -in to the output; R22 3.9 M from C10 to the gate line
Bias: 9 V -- R15 10 k -- D2 5.1 V zener (Vref), C8 15 uF; the 250 k trimmer across Vref,
     its wiper through R20 1 M to the gate line, C4 47 nF on it
R28 (block): 24 k from U2b's output (the last stage) to U2d's -in (the second stage's
     R11 / R10 junction), ElectroSmash's drawing of it
```

6. **To be modelled exactly.** The whole pedal, the LFO included, as one netlist: the
   all-pass stages, the JFETs as the solver's JFETs at their gate voltages, the mixer
   transistor, and the oscillator running in the time domain (a relaxation oscillator
   is a circuit like any other). Speed on the pedal's drive knob.
7. **To be approximated, and why.**
   - **The 2N5952s** -- matched by hand at MXR. One set inside the data sheet (IDSS 4-8 mA,
     VGS(off) -1.3 to -3.5 V), the four identical, and **the trimmer set by the
     procedure**: the gate swing from 5.1 V down to the set's cut-off. ESTIMATED.
   - **The op-amps** -- TL061 (or 741) as the solver's transconductor op-amp with the
     part's GBW and slew, which the Schmitt's switching needs; the stages could be ideal.
   - **The LFO's phase** at the start of a note is wherever the oscillator is: the pedal
     runs freely, as the hardware does.
8. **Why.** The phaser, asked for by the owner; its whole sound is the JFETs' resistance
   law and the oscillator's ramp, both of which the solver already has.

## What building it needs

Nothing new in the solver: JFETs, a PNP, op-amps and a time-domain oscillator. In the
pedal slot, its Speed is the drive knob and its level is fixed (the Phase 90 has no
level control; the slot's level match holds it at unity). If R28 is to be switchable, the
slot needs a two-position control -- a stepped tone knob, as `Taper::Steps` already
gives -- or the two revisions become two pedals.
