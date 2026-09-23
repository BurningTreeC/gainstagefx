# Jazz 120 (Roland JC-120 Jazz Chorus) research log

Engineering identity: **Roland JC-120 Jazz Chorus**, the current-production
JC-120UT/JT. Proposed stable id `amp_roland_jc120` (`circuit`), power stage
`power_roland_jc120_ss`, cabinet `cab_roland_jc120_2x12`. Proposed display:
**Jazz 120**.

Status: **RESEARCHED 2026-09-22, CLEARED FOR CODE. CH-1 built and validated.**

## Sources

Four documents, because the JC-120 is not one amplifier.

| document | what it covers | scan |
|---|---|---|
| **JC-120UT/JT Service Notes, Sep. 2000** — `docs/schematics/roland_jc120ut_jt_service_notes_2000.pdf` | the current-production amplifier | **1334 ppi**, vector-derived stencil |
| JC-120/JC-160 Service Notes, Fifth Edition (79-5) — `docs/schematics/roland_jc120_jc160_service_notes.pdf` | serials 258100+ / 789500-248099 | 200 ppi bitonal A3 |
| JC-120/JC-160 Service Notes, Third Edition (79-3) — `docs/schematics/roland_jc120_jc160_service_notes_79-3_600ppi.pdf` | the TA-7122AP preamp | 600 ppi |
| Factory schematic, serials 380100-410599 — `docs/schematics/roland_jc120_schematic_sn380100_410599.jpg` | the earliest discrete preamp | 2525 x 2139 JPEG |

Provenance:

