# Solid 200 (Randall Century 200) research log

Engineering identity: **Randall Century 200**, the early-1990s solid-state
guitar amplifier — specifically the **Century 200 II** as listed by the dealers
who trade them. Proposed stable id `amp_randall_century200` (`circuit`).
Proposed display: **Solid 200**.

Status: **RESEARCHED 2026-09-22, NOT CLEARED FOR CODE.** The checkpoint is
below and question 2 is not satisfied. This is the same stop as
[studio_pre.md](studio_pre.md), and for the same reason: the only legible
Randall solid-state drawings in public circulation are of a **different
amplifier**, and building this one out of that one is exactly what the rule
exists to prevent.

## The history checks out; the schematic does not

The brief's historical claim was verified independently rather than assumed, and
it holds:

- The **Century 200** is the amplifier behind *Vulgar Display of Power* (1992)
  and the *Far Beyond Driven* era. Ground Guitar's gear history of Dimebag
  Darrell's [Randall Century 200 II](https://www.groundguitar.com/dimebag-darrell-gear/dimebag-darrells-randall-century-200-ii/)
  is explicit, and the dealer listings for surviving units say the same.
- The **RG100ES** belongs to *Cowboys From Hell* and, later, *The Great Southern
  Trendkill*. It is **not** the Vulgar amplifier.
- Period reporting also has the studio running several Century 200 heads through
  a Bradshaw splitter with one position swapped for a Mesa for clean sounds.

So the target is right. The problem is entirely one of documentation.

## Eight-question checkpoint

1. **Revision.** Century 200 / Century 200 II, circa 1990-1992. Randall's own
   legacy manual for the Century 3000/200 head gives the published output as
   120 W into 4 Ω and 100 W into 8 Ω.
2. **Original schematic found?** **No. ORIGINAL SCHEMATIC NOT PUBLICLY LOCATED.**
   - The drawing exists. It is a two-page sheet covering **RG 180, RG 200,
     Century 180 and Century 200, 1990-1992**, and it is sold as a paper or JPEG
     product — [Reverb listing](https://reverb.com/item/11611928-randall-rg-180-200-century-200-3000-schematic-1990-1992-white-paper),
     and [thecodemachine](https://thecodemachine.co.uk/Schematics/Search.php?manuf=Randall)
     lists Randall schematics at about £1.80. It is not published anywhere free.
   - [Randall's own legacy manuals page](https://www.randallamplifiers.com/legacy-manuals/)
     carries the Century 3000/200 **owner's** manual only — controls and ratings,
     no circuit.
   - The question has been asked publicly and gone unanswered: a Marshall Forum
     [thread](https://marshallforum.com/threads/randall-century-200-schematic.135528/)
     puts it exactly — "The RG100 schematics are available on the net, but I had
     no luck with the Century 200." It has no replies.
3. **Best source so far.** Randall's owner's manual for the Century 3000/200
   head: control set and power ratings, nothing electrical.
4. **Cross-check.** None possible. There is one number (120 W/4 Ω, 100 W/8 Ω)
   and a control list.
5. **Signal path.** **Not known.** Not written down here as though it were.
   Nothing about the JFET stages, the op-amp types, the clipping mechanism, the
   voicing networks or the output stage is verified for *this* amplifier.
6. **To be modelled exactly.** Deferred until 2 is satisfied.
7. **To be approximated.** Deferred.
8. **Why.** Deferred.

## Why the RG100ES is not an acceptable substitute

It is the obvious shortcut and it is forbidden here. The RG100ES schematic is
freely available, the two amplifiers are from the same maker and the same
general era, and the result would be a plausible-sounding solid-state high-gain
model. It would also be a model of the **Cowboys From Hell** amplifier wearing
the **Vulgar Display of Power** amplifier's name — which is both the wrong
circuit and the one historical confusion this whole research task was set up to
avoid. `CLAUDE.md`: "If question 2 fails, do not build it from a near relative."

If, after the drawing is obtained, specific blocks turn out to be genuinely
shared between the RG and Century families, those blocks may be reused — but
only once the Century sheet shows that they are, block by block, in writing.

## What is needed before this can be built

In order of preference:

1. The two-page Randall RG 180/200 + Century 180/200 schematic, 1990-1992. It is
   a small paid purchase from either vendor above; once obtained it goes in
   `docs/schematics/` (git-ignored) like every other manufacturer drawing here.
2. Failing that, a gut-shot photo set of a Century 200 II board good enough to
   read values directly — the method that confirmed the pedal drawings in this
   repository.
3. Failing both: **do not build it.**

## What it blocks

Everything downstream of the amplifier in the *Vulgar Display of Power* preset.
The front-end devices do not depend on it and can proceed independently:

| Device | Status |
|---|---|
| Furman PQ-4 parametric EQ | research not yet started |
| MXR six-band graphic EQ (blue, M109 family) | research not yet started |
| Rocktron Hush 2-B | research not yet started |
| Randall 412JB / Jaguar 4x12 cabinet | research not yet started |

The preset itself is blocked, because its entire premise is that the sound comes
out of the modelled amplifier rather than out of an EQ curve at the end of the
chain. A preset built on a substituted amplifier would be precisely the "giant
arbitrary post-EQ" the brief rules out.

## Decision, 2026-09-22

**Stop here.** The owner will obtain the schematic. Until it lands in
`docs/schematics/`, no Century 200 code is written and no *Vulgar Display of
Power* preset is created. The four front-end devices and the cabinet are
researched and built independently, so the chain is ready the moment the
drawing arrives.
