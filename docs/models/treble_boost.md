# Treble Boost (Dallas Rangemaster) research log

Engineering identity: **Dallas Rangemaster Treble Booster**, OC44 germanium PNP.
Proposed stable ids `pedal_dallas_rangemaster` (pedal slot) and
`pedal_dallas_rangemaster_circuit`. Proposed display: **Treble Boost**.

Status: **IMPLEMENTED 2026-10-01 as the stock unit** (`circuits::treble_boost`; pedal id
`pedal_dallas_rangemaster`, circuit id `pedal_dallas_rangemaster_circuit`). The preset that
wants it wants a *modified* one, whose modification is not documented.

## What it is for

*Paranoia '70* ([brum_100.md](brum_100.md)): Iommi "put it into the Laney because it
boosted the input and gave it the overdrive he was looking for", and "a guy in another
band told Iommi he could make the Rangemaster sound better for him, and when he brought
it back it had a lot more sustain. Iommi used it on all the early Sabbath stuff, right up
until Heaven and Hell"
([Guitar Player](https://www.guitarplayer.com/guitarists/tony-iommi-modified-treble-booster-thrown-out);
[MusicRadar](https://www.musicradar.com/artists/tony-iommi-secret-to-black-sabbath-electric-guitar-tone)).
DOCUMENTED that it was modified; **what the modification was is not documented** in any
source located.

## Eight-question checkpoint

1. **Revision.** The original Dallas Rangemaster with the Mullard OC44, 9 V, positive
   ground.
2. **Original schematic found?** **No factory drawing.** The circuit is recovered from
   original units, with one set of values agreed across sources. This is the standing of
   the Round Fuzz's sources; for a single-transistor circuit whose every part is visible
   on the board it is enough. What cannot be recovered is Iommi's modification, and the
   preset has to say so rather than invent one.
3. **Best source.** [ElectroSmash's "Dallas Rangemaster Treble Booster Circuit Analysis"](https://www.electrosmash.com/dallas-rangemaster)
   (mirror: [electrosmash.mas-effects.com](https://electrosmash.mas-effects.com/dallas-rangemaster)).
4. **Cross-check.** Its own arithmetic, which a model has to reproduce: voltage gain
   "Gv = 80 = 38dB", input impedance "10K to 12K approx.", input high-pass "fc=2.6KHz"
   (C1 against R1 || R2 and the base), output high-pass 15.9 Hz, emitter 0.8 Hz; the
   commonly agreed bias "collector voltage at -7.0V", about **0.2 mA** (corrected
   2026-10-01: this line said 2 mA, after the analysis's own "Ic = 1.9 mA" step, which
   divides the base's voltage from the *negative* rail by the emitter resistor; the
   analysis's gm of 0.008 S, its 2 V across the 10 k load and the -7 V everyone agrees on
   are all 0.2 mA). The
   [Wikipedia article](https://en.wikipedia.org/wiki/Dallas_Rangemaster_Treble_Booster)
   gives the same topology.
5. **Signal path and values.**

```text
INPUT -- C1 5 nF -- base of Q1 OC44 (germanium PNP)
   R1 470k and R2 68k: the base divider across the 9 V supply (positive ground)
   R3 3.9k emitter, C3 47 uF across it
   collector load: the 10k BOOST pot (log), its wiper to C2 10 nF -- OUTPUT
supply: 9 V, C4 47 uF
```

6. **To be modeled exactly.** Every part above; the 5 nF into a ~10 k input is the whole
   character of the pedal, and it has to meet the guitar's pickup inductance and cable,
   which the plugin's source impedance stands for.
7. **To be approximated, and why.**
   - **OC44**: from the Gummel-Poon parameters Holmes, Holters and van Walstijn
     extracted from a vintage OC44 ("Comparison of germanium bipolar junction transistor
     models for real-time circuit simulation", DAFx-17; published as a SPICE model with
     the paper's code, [grwhitehead.github.io/germaniumbjts](https://grwhitehead.github.io/germaniumbjts/)):
     IS 1.423 uA, BF 307, BR 20.27, VAF 8.167 V, ISE 30.54 nA, NE 1.316. The solver's
     transistor is Ebers-Moll with no recombination term, so its beta is the *effective*
     gain those parameters give at this circuit's 0.2 mA: **97**, DERIVED -- beside
     ElectroSmash's assumed 100 and Mullard's minimum hFE of 100 at 1 mA. The resistors
     are the stock values; nothing was re-biased.
   - **Iommi's modification**: unknown. The preset uses the stock unit and says so.
8. **Why.** A one-transistor circuit whose values every source agrees on, and a
   published analysis with numbers to measure against.

## Implemented, 2026-10-01

`circuits::treble_boost`, `tests/treble_boost.rs`, `examples/treble_boost_op.rs`.

| | model | source |
|---|---|---|
| emitter current | 0.24 mA | ElectroSmash ~0.2 mA |
| collector | -6.6 V | "-7.0 V" |
| stage gain, collector over base, 10 kHz | 37.8 dB | "80 = 38 dB" |
| input high-pass, stiff source, -3 dB | about 2.3 kHz | "2.6 kHz" (Zin taken as 12 k) |

From the plugin's 10 k source the plateau is 4.6 dB lower: the guitar's impedance meets
the pedal's ten kilohms, which is the interaction the analysis describes. With a stiff
source it already makes 6 % distortion at 10 mV: the emitter is bypassed, so the whole
input is across a germanium junction.

**The knob.** Its one pot is the collector load's wiper, so it is a volume after a fixed
gain. Named **boost** (the original's pot "controlling the boost setting", WIDELY
REPORTED; no source agrees on lettering). In the pedal slot it is the **level** knob --
its noon is unity, like every pedal's -- and the drive knob is greyed
(`Pedal::has_drive`). Selected as the circuit it is the Drive knob, and the voice is in
`Gain::drive_is_channel_volume`, so the make-up does not cancel it.
