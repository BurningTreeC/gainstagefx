# British TG (EMI TG12345 microphone amplifier) research log

Engineering identity: the microphone amplifier of the **EMI TG12345** transistor console
(Abbey Road, from 1968), the "Type D" amplifier of its microphone cassette. Proposed
stable id `pre_emi_tg12345` (`circuit`). Proposed display: **British TG**.

Status: **RESEARCHED 2026-10-01, NOT CLEARED FOR CODE.** Question 2 is not satisfied yet:
EMI's drawings are not published, and the redraws that exist are of the **Mk II** and
were not retrieved and checked. The same stop as [studio_pre.md](studio_pre.md).

## Eight-question checkpoint

1. **Revision.** Undecided, and it has to be decided first: the Mk I (1968, the
   console on *Abbey Road*), the Mk II, and the later Mk IV differ. Which one "the TG"
   means in this repository is the owner's call; the evidence below is for the Mk II.
2. **Original schematic found?** **No.**
   - "EMI prefers not to publish the original schematics" (GroupDIY,
     [EMI TG mic preamp schematic](https://groupdiy.com/threads/emi-tg-mic-preamp-schematic.66273/)).
   - A forum member has drawn **TG12345 Mk II** circuit diagrams "for repair or DIY use"
     -- Types A to R including **D (Mic Pre)** -- reverse-engineered from the equipment
     and EMI's service manual, his own copyright, in PDF and DipTrace on Google Drive
     ([TG Diagrams](https://groupdiy.com/threads/tg-diagrams.77280/)). He notes that
     "some of the data in the manual is left open to the interpretation of text so some
     of this may have to be altered", and a transcription error in Amp B was reported
     in the thread.
   - "EMI Recording Console Handbook + Mk2" circulates on Scribd; not retrieved.
3. **Best source so far.** Secondary descriptions: an input transformer of 1:3.16
   (10 dB), a 12-step attenuator, then the Type D amplifier, three BC109s with a ±5 dB
   fine trim (20-30 dB), a second stage of a BC109 and an ACY21 (germanium) for 10 dB,
   an output transformer of 1:1.78, ±20 V rails, up to about 50 dB overall. WIDELY
   REPORTED (GroupDIY); not a drawing.
4. **Cross-check.** None yet.
5. **Signal path.** Not written down as though known.
6-8. Deferred until 2 is satisfied.

## What would clear it

The Mk II Type D redraw retrieved and read against the handbook's text, value by value,
with the reported errata applied -- the standing of the traced drawings the Green 808 and
Brum 100 were built from -- *and* the owner's decision that the Mk II is the TG wanted.
For a Mk I, EMI's own Mk I drawings or a trace of a Mk I module.
