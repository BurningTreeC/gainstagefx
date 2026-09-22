# Jazz 120 (Roland JC-120 Jazz Chorus) research log

Engineering identity: **Roland JC-120 Jazz Chorus**, the analog BBD-chorus
revision covered by PART 1 of the Fifth Edition service notes. Proposed stable
id `amp_roland_jc120` (`circuit`), power stage `power_roland_jc120_ss`, cabinet
`cab_roland_jc120_2x12`. Proposed display: **Jazz 120**.

Status: **RESEARCHED 2026-09-22, CLEARED FOR CODE.**

## Sources

- **Roland JC-120/JC-160 Service Notes, Fifth Edition** (authority), downloaded
  to `docs/schematics/roland_jc120_jc160_service_notes.pdf` from
  [this archive copy](http://bee.mif.pg.gda.pl/ciasteczkowypotwor/%23pro_audio/Roland/Roland%20JC-120,%20JC-160%20(79-5)%20Jazz%20Chorus%20Service%20Manual.pdf).
  Ten scanned pages: PART 1 (pp. 1-4) and PART 2 (pp. 5-8, the Fourth Edition
  content carried forward). No text layer; read as 100-600 dpi renders.
- [Roland Jazz Chorus, Wikipedia](https://en.wikipedia.org/wiki/Roland_Jazz_Chorus)
  for revision history context only.

## Which revision, and why it matters

The service notes are explicitly split by serial number, and **the two parts are
different amplifiers**:

| | JC-120 serials | JC-160 serials |
|---|---|---|
| PART 1, pp. 1-4 | 258100 and up (except USA); 293150 and up (USA only) | 254510 and up |
| PART 2, pp. 5-8 | 789500-248099; 950100-283149 (USA only) | 782921-244509 |

PART 2 additionally covers JC-120A 810600-001099. The title page states that this
Fifth Edition "makes Fourth Edition (Printed MAR. 20, 1979) obsolete and includes
its contents in PART 2 of this edition."

**PART 1 is the target.** Its semiconductor list gives the chorus devices
directly: `IC3 — MN3007` (BBD) and `IC4 — MN3101` (clock generator/driver), with
`IC1, IC2, IC5 — NJM4558BD`. That is the architecture the brief asked for, and
it is pinned by serial number rather than inferred. PART 2 is a different chorus
circuit and must not be mixed into it.

**One discrepancy recorded rather than resolved.** The brief specified "Fifth
Edition, January 20, 1983". This copy is headed Fifth Edition and supersedes a
March 1979 Fourth Edition; the archive file name carries `79-5`. No January 1983
print date is visible on the scan. The *contents* are what matter and they are
unambiguous (serial ranges plus the MN3007/MN3101 parts list), but the printing
date is UNVERIFIED against this copy.

## Eight-question checkpoint

1. **Revision.** JC-120, PART 1 of the Fifth Edition service notes, serials
   258100 and up (293150 and up in the USA). Analog MN3007/MN3101 chorus.
2. **Original schematic found?** **Yes.** Manufacturer service notes with the
   full schematic sheet, PCB artwork for both pre-main boards, the power supply
   board, the parts list and the factory signal-voltage conditions.
3. **Best source.** The service notes themselves.
4. **Cross-check.** The published specification table, the parts list and the
   schematic agree with each other; the potentiometer values on the parts list
   (below) match the pots drawn on the schematic sheet.
5. **Signal path and values.** DOCUMENTED from the notes:

   **Published specification** (Fifth Edition, p. 1):
   - Output 120 W rms; speakers JC-120 30 cm × 2 (JC-160 25 cm × 4)
   - Power consumption 400 VA
   - Input sensitivity @ 1 kHz: **HIGH 20 mV, LOW 60 mV, MAIN IN 800 mV**
   - Line output 100 mV @ 1 kHz, 100 Ω
   - Residual noise 7-35 mV per channel, varying with control settings
   - **Tone control range: TREBLE 17 dB @ 10 kHz, MIDDLE 13 dB @ 350 Hz,
     BASS 14 dB @ 50 Hz, BRIGHT 5 dB @ 10 kHz (Volume at centre)**
   - Dimensions 750 × 540 × 270 mm, 28 kg

   **Controls, from the parts list** — note how low-valued the Roland active-network
   pots are, which is not guessable and is why the list matters:
   | Control | Value |
   |---|---|
   | Volume | 1 MΩ B |
   | Treble | 250 kΩ B |
   | Bass | 250 kΩ A |
   | Middle, Reverb | 10 kΩ B |
   | Distortion (with switch) | 10 kΩ C |
   | Vibrato Depth | 100 kΩ B |
   | Vibrato Speed | 100 kΩ A |
   | Bright | switch SLR-022 |
   | Vib/Chorus | switch ESL-3654 |

   **Semiconductors, from the parts list:** JFETs 2SK117-GR and 2SK246-GR;
   bipolars 2SA970-GR, 2SC1815-GR, 2SC2240-GR, 2SB847-C, 2SD667-C, 2SD669-AC,
   2SB849-AC, 2SC2229, 2SD313-E, 2SA970-GR, 2SC3182/2SC2773/2SB849 in the output
   stages; ICs NJM4558BD, MN3007, MN3101; diodes 1S2473, 1N4003, S5500G, RD16E82,
   RD30E84.

   **Supply.** Secondary winding ratings (DC) 37 V × 2 @ 2.5 A, 3300 µF input.

   **Reverb.** Tank part `Z-3F` (12389101).

   **Speaker.** JC-120 uses Roland **30-103D** (2241020400) — the part number the
   brief expected, confirmed on the parts list.

   **Factory measurement conditions**, so the tone targets are reproducible:
   1 kHz sine, Bass fully clockwise, Treble fully clockwise, Middle fully
   counter-clockwise.
6. **To be modelled exactly.** The JFET/bipolar preamp stages and the op-amp
   stages from the schematic sheet; the High/Low input network; the active
   tone network with its real pot values and tapers; the Bright network; the
   distortion circuit's own clipping mechanism; the two independent solid-state
   power amplifiers; the chorus/vibrato routing and the dry/wet speaker split.
7. **To be approximated.** The MN3007 bucket brigade is not simulated stage by
   stage. It becomes a modulated fractional-delay line carrying the documented
   clock range, the companding/pre- and de-emphasis filtering around it and the
   BBD bandwidth limit — an equivalent, labelled APPROXIMATED, validated against
   the schematic's filter values rather than tuned by ear. The 30-103D has no
   published Thiele/Small data, so the speaker and the open-back 2x12 are
   ESTIMATED as in [speakers.md](speakers.md), with the two driver positions kept
   independent so the chorus stays genuinely stereo.
8. **Why.** The plugin has no solid-state power amplifier and no BBD at all. Both
   are reusable well beyond this model, and the JC-120's defining quality — very
   high clean headroom with no sag and a hard transistor overload — is precisely
   what the existing tube catalogue cannot produce.

## Architecture note

The amplifier is stereo by construction: one power amplifier and one 12" driver
carry the modulated signal, the other the dry. That is the source of the chorus's
spatial character and it must not be replaced by a mono chorus with a stereo
widener downstream. This is the first model in the plugin that is natively
stereo at the speaker, and it will need the acoustics path to keep left and
right independent.

## Open questions

- The Fifth Edition print date (see above).
- The MN3101 clock range and the LFO sweep limits must be read off the PART 1
  schematic sheet during implementation rather than taken from BBD folklore.
