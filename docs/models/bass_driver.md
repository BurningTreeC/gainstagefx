# Bass Driver (Tech 21 SansAmp Bass Driver DI) research log

Engineering identity: **Tech 21 SansAmp Bass Driver DI** (1994 on), an all-analogue "tube
amplifier emulation" preamplifier and DI in a pedal: a drive stage clipping on a zener pair,
fixed filters, a Blend with the dry signal, an active EQ and both a 1/4" and a balanced XLR
output. Proposed stable id `pedal_sansamp_bass_driver` (`pedal`), as the roadmap reserves it,
and a `_circuit` id if it is offered as a circuit too. Proposed display: **Bass Driver**.

Status: **CLEARED 2026-10-01** on independent traces of real units, with measurements.
Not built; the version is to be chosen (question 1).

## Eight-question checkpoint

1. **Revision.** Three generations, each traced:
   - **V1 early** (1994-97): NJM064 / TL062 op-amps, through-hole J113 switching, the
     drive stage potted in a module; Bass, Treble, Presence, Drive, Blend, Level.
   - **V1 late**: the same panel, TLC2264 / TLC2262, the drive stage in the "CH34-9"
     module (a TLC2262 and a DZ23C3V3 zener pair), and a deeper fixed mid-cut filter
     (IC2C, IC2D, IC3B) between the drive stage and the Blend.
   - **V2** (current production): adds a **Mid** control with a 500 / 1000 Hz shift and a
     **Bass shift** (80 / 40 Hz), simplifies the fixed filter, the drive stage in the
     "CH40" module (TLC2262, BZB984-C3V3 zener pair).
   **Chosen 2026-10-01 by the owner: V2.** V2 is the one in production and maps onto this plugin's
   panel without a gap (Drive, Presence, Blend, Level, Bass, Mid, Treble; the two shift
   switches onto the panel's low and mid switches). V1 late is the sound of the records
   from the 1990s and 2000s. The id would name the one built.
2. **Original schematic found?** **Yes, as traces of real units** -- no factory drawing
   is published. The tracer (kanengomibako, kanengomibako.github.io pages 00283-00285)
   bought the units, photographed both sides of each board, and drew them in KiCad:
   **V2** from a unit bought January 2022 (corrected April 2022); **V1 late** January
   2022 (corrected February and April 2022); **V1 early** by destructive tracing, a
   second unit bought to read the values hidden under the potting. The same tracer drew
   the Behringer BDI21 separately, which settles a question that hung over the older
   forum schematic (freestompboxes t=422, "Diz", 2006): its own layout author wrote in
   2017 that it was never "corroborated with an original tech 21", and posters thought it
   the Behringer. That schematic is **not** used. Copies of the three traces are in
   `docs/schematics/sansamp_bddi/` (git-ignored).
3. **Best source.** The tracer's KiCad drawing of the version chosen, read against the
   board photographs where a value matters.
4. **Cross-check.**
   - **Measured responses of the real units** on the same pages: the drive stage at
     several settings, Presence, Bass and Treble at both ends, the V2's Mid at both
     shifts and both ends -- curves to compare the model against, as the Braunbuch's
     figures were for the V76.
   - Tech 21's owner's manual (v2): Bass, Mid and Treble "cut or boost +-12dB from unity
     gain (12 o'clock)"; Mid shift 500 / 1000 Hz (the tracer measured 400-450 / 800-900);
     Bass shift 80 / 40 Hz; input 1 M; Blend at minimum bypasses the emulation while "the
     Mid, Bass, Treble and Level controls remain active".
5. **Signal path and values** (V2, from the trace).

```text
input C24 47 nF, R45 10 k, D5 / D6 to the rails, R2 1 M -- U1B buffer
   (J2 PARALLEL OUTPUT: wired to the input, untouched)
emulation: R5 100 k, C5-C8 22 nF, R6 2.2 k, R7 22 k (a passive twin-T notch)
   -- U1A with PRESENCE (VR58 100 k B, R10 3.3 k, C34 10 nF, C10 100 pF)
   -- R12 10 k, C22 1 uF, R14 22 k -- DRIVE VR13 100 k B -- U901A (CH40), Q2 MMBF4393
   switching it -- U901B with D901 BZB984-C3V3 (a zener pair) and R903 220 k across it
   -- R47 33 k, C42 10 nF, R16 22 k, R18 33 k, C16 470 pF, C14 47 nF -- U2B (Sallen-Key)
   -- C41, R19 / R20 33 k, C17 2.2 nF, C18 1 nF -- U2A -- C2 1 uF
BLEND VR50 100 k B between the dry buffer and the emulation
LEVEL VR51 100 k B -- U3B -- MID (U3A, VR1 100 k B, C19 / C21 10 nF, SW5 shift)
   -- BASS / TREBLE (U6B Baxandall: VR48 / VR57 100 k B, C27 100 nF, C20 47 nF,
   C28 10 nF with SW4 shift, C29 1 nF, C30 4.7 nF, C31 22 nF)
   -- Q4 switching -- U6A 1/4" output (SW1 +10 dB) and U7A / U7B balanced XLR (SW2 -20 dB)
```

6. **To be modelled exactly.** The emulation path from the buffer to the Blend, the Blend,
   Level, the EQ with both shifts, the 1/4" output stage; the switching JFETs held in
   the effect-on state as fixed resistances.
7. **To be approximated, and why.**
   - **The op-amps** (TLC2262 / TLC2264) as the solver's op-amp on the 9 V supply and its
     4.5 V bias -- the failure mode the project notes for followers on a bias applies,
     so the sections may need ground-referencing as `rodent` does.
   - **The zener pair** as two Shockley diodes with a breakdown -- or as the diode model
     with the part's 3.3 V knee -- to be decided against the measured drive curves.
   - **The footswitch logic** (HEF4013) is not built; the effect is on.
   - **The balanced output** is the same signal; not built separately.
8. **Why.** Three careful traces of real units, with photographs and measurements, by one
   tracer who also separated the clone from the original.

## As a DI

The Bass Driver *is* a DI: its XLR carries the processed signal to the desk while the
parallel jack carries the untouched bass to an amplifier, and its own Blend mixes dry into
the processed path before the EQ. In this plugin's terms its output is the DI tap after the
pedal. See `docs/DI.md`.
