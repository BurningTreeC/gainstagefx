# American 4x10 (Gallien-Krueger 410RBH) research log

Engineering identity: **Gallien-Krueger 410RBH**, the RBH-series 4x10 with horn: four
10-inch woofers and a horn in a vented box, a passive 18 dB/octave crossover with a horn
attenuator, and a bi-amp Speakon input. The cabinet GK's own heads are shown and sold
with, the 800RB among them. Proposed stable ids `cab_gk_410rbh` (`cab_model`) and
`spk_gk_p10_200` (`speaker`); proposed display **American 4x10** and **Cast Bass 10**.

Status: **IMPLEMENTED 2026-10-02, horn included** (`CabinetProfile::AMERICAN_410`,
`SpeakerProfile::CAST_BASS_10`), on the owner's decision to build on estimates. The
acoustics gained a front vent, a passive crossover on the woofers and a horn branch with
its attenuator (the panel's Horn knob). Results below.

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

## Results (2026-10-02)

Measured by `tests/american_410.rs`; `examples/american_410_op.rs` prints the tuning.

- **The vents tune the box**: 143.5 l net, two 3 in tubes (45.6 cm2 each) 52 mm deep,
  12.3 cm with the ends corrected -- Helmholtz 42 Hz. The four tens' impedance in it has
  the vented box's two peaks either side of a dip at 40.6 Hz; sealed, the same box has
  one peak, at 69 Hz.
- **And radiates as a vented box**: at a metre, what the vents add over the same box
  sealed is what **Small's theory of the vented box** gives for these drivers and this
  box, to 0.2 dB from 35 to 120 Hz (+2.2 dB at 40 Hz, +2.5 at 60). That checks the
  cavity, the ports' acoustic mass and radiation resistance, their radiation from the
  baffle, and the drivers' motion in the load, together. (The first test of it expected
  5 dB or more at 40 Hz; that was the expectation's error, not the box's.)
- **The crossover**: -0.1 dB at 1.5 kHz, -18.3 dB at 6 kHz and -37 dB at 12 kHz against
  the same tens full range -- a third-order low-pass at 3 kHz.
- **With these estimated drivers** the cabinet is -3 dB near 57 Hz and -10 dB near 38 Hz,
  whatever the tuning (Small's function, Qts 0.42, alpha 2.06): GK's "usable response
  31 Hz" names no level and is not fitted. The estimated drivers' sensitivity is some
  99 dB at 1 W for the four (DERIVED from the estimates by Small's efficiency
  formula, not measured), against GK's 106 dB (1999) and 102 (the later Paragon
  cabinet); 106 is not what four tens of this kind do, and is not fitted either.
- **Every other cabinet is untouched**, to the bit: no vent and no crossover leave the
  open-back opening and the pass-through filters exactly as they were (both frozen
  baselines and every cabinet test pass).

- **The horn** (added the same day): fed from the speaker's terminals in the loaded
  power stage through the crossover's third-order high-pass and the attenuator, with its
  own decimator and padding beside the cone's stream, and radiated from its mouth on the
  baffle in the acoustic stage. A metre out it puts 6 kHz back level with the tens' band
  at 1 kHz (-44.9 dB against -46.4), where the tens alone are 40 dB down behind the
  crossover; on a close microphone at a ten's cone, far off the horn's axis, it adds
  3 dB. With the attenuator down the output is the cabinet without a horn, to the bit.

**Approximated**: the P10/200 beyond its 32 ohm, 200 W and 43 Hz (Re, Mms, Qms and Qes
ESTIMATED; coil and breakup the B810's as priors); the vents' size and depth and the
layout (ESTIMATED); the crossover's corner (3 kHz, ESTIMATED) and its place -- on the
cone's motion in the acoustic stage rather than in the amplifier's load, where its
series inductor would cost every speaker-loaded stage the unknowns for a difference
only near the corner; **the horn**, the P508, of which nothing is published: 102 dB for
2.83 V at a metre, 2 to 13 kHz, a cos^2 coverage (a 90 degree horn), a point source at
its mouth, and the attenuator's law square in the knob (-12 dB at its middle), all
ESTIMATED; it takes no load from the amplifier, and the plugin's tone section (which on
the physical path sits after the power stage) reaches the cone's stream only.

## What building it needed

Not a profile alone. The acoustics have sealed and open-back boxes and no tweeter:

1. **A vent** (`acoustics::cabinet::Vent`): the open back's opening element -- an
   acoustic mass with radiation resistance in the speaker load, and a radiator in the
   cavity model -- given a tube's area and length with both ends corrected (flanged
   outside, free inside), and placed on the **front** at the ports. Generic: every
   ported bass cabinet the roadmap names can use it.
2. **A crossover** (`CabinetProfile::crossover_hz`): a third-order Butterworth low-pass
   on the cone's motion in `acoustics::stage`.
3. **A horn** (`acoustics::cabinet::Horn`): the speaker terminal's voltage through the
   crossover's high-pass and the attenuator (`cab_horn`, appended), a second stream
   through the back half -- its own decimator, its own padding, across the pipelined
   hand-over beside the cone's value -- and a point source in the acoustic stage at the
   horn's position, read by every capsule at its own distance and angle.
4. **The profiles**, `cab_gk_410rbh` and `spk_gk_p10_200`, appended to their enums,
   with `migrate()` untouched (a new choice, nobody's default).
