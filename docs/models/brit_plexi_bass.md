# Brit Plexi Bass (Marshall 1992 Super Bass) research log

Engineering identity: **Marshall JMP 1992 Super Bass 100 W**, as Unicord drew it in July
1970. Proposed stable id `amp_marshall_1992` (`circuit`); its power stage is very likely
the Brit Plexi EL34's (see question 5). Proposed display: **Brit Plexi Bass**.

Status: **RESEARCHED 2026-09-25. Cleared for code** for the stock amplifier. The presets
that want it want *modified* ones, which is recorded below. Not built.

## What it is for

Two blocked presets name it:

- *Spiral '96*: Adam Jones' mid-70s Super Bass. What was done to it is **not documented**:
  forum accounts conflict -- "a '75 Super Bass ... stock (Friedman just services it)"
  against "a 78ish JMP wired [to] superlead specs with a few minor changes", channels
  jumpered with a cable, EL34s biased at 25 mA ([Rig-Talk](https://www.rig-talk.com/forum/threads/adam-jones-super-bass.44195/);
  [Mixdown](https://mixdownmag.com.au/features/rig-rundown-adam-jones-of-tool/)). Both
  are forum-level claims (PLAUSIBLE at best), and one of them makes the amplifier a 1959
  rather than a 1992.
- *Californicated '99*: "John's main amp during the Californication album was a 1965
  Marshall JTM-45, often paired with a Marshall Super Bass"
  ([Ground Guitar](https://www.groundguitar.com/john-frusciante-gear/)) -- WIDELY
  REPORTED, and the JTM45 is not modelled either.

So building the stock 1992 is well founded, and it unblocks neither preset on its own:
Spiral needs a decision on the conflicting accounts and Californicated needs a JTM45.

## Eight-question checkpoint

1. **Revision.** The 1970 JMP 1992 as drawn by Unicord (the US distributor): drawing
   **70-13-11**, July 1970, 3 x ECC83, 4 x EL34. Mid-70s units are reported to be
   close to it; that is not checked here.
2. **Original schematic found?** **Yes**: Unicord 70-13-11, the same drawing set and
   date as the 1959 drawing `plexi.rs` was built from (70-6-11 issue B).
   [schematicheaven.net/marshallamps/jmp_super_bass_100w_1992.pdf](https://schematicheaven.net/marshallamps/jmp_super_bass_100w_1992.pdf);
   a copy is in `docs/schematics/` (git-ignored). Legible.
3. **Best source.** That drawing, with the Brit Plexi's Marshall pot-law and supply
   sources (`docs/models/brit_plexi.md`) for what it leaves out.
4. **Cross-check.** The 1959 drawing of the same set: every difference should be a
   *voicing* difference and the rest identical, which is what it shows (below). The
   Laney Supergroup, which forum sources describe as "a Superbass spec tone stack", has
   the same 56 k slope and 22 nF bass and mid capacitors on its own traced drawing
   ([brum_100.md](brum_100.md)).
5. **Signal path and values**, against the 1959 of the same drawing set:

   | | 1992 Super Bass | 1959 Super Lead (`plexi.rs`) |
   |---|---|---|
   | V1 cathodes | **shared**, 820 ohm with a "320" electrolytic (units not printed) | split: bright 2.7 k / .68 uF |
   | V1 coupling | **.022 on both channels** | bright .0022, normal .022 |
   | bright cap round the volume | **none** | 5000 pF |
   | mixer | 470 k / 470 k, 500 pF across one | the same |
   | V2a cathode | **820 ohm ("1K" in brackets: later value)**, no bypass drawn | 820 ohm, .68 uF |
   | cathode follower | 100 k | the same |
   | stack | **250 pF**, **56 k**, .022, .022, 250 k / 1 M / 25 k | 500 pF, 33 k, .022, .022 |
   | inverter | ECC83 long-tailed pair, 82 k / 100 k plates, 10 k and 470 ohm, 1 M legs, "47K (100K)" tail, 5 k presence with .1 uF | the same family |
   | power | 4 x EL34, 5.6 k grid stoppers, 1 k screens, 100 uF / 56 k supply | the same |

   Bracketed values are Unicord's own notation for a later production value.
6. **To be modeled exactly.** The preamp above, as a second voicing of the `plexi.rs`
   construction rather than a copy of it -- the same node structure with the 1992's
   values -- and the power stage on the Brit Plexi EL34 once the two drawings' power
   sections are compared value by value (they look identical at this reading).
7. **To be approximated, and why.** The V1 cathode capacitor's units ("320"): a
   bypass electrolytic by its symbol, and at any value from tens of microfarads up it is
   a full bypass above 20 Hz; PLAUSIBLE. The pot laws come from Marshall's 1988 1959
   drawing, as for the Brit Plexi.
8. **Why.** A Unicord drawing of the same set as one already built from, and a
   difference table that is small.

## Before code

- Compare the 1992's and 1959's power sections line by line; if they match, share
  `PLEXI_EL34`.
- Decide what *Spiral '96* is. If the "Super Lead specs" account is right, that preset
  wants the existing Brit Plexi with its channels jumpered, not this.
