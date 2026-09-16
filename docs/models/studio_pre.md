# Cali Studio Pre (Mesa/Boogie Studio Preamp) research log

Engineering identity: **Mesa/Boogie Studio Preamp**, the 1U rackmount two-mode valve
preamplifier. Proposed stable id `pre_boogie_studio` (`circuit`). Proposed display:
**Cali Studio Pre**.

Status: **RESEARCHED 2026-09-16, NOT CLEARED FOR CODE.** The checkpoint is below and
question 2 is not satisfied: the only Studio Preamp drawing located is a hand-drawn sheet
that cannot be read value-by-value at the resolution available.

## First: the name in this repository was wrong

`PRESETS.md` listed the amplifier behind *Never Mind '91* as "a Mesa Studio .22 preamp".
There is no such product. Two different Mesa units are being run together:

- the **Studio .22**, a 20 W **EL84 combo** (Mesa's own sheet, drawn 3-30-86, is in
  `docs/schematics/boogie_studio22.pdf`: five 12AX7s, two 6BQ5/EL84s, a five-band graphic,
  reverb, 8 and 4 ohm taps and a Direct Out);
- the **Studio Preamp**, a **rackmount preamplifier with no power stage at all**, which is
  the one on the records.

Ground Guitar's page carries the correction in as many words: "there's no .22 in the
product name. Just Mesa/Boogie Studio Preamp. The Studio .22 was a complete amp running
EL84 power tubes." PRESETS.md has been corrected.

This matters for what gets built. The rig is a **preamplifier into a solid-state power
amplifier into 4x12 cabinets** -- which is a chain the plugin can already express (power
stage Bypass, a physical cabinet) once the preamplifier itself exists. It is not an
EL84 combo, and modelling the Studio .22 would have produced the wrong sound under the
right name.

## Eight-question checkpoint

1. **Revision.** The Studio Preamp as sold from the late 1980s: two modes (Rhythm and
   Lead) sharing one tone stack, a five-band graphic, three-spring reverb, a stereo
   effects loop and two pairs of outputs, one of them speaker-simulated. Five 12AX7s.
2. **Original schematic found?** **Partly, and not well enough.**
   - `schematicheaven.net/boogieamps/boogie_studiopreamp.pdf` holds three sheets. Sheet 2
     is the Studio Preamp itself -- **hand-drawn**, and at the scan's resolution the
     reference designators and values cannot be read reliably. It does carry a legible
     valve map: *V1 input + tone; V2a third triode (rhythm) / fifth triode (after V3,
     lead); V2b reverb mix and EQ send; V3 lead overdrive tube; V4 main outs; V5B reverb
     send, V5A reverb receive.*
   - Sheet 1 of the same file is **legible** but is a different product: "MESA-BOOGIE
     STUDIO CALIBER PREAMP PART ONE, FILE STUDCAL1.S01", with its own Rhythm and Lead
     channels and node voltages (V1a 140 V, V4b/V4a 132 V on the rhythm side; V1b 137 V,
     V2a 133 V, V2b 136 V, V3a 132 V on the lead side; 1k5 cathodes throughout, 100 k
     plates, a 250 kA/250 kB/25 kL stack).
   - Sheet 3 is the supply and the LDR mode switching.
3. **Best source so far.** Mesa's own
   [Studio Preamp owner's manual](https://mesa-boogie.imgix.net/media/User%20Manuals/StudioPre.pdf),
   which is authoritative for the control set and the architecture but carries no values:
   Volume ("sets gain in Rhythm mode and signal feed to Lead mode"), Master ("rhythm master
   volume and effects send level"), Lead Drive ("gain, sustain and sensitivity of Lead
   mode"), Treble/Middle/Bass, a **Lead Fat** switch that "re-voices the Treble control to
   upper mid-range", Rhythm Bright and Lead Bright switches, Output Level A and B, a
   five-band graphic with an OUT/IN/**AUTO** switch that engages it with Lead mode, reverb,
   and an effects loop switchable between LOW and LINE.
4. **Cross-check.** The Studio Caliber sheet and the Studio .22 sheet, which are the same
   design language (100 k plates on 200 V, 1k5 cathodes, the 250 k/250 k/25 k Mesa stack,
   the same five-band graphic around four transistors) and which bound what the Studio
   Preamp's values can plausibly be -- but bounding is not reading, and this model is not
   to be built on inference.
5. **Signal path.** Known in outline from the valve map and the manual; **not** known
   value-by-value. Not written down here as though it were.
6. **To be modeled exactly.** Deferred until 2 is satisfied.
7. **To be approximated.** Deferred.
8. **Why.** Deferred.

## What is needed before this can be built

A legible Studio Preamp drawing. Options, in order of preference:

1. A better scan of the same hand-drawn sheet (Mesa's support, the Boogie Board forum
   archives, or the Mesa schematic sets that circulate as multi-page PDFs).
2. A gut-shot photo set of the board good enough to read the values directly, which is how
   the pedal drawings in this repository were confirmed.
3. Failing both: **do not build it**. The repository's rule is schematic first, and the
   nearest legible relative (Studio Caliber) is a different product. Building that one
   under this name would be exactly the "approximate alias" that PRESETS.md refuses.

## What it unlocks

*Never Mind '91* and *Seattle Ten '91*, both of which are waiting on it, plus a documented
rig shape the plugin has never had: a valve preamplifier into a **solid-state** power
amplifier (a Crown Power Base 2, later Crest 4801s) into Marshall 4x12s. See
[rig research](../../PRESETS.md) -- and note that the same sessions used a **Boss DS-1**
(see [orange_dist.md](orange_dist.md)), an Electro-Harmonix Small Clone, a Fender Bassman
and a Vox AC30, of which the AC30 is already modelled.
