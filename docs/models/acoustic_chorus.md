# Acoustic Chorus (Roland AC-90) research log

Engineering identity: **Roland AC-90 Acoustic Chorus**, the 45 W + 45 W stereo
two-way acoustic instrument amplifier. Proposed stable id `amp_roland_ac90`
(`circuit`), power stage `power_roland_ac90_ss`, cabinet `cab_roland_ac90`.
Proposed display: **Acoustic 90**.

Status: **RESEARCHED 2026-09-22. RECORDED ONLY -- NOT SCHEDULED FOR CODE.**

Decision, 2026-09-22: this log stands as a record of what *is* available, and
nothing is built from it for now. It is **not** a replacement for the AC-60.
The AC-60 remains the amplifier that was asked for; it is blocked on its own
service notes, exactly as the Century 200 is blocked on its schematic. If those
turn up, the AC-60 gets its own log and its own model. The research below is
kept because it is the answer to "what Roland acoustic documentation can
actually be had", and because it records the BC-30/60 file-naming trap.

## Why the AC-60 could not be built, and what exists instead

The AC-60 was the model proposed, and the research did not support it. Its
service notes exist but are behind a paywall or a registration wall everywhere
they surface (pdfcoffee, themanualsservice); nothing legible is publicly
available. The **AC-90's** are, in full, with a text layer, from the same Roland
archive the JC-120 notes came from.

Building an AC-60 out of AC-90 data is precisely what `CLAUDE.md` forbids and
what [studio_pre.md](studio_pre.md) stops for. The AC-90 is therefore recorded
as what *is* documented, not substituted for what is not.

**One caution recorded while looking.** The same archive holds a file called
`Roland - AC30 - AC60.pdf`. It is **not** an AC-30 or an AC-60: it is the
**BC-30/BC-60 Blues Cube** service notes, First Edition, July 1995, an electric
guitar amplifier with a RECTIFEX switch. It was downloaded, opened, found to be
a different product and renamed
`docs/schematics/roland_bc30_bc60_bluescube_service_notes.pdf`. Trusting the
file name would have produced an "AC-60" that was a Blues Cube.

## Sources

