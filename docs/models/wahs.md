# Black Wah and Chrome Wah (Dunlop Cry Baby GCB-95, Vox V847) research log

Engineering identity: the **Dunlop Cry Baby GCB-95** (1982 on; the buffered board from
rev F, mid-1991) and the **Vox V847** (the reissue of Thomas Organ's V846 of 1967): a
transistor common-emitter stage with an LC network in its feedback, tuned by the
treadle's 100 k pot, into an emitter follower. Proposed stable ids `wah_dunlop_gcb95` and
`wah_vox_v847` (a new `wah` parameter, see below); proposed display **Black Wah** and
**Chrome Wah**.

Status: **CHECKPOINT 2026-10-02 -- the Vox clears on Thomas Organ's own drawing once it
is in hand; the Cry Baby on traces, as there is no Dunlop drawing. Not built.** The
owner approved the control design (2026-10-02, "To all questions yes").

## Eight-question checkpoint

1. **Revision.**
   - Cry Baby: the GCB-95 from rev F (mid-1991), with Dunlop's input buffer (Q0, an
     MPSA13 follower) ahead of the Vox circuit; R4 390 ohm against the Vox's 510, R8 82 k
     against 100 k (ElectroSmash). The pre-1991 board is the Vox circuit unbuffered.
   - Vox: the V847, which is the V846 circuit with modern parts: two MPSA18s, no buffer.
2. **Original schematic found?**
   - **Vox: yes, to be obtained.** Thomas Organ's V846 schematic of 1967 is listed on
     elektrotanya ("VOX V846 WAH 1967 SCH", free with an account) -- the factory
     drawing of the circuit the V847 reissues. Not yet in `docs/schematics/`.
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

## Playing one: the controls (approved 2026-10-02)

The plugin has no pedal input, so the treadle is a **parameter**:

- **Treadle** (`wah_treadle`, 0 heel to 1 toe), smoothed (some 20 ms) so that a stepped
  automation lane or a MIDI controller's 128 steps do not zipper. Host-automatable:
  in REAPER an expression pedal's MIDI CC is linked to it with MIDI learn (or
  parameter modulation) and plays it live, or it is drawn as automation.
- **Mode** (`wah_mode`): **Manual** -- the treadle is where the parameter is; **Auto** --
  an envelope follower on the input moves it from the heel toward the parameter's
  position as the player picks harder, an auto-wah. Its attack and release (a few
  milliseconds, some hundred and fifty) are a choice, not hardware: ESTIMATED and
  labelled. A fixed, "cocked" wah is Manual with the treadle left where it is.
- **Sensitivity** (`wah_sense`), for Auto: how much input reaches full travel.
- **Where it sits**: a selector of its own, **Wah** (Off, Black Wah, Chrome Wah), ahead
  of the pedal slot -- guitar, wah, pedal, circuit -- so that a wah and a drive can be
  used together, as on every pedal board, rather than the wah taking the drive's
  place in the one slot. Appended parameters, `migrate()` giving Off and the middle.

Plugin-internal MIDI (a CC mapped without the host) can come later.
