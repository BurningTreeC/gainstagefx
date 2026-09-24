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

The tool then finds the **components** as well, because a two-terminal part is
a break in a wire: every gap between two collinear segments of different nodes
is a part, and its two ends name the nodes it bridges. That turns the node list
into a netlist, and it is how the last three roles were settled -- the three
that two earlier attempts had guessed wrong.

**What the parts actually do**, finally:

* **Q13 (2SC2229) is the voltage amplifier stage.** Its base is straight off
  the pair's collector, its emitter degenerated by R83's 68 ohms into -VE, and
  its collector is the bottom of the string above it.
* **Q14 and Q12 are a two-transistor bias spreader, not a current mirror.** Q14
  sits across R76 + R75 -- collector on the upper driver's base, emitter on the
  lower one's -- and Q12 senses their junction and drives Q14's base, which
  makes the pair a precise voltage source between the two driver bases. C44's
  0.068 uF across the whole thing carries the audio past it while the direct
  voltage stays set. *This is what both wrong versions got wrong*, and taking
  the two for a mirror is what pinned the input pair against the negative rail.
* **C43 bootstraps that string** to the output, so it is a current source.

**It is checked against the sheet twice, and the two checks are independent.**
One is a pair of resistors read off the drawing; the other is a measurement
somebody took on a bench:

| | printed | model |
|---|---|---|
| closed-loop gain (R88/R93 says 26.7) | 26 dB | **25.7 dB** |
| output at the printed 3.10 Vpp input | 61.8 Vpp | **60.0 Vpp** |
| that, into 8 ohms | 60 W | **56 W** |

and the distortion falls as the level rises -- 1.11 % at 50 mV in, 0.06 % at
the rated output -- which is what a feedback amplifier with a biased class-AB
output stage does and what a mis-wired one does not. It clips just above the
rated output, which is also what holds the DERIVED +-40 V rails honest: if they
were wrong, 61.8 Vpp would not sit just under the ceiling, and a manufacturer
does not rate an amplifier anywhere else.

`circuits::jc120_power`, 27 unknowns. `tests/jc120_power.rs`, six tests.

**And it is speaker-loaded**, which needed a change to shared machinery rather
than to this model. `Chain` reaches a loaded output stage through `PowerModel`,
whose `spec()` returned a `PowerSpec` -- a phase inverter, a bias supply and an
output transformer -- so it could only ever name a valve stage. It now returns
`Option`, and every caller has to say what it does about `None`, which is what
stops the two definitions of "the block behind this voice" drifting apart
again. They had drifted: `Chain` built its power blocks from `power_stage` and
`examples/calibrate` from `build_power`, so a voice whose output stage is not a
`PowerSpec` was calibrated with it and played without it. On the JC-120 that
was eighteen decibels of make-up taking out a gain that was never put in.

That matters more here than the arithmetic. A loudspeaker's impedance is
nothing like the resistor that stands for it, and this amplifier has the
feedback to hold its output voltage across all of it -- so what changes is the
current, and with it where the output pair runs out.

Everything above that is settled, and the global feedback is settled twice over
-- the divider and the sheet's own test points.

**Why this cannot be a `PowerSpec`.** `circuits::power::PowerSpec` is
valve-shaped throughout — phase inverter, grid leaks, grid stoppers, a bias
supply, an output transformer — and this amplifier has none of those. It takes
its own module, the way the 73P's line-driver output block does and for the same
stated reason: sharing the machinery would mean sharing a topology it has not
got.

## The chorus, read off the sheet

Read 2026-09-23 off the EFF BOARD of the Sep. 2000 notes, before any code.

**The parts.** IC3 is an **MN3007**, a 1024-stage bucket brigade; IC4 an
**MN3101**, its two-phase clock generator. Around them: IC2A (NJM5218) and its
network as the anti-alias filter into the delay, IC2B and its network as the
reconstruction filter out of it, R75 and R78 33 ohm on the clock lines, and
IC5A/IC5B as the low-frequency oscillator.

**The two figures that decide how it sounds, and both are printed.**

| | printed | what it means |
|---|---|---|
| clock period | **2.5-12 µs** | 400 kHz down to 83.3 kHz |
| LFO, CHORUS mode | **triangle, 10 Vpp, 1.2 s** | 0.83 Hz, fixed -- no Speed control in this mode |

