# Modern 33 (Fortin 33) research log

Engineering identity: the **Fortin 33**, Fredrik Thordendal's boost: one BC550C and one
TL071 as a single composite non-inverting stage on a charge-pumped ~33 V rail, its gain
shaped by the network on the transistor's emitter, into a 5 k level pot. Stable id
`pedal_fortin_33` (proposed in the roadmap). Display **Modern 33**.

Status: **IMPLEMENTED 2026-10-02** (`circuits/modern_33.rs`, pedal and circuit), on a
trace of a genuine unit. Results below.

## Eight-question checkpoint

1. **Revision.** One: the 33 as sold (2017 on). Fortin made one circuit; the "Grind" is a
   different pedal. Its circuit is, by the tracer's and a second analyst's account, the
   **TC Electronic Integrated Preamp** (late 1970s, 1980s) copied nearly verbatim, minus
   that pedal's tone stack and with a few resistor values tweaked (PedalPCB forum,
   "Triangulum - schematic error?", Chuck D. Bones and Robert of PedalPCB).
2. **Original schematic found?** **No factory drawing of the 33; a trace of a genuine
   unit, yes** -- PedalPCB's "Triangulum" (document revision 03.02.20). Its parts list says
   "R9, R10, Q2, D1, and D2 are present in the original but serve no functional purpose",
   and when a builder doubted Q2's wiring, PedalPCB answered that the schematic "is
   definitely 'correct' with respect to the original", Q2 having been checked on a unit.
   Odd values were reproduced with series pairs (2K62 = 820R + 1K8, 16K7 = 4K7 + 12K)
   "rather than round up (or down) to the nearest E12 value" -- a tracer copying what is
   on the board. The standing of the Cry Baby and the Bass Driver here: a trace of the
   real thing, not of a clone kit (where the Darkglass B3K stopped).
3. **Best source.** PedalPCB's Triangulum schematic and parts list. PCB Guitar Mania's
   "Bunker 33" is the same circuit (not independent evidence: its values could be copied).
4. **Cross-check.** TC Electronic's own factory schematics of the Integrated Preamp exist
   (forum accounts: LM741 in early units, TL071 and LF356 listed as alternatives, R11
   820 k in one early drawing and 1 M in others) -- **not yet obtained**; they are the
   check on the shared front end. The published DC analysis of the Integrated Preamp
   (D. Saulheimer, 2021) explains the bias divider sitting near the top rail (10 k /
   100 k / 1 M there, 10 k / 100 k / 820 k in the 33) as intentional, for headroom: the
   33's reference sits at 0.88 of its rail. Fortin's claim, "+22 dB of clean gain",
   is the figure to measure the model against. Builders report the rail at about 33 V.
5. **Signal path and values** (Triangulum):

```text
IN -- R8 4K7 -- C10 47n -- Q1 base (BC550C), R5 1M to VREF
   [Q2 BC550C, R9 22K, R10 1M, D1/D2 1N4148: a mute circuit copied from the TC
    pedal with its jack connection left out; does nothing]
Q1 collector -- R3 100K to VCC, and to IC1 (TL071) inverting input; C8 100p out to (-)
IC1 (+) at VREF. IC1 out -- R7 220K -- Q1 emitter (the loop's DC and AC feedback)
Q1 emitter -- C12 1u -- node E, which returns through three legs:
   R18 820R + R12 1K8 -- C14 100n -- R13 220R -- ground
   R17 3K9 -- node B: C16 470n to ground, R16 20K + R6 1K to ground
   R11 4K7 + R4 12K -- C13 10n -- node O (the output)
IC1 out -- C11 4u7 -- node O: LEVEL A5K to ground, wiper to OUT;
   R14 2K2 -- node A -- R15 47R + R2 120R -- node B, C15 47n across R15 + R2
Supply: 9 V -- D100 1N5817 -- TC1044 (IC100) driving a five-stage diode-capacitor
   multiplier (D101-D106 1N4001, C102-C106 10u), C107 100u: VCC ~33 V, R1 100K load.
   VREF = VCC x 820K / (10K + 100K + 820K), C9 4u7 at the 10K/100K junction.
```

   The gain is 1 + 220 k / Z(E): at low frequencies C12, C14 and C16 open and the gain
   falls toward unity (the "tight" low cut); in the mids the 2.6 k + C14 leg and the 3K9
   path set it; node O's return through C13 feeds the output back into E above some
   kilohertz. The whole is solved, so no reduction needs deriving.
6. **To be modelled exactly.** The stage, transistor and op-amp, every network part, the
   level pot, the rail and reference as drawn; the inert mute circuit left out (it is
   inert by the tracer's own note, and with its jack contact missing it cannot switch).
7. **To be approximated, and why.**
   - **The charge pump**: an ideal ~33 V rail (ESTIMATED from builders' reports). Its
     switching ripple and the multiplier's output impedance are not in the audio path
     that matters; the rail's sag under the stage's few milliamps is negligible.
   - **TL071**: the solver's transconductor op-amp with the data sheet's GBW and slew.
   - **BC550C**: Ebers-Moll with the C group's hFE (ESTIMATED within the sheet).
8. **Why.** Asked for by the owner (2026-09-16) for the modern high-gain chain: the boost
   ahead of an already-distorted amplifier.

## Results (`tests/modern_33.rs`, `examples/modern_33_op.rs`)

- **+22.3 dB at 1 kHz** from the trace's values, against Fortin's "+22 dB of clean gain"
  -- nothing fitted. +2.8 dB at 82 Hz (the low E), +4.1 at 110, +13.2 at 330, +21.7 at
  750, +20.4 at 5 kHz: the boost is the middle of the band, the low strings held back
  some 20 dB below it, which is the "tight" in front of a distorting amplifier.
- **Bias** as the TC analysis explains it: VREF 29.09 V (0.88 of the rail), the collector
  held there, the op-amp's output at 19.8 V -- near the middle of its swing.
- **Clean**: third harmonic 0.003 % at 0.5 V in (a guitar is 0.12 V); 0.9 % at 1 V, 16 %
  at 2 V, where the 33 V rail finally runs out.
- **LEVEL_REST 0.683**: unity through the pedal for a guitar's 110 + 330 Hz at its level
  (full LEVEL is +7.4 dB on that bass-heavy mix). The slot's drive knob is greyed.
- Same response at 44.1 to 192 kHz; realtime-safe in the slot. 4.00 Newton passes a
  sample, 7.1 % of a channel's budget alone on a loaded machine (the Green 808 8.0).
- As a circuit its one knob is the Drive knob, as the Treble Boost's is.
