# Treble Boost (Dallas Rangemaster) research log

Engineering identity: **Dallas Rangemaster Treble Booster**, OC44 germanium PNP.
Proposed stable ids `pedal_dallas_rangemaster` (pedal slot) and
`pedal_dallas_rangemaster_circuit`. Proposed display: **Treble Boost**.

Status: **RESEARCHED 2026-09-25. Cleared for code as the stock unit.** The preset that
wants it wants a *modified* one, whose modification is not documented. Not built.

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
   commonly agreed bias "collector voltage at -7.0V", about 2 mA. The
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
   - **OC44**: no SPICE model is agreed on. The catalogue's germanium PNP (as used for
     the Round Fuzz's AC128) re-fitted to an OC44's published gain and leakage;
     ESTIMATED. Bias it to the -7 V collector the sources agree on, as a builder would.
   - **Iommi's modification**: unknown. The preset uses the stock unit and says so.
8. **Why.** A one-transistor circuit whose values every source agrees on, and a
   published analysis with numbers to measure against.