A 1024-stage BBD delays by `N / (2 f_clk)`, so that clock range is a delay of
**1.28 ms to 6.14 ms** -- a sweep of nearly five to one, which is why this
chorus is as pronounced as it is. Over a 0.6 s half period that is
`4.86 ms / 0.6 s` = 0.81 % of pitch shift, about 14 cents, which is musical
rather than seasick and is what the amplifier is known for.

**Vibrato is the same hardware with the controls connected.** SW3 (SRBM, four
sections) switches VIB / OFF / CHORUS; in vibrato the panel's SPEED (VR6
100 kA) and DEPTH (VR7 100 kB) reach the oscillator, and the CH1 board's own
note gives that sweep as **120 ms to 500 ms** -- 8.3 Hz down to 2 Hz.

**What cannot be solved, and what is done instead.** The clock runs between
83 kHz and 400 kHz. Simulating the bucket brigade stage by stage would mean
running the whole chain at four hundred kilohertz to model a delay line, which
is not a trade this plugin makes anywhere. So the MN3007 is **APPROXIMATED** by
a modulated fractional delay carrying the stage count and the printed clock
range, with the sheet's own filters around it -- and it is labelled as the one
approximation in this amplifier, the way the spring tank is in the AB763s.

What is *not* approximated is everything the sound actually comes from: the
delay range and the LFO period are the printed figures, and the filters either
side are the drawing's own values.

**No companding.** There is no NE570 or equivalent anywhere on this board --
the JC-120 runs the BBD open, with the filters and nothing else. That is worth
recording because most BBD choruses of the period do compand, and a compander
invented here would be an invented sound.

**It is stereo at the speaker, and that is the whole point.** `CN6` carries
`CHO_CV` from this board to the main amplifier board: one power amplifier and
one 12 in take the delayed signal and the other take the dry, which is where
the width comes from. A mono chorus with a widener after it is a different
effect with the same name.

## Two defects found by measuring the deadline, 2026-09-23

Reported from playing: the amplifier crackles and stutters on a strong attack.
`examples/stutter.rs` is the measurement it needed -- one callback at a time,
over a sweep of input levels, with the presets set up exactly as they ship --
because a throughput average hides a missed deadline entirely. It found two
things, and neither was a tuning problem.

### The JFET channel was one-directional

`Jfet::drain` returned **zero current with zero slope** for every `vds <= 0`.
A JFET's channel is symmetric -- source and drain are the same diffusion and
which is which is decided by the voltage across them -- so an ohmic channel
conducts both ways. The old law was not an approximation of that; it was a
cliff, and CH-1's second 2SK184 is driven across it.

The consequences were measured on `q1_d`, that stage's drain:

| | before | after |
|---|---|---|
| Newton passes a sample | 12.05 | **2.58** |
| fallbacks in 9,600 samples | 40,760 | **0** |
| `q1_d` excursion | **-23.2 .. +27.2 V** | 0.7 .. 26.8 V |

The channel runs from one +27.2 V supply, so -23 V was never a solution: it is
what the fallback left behind, and converted to audio it is the crackle. Fixed
in `Jfet::drain`, guarded by `src/dsp/device.rs`'s `jfet_channel_tests` and by
`tests/jazz120.rs`.

The fix reaches every JFET circuit in the plugin, and the regenerated
calibration says how far: of the whole catalogue **one row moved, by 0.06 dB,
and it is this amplifier's**. The JFET pedals are bit-identical, because none of
them takes a drain below its source.

### The power amplifier's rails had no impedance to the output devices

The output transistors' collectors sat directly on the reservoir node. A
saturating device is then a path of no resistance between a stiff supply and a
signal node, and Ebers-Moll in saturation drives both junction conductances
toward 1e13 S -- an unconditioned matrix rather than a hard one.

What was missing is real and simply absent: the reservoir capacitor's ESR, the
wiring from the DI board to the TR board, and each transistor's own collector
ohmic resistance. `RAIL_WIRING_OHMS`, ESTIMATED at 50 mOhm. The model is
insensitive to it across the range that is arguable -- 20 mOhm and 200 mOhm
were both measured and the output moved in the third decimal place.

