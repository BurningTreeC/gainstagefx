# Black Wah and Chrome Wah (Dunlop Cry Baby GCB-95, Vox V847) research log

Engineering identity: the **Dunlop Cry Baby GCB-95** (1982 on; the buffered board from
rev F, mid-1991) and the **Vox V847** (the reissue of Thomas Organ's V846 of 1967): a
transistor common-emitter stage with an LC network in its feedback, tuned by the
treadle's 100 k pot, into an emitter follower. Proposed stable ids `wah_dunlop_gcb95` and
`wah_vox_v847` (a new `wah` parameter, see below); display **Black Wah** and
**Chrome Wah**.

Status: **IMPLEMENTED 2026-10-02** (`circuits/wah.rs`, the `wah` slot ahead of the
pedal), both from ElectroSmash's traces. The owner approved the control design
(2026-10-02, "To all questions yes"). **Owed:** Thomas Organ's 1967 V846 drawing, to
check the Vox against when it is in `docs/schematics/vox_v846/` (the folder is empty
as of this entry). ElectroSmash's V847 drawing was read part by part against the
netlist and agrees with it, so the drawing is a cross-check, not a missing source.

## Eight-question checkpoint

1. **Revision.**
   - Cry Baby: the GCB-95 from rev F (mid-1991), with Dunlop's input buffer (Q0, an
     MPSA13 follower) ahead of the Vox circuit; R4 390 ohm against the Vox's 510, R8 82 k
     against 100 k (ElectroSmash). The pre-1991 board is the Vox circuit unbuffered.
   - Vox: the V847, which is the V846 circuit with modern parts: two MPSA18s, no buffer.
2. **Original schematic found?**
   - **Vox: yes, to be obtained.** Thomas Organ's V846 schematic of 1967 is listed on
     elektrotanya ("VOX V846 WAH 1967 SCH", free with an account) -- the factory
     drawing of the circuit the V847 reissues. Not yet in `docs/schematics/`. Built
     meanwhile from ElectroSmash's trace of a V847, the Cry Baby's standing.
   - **Cry Baby: no.** Dunlop has never published one; the schematic that circulates
     carries designators that do not match the board, and a corrected trace from a real
     PCB is on TDPRI ("Crybaby GCB-95 schematic with correct component designators").
     ElectroSmash's analysis has the full parts list of a rev F board. Traces of
     genuine units, the Bass Driver's standing.
3. **Best source.** For both, ElectroSmash's analyses and parts lists (GitHub archive of
   the site, `mstratman/electrosmash-pdf-backups`); for the Vox, the 1967 drawing to
   check them against.
4. **Cross-check.** ElectroSmash's figures for both: a resonant peak at 750 Hz with the
   treadle in the middle, swept from 450 Hz to 1.6 kHz, up to 18 dB of lift; input
   impedance 69.5 k (Vox) and about 1 M (Cry Baby, 750 k with Rin3); the active
   filter's gain 22 k / 390 = 35 dB open, 19 dB with its feedback.
