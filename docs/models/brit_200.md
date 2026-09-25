# Brit 200 (Marshall 1967 Major) research log

Engineering identity: **Marshall 1967 Major**, the 200 W lead head with four KT88s, as
Unicord drew it in July 1970. Proposed stable ids `amp_marshall_1967_major` (`circuit`)
and `power_major_kt88` (`power_amp`). Proposed display: **Brit 200**.

Status: **RESEARCHED 2026-09-25. Cleared for code; not needed by any preset.** The rig
research that put it in the preset list does not hold up (below). Not built.

## The preset attribution was wrong

[PRESETS.md](../../PRESETS.md) and the roadmap listed a Marshall Major as the missing
amplifier for *Desert Deaf '02*, *Californicated '99* and *Blood Sugar '91*. The sources
say otherwise:

- *Songs for the Deaf* (2002): "The Ampeg VT-40 is probably one of the most important
  amplifiers in Josh Homme's 2002 guitar rig", with Ampeg V-4B heads, and "a Peavey
  Standard, a Musician and various other PAs"
  ([Boost Guitar Pedals](https://www.boostguitarpedals.co.uk/blogs/gear-of-the-gods/josh-hommes-gear-on-songs-for-the-deaf-2002);
  [Reverb](https://reverb.com/news/tones-for-the-deaf-nailing-josh-hommes-sound)). No
  Marshall Major.
- *Blood Sugar Sex Magik* (1991): "two Marshalls: a guitar head for edge and a bass head
  for punch and low end", split through a DOD stereo chorus, with solos often direct to
  the board ([Guitar.com](https://guitar.com/features/artist-rigs/the-gear-used-on-blood-sugar-sex-magik-red-hot-chili-peppers-rhcp/)).
  Which Marshalls is not stated.
- *Californication* (1999): a 1965 JTM45 "often paired with a Marshall Super Bass"; the
  Major is Frusciante's **live** amplifier from the Californication tour on, "almost
  always paired with" a Silver Jubilee
  ([Ground Guitar](https://www.groundguitar.com/john-frusciante-gear/)).

All three are WIDELY REPORTED rig accounts, and none of them puts a Major on those
records. The blocked-preset tables are corrected to match.

## Eight-question checkpoint

1. **Revision.** The 1967 Major (lead) as drawn by Unicord, **70-02-12**, July 1970:
   ECC83 x 2, ECC82 x 1, KT88 x 4.
2. **Original schematic found?** **Yes**: Unicord 70-02-12, the same drawing set as the
   1959 and 1992 ([brit_plexi_bass.md](brit_plexi_bass.md)).
   [schematicheaven.net/marshallamps/major_1967u_lead_200w.pdf](https://schematicheaven.net/marshallamps/major_1967u_lead_200w.pdf);
   a copy is in `docs/schematics/` (git-ignored), with the 1966 200 W PA drawing and a
   second 200 W sheet for comparison.
3. **Best source.** That drawing.
4. **Cross-check.** The same set's 1959 and 1992 drawings, whose V1 and mixer it shares,
   and the separate "Major 200W" sheet in `docs/schematics/`, not yet compared value by
   value.
5. **Signal path and values** (read from the drawing; Unicord prints no designators).

```text
V1 ECC83: the 1959's two channels -- 68k stoppers, 1M, .0022 (bright) and .022
   couplings, 1M volumes, 470k / 470k mixers with 500 pF
V2a ECC83: 100k plate, cathode 470 ohm with 25 uF, over a 47k || 470 ohm leg that the
   presence network (5k pot, .68 uF) and the feedback return to
tone stack straight off V2a's plate -- no cathode follower: 500 pF, 33k slope,
   .022 / .022, 250k TREBLE, 1M BASS, 25k MIDDLE
V2b ECC83 recovery stage: 100k plate, 2.7k cathode unbypassed, 1M grid
V3 ECC82 inverter with .047 couplings, 270 / 270 ohm and 1.5k
4 x KT88: 5.6k grid stoppers, 250 ohm 5 W, 47k 2 W / 68k bias network
supply: 8 x A1010 bridge, 375 uF / 200 uF pairs, 25k 5 W and 10k 1 W droppers
```

   The structure is not a 1959 with bigger valves: the stack is **between two gain
   stages** and driven from a plate, which is why a Major sounds the way it does.
6. **To be modeled exactly.** The preamp above; a KT88 power stage with its own supply.
7. **To be approximated, and why.** No KT88 fit in the catalogue (ESTIMATED from a
   data sheet when built); the output transformer as for the other power stages; the
   V3 wiring needs a closer reading (at this scan it could be a cathodyne-like split or
   a long-tailed pair) before code.
8. **Why.** A factory-set drawing; but nothing in the preset list needs it, so it waits
   behind the devices that are needed.