**D11 is now a fixed drop** rather than a diode, for a separate reason: it puts
two nodes on the nonlinear boundary and the bootstrap pushes `vb` around enough
to make it switch, while its own current is a near-constant 4.9 mA behind
1.36 k and 47 uF. Boundary 20 of 31 unknowns became 18. See `D_RAIL_DROP`.

At the amplifier's clipping point, the two together:

| | before | after |
|---|---|---|
| Newton passes a sample | 4.18 | **2.58** |
| fallbacks in 9,600 samples | 2,254 | **56** |
| microseconds a sample | 7.45 | **3.41** |

### What it is worth at the panel

`Jazz Clean`, one callback at a time, 64 samples at 48 kHz against a 1,333 us
budget, over three seconds of plucked attacks:

| input | worst before | worst after | missed before | missed after |
|---|---|---|---|---|
| -24 dBFS | 5,474 us | 817 us | 7 | **0** |
| -18 dBFS | 6,372 us | 1,282 us | 81 | **0** |
| -12 dBFS | 7,005 us | 1,056 us | 207 | **0** |
| -6 dBFS | 10,831 us | 1,358 us | 331 | **1** |
| 0 dBFS | 10,168 us | 1,452 us | 420 | **3** |

CH-1 went from 857 fallbacks to none at every level.

### The Drive knob, and the preset it had put in the wrong place

**Fixed the same day.** The Jazz 120's Drive is `VR1`, the real channel Volume
pot, exactly as both AB763s' are -- but it did not take `set_drive`'s
fixed-calibration branch, so the drive-dependent make-up cancelled it. Measured
across the knob's travel, from 0.1 to 0.9:

| voice | Drive authority |
|---|---:|
| Jazz 120, before | **0.0 dB** |
| Jazz 120, after | 8.5 dB |
| Twin | 28.8 dB |
| Deluxe | 16.8 dB |
| 5150, whose Drive is a real gain control | 0.0 dB -- correct |

Turning Drive down did not quieten the amplifier, and it did not stop the
amplifier driving its own power stage 3.7 times past full output. The predicate
is now `Gain::drive_is_channel_volume`, shared with the test that guards the
make-up invariant.

With the knob working, the shipped preset was in the wrong place and it was not
close:

| Drive | THD | third harmonic |
|---:|---:|---:|
| 0.10 | 1.00 % | 0.01 % |
| **0.16, where it ships now** | **1.59 %** | **0.02 %** |
| 0.17, the knee | 1.63 % | 0.53 % |
| 0.20 | 6.72 % | 5.73 % |
| 0.55, where it shipped | **29.3 %** | 27.2 % |

A preset called *Jazz Clean* at 29 % distortion. At 0.16 it is 1.6 % and
effectively all second harmonic, which is what two JFET stages give and is the
amplifier's own colour rather than clipping. `output_trim` re-measured to +6.0.

Everything above 0.20 is still there to be turned up to, and it now gets louder
as well as dirtier, which is what the pot does.

### Where the deadline ended up

| Jazz Clean | worst before | worst now | missed before | missed now |
|---|---:|---:|---:|---:|
| -24 dBFS | 5,474 µs | 508 µs | 7 | **0** |
| -18 dBFS | 6,372 µs | 920 µs | 81 | **0** |
| -12 dBFS | 7,005 µs | 1,226 µs | 207 | **0** |
| -6 dBFS | 10,831 µs | 987 µs | 331 | **0** |
| 0 dBFS | 10,168 µs | 1,605 µs | 420 | **1** |

## Half the output stage was switched off, 2026-09-23

Found by measuring with a **real DI take** instead of synthetic attacks, and by
running the measurement **at real time** instead of flat out. Both of those
changes mattered, and neither of them is about the circuit.

### What the synthetic test could not see

A plucked-sine generator is a poor model of playing. The take used here --
31 seconds of a guitar DI at 48 kHz -- has a **crest factor of 19.8 dB**, long
quiet decays between phrases, and consequently spends most of its life near
zero. A synthetic pluck at four notes a second never really goes quiet, so it
sat away from the crossover region and never asked the output stage the hard
question.

Measured on the same preset at the same level:

| material | power-stage fallbacks |
|---|---:|
| synthetic attacks | 1,361 |
| the DI take | **149,740** |

A hundred and ten times more, from the same code at the same level.