- The 2000 notes and the 600 ppi 79-3 came out of `Roland JC-120.zip` at
  [schematicsforfree.com](https://schematicsforfree.com/files/Audio/Products/Musician/R/Roland/)
  (18 MB; the server truncates the first request, so it needs `curl -C -`).
- The 380100-410599 sheet is an attachment on the Music Electronics Forum thread
  [Roland JC120 early model schematic anyone?](https://music-electronics-forum.com/forum/schematic-requests/43435-roland-jc120-early-model-schematic-anyone),
  which is also where the 79-3 was originally posted.
- [Roland Jazz Chorus, Wikipedia](https://en.wikipedia.org/wiki/Roland_Jazz_Chorus)
  for revision history context only.

Dead ends, recorded so they are not tried again: `tangible-technology.com`'s
Roland directory (the index that several forum threads point at) now returns 404
for every file, the Internet Archive was offline, `manualmachine.com` returns 403
and diyAudio blocks non-browser fetches.

## Which revision, and why it matters more than it looks

**CH-1 is a different circuit in every generation.** Not a value change — a
different topology and a different tone network:

| generation | input stage | Treble / Bass / Middle | slope | treble cap |
|---|---|---|---|---|
| s/n 380100-410599 | 2SA493GR pair, shunt feedback | 50 k B / 50 k A / 5 k B | R8 10 k | C9 .0033 |
| 79-3, s/n 512000+ | TA-7122AP integrated preamps | 50 k B / 100 k A / 5 k B | — | C10 .0015 |
| 79-5, s/n 789500-248099 | two 2SK117GR, compound | 50 k B / 100 k A / 5 k B | R10 18 k | C3 .0015 |
| **2000 UT/JT (modelled)** | **one 2SK184, common source** | **250 k B / 250 k A / 10 k B** | **R18 100 k** | **C9 150 pF** |

**The 2000 UT/JT is the target**, for two reasons. It is the JC-120 that is in
production and on stages now, and its sheet is 1334 ppi where every one of the
others is at or past the edge of legibility.

This distinction is not academic. The first attempt at this circuit was built
against the 79-5 sheet's *published tone-control ranges* while reading the
*layout* off it at 200 ppi, and the two disagreed by 30 dB. See **What went
wrong the first time** below.

## Eight-question checkpoint

1. **Revision.** JC-120UT/JT, PA BOARD ASSY, Sep. 2000. The sheet distinguishes
   four sub-variants by a circled mark — Japanese `JC-120UT` / `JC-120JT`,
   export `JC-120UT Old` / `JC-120UT New` — and CH-1 differs between them in
   exactly two capacitors: C9 120 pF (part 0112637900) on the old export
   amplifier against 150 pF (0112638901) elsewhere, and C7 220 pF against
   330 pF. Both are carried in `StackValues`.
2. **Original schematic found?** **Yes, and it is unambiguous.** Manufacturer
   service notes with the PA BOARD and MAIN BOARD circuit diagrams, PCB artwork
   for every board, the parts list, the exploded view and printed signal
   voltages at the test points. `pdfimages -list` reports the two schematic
   pages as **17442 x 11747** and **15277 x 11497**, 1 bit CCITT, **1334 and
   1305 ppi**. Nothing in CH-1 is ambiguous at that resolution: every connection
   is followed and every value read.
3. **Best source.** The 2000 service notes themselves.
4. **Cross-check.** The sheet carries its own operating points, and they are an
   independent check on the values because they were measured on hardware rather
   than derived from the drawing:

   | test point | printed | model |
   |---|---|---|
   | CH1 INPUT HIGH, 1 kHz | -30 dBm, 69.3 mVpp | — |
   | J3 drain | -10 dBm, 693 mVpp | **19.0 dB** against 20 |
   | CN3, channel out | +13 dBm, 9.79 Vpp | **43.7 dB** against 43 |

   taken with the Setting block's `VOL, EQ MAX` and `BRI OFF`. Both within a
   decibel with nothing fitted — the JFET model is the datasheet's two numbers
   and every resistor is read off the sheet. The published input impedance,
   680 kOhm, is the third: it is R8 and nothing else, and
   `tests/jazz120.rs` measures it.
5. **Signal path and values.** DOCUMENTED, read off the PA BOARD sheet:

   **CH1 INPUT BOARD.** JK1 HIGH through R1 33 k and JK2 LOW through R2 68 k
   into one node. Printed sensitivities -30 dBm and -20 dBm at 1 kHz.

   **HEAD AMP, first stage.** R6 0 ohm, C4 150 pF to ground, C5 .068 coupling,
   R7 680 gate stopper into **J3, a 2SK184 in common source**: R16 15 k drain
   load off +VO, R11 6.8 k and R13 1.5 k both from source to ground (two parts,
   one node, 1.229 k unbypassed), C8 15 pF drain to gate, and R8 680 k returning
   the gate to the R14 68 k / R9 1.8 k divider off +VO.

   **Tone network**, passive, bottom to ground:

   Node by node, from J3's drain:

   | from | through | to |
   |---|---|---|
   | J3 drain | C9 150 pF | VR2 TRE term 3 |
   | VR2 TRE **wiper** | — | VR1 VOL term 3 — *the network's only output* |
   | J3 drain | R18 100 k | node S, the slope node |
   | node S | C13 .082 | node D = VR2 term 1 = VR4 BASS term 3 |
   | node S | C12 .056 | node E = VR3 MID term 3 |
   | VR4 BASS term 1 | R19 1 k | node E |
   | VR3 MID term 1 | — | ground |

   In words: the Treble pot's wiper is the network's **only** output. Its
   bottom terminal is the Bass pot's top. The slope resistor R18 feeds **two**
   caps into **two** entry points — C13 .082 into the Treble/Bass junction and
   C12 .056 into the Middle pot's top, on the far side of R19. Both the Bass and
   the Middle pots are rheostats with the wiper strapped to terminal 3.

   That second cap is the one structural difference from a Fender FMV stack, and
   it is the reason a single-slope-cap model cannot reproduce this network.

   **Volume and Bright.** VR1 1 MB to ground. C7 330 pF from the stack output to
   a node, R4 680 k from that node to the wiper, and SW2 (BRI) across R4 — so
   the switch **shorts R4** rather than switching C7 in and out.

   **Second stage.** C2 .068, R1 680 into **Q1, a second 2SK184 in common
   source**: R15 33 k drain load, R12 1.5 k source with R10 68 ohm + C6 47 uF
   across it, R3 1.5 M gate return into the R2 330 k / R5 680 divider.

   **Output.** Q2, a 2SC1815 emitter follower with its collector on the rail and
   R17 6.8 k, out through C10 4.7 uF to CN3.

   **Supply.** The main amplifier board makes +VA with a 1.5 k feed, an MTZ-30B
   zener and a 2SD1763 follower — so **+VA is 30 V, documented by the zener**.
   R20 680 ohm drops it to +VO behind C11 470 uF, and *the whole channel runs
   from +VO*, both stages and both bias dividers. +VO is therefore not a chosen
   number: it is 30 V less what the channel draws, which the DC solve puts at
   27.2 V.

   **Published specification** (2000 notes, p. 2): 120 W (60 + 60 into 8 ohm),
   nominal input -30 dBu, input impedance 680 kohm, speakers 12-4782/G1208F01
   (old UT) or **30-103D-3** (JT and new UT).
6. **To be modelled exactly.** CH-1 as above — done. Then CH-2's preamp and its
   distortion circuit's own clipping mechanism, the reverb send and recovery,
   the two independent solid-state power amplifiers, and the chorus/vibrato
   routing with the dry/wet speaker split.
7. **To be approximated.** The MN3007 bucket brigade is not simulated stage by
   stage; it becomes a modulated fractional-delay line carrying the documented
   clock range, with the companding and the pre/de-emphasis filtering around it
   taken from the schematic's own values. Labelled APPROXIMATED. The 30-103D has
   no published Thiele/Small data, so the speaker and the open-back 2x12 are
   ESTIMATED as in [speakers.md](speakers.md), with the two driver positions kept
   independent so the chorus stays genuinely stereo.
8. **Why.** The plugin has no solid-state power amplifier and no BBD at all.
   Both are reusable well beyond this model, and the JC-120's defining quality —
   very high clean headroom with no sag and a hard transistor overload — is
   precisely what the existing tube catalogue cannot produce.

## Progress

**2026-09-22 — CH-1 complete and validated.** `circuits::jazz120`, the clean
channel from the jacks to CN3. 25 MNA unknowns. `tests/jazz120.rs` (9 tests) and
`examples/jc120`.

Devices added to `dsp::netlist` from the Toshiba sheets rather than assumed:
`JfetSpec::J2SK184` and `BipolarSpec::NPN_2SC1815_GR`. The JFET's IDSS is the
middle of the GR rank's published 2.6-6.5 mA; its pinch-off is *derived* from the
sheet's other figure, `|Yfs| = 15 mS` at zero bias, through
`|Vp| = 2 Idss / gm0` = 0.6 V. The 2SC1815's beta is the middle of its GR rank's
published 200-400.

**Validated** — every row is a number printed by Roland, not one read out of the
model:

| check | published | model |
|---|---|---|
| jack to J3's drain, 1 kHz, VOL/EQ MAX | 20 dB | **19.0 dB** |
| jack to CN3, same conditions | 43 dB | **43.7 dB** |
| input impedance | 680 kohm | **5.7 dB loss into a 680 k source** |
| +VA | 30 V (MTZ-30B) | +VO **27.2 V** after R20 |
| J3 drain, Q1 drain | mid-rail, by construction | **14.5 V, 16.1 V** on 27.2 |
| Q2 emitter headroom | 9.79 Vpp at CN3 | **15.4 V** on a 27.2 V rail |

**2026-09-22 (cont.) -- wired into the plugin.** `Circuit::Jazz120`
(`amp_roland_jc120`, display **Jazz 120**), appended to the circuit list, with
`Gain::Jazz120` appended to `Gain::ALL` behind it. Drive turns the amplifier's
own Volume, as it does on the two Fenders, and the panel says `VOLUME`. Bass,
Middle and Treble drive the circuit's own stack through `own_tone`. One factory
preset, **Jazz Clean**.

**No power stage.** This is the first voice in the catalogue with none, so
`Matched` resolves to nothing and the channel hands straight over to the tone
section. That is the honest state until the two 60 W transistor amplifiers are
built, and it is recorded on the panel rather than filled in with a valve stage
the amplifier has not got.

**The Bright and sensitivity selectors are no longer the Twin's alone.** They
were gated on `Circuit::Twin` and carried its labels in the source, so the
American Deluxe -- which has the same switched jacks -- did not get them either.
`Gain::input_jacks` and `Gain::bright_switch` now carry the slots, the values
*and* the labels, and the panel reads them from whichever circuit is selected:
High/Low with Roland's own -30 and -20 dBm on the Jazz 120, the AB763's
1 MOhm / -6 dB on the two Fenders, and the two gated separately, because the
Deluxe has the jacks but its Bright capacitor is soldered in.

## What went wrong the first time, and how it was caught

Recorded because the failure mode is general and the diagnosis was right even
though the conclusion was wrong.

The first build read the 79-5 sheet at 200 ppi and fitted its ambiguous values
to the *published* tone-control ranges — Treble 17 dB at 10 kHz, Middle 13 dB at
350 Hz, Bass 14 dB at 50 Hz. It could not be done:

| candidate reading | Treble @10 k | Middle @350 | Bass @50 |
|---|---:|---:|---:|
| as read: 150 pF treble cap, 100 k slope | 8.1 | 2.8 | **13.5** |
| 1500 pF treble cap | **16.2** | 0.2 | 6.7 |
| 10 k slope resistor | 4.3 | **11.6** | 8.2 |
| **published** | **17.0** | **13.0** | **14.0** |

Each control could be brought onto its figure alone and never together, and a
wider sweep put the ceiling on Middle at 14.5 dB — but only with a 10 k slope,
which collapsed Treble to 4 dB. **That pattern was read correctly**: three
mutually exclusive fits mean the topology is wrong, not the values, exactly as a
single consistent fit had confirmed the Deluxe's Fender stack. Nothing was
adopted, and the hypothesis on record was that the stack sat inside a feedback
loop around the stage behind the Volume.

It did not. The real answer was that **the pot values and the published ranges
came from two different amplifiers**. The 250 k / 250 k / 10 k pots on the
79-5 parts list belong to the amplifier that sheet's *front* covers; the 17/13/14
figures on its specification page describe the 50 k / 100 k / 5 k network of the
generation it supersedes. Fitting one to the other could only ever fail.

Two lessons worth carrying:

- **A specification page and a schematic page in the same document need not
  describe the same circuit.** Roland's service notes carry superseded editions
  forward wholesale, by serial-number range.
- The fix was not cleverness, it was **a better scan**. Three hours of fitting
  against the wrong target was settled in one reading at 1334 ppi.

The corrected circuit's published ranges, for the record — this network is far
more authoritative than the one those figures describe, which is itself
corroboration that they are not about it:

| | Treble @10 k | Middle @350 | Bass @50 |
|---|---:|---:|---:|
| 2000 UT/JT, modelled | 45.9 | 16.6 | 28.7 |
| earlier JC-120, published | 17.0 | 13.0 | 14.0 |

One other error the DC solve caught, before any of that: R16 15 k was first
taken from the 79-5 sheet as `R7 220 k` and treated as a drain load, which
starved the drain to 0.15 V. On the 2000 sheet it is unambiguously a 15 k drain
load on a common-source stage, and the drain idles at 14.5 V.

## The power amplifiers, read off the sheet

Read 2026-09-22 off page 7 of the Sep. 2000 notes at 1334 ppi, before any code.
Two of them, L and R, identical part for part.

**Topology.** A textbook complementary three-stage amplifier, and nothing about
it is a guitar amplifier: differential pair, voltage amplifier stage with an
active load, `Vbe` multiplier, complementary drivers, complementary output pair
on shared emitter resistors, global feedback.

| stage | parts |
|---|---|
| input | C49 .068, R98 6.8 k series; R97 68 k to ground; D14 1SS133 + Q17 **2SK381E** shunting the node (the power-on mute) |
| differential pair | Q15, Q16 **2SA970-GR** (PNP); R91 33 k tail from +VE; R92 and R87 1.5 k collector loads |
| feedback | R88 **68 k** from the output to Q15's base, R93 **3.3 k** to ground behind C48 4.7 µF; C47 across R88 marked **OPEN** |
| VAS | Q12 **2SA1015-GR** with R78 1.5 k / R82 6.8 k, Q14 **2SC1815-GR**, R76 and R79 3.9 k, R75 6.8 k; C41 and C45 150 pF/500 V; C43 47 µF (see below) |
| bias | Q13 **2SC2229-O** as the `Vbe` multiplier, R83 68, C46 68 pF |
| drivers | Q10 **2SD669-AC**, Q11 **2SB649-AC**, through R58 and R59 6.8 Ω fusible |
| output | **2SC4386 / 2SA1671** on the TR BOARD through W8, R62 and R63 **0.33 Ω / 5 W** |
| output network | R69 330 Ω fusible, R86 6.8 Ω / 0.5 W, C44 .068 |
| supply | a CP1002 bridge off a centre-tapped secondary on the DI board, ±VE behind 3300 µF / 63 V |

**Two things the sheet settles that would otherwise have been guesses.**

*The closed-loop gain is not estimated.* R88 68 k over R93 3.3 k is
`1 + 68/3.3 = 21.6`, which is **26.7 dB** — and the sheet's own two test points
are `+3 dBm, 3.10 Vpp` at the power amplifier's input and `+29 dBm, 61.8 Vpp`
at its output, which is **26 dB**. The divider and the measurement agree to
seven tenths of a decibel, so the feedback network is read correctly and the
amplifier's gain is documented rather than fitted.

*The DC balance names the pair.* R97 is 68 k and R88 is 68 k, which is not a
coincidence: with C48 blocking R93 at DC, both bases of the differential pair
see 68 k to ground, which is how a PNP input pair is balanced. That the two
values match is the cross-check that R88 is the feedback resistor and R93 the
gain-setting leg rather than the other way round.

**The rail voltage is DERIVED, not read.** No secondary voltage is printed on
the sheet and the parts list gives the transformer only by part number
(G245713501 / G245713601). It comes out of the printed output instead: 61.8 Vpp
into 8 Ω is 21.85 V rms, 59.7 W — Roland's published 60 W — so the rails have to
clear ±30.9 V plus the output pair's saturation and the 0.33 Ω emitter
resistors. The 63 V reservoir capacitors put a ceiling on it. That is enough to
pin the figure to about ±40 V unloaded, and it is labelled DERIVED with this
reasoning rather than taken from a comparable amplifier.

**The VAS's load is a bootstrap, and the sheet says so plainly at 3400 ppi
effective.** The question was whether the R76/R75 junction sits *on* the output
line -- making the string a divider across the output -- or crosses it, with C43
as the only connection. Read at 260 % of the native 1334 ppi scan, **there is no
junction dot at that crossing.** The vertical is the same width above and below
it, against four unmistakable filled junctions within the same crop: R88's right
end and C43's bottom on the same horizontal, and R76's and R79's bottoms
directly above.

So the string is:

```
+VE ── R78 1.5 k ──┬── R82 6.8 k ──┬── R79 3.9 k ── Q12 (2SA1015)
                   │               │
                C43 47 µF       R76 3.9 k
                   │               │
                   │          (crosses the output line, no connection)
                   │               │
                   └──► output  R75 6.8 k
                                   │
                              Q13 (2SC2229), R83 68
```

C43's 47 µF is the *only* path from that string to the output, which is what a
bootstrap is: the load on the VAS's collector is held a constant voltage above
the output, so it behaves as a current source and the stage keeps its open-loop
gain all the way to the rail. A divider across the output would have done the
opposite. The two are not equivalent and it is now read rather than assumed.

**What the VAS is, corrected.** A first netlist was written taking Q12
(2SA1015) and Q14 (2SC1815) for a current mirror or cascode, on the strength of
the parts being a complementary pair sitting side by side. The DC solve threw
it out at once -- both differential-pair collectors pinned within a volt of the
negative rail -- and a proper read at 220 % says why:

* **Q12 is not in the signal path.** Its emitter is on the bootstrapped node and
  its base on the R79/R76 divider, with its collector on Q14's base. That is an
  **overcurrent limiter**: it stays off until the drop across R79 reaches a
  `Vbe` and then robs Q14 of base drive.
* **Q14 is the voltage amplifier stage**, its collector on the bootstrapped node
  that R82 feeds and C43 holds above the output.

So the pair's output drives Q14's base directly and Q12 sits across it as a
clamp -- the opposite way round from what was built. **The module was removed
rather than left in the tree**, because a power amplifier that solves to a wrong
operating point is worse than none: it would be wired up.

**The topology is now measured rather than read.** Judging junctions by eye had
cost two wrong netlists, so `tools/schematic/trace.py` was written to settle
them in the pixels instead: it measures the stroke width, separates the
junction dots from the wires by morphology, joins wires into electrical nodes
with union-find, and draws its reconstruction back over the scan in colour so
it can be checked. Run over this amplifier it finds 51 junction dots and
resolves the board into clean nodes:

| node | what it is | wires |
|---|---|---|
| 8 | the small-signal supply, behind D11/R72/R68 with C42 | `y4921 x12378..14038`, feeding R91 and R78 |
| 10 | the R78/R82 junction -- **the bootstrapped point** | `y5144 x13205..13386` |
| 1 | the VAS collector node: R82 bottom, R79 top, Q12 emitter, Q14 collector, C41, C44, Q10 base | `y5362 x13357..14435` |
| 2 | **the output** | `y5857 x13087..15194` |
| 3 | the lower driver's node: Q11 base and the bottom of the bias string | `y6133 x13224..14162` |
| 4 | +VE, Q10's collector | `y4914 x14331..15146` |
| 5 | -VE | `y6633 x12223..15155` |
| 6 | the input pair's collector node, R92/R87 | `y6251 x12208..13340` |
| 7 | Q15's base: R88's far end and R93's top | `y5865 x12588..13016` |

Two results fall straight out of it, and both were the things in doubt:

* **C43 joins node 10 to node 2** -- the R78/R82 junction to the output, and
  nothing else joins them. That is the bootstrap, measured.
* **R88 joins node 2 to node 7** -- the output to the inverting base. The
  feedback resistor is where the gain calculation said it was.

**What remains** is not topology but bookkeeping: which terminal of each
transistor sits on which of those nodes. The tool finds nodes, not components,
so that last step is a reading -- but it is now a reading against a known node
list rather than against a picture, which is a different kind of task and the
one the next pass does.

Everything above that is settled, and the global feedback is settled twice over
-- the divider and the sheet's own test points.

**Why this cannot be a `PowerSpec`.** `circuits::power::PowerSpec` is
valve-shaped throughout — phase inverter, grid leaks, grid stoppers, a bias
supply, an output transformer — and this amplifier has none of those. It takes
its own module, the way the 73P's line-driver output block does and for the same
stated reason: sharing the machinery would mean sharing a topology it has not
got.

## Architecture note

The amplifier is stereo by construction: one power amplifier and one 12" driver
carry the modulated signal, the other the dry. That is the source of the chorus's
spatial character and it must not be replaced by a mono chorus with a stereo
widener downstream. This is the first model in the plugin that is natively
stereo at the speaker, and it will need the acoustics path to keep left and
right independent.

## Open questions

- The power amplifier's output stage interconnection -- Q14's emitter return,
  the `Vbe` multiplier and the drivers across it. See above; everything else on
  that board is read.
- CH-2, the reverb and the BBD chorus/vibrato are still to read off the 2000
  sheet. The MN3101 clock range and the LFO sweep limits come off that sheet
  during implementation rather than from BBD folklore.
- The Chorus control belongs in the Tone section, per the standing rule for every
  circuit that has one.
