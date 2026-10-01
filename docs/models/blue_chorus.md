# Blue Chorus (Boss CE-2) research log

Engineering identity: **Boss CE-2 Chorus** (1979): an input buffer, pre-emphasis, a
Sallen-Key anti-aliasing filter, an MN3007 bucket brigade on an MN3101 clock swept by a
triangle LFO, a matching reconstruction filter and de-emphasis, and the delayed signal
mixed half and half with the dry one. Two knobs, Rate and Depth. Proposed stable ids
`pedal_boss_ce2` (`pedal`) and `pedal_boss_ce2_circuit` (`circuit`); proposed display
**Blue Chorus**.

Status: **CHECKPOINT 2026-10-02 -- not cleared yet: the factory drawing exists and has not
been obtained.**

## Eight-question checkpoint

1. **Revision.** The CE-2 of 1979, made in Japan, until 1991, and its bass sibling the
   CE-2B (the same service notes cover both). To be confirmed on the notes' own revision
   letters.
2. **Original schematic found?** **It exists; not in hand.** Boss's **CE-2 Service
   Notes** -- circuit diagram, parts list, adjustments -- are cited as the source of
   ElectroSmash's analysis and listed by Scribd, eServiceInfo, elektrotanya,
   audioservicemanuals and manuals.plus. Every copy tried on 2026-10-02 was behind a
   login or refused the fetch. **Not to be built from a redraw** (a Tonepad layout and
   several redrawn schematics circulate) while the factory drawing is obtainable.
3. **Best source.** The service notes, once in `docs/schematics/` (git-ignored).
4. **Cross-check.** ElectroSmash's analysis of the same drawing, block by block, with
   its figures: input impedance 407 k; pre-emphasis +15 dB above 2.3 kHz (corners 410 Hz,
   2.341 kHz) and the de-emphasis its mirror; anti-aliasing and reconstruction each a
   Sallen-Key pair at 6.6 and 6.9 kHz, behind high-passes at 48 Hz and 14.6 Hz; MN3007
   1024 stages, delay 5.12 to 51.2 ms across the MN3101's range; LFO a TL022 to 3.5 Hz,
   smoothed by a 72 Hz low-pass; dry and wet at 50 % (R21 = R22 = R24 = 47 k). To be
   checked against the notes, not used in their place.
5. **Signal path and values.** From the notes, when obtained.
6. **To be modelled exactly.** The analogue path -- buffer, emphasis, both filters, the
   mixer -- as netlists, and the LFO's waveform and range.
7. **To be approximated, and why.** The bucket brigade itself, as the JC-120's already
   is (`dsp::bbd`): a fractional delay whose length the clock range gives, because
   solving 1024 stages at the clock rate is not a trade this plugin makes; the CE-2's
   own clock range and LFO in place of the JC-120's. ESTIMATED where the notes are
   silent (the MN3101's clock against its control voltage).
8. **Why.** The chorus asked for by the owner, and the pedal-slot partner of the
   JC-120's chorus already built.

## What building it needs

A pedal in the slot is one netlist today. The CE-2 is two netlists with a delay between
them -- the path into the bucket brigade, and the path out of it into the mixer, which
also takes the dry signal from the first. The slot needs that shape (netlist, delay,
netlist summing with a tap of the first), which `dsp::bbd` can fill. A design decision
before code, recorded here when made.