- **Roland AC-90 Service Notes, July 2007** (authority), `17058507E0`, issued by
  RJA, downloaded to `docs/schematics/roland_ac90_service_notes.pdf` from
  [this archive](http://bee.mif.pg.gda.pl/ciasteczkowypotwor/%23pro_audio/Roland/Roland%20AC-90%20Acoustic%20Chorus%20Service%20Manual.pdf).
  Specifications, parts lists, block/wiring diagram, and component-level circuit
  diagrams for the Main board (Digital, Analog, Power Supply), the XLR, Jack,
  Panel, PS1 and Amp boards.
- [Roland AC-90 owner's manual](https://static.roland.com/assets/media/pdf/AC-60_OM.pdf)
  family, for the control descriptions.

## Eight-question checkpoint

1. **Revision.** AC-90 as documented by the July 2007 First-Edition service
   notes. Not the AC-60, not the AC-33, not the later AC-90A.
2. **Original schematic found?** **Yes for the signal path, not yet for the
   power amplifier.** Every sheet present is component-level: each resistor,
   capacitor, diode, JFET, transistor and IC carries a reference designator and
   a value.

   The copy obtained is **truncated**. Its table of contents runs to document
   page 42, and the file stops at page 29. Present: Main Board Analog + XLR
   (the guitar and mic front ends, the EQ, the Master and the phones amp), Main
   Board Digital, and Main Board Power Supply. **Missing: the Jack, Panel and
   PS1 boards and, importantly, the Amp Board circuit diagram** -- the power
   amplifier itself, which the contents place on page 42.

   That is enough to build the channel and not enough to build the power stage
   from the drawing. See "Open questions".
3. **Best source.** The service notes themselves.
4. **Cross-check.** The parts list, the block diagram and the circuit diagrams
   agree; the published specification agrees with the level annotations printed
   on the schematic itself (`Gt.INPUT -7.5 dBu`, `MIC.INPUT -6.6 dBu`,
   `-0.5 dBu`, `+2.5 dBu`, `-9.7 dBu`, `-10.2 dBu`, `-3.7 dBu` and `1.3 dBu` at
   the phones, stage by stage).
5. **Signal path and values.** DOCUMENTED.

   **Published specification:**
   - Rated power output **45 W + 45 W**
   - Speakers: woofer **20 cm (8") x 2**, tweeter **8 x 5 cm x 2** -- a
     genuinely two-way stereo system, which is what gives an acoustic amplifier
     the top octave a guitar cabinet does not have
   - Nominal input: GUITAR **-10 dBu**; MIC/LINE **-50/-10 dBu**; AUX IN -10 dBu
   - Nominal output: DI/TUNER, LINE OUT and SUB WOOFER OUT all **+4 dBu**
   - `0 dBu = 0.775 Vrms`

   **Guitar channel controls:** **PICKUP switch (PIEZO/MAGNETIC)**, **SHAPE
   switch**, Bass/Middle/Treble, Volume, **Chorus switch**.
   **Mic/Line channel:** phantom, Volume, Bass/Middle/Treble, Select
   (MIC/LINE), Chorus switch.
   **Master section:** Chorus knob, Reverb/Delay knob, **Anti-Feedback**
   (Frequency knob, Start button), Mute, Master.

   **Guitar front end, read off the analog sheet:** `JK1` YKB21-5012 into a
   `BLM21AG102SN1` ferrite, `R71` 10 k, `C90` 0.1 uF, `C65` 10 pF and a `DA7`
   1SS302 clamp pair; `R81` 2.2 M and `R76` 220 k around the pickup switch
   (`M_MAG_AH`), into `IC8A` NJM2082M with `R54` 1.2 k, `C70` 0.012 uF,
   `C71` 47 pF, `R65` 6.8 k and `R66` 12 k. Then `IC9A`/`IC9B` NJM2100M with
   `R70` 12 kD, `R69` 18 kD, `R79` 56 kD and `R77` 5.6 kD.

   **The effects are digital.** `IC10` is an **AK4552VT** codec (LIN/RIN,
   LOUT/ROUT, LRCK/MCK/BCLK, SDTI/SDTO) feeding a DSP on the digital board. The
   chorus, the reverb/delay and the anti-feedback notch all live there, not in
   an analog delay line. This is a 2007 design and there is no BBD in it.

   **Output side:** `IC17A`/`IC12A`/`IC12B`/`IC17C` UPC4072G2 around
   `2SK880GR` JFET mutes and `2SC4116GR` switches, the `RK0971410 10KBX4`
   Master (`VR1A`/`VR1B`), then `M_POUTL`/`M_POUTR` to the power block, plus the
   phones amplifier on `IC18A`/`IC18B` NJM4556AM.
6. **To be modelled exactly.** The guitar channel's input network and its
   PIEZO/MAGNETIC loading -- which is the whole reason this amplifier suits a
   piezo and a guitar amplifier does not -- the op-amp gain stages with their
   real values, the SHAPE filter, the three-band EQ, the Master and the power
   amplifier.
7. **To be approximated.** The chorus, the reverb/delay and the anti-feedback
   are digital in the original, so DSP equivalents are honest here in a way they
   would not be for an analog circuit; they will be labelled APPROXIMATED
   against the published control ranges. Op-amp stages that stay comfortably
   linear are solved analytically rather than in Newton, per the repo's rule
   against Newton-solving linear op-amps. The two-way cabinet has no published
   Thiele/Small data, so the woofer/tweeter system is ESTIMATED as in
   [speakers.md](speakers.md), with left and right kept independent because the
   amplifier is genuinely stereo.
8. **Why.** It gives the plugin its first wide-band, two-way, full-range
   amplifier -- the opposite end of the catalogue from a guitar speaker -- and
   it does so from a complete drawing rather than from published curves. It also
   shares the stereo-at-the-speaker requirement with the JC-120, so the two pay
   for that work together.

## The chorus control

Per the standing requirement, the Chorus knob belongs in the **Tone section** of
the panel, not among model-specific parameters. The AC-90 has both a per-channel
Chorus switch and a master Chorus knob; the panel exposes the knob, with the
switch implied by the knob being at zero.

## Open questions

- Which of the SHAPE switch's two positions the published response figures
  describe; to be read off the sheet during implementation rather than assumed.
- **The Amp Board sheet is missing from the copy in hand** (see question 2).
  Until a complete copy of the service notes turns up, the power amplifier is
  either left out -- the channel can run into any existing power stage, or into
  none -- or built as a documented solid-state approximation to the published
  45 W + 45 W rating and labelled ESTIMATED. It will not be inferred from the
  AC-100's or the Blues Cube's output stage: that is the same substitution the
  Century 200 is stopped for.
