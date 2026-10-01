# American 4x10 (Gallien-Krueger 410RBH) research log

Engineering identity: **Gallien-Krueger 410RBH**, the RBH-series 4x10 with horn: four
10-inch woofers and a horn in a vented box, a passive 18 dB/octave crossover with a horn
attenuator, and a bi-amp Speakon input. The cabinet GK's own heads are shown and sold
with, the 800RB among them. Proposed stable ids `cab_gk_410rbh` (`cab_model`) and
`spk_gk_p10_200` (`speaker`); proposed display **American 4x10** and **Cast Bass 10**.

Status: **CHECKPOINT 2026-10-01 -- buildable with estimates, not built.** GK documents
the box, its complement and two performance figures; it publishes nothing about the
driver beyond its impedance and rating. Building it needs two things the acoustics do not
have yet (a front vent and a horn branch); see the end.

## Eight-question checkpoint

1. **Revision.** Two generations share the name:
   - **RBH, Eminence-built** -- GK's *RBH Series Cabinets Owners Manual* (PageMaker,
     2 December 1999), 410RBH/8 and /4: **4 x P10/200 cast frame, 1 x P508 horn**, 800 W,
     **24 W x 27.5 H x 18 D in**, 100 lb, **3/4 in birch with dado joints**, black carpet,
     **two front vents**, **passive crossover 18 dB/oct** with attenuator and bi-amp mode,
     power-compensating horn protection, **sensitivity 106 dB**, **usable response 31 Hz
     to 13 kHz**. Shown with the 700RB, the bi-amp head of the time.
   - **RBH, Paragon** -- GK's current listing: four GK-Paragon 10B200-32 cast-frame
     woofers and a 5H50-8 tweeter, 27.5 W x 23 H x 18 D in, 50 Hz to 19 kHz, 102 dB, 92 lb.
     The figures, the tweeter and the dimensions all differ: a later cabinet under the
     same name.
   **Proposed: the Eminence-built RBH of the 1999 manual**, the one with GK's own
   specification sheet, and the generation of the 800RB's later production. The
   800RB's own launch-era (1983-91) cabinets are not researched here.
2. **Original documentation found?** **For the box, yes; for the driver, no.** GK's
   owner's manual is the primary document for the cabinet (above). No cabinet drawing,
   no vent dimensions, no crossover schematic or frequency, no horn data. For the
   P10/200: GK's parts store lists a 10-inch ceramic driver, **32 ohm, 200 W**, for the
   RBH 410 (DOCUMENTED); retail and forum listings call it an Eminence special design for
   GK, part 082-0000-A, and give a resonance of **43 Hz** (SECONDARY). No Thiele-Small set
   is published by GK or Eminence, and none measured was found.
3. **Best sources.** GK's 1999 owner's manual (local copy in
   `docs/schematics/gk_410rbh/`, git-ignored); GK's current parts listing for the driver;
   photographs of the front for the vents' and horn's positions.
4. **Cross-check.** GK's two performance figures are the targets an estimate has to meet:
   **106 dB** (four 32 ohm drivers in parallel, 8 ohm, so about 100 dB 1 W / 1 m a
   driver -- high for a ten, DERIVED) and **31 Hz usable** (a vented alignment tuned in
   the 30s, DERIVED). The SVT's 8x10 and the Legend B810 already in the catalogue give a
   sealed bass reference to compare against.
5. **Box and complement.**
   - Outside 610 x 699 x 457 mm (24 x 27.5 x 18 in); 19 mm birch: inside 572 x 661 x
     419 mm, **158 L gross**, about 140 L net after four drivers, the horn, two vents
     and bracing (DERIVED / ESTIMATED).
   - Four tens, one chamber (no divider is described), two front vents, the horn on the
     baffle (positions from photographs, ESTIMATED).
   - Full-range mode: woofers behind the crossover's low-pass, horn behind its high-pass
     and attenuator, both 18 dB/oct.
6. **What can be modelled exactly.** The box: outer and panel dimensions, material,
   driver count and layout, one chamber, vents on the front. The electrical topology of
   the crossover (third-order each way) and the load it presents to the amplifier -- the
   SS 800 sees the woofers' and the horn's impedances through it.
7. **What would be approximated, and why.**
   - **The P10/200** -- every Thiele-Small figure but Fs, impedance and rating, chosen
     within physical bounds for a 200 W cast-frame ten and **fitted to GK's 106 dB and
     31 Hz**, as the Jazz 12 is fitted to its one published measurement. ESTIMATED.
   - **The vents** -- area and length chosen for a tuning that gives the 31 Hz, inside
     what two front vents on this baffle can be. ESTIMATED.
   - **The crossover frequency** and **the horn** -- GK states neither. A crossover in
     the low kilohertz and a horn band ending at the manual's 13 kHz. ESTIMATED.
   - **The horn protection** (a lamp or PTC in series, "power compensating") is out of
     the audio path until the horn is overdriven: not built.
8. **Why.** The roadmap's classic solid-state chain ends in a 4x10, and this is the one
   GK documents. Through the SVT's sealed 8x10 the 800RB has no vents and no horn, and
   the horn above a few kilohertz is half of what the Hi Boost and the Mid Contour are
   for.

## What building it needs

Not a profile alone. The acoustics have sealed and open-back boxes and no tweeter:

1. **A vent.** The speaker load already carries the cavity's **opening** as an acoustic
   mass with radiation resistance (`acoustics::speaker::LoadValues`, the open back's
   Helmholtz element), and `acoustics::enclosure::CavityRadiation` already radiates its
   flow. A vent is the same element with a tube length and a position on the **front**
   rather than an open fraction of the back. Generic: every ported bass cabinet the
   roadmap names needs it.
2. **A horn branch.** The crossover's high-pass and attenuator as part of the load the
   power stage drives, a horn radiator at its baffle position fed from that branch, and
   the woofers behind the low-pass. Also generic: the modern bass cabinet needs it too.
3. **The profiles**, `cab_gk_410rbh` and `spk_gk_p10_200`, appended to their enums
   with `migrate()` untouched (a new choice, nobody's default).

Decision for the owner: build on these terms (documented box, estimated driver, vents
and crossover, all labelled), or wait for a measurement of a P10/200.