Running the harness **paced** rather than flat out is the other half. A loop
that processes blocks back to back keeps the core's caches hot and its
predictors trained; a host hands over a block every 1,333 µs and leaves the
core idle in between. On this machine -- governor `performance`, so this is not
clock scaling -- pacing costs roughly half as much again per callback. That is
locality, and a DAW pays it too.

### R79, and what its absence did

`tools/schematic/trace.py` on page 7 of the service notes found two junction
dots at **(13537, 5363)** and **(13537, 5582)** -- one on the upper driver's
base rail and one on the spreader's sense node -- with the label `R79 3.9K`
between them. **R79 is in parallel with R76**, and it was missing here.

With the upper leg at 3.9 k instead of 1.95 k, the spreader held the two driver
bases **1.48 V** apart. A Darlington output stage needs four base-emitter
junctions, about 2.4 V. What that meant, measured at the operating point:

| | before | after |
|---|---:|---:|
| spreader voltage | 1.48 V | **2.42 V** |
| NPN half, base-emitter | +0.581 V | +0.583 V |
| **PNP half, base-emitter** | **-0.341 V** | **+0.515 V** |
| idle current, NPN / PNP | 9.60 / **0.00** mA | 10.34 / 0.74 mA |

The PNP half was **reverse biased and completely off**. The amplifier was
running single-ended with a dead zone across every zero crossing -- audibly,
crossover distortion on an amplifier whose entire reputation is being clean;
numerically, an ill-conditioned solve every time the signal passed through
zero, which real playing does constantly at every level. That is why the
fallback count was high even at **-24 dBFS**, nowhere near clipping.

**Four of the six sheet-derived tests passed throughout.** The printed 26 dB
gain, the printed input and output levels, the distortion trend and the
clipping point are all measured with the amplifier well away from crossover,
so none of them could see it. `both_halves_of_the_output_stage_are_turned_on`
now checks the thing they could not, and the spreader's assertion was tightened
from `0.8..3.0` V -- which passed at 1.48 -- to `2.2..3.0`.

### And then the node between the two spreader transistors

Correcting the bias exposed a second defect underneath it. Q12's collector
meets Q14's base and **nothing else** -- that is what the sheet draws, and it is
perfectly fine on a board where every junction has capacitance and every track
has stray. In a solver it is a node held only by two reverse-biased junctions'
leakage, about 1e-12 S each. Once the output stage was biased hard enough to
drive the spreader into cutoff at clipping, any imbalance between those two
moved the node by a billion volts: `q14_b` reached **-1.06e9 V**, the amplifier
latched against its rails and stopped crossing zero, and the solve went to 43.6
Newton passes a sample and thirty-one million fallbacks.

The fix is the two parts' own **collector-base capacitance**, `Cob`, from their
data sheets -- 3.5 pF for the 2SA1015 and 2.0 pF for the 2SC1815. Five
picofarads changes no response anywhere at audio, and it raises that node's
conductance from 1e-12 S to about 5e-7 S, which is the difference between an
ill-conditioned row and a well-posed one.

### Where the power stage ended up

Sine into the speaker-loaded stage, one second at each level:

| input | before today | after | | |
|---|---|---|---|---|
| | passes / fallbacks | passes / fallbacks | output | zero crossings |
| 1.0 V | 2.31 / 10 | **2.09 / 0** | ±16.1 V | 221 |
| 3.0 V | 4.21 / 2,969 | **2.49 / 2** | ±38.7 V | 221 |
| 5.0 V | 5.16 / 4,242 | **2.66 / 733** | ±41.3 V | 221 |
| 8.0 V | — | **2.80 / 1,566** | ±42.1 V | 221 |

It clips and recovers at every level now, instead of latching.

### What is still open

The published input sensitivities do not come out of the network. The sheet
gives HIGH as -30 dBm and LOW as -20 dBm, which is 10 dB apart, but R1 33 k and
R2 68 k feed the same node against R8's 680 k, and that divider is **0.4 dB**
apart -- measured through the whole channel. Either the LOW jack works against a
different shunt than the one modelled, or the printed figures are specified
differently from how they read. The 680 kOhm input impedance and the two test
points at CN3 are reproduced, so whatever this is, it is local to the jacks.
Worth a return to the sheet; it does not affect anything measured above.

