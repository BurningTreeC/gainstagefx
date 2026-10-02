# American VT-40 and American V-4B (Ampeg VT-40, V-4B) research log

Engineering identity: two Ampeg valve amplifiers of 1971 from one family -- the
**VT-40**, a 60 W 4x10 combo with reverb, and the **V-4B**, the 100 W bass head -- both
with a 12AX7 input per channel, the passive bass and treble of a packaged network into a
6K11 compactron, the midrange selector's tapped inductor (the SVT's idea), a 12DW7, a
12AU7 inverter and four 7027As. Proposed stable ids `amp_ampeg_vt40`,
`amp_ampeg_v4b`, `power_ampeg_7027a`. Proposed display **American VT-40** and
**American V-4B**, after the American SVT.

Status: **RESEARCHED 2026-10-02, CLEARED** (question 2 on Ampeg's own factory
drawings), **with one gap**: the packaged tone network's parts are known but not their
positions. Not built.

## Eight-question checkpoint

1. **Revision.** The 1971 drawings, both: the VT-40 service section (12/71-195 to 202:
   specification, parts lists, transformer data, a.c. and tube-pin voltage tables,
   chassis layout, schematics) and the V-4B schematic 12/71-223 with its power and tone
   board pictorials (12/71-222, -221). The album rig accounts for *Songs for the Deaf*
   (2002) put a VT-40 at the centre and V-4Bs beside it (`brit_200.md`); their years are
   not given, and the 1971 circuit is what Ampeg documents.
2. **Original schematic found?** **Yes: Ampeg's own**, published on its support page
   (ampeg.com/support/schematics.html), with DC voltages on the drawings and, for the
   VT-40, a pin-by-pin tube voltage table. In `docs/schematics/ampeg_v4b/` (not
   committed).
3. **Best source.** Those drawings. Ampeg's page also carries the V-4, VT-22 (the 2x12
   VT-40) and the V-4B's later revision (V-4BH).
4. **Cross-check.** The voltages on the drawings: the V-4B's 7027A plates 540 V and
   screens 527 V, bias -64 V, the 12AU7 inverter's plates 200 V and cathodes 9.7 V, the
   12DW7 at 325 / 205 V, the 6K11's sections at 192 / 178 / 325 V; the VT-40's at
   354 V on the preamp rail, V1 205 / 2.2 V, the 6K11's 209 / 194 V. The VT-40's
   specification: 60 W RMS into 2, 4 or 8 ohm. The SVT's own midrange inductor and
   switching (built, `american_svt.md`), which these drawings repeat.
5. **Signal path and values** (VT-40, from the drawing; the V-4B the same family):

```text
V1 12AX7 (channel 1, "bright"): C1 .005 in, R1 220 k, R5 390 k plate, R3 33 k / R4 6.8 k
   cathode with C2 10 uF and the SENSITIVITY switch (R2 6.8 k), C3 .01 to VOL 1M lin
   (C102 120 pF, C101 .001 the bright and ultra hi parts)
V1b: R6 68 k plate, R7 3.3 k cathode, C4 .01 to the P.E.C. 6470000 (bass and treble),
   BASS 1 M log, TREBLE 1 M log, R101 120 k
V201 6K11: the tone network's recovery and the midrange (VR105 50 k lin with the
   inductor), R204 220 k, R207 470 k, R201 1 M, R202 560, R203 7.5 k, R208 56 k,
   R211 47 k, R210 6.8 k, R212 620, C203 / C204 .33 uF
V3 12DW7, V4 12AU7, the inverter, and V5-V8 7027A -- values here from the V-4B's
   drawing (12/71-223), the VT-40's still to be read from its own: R30 / R31 47 k
   plates, R28 1.5 k, R29 / R27 1 M, R33 820 k; 47 k grid leaks, 470 R 2 W screens; the
   output transformer's 2 / 4 / 8 ohm taps; R34 4.7 k // C14 .001 in the feedback
VT-40 only: V202 6CG7 and V203 12AX7, the reverb's driver and recovery
```

6. **To be modelled exactly.** Each stage as drawn, the midrange inductor and its
   selector, Ultra Hi, the sensitivity switch, the 7027A push-pull with fixed bias.
7. **To be approximated, and why.**
   - **The P.E.C. 6470000** (Centralab), the bass and treble network in a seven-pin
     package: its parts are WIDELY REPORTED -- two resistors, 220 k and 22 k, and four
     capacitors, 470 pF, 4.7 nF, 1 nF and 10 nF (Music Electronics Forum, "71 Ampeg
     V4B Questions"); the drawing shows its topology between pins 1 to 7 but not which
     part sits where. **Open**: a drawing with it discrete (the thread cites Ampeg's
     B-12XT; the file Ampeg's page links under that name is a B-15T owner's guide) or a
     measurement, before the network is built. Placing the six parts by a Baxandall's
     own logic would be PLAUSIBLE, not documented.
   - **The 6K11** (a compactron triple triode) and **the 6CG7** need fits from their
     data sheets; the **7027A** is a 6L6GC in another base with higher ratings, the
     solver's 6L6GC fit a first approximation (PLAUSIBLE).
   - **The reverb** (VT-40): the plugin's spring tank, as the Twin's.
8. **Why.** The album preset *Desert Deaf '02* is blocked on them (`docs/ROADMAP.md`,
   section C).
