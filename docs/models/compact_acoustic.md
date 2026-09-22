# Compact Acoustic (AER Compact 60/3) research log

Engineering identity: **AER Compact 60/3**, the two-channel 60 W acoustic
combo. Proposed stable id `amp_aer_compact60_3` (`circuit`), power stage
`power_aer_dmos_60`, cabinet `cab_aer_compact60`. Proposed display:
**Compact Acoustic**.

Status: **RESEARCHED 2026-09-22, CLEARED FOR CODE WITH LIMITS.** Question 2
fails in the usual sense — there is no component-level schematic — but this is
not the `studio_pre` situation, because there is no near relative being
substituted. What exists instead is a manufacturer block diagram plus an
unusually complete published specification, and the model is built from those
and labelled accordingly. Read the "What this means for the model" section
before writing code.

## Sources

- **AER Compact 60/3 user manual, 2012_07_GB**, including the sheet AER itself
  titles *Blockschaltbild / Circuit Diagram / Schéma Fonctionnel*, drawing number
  `B090216C_20110401`. Saved to
  `docs/schematics/aer_compact60_3_block_diagram.pdf` from
  [zikinf](https://www.zikinf.com/manuels/aer-compact-60-3-diagramme-en-43271.pdf).
- [Compact 603 user manual, full](https://www.djangobooks.com/archives/aer_compact60_manual.pdf).
- [AER official downloads](https://www.aer-music.de/downloads/?lang=en).

## Eight-question checkpoint

1. **Revision.** Compact 60/3 specifically — the third generation, 2012
   documentation. Not the Compact 60/1 or /2, and **not** the Compact 60/4,
   whose electronics are not proven identical and which is used for context only.
2. **Original schematic found?** **No. ORIGINAL SCHEMATIC NOT PUBLICLY LOCATED.**
   AER's "circuit diagram" is a functional block diagram. Extracted in full, the
   only component values it carries are two `6k8` mic input resistors, one
   `470R`, and the `+48 V` and `+9 V` phantom rails. There are no reference
   designators, no semiconductor types and no filter component values.
3. **Best source.** The manual's block diagram for architecture and the
   technical-data table for behaviour. Both are manufacturer documents.
4. **Cross-check.** The block diagram's signal order agrees with the manual's
   control descriptions and with the published specification table.
5. **Signal path.** Architecture DOCUMENTED, values NOT.

   From the block diagram, in order:

   ```
   CH1: LINE in -> PAD -> PREAMP -> GAIN -> COLOUR -> BASS/MIDDLE/TREBLE
                                                   -> CLIP detect -> EFFECT PAN
   CH2: MIC/LINE -> MIC GAIN H/L (+48 V) -> PREAMP VOICE -> GAIN
                                         -> BASS/TREBLE -> CLIP detect
   both -> SELECT -> MASTER -> SUBSONIC -> LIMITER -> POWER AMP -> DUAL CONE SPEAKER
   with FX send/return, PHONES, LINE OUT, TUNER, DI and a footswitch tap.
   ```

   Published behaviour (the numbers the model is actually built to):
   - Channel 1 input 2.2 MΩ ‖ 350 pF, unbalanced, nominal 100 mV / -20 dBV
   - HIGH/LOW attenuator ≈ -10 dB; minimum input HIGH 22 mV / -33 dBV,
     LOW 68 mV / -23 dBV; maximum HIGH 3.5 V / +11 dBV, LOW 5 V / +14 dBV
   - Colour: **-3 dB @ 700 Hz, +10 dB @ 8 kHz**
   - Channel 1 EQ: Bass ±8 dB @ 100 Hz shelving, Middle ±6 dB @ 800 Hz,
     Treble ±8 dB @ 10 kHz shelving
   - Channel 2 EQ: Bass ±8 dB @ 100 Hz, Treble ±11 dB @ 10 kHz
   - Power amp 60 W into 4 Ω, monolithic IC with DMOS output;
     THD+N < 0.1 % at 6 W / 4 Ω; **limiter threshold ≈ 50 W / 4 Ω**
   - A-weighted S/N ≈ 95 dB
   - Speaker 8" / 200 mm dual-cone, 4 Ω, bass-reflex; cabinet 12 mm birch ply,
     ≈ 260 × 325 × 235 mm
   - Four digital effects: two reverbs of differing pre-delay, a ≈ 320 ms
     delay, and a chorus
6. **To be modelled exactly.** Nothing can be, in the netlist sense, because no
   netlist exists. What *is* exact is the input loading — 2.2 MΩ ‖ 350 pF is a
   documented two-component network and it genuinely interacts with a piezo
   source, so it goes in as a real load rather than as a gain constant. The
   HIGH/LOW pad is a documented -10 dB with documented headroom either side.
7. **To be approximated.** Everything else, and each is labelled in the code
   comment that carries it:
   - Colour filter, active EQ and subsonic filter: **DERIVED** — filter forms
     chosen to realise the published corner frequencies, ranges and shelf/peak
     characters, fitted to the specification rather than to a circuit.
   - Adaptive limiter: **DERIVED** from the documented 50 W / 4 Ω threshold via
     `P = V_rms² / R`, with the supply-dependent behaviour the manual describes.
     Not a studio brick-wall limiter bolted on after the amplifier.
   - DMOS power amp: **APPROXIMATED** as a stiff, low-distortion, rail-limited
     stage meeting the published 60 W / 4 Ω and THD+N figures. No sag.
   - Speaker and cabinet: **ESTIMATED**, constrained by the documented 8" dual
     cone, 4 Ω, the bass-reflex alignment and the stated internal volume. No
     Thiele/Small parameters are published and none will be invented.
   - The four digital effects: the originals are digital, so DSP equivalents are
     honest here in a way they would not be for an analog circuit.
8. **Why.** This is the only wide-band, full-range, non-guitar amplifier in the
   plugin, and the whole point of it is that it is *not* a guitar speaker with a
   tone stack. The published response targets are testable, which is what makes
   building from specification defensible where building a tube preamp from
   specification would not be.

## What this means for the model

The rule this repository follows is schematic first, and the reason
`studio_pre.md` stops is that its near relative was a *different product* whose
values would have been silently substituted. Nothing like that is happening
here: no AER schematic is being guessed at from another AER, and no circuit
values are being invented. The model is built to published, measurable
manufacturer specifications and every block says so.

If that is judged too loose for this repository, the alternative is to stop, and
the unblock is the same as for `studio_pre`: an AER service schematic, or a
gut-shot photo set good enough to read the EQ and limiter networks directly.

## Open questions

- The subsonic filter's corner and order are not published. It will be DERIVED
  from its stated purpose (infrasonic rejection and limiter protection without
  removing acoustic body) and flagged, not asserted.
- The power amplifier IC is not identified in any public AER document, so no
  DMOS part data can be cited.

## Decision, 2026-09-22

**Build from published specification.** Confirmed by the owner: because no near
relative is being substituted and the published response targets are exact and
testable, the model proceeds with every block labelled DERIVED, APPROXIMATED or
ESTIMATED in both the code comment that carries the value and this log.
