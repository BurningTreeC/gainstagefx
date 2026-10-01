# Bass Driver (Tech 21 SansAmp Bass Driver DI) research log

Engineering identity: **Tech 21 SansAmp Bass Driver DI** (1994 on), an all-analogue "tube
amplifier emulation" preamplifier and DI in a pedal: a drive stage clipping on a zener pair,
fixed filters, a Blend with the dry signal, an active EQ and both a 1/4" and a balanced XLR
output. Proposed stable id `pedal_sansamp_bass_driver` (`pedal`), as the roadmap reserves it,
and a `_circuit` id if it is offered as a circuit too. Proposed display: **Bass Driver**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::bass_driver`, V2), as a pedal and as a
circuit, on independent traces of real units.
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
   - **The tracer's own LTspice simulation of each trace**, plotted on the same pages:
     the drive stages at five settings, Presence, the emulation path, Bass at both
     shifts, Treble, the V2's Mid at both shifts. These are *not* measurements of the
     units -- this log said so at first, wrongly -- so they check this netlist's reading
     of the drawing against an independent simulation of the same drawing, not the
     hardware.
   - Tech 21's owner's manual (v2): Bass, Mid and Treble "cut or boost +-12dB from unity
     gain (12 o'clock)"; Mid shift 500 / 1000 Hz (the tracer's figures 400-450 / 800-900);
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
   -- R47 10 k, C42 10 nF, R16 22 k, R17 10 k / C14 47 nF, R18 33 k, C15 10 nF,
   C16 470 pF -- U2B (Sallen-Key)
   -- C41, R19 / R20 33 k, C17 2.2 nF, C18 1 nF -- U2A -- C2 1 uF
BLEND VR50 100 k B between the dry buffer and the emulation
LEVEL VR51 100 k B -- U3B -- MID (U3A, VR1 100 k B, C19 / C21 10 nF, SW5 shift)
   -- BASS / TREBLE (U6B Baxandall: VR48 / VR57 100 k B, C27 100 nF, C20 47 nF,
   C28 10 nF with SW4 shift, C29 1 nF, C30 4.7 nF, C31 22 nF)
   -- Q4 switching -- U6A 1/4" output (SW1 +10 dB) and U7A / U7B balanced XLR (SW2 -20 dB)
```

6. **Modelled exactly.** The buffer, the notch, U1A with Presence, both drive stages on
   their shared track, the zener pair, both Sallen-Key low-passes, Blend, Level, U3B, Mid
   with its shift, Bass and Treble with the bass shift, U6A and the output network.
7. **Approximated, and why.**
   - **Built about the 4.5 V bias** as its ground, on +-4.5 V rails: every stage is
     capacitor-coupled and every op-amp sits at the bias, so this is the same circuit,
     and it is the project's answer to followers on a bias (failure mode 4).
   - **The op-amps** (TLC2262) as the solver's ideal op-amp to a rail, rail to rail on a
     fresh battery (ESTIMATED; on the adapter, behind D12, about 0.3 V less); the buffer
     and the two Sallen-Key followers, which never reach a rail, as linear op-amps.
     Their 0.8 MHz and 0.55 V/us are not modelled.
   - **The zener pair** (BZB984-C3V3, anode to anode) as each zener's forward junction
     and its breakdown behind 50 ohm of bulk resistance, the way a SPICE zener has it:
     3.3 V at 5 mA and 65 ohm there against Nexperia's 3.1-3.5 V and at most 95 ohm.
     APPROXIMATED. The first fit, a single soft exponential through the data sheet's
     5 uA at 1 V, leaked a 2.5 M shunt across R903 at rest and took 0.75 dB off the
     drive stages; that figure is a maximum, and the tracer's simulation has the
     stages' arithmetic exactly.
   - **The switching** (HEF4013, Q1, Q2, Q4) held in the effect-on state: Q4 as 100 ohm,
     Q1 and Q2 open. Input clamps D5 / D6 left out: they conduct only past the rails.
   - **The 1/4" output switch** at -10 dB, the instrument level the manual gives for a
     stomp box: U6A a follower. **The balanced output** is not built.
   - **In the pedal slot** the two shift switches stay where the circuit is built,
     80 Hz and 500 Hz; as a circuit they are the panel's low and mid switches.
8. **Why.** Three careful traces of real units, with photographs, by one tracer who also
   separated the clone from the original; and that tracer's own simulation to check the
   transcription against.

## What the model gives

`cargo run --release --example bass_driver_op`; `tests/bass_driver.rs`. Small-signal,
between taps, against the tracer's LTspice of the same trace:

| Stage | Tracer's simulation | This model |
|---|---|---|
| Drive stages, five settings | 12.9, 21.5, 28.5, 35.5, 48.5 dB | 12.8, 21.6, 28.4, 35.6, 48.5 |
| Presence, top, 8 kHz | +27.5 dB | +27.7 |
| Emulation: 85 Hz peak, 700 Hz notch, 2.8 kHz peak | -3 / -35 / -10 dB | -3.2 / -35.6 / -9.2 |
| Mid, 500 shift at 450 Hz; 1000 shift at 900 Hz | +-18 dB | +18.2 / -18.6; +17.6 / -18.3 |
| Bass, 40 shift near 45 Hz; 80 shift near 90 Hz | +14 / -17; +11 / -13 dB | +13.9 / -16.7; +11.1 / -13.2 |
| Treble, top, 3 kHz | +17 dB | +16.7 |

Level rests at **0.1518** for unity through the pedal: a guitar chord in, every other
control at noon -- Blend too, the project's rule for a pedal's rest and where the slot's
knobs start. Blend at noon passes half the emulation to Level, so Blend up is about 6 dB
hotter (the first fit, at Blend up, read 6.5 dB low in front of Crunch in the slot's
level-match test). Level at noon is loud regardless: U3B adds 10 dB after it.
Calibration at full drive: 21 % distortion from a guitar's level.

**In the panel**: as a pedal, the slot's tone row is presence, bass, mid, treble, blend
(the slot grew to five tone knobs for it); as a circuit, Bass, Mid and Treble are the
stack's knobs, Presence the panel's, **Blend** the circuit's own control beyond the stack
(the knob the Metal Zone's Mid Freq uses), and the shifts the low and mid switches.

## As a DI

The Bass Driver *is* a DI: its XLR carries the processed signal to the desk while the
parallel jack carries the untouched bass to an amplifier, and its own Blend mixes dry into
the processed path before the EQ. In this plugin's terms its output is the DI tap after the
pedal. See `docs/DI.md`.