## Architecture note

The amplifier is stereo by construction: one power amplifier and one 12" driver
carry the modulated signal, the other the dry. That is the source of the chorus's
spatial character and it must not be replaced by a mono chorus with a stereo
widener downstream. This is the first model in the plugin that is natively
stereo at the speaker.

### Where the split is taken, and why it is not where the board is

**Built 2026-09-23.** `src/dsp/bbd.rs` is the delay line, its oscillator and the
filters either side; `Chain::process` takes the split; the panel's **CHORUS**
knob is in the Tone section beside the Twin's Reverb, Speed and Intensity.

On the amplifier the EFF BOARD sits between CH-1's output and the two power
amplifiers. In the plugin the split is taken at the **end** of the chain, after
the power stage and the speaker, and this is the model's one placement
approximation. The argument:

- The JC-120's output stage is a complementary transistor amplifier with
  26.7 dB of loop gain to spare, and the speaker is a passive load. Below
  clipping both are **linear and time-invariant**.
- An LTI filter commutes with a swept fractional delay to first order in the
  sweep's *rate of change*. Here that rate is 4.864 ms over a 0.6 s half
  period: **0.81 %**. Filtering before the delay rather than after therefore
  moves a filter corner by 0.81 % and changes nothing else.
- The alternative is a second power-stage simulation and a second acoustics
  path, configured identically, for that 0.81 %. That is twice the most
  expensive part of the chain.

Above clipping the two do differ: the modulated speaker would clip on its own
delayed signal rather than on the dry one. That is the limit of the
approximation and it is stated here rather than discovered later. `APPROXIMATED`.

What is *not* approximated: each speaker carries **one signal whole**. There is
no wet/dry sum anywhere in the path, and the knob interpolates between SW3's two
positions -- OFF, both speakers dry, and CHORUS, one speaker carrying the delay
and nothing else. Intermediate positions are the plugin's, because on the
hardware this is a switch: in CHORUS the oscillator is a fixed 1.2 s triangle
with no panel control over it at all.

**The microphone pans give way to it.** Two capsules panned apart and a second
speaker carrying a delay are two different pictures of the same cabinet, and
superimposing them is neither. With a chorus in the circuit the stereo field is
the amplifier's; both microphones are still heard, summed at their blend.

**It needs a stereo bus.** A mono source duplicated across one -- which is how a
guitar arrives -- gives the chain both outputs and the split lands. A genuinely
stereo input is already two independent amplifiers, each owning its own side,
and there each one's mono output is its dry side. That is the same condition the
microphone pans already carry; see `stereo_source` in `plugin.rs`.

### What was measured

`tests/chorus.rs`, twelve tests. The printed figures are asserted as literals
and the line is then measured against them:

| claim | how |
|---|---|
| 1.28 ms and 6.144 ms | `N / (2 f_clk)` at both printed clock periods |
| the line actually sweeps 4.864 ms | an impulse at the oscillator's two ends, differenced so the filters' 98 µs group delay cancels |
| 1.2 s in CHORUS, 120-500 ms in VIBRATO | `Bbd::period` at every Speed |
| about 14 cents | 4.864 ms over a 0.6 s half period |
| SW3 OFF changes nothing | left and right identical **bit for bit**, at all five rates |
| the dry side is untouched in CHORUS | left equals the mono path exactly, sample for sample |
| no allocation | `assert_no_heap` over a chorusing chain at all five rates |

## Open questions

- CH-2 and the reverb are still to read off the 2000 sheet.
- **Vibrato is built but not exposed.** `Bbd::Mode::Vibrato` carries the CH1
  board's own 120-500 ms sweep and the panel's Speed reaches it, but SW3's third
  position has no control on the plugin's panel yet. It needs a selector rather
  than a knob, and the Tone section's second row is full.
- ~~The Roland 30-103D is still APPROXIMATED by the American ceramic 12".~~
  **Resolved 2026-09-24.** The amplifier's own cabinet (Jazz Open 2x12, Roland's
  documented 750 x 540 x 270 mm) and driver (Jazz 12, the 30-103D) are built, and both Jazz
  presets play through them. The driver's voicing is fitted to a published measurement of
  a JC-120 through its return jack, to 1.16 dB rms; see `speakers.md` and `cabinets.md`.
