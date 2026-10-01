# Bass Fuzz Pi (EHX Bass Big Muff Pi) research log

Engineering identity: **Electro-Harmonix Bass Big Muff Pi** (2008 on): a Big Muff on one
board (EC-D40) with a three-way switch -- Dry (a parallel clean path), Normal, Bass Boost
(low-passed signal mixed back after the tone stack) -- a flatter tone stack, and a direct
output. Proposed stable id `pedal_bass_big_muff` (`pedal`), as the roadmap reserves it.
Proposed display: **Bass Fuzz Pi**.

Status: **RESEARCHED 2026-10-01. Cleared in principle, blocked on obtaining the trace.**

## Eight-question checkpoint

1. **Revision.** **EC-D40 Rev.A, 2008** (silkscreen), the board of both traced units: one
   with production code 0909 (early 2009), one 0943 (week 43, 2009). Not the Nano Bass Big
   Muff, which is a different board.
2. **Original schematic found?** **Traces of real units, yes; obtained here, no.** EHX
   publishes nothing. Two independent traces of genuine boards are on freestompboxes
   (topic 31296): dylan159's of the 0909 unit (April 2021), and G. Bisson's revision of it
   against a 0943 board (August 2022), relabelled to the silkscreen, with photographs of
   both sides. The drawings are forum attachments that need an account; they are not in
   this repository. Kit Rae's Big Muff page declines to post current EHX products.
3. **Best source.** G. Bisson's revised drawing with its board photographs.
4. **Cross-check.** The thread's own: the silkscreen values are the parallel sums of
   doubled capacitors ("these match typical Russian big muff values" -- the tone-stack
   C10 = C19 = 470 pF stated as certain by a third builder); the Bass Boost "literally
   mixes some lowpassed signal with the tone stack output"; the Dry blend "doesn't affect
   the original circuit". The standard Big Muff analysis (ElectroSmash) for everything the
   two share; `circuits::bigmuff` (the Ram Fuzz, `docs/models/big_muff.md`), already built here, for the same.
5. **Signal path and values.** Not yet read: needs the drawing.
6. **To be modelled exactly.** The fuzz core and tone stack as traced; the three modes.
7. **To be approximated, and why.** **Three values are unknown** in both traces, being
   unmarked surface-mount parts nobody desoldered: **C31** (the Bass Boost low-pass with
   R30), **C38** (the Bass Boost path) and **C32** (the output high-pass with R37).
   Builders' replacements were found by ear (68 nF / 22 pF / 390 pF in one build; another
   suggests 3.9-5 nF for C31, a 300-400 Hz corner, and 100 nF for C32, 15 Hz). C32 sets a
   corner far below the band whatever its plausible value; **C31 and C38 shape the Bass
   Boost mode** and would be ESTIMATED -- or that mode left out until someone measures one.
8. **Why.** Two traces of genuine boards that agree, one with photographs.

**To unblock:** the two attachments in freestompboxes topic 31296 (an account is needed),
or a unit to measure C31 and C38 on.
