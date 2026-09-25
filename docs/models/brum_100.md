# Brum 100 (Laney Supergroup 100 Mk I) research log

Engineering identity: **Laney Supergroup 100 Mk I** (sold as the LA100BL), a 1969 build.
Proposed stable ids `amp_laney_supergroup` (`circuit`) and `power_supergroup_el34`
(`power_amp`). Proposed display: **Brum 100**.

Status: **RESEARCHED 2026-09-25. Cleared for code on a traced drawing**, with the
limitation in question 2. Not built. It is half of *Paranoia '70*; the other half is the
treble booster in [treble_boost.md](treble_boost.md).

## What it is for

*Paranoia '70*: Tony Iommi's rig for the early Black Sabbath records was "a 1965 Gibson
SG Special, modified Dallas-Arbiter Rangemaster treble booster and 100-watt Laney
Supergroup MK 1 head with a Laney 4x12 cabinet"
([Guitar World, "Tonal Recall"](https://www.guitarworld.com/artists/tonal-recall-part-1)),
with "the presence, middle and treble on 10 with no bass whatsoever" -- WIDELY REPORTED,
and the settings are Iommi's own account in that feature.

## Eight-question checkpoint

1. **Revision.** The Supergroup 100 Mk I, 1969: two channels (TREBLE and BASS, two jacks
   each), ECC83 x 3, EL34 x 4, a 20 H choke.
2. **Original schematic found?** **No factory drawing. A traced one.** "SUPERGROUP 100
   mk I build '69", three sheets (preamp, power amp, supply), drawn by "vddj",
   2008-09-22, revision 1.0, from a 1969 unit:
   [drtube.com/schematics/laney/](https://www.drtube.com/schematics/laney/Super_Group_100_Mk1_pwramp.pdf)
   (`Super_Group_100_Mk1_preamp.pdf`, `_pwramp.pdf`, `_psu.pdf`; copies in
   `docs/schematics/`, git-ignored). It is a clean CAD drawing with designators and
   values throughout. This is the same standing as the traced drawings the Green 808,
   Rodent and Round Fuzz were built from, and it is of *this* amplifier, not a near
   relative -- so question 2 passes, as a trace. A factory sheet (Schematics Unlimited
   lists "laney super group mk i preamp/poweramp schematic") sits behind a verification
   form and was not retrieved.
3. **Best source.** The traced drawing.
4. **Cross-check.** Forum descriptions of original units -- "very Marshallesque ...
   basically a Superbass spec tone stack with 25uF/1k5 split V1 cathode"
   ([Marshall Amp Forum](https://marshallforum.com/threads/laney-la60-la100-1968.102231/))
   -- agree with the trace on both counts it can check: split V1 cathodes at 1.5 k and
   25 uF, and a 56 k / 22 nF / 22 nF stack like the Super Bass's
   ([brit_plexi_bass.md](brit_plexi_bass.md)). That is WIDELY REPORTED agreement, not a
   value-level check against a second drawing.
5. **Signal path and values** (the trace's designators).

```text
TREBLE (J1/J2) and BASS (J3/J4) channels, each: R4/R5 (R9/R10) 68k stoppers,
   R6 (R11) 1M, U1B / U1A 12AX7A, R7/R12 100k plates, R8/R13 1.5k with C7/C8 25 uF
   C10/C11 22 nF -> P1 "GAIN TWO" / P2 "GAIN ONE", 1M (the volumes)
   -> R15/R16 470k mixers, C12 270 pF across R15 (the TREBLE channel's)
U2B 12AX7A: R17 100k plate, R18 820 ohm cathode, unbypassed
U2A 12AX7A cathode follower, R19 100k, direct-coupled
tone: C13 270 pF, R20 56k, C14/C15 22 nF, R23 250k TREBLE, R24 1M BASS (rheostat),
      R25 22k MIDDLE -> OUT (no master)
power amp: C16/C17 22 nF into U3 12AX7A long-tailed pair, R26/R27 1M legs, R22 10k,
      R28 470 ohm, plates R29 82k / R30 100k, C19 47 pF across the plates;
      feedback R21 100k to R37, a 3.3k PRESENCE pot with C18 100 nF
      C20/C21 100 nF -> R31/R32 220k to Vbias; 4 x EL34 with R33-R36 10k grid
      stoppers and R40-R43 470 ohm screen resistors; T2 8/16 ohm (J7)
supply: HT1 through T1 (20 H choke) to HT2, R39 2.7k to HT3, R38 22k to HT4,
      R14 10k to HT5; C22-C25 33 uF pairs, C9 15 uF
```

6. **To be modeled exactly.** Every value above, as its own preamp -- it is Marshall-like
   but not a Marshall: the mid pot is 22 k, the treble cap 270 pF, the bright cap 270 pF
   across the mixer, and the presence pot 3.3 k -- and its own EL34 power stage.
7. **To be approximated, and why.**
   - The supply voltages are not on the trace; the supply sheet gives the topology.
     ESTIMATED from the EL34s' idle until a measured unit's voltages are found.
   - Pot laws are not on the trace ("Key=T/B/M/P" and 50 %); taken from Marshall
     practice of the period. APPROXIMATED.
   - The output transformer's impedance: not on the trace. ESTIMATED as for the
     Brit Plexi.
8. **Why.** A complete, specific trace of the right amplifier, whose two checkable
   features agree with independent accounts.

## Before code

- The supply voltages (a measured unit, or Laney's own sheet if it can be obtained
  without circumventing anything).
- Which channel Iommi played through, and whether he jumpered them: not stated in the
  sources located.