5. **Signal path and values** (ElectroSmash's parts lists):

```text
Cry Baby GCB-95 (rev F):
  buffer: Cin1 0.01 uF, Cin2 22 pF, Rin1 2.2 M / Rin2 1.8 M bias, Rin3 1 k collector,
          Rin4 10 k emitter, Q0 MPSA13
  active filter: C1 0.01 uF, R1 68 k in, Q1 MPSA18 common emitter, R3 22 k collector,
          R4 390 emitter, R6 470 k / R8 82 k bias through L1 500 mH and R2 1.5 k,
          R7 33 k across L1 (the Q), C3 4.7 uF, C2 0.01 uF from Q2's emitter back to the
          base network (the loop that tunes it)
  output: VR1 100 k (the treadle) from Q1's collector through C4 0.22 uF, Q2 MPSA18
          follower on its wiper, R5 470 k bias, R9 1 k collector, R10 10 k emitter,
          C5 0.22 uF out
  supply: 9 V, a 9.1 V zener, C6 220 uF, C7 0.1 uF
Vox V847: the same without the buffer; R1 68 k is the input; R4 510, R8 100 k.
```

6. **To be modelled exactly.** Both circuits, transistors and all, with the treadle as
   the pot's position; the output's independence of the treadle (ElectroSmash: the jack
   is taken where VR1 does not reach it).
7. **To be approximated, and why.**
   - **The inductor**: 500 mH, DC resistance some 15 to 150 ohm by type, and the Fasel
     cores' saturation -- the solver's saturating core with a figure from the inductor
     makers' published data, ESTIMATED.
   - **The treadle's pot**: a wah pot's taper (Dunlop's "Hot Potz", the Vox's original)
     is custom and unpublished; the travel of the treadle onto the pot's rotation is
     mechanical. A reverse-audio-like law fitted to the published sweep (450 Hz heel,
     750 Hz middle, 1.6 kHz toe), ESTIMATED.
   - **The true-bypass switch** under the toe is not built: off is the wah removed.
8. **Why.** Asked for by the owner, with the question of how to play one.

## What was built, and against what (2026-10-02)

Both circuits are one netlist with two builds (`wah::Build`): the Cry Baby with its
buffer (the MPSA13 built as the Darlington pair it is), R4 390 and R8 82 k; the Vox
without, R4 510 and R8 100 k. ElectroSmash's two drawings were read part by part:
the tank node is L1, R7, C3 and R8; the loop is L1's and R7's other ends, R2 and C2;
the output is C5's far side, VR1's top, which is why the treadle moves the peak and not
the level. `tests/wah.rs`, `examples/wah_op.rs`:

| | Cry Baby | Vox | ElectroSmash |
|---|---|---|---|
| peak, heel down | 450 Hz, +21.3 dB | 495 Hz, +19.1 dB | ~450 Hz |
| peak, treadle in the middle | 746 Hz, +19.6 dB | 809 Hz, +17.4 dB | ~750 Hz |
| peak, toe down | 1586 Hz, +18.8 dB | 1658 Hz, +16.7 dB | ~1.6 kHz |
| 100 k source against 1 k, at 3 kHz | -0.9 dB | **-7.7 dB** | Vox input 69.5 k: -7.7 dB |
| below the peak (80 to 100 Hz) | about -13 dB | about -13 dB | about -12 dB (their curves) |

The sweep holds within 1 % at 44.1, 88.2, 96 and 192 kHz against 48. ElectroSmash's
own response plots show the same shape: peaks of 18 to 22 dB, a guitar's fundamentals
some 12 dB down below them, the highs falling past the peak.

**Fitted, and what that means.** The treadle's law -- a logarithmic track of span 10,
travelled from 92 % of its rotation at the heel to 9.5 % at the toe -- was fitted so the
Cry Baby's three peaks land on ElectroSmash's three figures. That holds the fit; it
does not test it. The plain audio track put the middle at 1.1 kHz. The Vox takes the
same law (its own pot's taper is unpublished too), APPROXIMATED, and so lands a little
higher; ElectroSmash's Vox curves stop near 1.15 kHz at their highest step, but their
steps are arbitrary pot positions and say nothing about the treadle's travel.

**Approximated, beyond the treadle:** the inductor is linear, 500 mH with a 15 ohm
winding (ESTIMATED; a Fasel's saturation on a very hot input is not modelled -- the
saturating-core idea above was not needed for the published figures and is left for
when a measurement asks for it); the MPSA18 at hFE 800, the middle of its sheet, and
the MPSA13 as two of them (ESTIMATED); the 9 V supply ideal (the zener and the
filter capacitors set its DC, which is all the circuit sees); the guitar a 10 k
resistive source (a pickup's inductance against the Vox's 69.5 k input is the
famous interaction, and is not modelled -- the input's loading is).

**In the chain.** A wah is fed a guitar's level, like a pedal, and hands on at what
follows it. The treadle glides with a 20 ms one-pole at the oversampled rate, and the
pot's two halves are set with `Simulation::set_realtime_value` whenever it moves. Auto's
follower is 5 ms attack, 150 ms release on the input at the host rate. In Auto the
treadle rests where the knob is and the follower pushes it toward the toe; full travel,
with Sense in the middle, at a quarter of the nominal level (0.032), ten times either
way across the knob. Realtime-safe and allocation-free at every rate, the treadle swept
by the follower. Two presets use one: *Plexi, Cocked Wah* (the Black Wah at 60 %,
Manual) and *Chime, Auto Wah* (the Chrome Wah in Auto on the AC30, resting at 10 %), the
second trimmed +14.4 dB, the wah's loss below its peak passing straight through a
clean amplifier.

## Playing one: the controls (approved 2026-10-02)

The plugin has no pedal input, so the treadle is a **parameter**:

- **Treadle** (`wah_treadle`, 0 heel to 1 toe), smoothed (some 20 ms) so that a stepped
  automation lane or a MIDI controller's 128 steps do not zipper. Host-automatable:
  in REAPER an expression pedal's MIDI CC is linked to it with MIDI learn (or
  parameter modulation) and plays it live, or it is drawn as automation.
- **Mode** (`wah_mode`): **Manual** -- the treadle is where the parameter is; **Auto** --
  the treadle rests where the parameter is and an envelope follower on the input pushes
  it toward the toe as the player picks harder, an auto-wah (the parameter doing what
  an auto-wah's Manual knob does; see the correction below). Its attack and release (a few
  milliseconds, some hundred and fifty) are a choice, not hardware: ESTIMATED and
  labelled. A fixed, "cocked" wah is Manual with the treadle left where it is.
- **Sensitivity** (`wah_sense`), for Auto: how much input reaches full travel.
- **Where it sits**: a selector of its own, **Wah** (Off, Black Wah, Chrome Wah), ahead
  of the pedal slot -- guitar, wah, pedal, circuit -- so that a wah and a drive can be
  used together, as on every pedal board, rather than the wah taking the drive's
  place in the one slot. Appended parameters, `migrate()` giving Off and the middle.

Plugin-internal MIDI (a CC mapped without the host) can come later.

## Correction, 2026-10-02: Auto barely moved

Reported by the owner the same day: "The wah 'auto' mode seems not to work". Two faults,
both in the control design rather than the circuit:

- **Sense was calibrated on a sine.** Full travel at the middle of Sense was set at the
  follower's reading of a sine at the nominal -18 dBFS, 0.08. A guitar never reads that:
  the fixture take, whose peaks reach -18.9 dBFS, gives 0.031 at its 90th percentile
  and 0.039 at its loudest, so the treadle never passed half its travel. Full travel is
  now a quarter of the nominal level, 0.032 -- the take's attacks.
- **Auto opened from the heel only as far as the knob.** At the knob's default middle
  that halved what was left, 450 to 570 Hz in all; with the knob at the heel it did
  nothing. The knob is now where the treadle rests, and the follower pushes it toward
  the toe, as an auto-wah's Manual knob sets where its sweep starts.

On the take, at the default Sense with the knob at the heel, the treadle now spends a
tenth of the time below 0.01 and a tenth above 0.95, its median 0.43
(`auto_sweeps_the_whole_travel_on_a_guitar_at_the_nominal_level`). The test that passed
before drove a 0.2 sine, ten times a guitar's follower reading, which is how it missed
this. v0.42.0 shipped with the old behaviour; the *Chime, Auto Wah* preset's knob moved
from 0.85 (the old top of its sweep) to 0.1 (the new resting point), its level unchanged.
