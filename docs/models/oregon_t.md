# Oregon T (Sunn Model T) research log

Engineering identity: **Sunn Model T**, the 150 W valve head, in the revision Sunn drew
on 25 September 1973. Proposed stable ids `amp_sunn_model_t` (`circuit`) and
`power_sunn_model_t_6550` (`power_amp`). Proposed display: **Oregon T**.

Status: **RESEARCHED 2026-09-25. Cleared for code** (question 2 is satisfied by a
factory drawing). Not built.

## What it is for

*Unknown Garden '94* in [PRESETS.md](../../PRESETS.md): "Kim Thayil's amplifier of choice
for much of *Superunknown* was the Sunn Model T" -- WIDELY REPORTED
([Alto Music's album feature](https://www.altomusic.com/blogs/news/essential-elements-of-essential-classics-soundgarden-superunknown);
[Premier Guitar's Soundgarden rig rundown](https://www.premierguitar.com/rig-rundown-soundgardens-kim-thayil-chris-cornell-and-ben-shepherd)
shows Model Ts in the live rig). Which year's Model T was on the record is not stated in
any source located. The album was tracked in 1993, before Fender's 1998 reissue, so it is
an original.

## Eight-question checkpoint

1. **Revision.** Sunn's drawing **D-1029, revision A, ECN 441, 9/25/1973**: the
   three-12AX7 preamp with BRITE and NORMAL channels, a master (`VOL`) and a presence
   control, four 6550s. Earlier (1970-72) Model Ts differ; the 1998 Fender reissue
   ("Rev B" drawings exist) is a different amplifier and is out of scope.
2. **Original schematic found?** **Yes.** Sunn Musical Equipment Company, "MODEL 'T'",
   D-1029 A, with a voltage chart (no-signal and 150 W rms) and its measurement
   conditions. [schematicheaven.net/newamps/sunn_model_t.pdf](https://schematicheaven.net/newamps/sunn_model_t.pdf);
   a copy is in `docs/schematics/` (git-ignored). Legible value by value at 200 dpi.
3. **Best source.** That drawing.
4. **Cross-check.** The drawing's own **voltage chart**, which is the strongest check
   available: twenty node voltages at no signal and at 150 W into 4 ohms, under stated
   conditions (1 kHz into the BOTH IN jack, both volumes, bass, mid, treble and master
   at 10, presence at 10, input set for 24.5 V rms across 4 ohms). A model has to land
   on those before anything else about it is trusted. The published description of it
   as "inspired by the Fender Bassman 5F6A" is a WIDELY REPORTED characterisation and
   is not a check on values.
5. **Signal path and values** (Sunn's designators).

```text
J1-J5: BRITE in/out, BOTH in, NORMAL in/out
V1 12AX7A, both triodes: R1/R4 68k grid stoppers, R2/R3 1M grid leaks,
   shared cathode R5 680 ohm with C1 250 uF 25 V, plates R6/R7 100k from C (+330 V)
   C2/C3 .022 -> R8/R9 1M (VOLUME, linear) -> R10/R11 470k mixers,
   C4 470 pF across R10 (the BRITE channel's)
V2 12AX7: gain stage (pins 6-8) with R13 820 ohm bypassed, R12 100k plate;
   cathode follower (pins 1-3) with R14 100k
tone: C5 270 pF, R15 250k TREBLE, R16 1M BASS, R17 56k slope, R18 25k MID,
      C6/C7 .022 -> R19 1M VOL (master, linear)
V3 12AX7A long-tailed pair: C8 .022 in, R21/R24 1M, R22 4.7k tail, R23 470 ohm,
   C10 .1 uF, R26 10k; plates R27 82k / R28 120k from B (+400 V); C11 270 pF
   across the plates; R20 22k feedback from the 16 ohm tap; presence R25 25k
   reverse log with C9 .022 uF
C12/C13 .25 uF -> R29/R30 100k to E (bias, -55 V)
4 x 6550: R31-R34 1k grid stoppers, R35/R38/R39/R42 47 ohm 5 W in the plates,
   screens through R36/R37/R40/R41 1.5k 5 W to taps on T1's primary
   (ultralinear -- the tap labels are too small to read on this scan: PLAUSIBLE),
   A+ 500 V (no signal) / 481 V (150 W)
T1 secondary: 16/8/4/2 ohm selector S4; line out R51 33k / R52 680 ohm
supply: CR2/CR3 22-0416, L3/L4 28-3100 chokes, C17/C18 20 uF 600 V, R46/R47 10k
```

6. **To be modeled exactly.** The BRITE channel from J1 (or the jumpered pair from J3,
   which is the BOTH IN jack Sunn itself tests with), V1-V3 with every value above, the
   stack, the master, the long-tailed pair with its feedback and presence, and the four
   6550s with their plate and screen resistors -- a new `PowerSpec`, because no existing
   one has 6550s or ultralinear screens.
7. **To be approximated, and why.**
   - **6550**: the catalogue has no 6550 pentode fit; one has to be made from a data
     sheet before the power stage exists. ESTIMATED until then.
   - **T1**: the ultralinear tap fraction and the transformer's leakage and core are not
     on the drawing. The tap is ESTIMATED (typically 40 %), the rest as for the other
     power stages.
   - The supply chokes and 20 uF reservoirs are on the drawing and can be built; the
     transformer's source resistance is ESTIMATED from the chart's A+ sag, 500 V to
     481 V at 150 W.
8. **Why.** A factory drawing with a voltage chart; the only gaps are a valve fit and an
   output transformer's tap, both of which the chart constrains.

## Before code

- A 6550 fit (`PentodeSpec`) from a data sheet, checked against the chart's no-signal
  plate and screen voltages.
- Decide the tap fraction from the chart's 150 W column rather than by ear.
