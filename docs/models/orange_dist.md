# Orange Dist (Boss DS-1) research log

Engineering identity: **Boss DS-1 Distortion**, the Japanese original. Stable id
`pedal_boss_ds1` (pedal slot). Display: **Orange Dist**.

Status: **RESEARCHED 2026-09-16**, checkpoint below. No code yet.

## Eight-question checkpoint

1. **Revision.** The **1978 first release**, which carries the Toshiba **TA7136AP** --
   which is not an op-amp but a single-ended preamplifier IC -- and which ran until the
   1994 change. That is the pedal on the records this would be built for. The later
   versions are the *same passive circuit* with a different chip in the same socket
   position, so the revision question here is entirely "which amplifier":
   - 1978 Toshiba TA7136AP;
   - 1994 Rohm BA728N;
   - 2000 Mitsubishi M5223AL (needs R40, a 1 k pull-up, for class-A operation);
   - 2006 New Japan Radio NJM2904L.
2. **Original schematic found?** Not a Boss sheet. The circuit is fully recovered
   value-by-value in the source below, with Boss's own designators.
3. **Best source.** [ElectroSmash's "Boss DS-1 Distortion Analysis"](https://www.electrosmash.com/boss-ds1-analysis)
   (mirror: [electrosmash.mas-effects.com](https://electrosmash.mas-effects.com/boss-ds1-analysis)),
   which gives every part, works the gains and corners out from them, and lists the four
   chip revisions.
4. **Cross-check.** Its own arithmetic, which is what the model is measured against:
   - transistor booster "R8/R9 = 10K/22 = 454" raw, **35 dB (56x)** with feedback, above
     3.3 kHz;
   - op-amp stage "Gv max = 1 + (100K/4.7K) = 22.3 (26.5 dB)", minimum 1x;
   - its high-pass corner "fc max = 1/(2*pi*C8*R13) = 72 Hz" at one end of VR1 and 3 Hz at
     the other, so **the bass is clipped less than the treble** -- the pedal's distortion
     is frequency selective;
   - clipping at **±0.7 V** on the 1N4148 pair;
   - tone: a Big-Muff-style passive scoop, low-pass at **234 Hz**, high-pass at
     **1,063 Hz**, a notch "around 500 Hz", about **-12 dB** of insertion loss;
   - input impedance ~470 k, bias 4.5 V.
   A second, independent implementation study:
   [Néstor Nápoles López, MUMT 618 (McGill)](https://napulen.github.io/reports/mcgill/mumt618/).
5. **Signal path and values.**

```text
INPUT -- C1 47nF (7.2 Hz) -- R1 1k -- Q1 2SC2240 emitter follower, R2 470k to 4.5 V,
     R3 10k emitter
  -- C2 .47uF (3.3 Hz with R4 100k) -- C3 47nF (33 Hz with R5 100k)
  -- Q2 2SC2240 common emitter: R8 10k collector, R9 22 emitter, R7 470k shunt feedback,
     C4 250pF in the loop            <- 35 dB, and it clips asymmetrically on its own
  -- C5 68nF (23 Hz with R10 100k) -- R39 47k, D8 1N4148 as input protection
  -- U1 first half: unity buffer
  -- U1 second half, non-inverting: VR1 100k (Dist) over R13 4k7, C8 .47uF in the leg,
     C7 100pF across the loop
  -- D4/D5 1N4148 back to back to ground, R14 2.2k in series, C10 .01uF across the diodes
     (the cap is what stops it sounding grainy)
  -- tone: R16 6.8k with C12 .1uF, R17 6.8k with C11 .022uF, VR3 20kB
  -- VR2 100kB (Level)
  -- C13 .047uF -- Q3 2SC2240 emitter follower, R19/R20 1M, R21 10k, C14 1uF, R23 100k
supply: 9 V, D1 1N4004 reverse protection, R24/R25 10k make 4.5 V, C23 100uF, C15 47uF
```

6. **To be modeled exactly.** Every part above: both emitter followers, the booster with
   its shunt feedback (and its own asymmetric clipping, which is half of what this pedal
   is), the gain stage with its frequency-selective clipping, the diode pair with C10
   across it, the passive scoop and the level control.
7. **To be approximated, and why.**
   - The **TA7136AP** is the open question. It is a single-ended preamplifier IC, not a
     rail-to-rail op-amp, and no SPICE model was located; the plan is the same
     transconductor-into-an-integrator used for the LM308 and the 741, with its
     data-sheet gain-bandwidth, and its asymmetric swing noted as ESTIMATED until a data
     sheet is read. If it cannot be sourced, model the **BA728N** version instead and say
     so on the panel description, because that one is an ordinary dual op-amp.
   - **2SC2240**: the catalogue's Ebers-Moll NPN with high-beta, low-noise parameters, as
     with the 2SC3378. APPROXIMATED.
   - Battery resistance is the supply's 47 ohm, as with the other pedals.
8. **Why.** No Boss drawing and no model for the Toshiba part.

## What it is for

The plugin's existing distortions clip either in a loop (Green 808, Yellow Dist's
predecessor stage) or straight to ground after a single op-amp. The DS-1 is the one where
**a transistor stage clips first, asymmetrically, and the diode stage clips what is left**
-- two different clippers in series, with a passive scoop after them. It is also the third
of the four pedals this session's research covers, and the one most often named on the
records the preset list is still waiting on.

## Still to do before code

- Find a TA7136AP data sheet (gain-bandwidth, swing, whether the asymmetry is worth
  modelling) or decide to build the BA728N revision.
- Implementation, then tests against ElectroSmash's own figures: 35 dB from the booster,
  26.5 dB from the gain stage, 72 Hz to 3 Hz of corner sweep on the Dist control, the
  500 Hz notch and -12 dB of tone-stack loss.
