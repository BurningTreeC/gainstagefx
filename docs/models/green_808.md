# Green 808 (Ibanez TS808 Tube Screamer) research log

Engineering identity: **Ibanez TS-808 Tube Screamer** (Nisshin Onpa/Maxon, late 1970s),
JRC4558D. Stable id: `ts808` (existing `circuit` value) and `pedal_ts808` (pedal slot).
Display: Green 808.

## Checkpoint (2026-09-15, before routing into the independent pedal slot)

1. **Revision.** TS-808 with JRC4558D, 1N914/MA150 clipping pair, 2SC1815 buffers.
   Not TS9 (different output network) and not the TS808 reissue (op-amp variants).
2. **Original manufacturer schematic?** Not obtained as a manufacturer document.
   Located but not retrieved: a service-manual listing on elektrotanya.com
   ("IBANEZ TS808 ORIGINAL SCH").
3. **Best available** (three independent secondary sources).
   - [Steve Cerutti's TS-808 drawing](https://schematicheaven.net/effects/ggg_ibanez_ts-808_ORIGINAL.pdf)
     (a traced redraw, "for reference only").
   - The ElectroSmash Tube Screamer analysis drawing (project-local copy
     `docs/schematics/ts808.png`; the site was unreachable at DNS level during this session).
   - [R. G. Keen, "The Technology of the Tube Screamer" (1998)](http://www.geofex.com/article_folders/tstech/tsxtech.htm):
     describes the tone shunt as 220 ohm with 0.22 uF ("less than 220 ohms ... about 3.2 kHz"),
     the level control as "a 100K audio control", and the 808/9 output-resistor table.
4. **Cross-check.** The two drawings are independent and agree with each other on every
   value compared below.
5. **Values vs the existing netlist (`src/circuits/ts808.rs`).**

| Part | Cerutti | ElectroSmash | Code | Status |
|---|---|---|---|---|
| Input cap C1 | 0.02 uF | 0.02 uF | 22 nF | small conflict |
| R1 / R2 / emitter | 1 k / 510 k / 10 k | 1 k / 510 k / 10 k | same | agree |
| C2 / bias R | 1 uF BP / 10 k | 1 uF NP / 10 k | same | agree |
| R4 / C3 | 4.7 k / .047 uF to Vr | 4k7 / 0.047 | 4.7 k / 47 nF to gnd | AC-equivalent (Vr bypassed by 47 uF) |
| R6 / Drive pot | 51 k / 500 k A | 51 k / 500 k log | same (reverse-log law) | agree |
| C4 | 51 pF | 51 pF | 51 pF | agree |
| Tone series R7 / C5 / bias | 1 k / .22 uF / 10 k | 1 k / 0.22 uF / 10 k | same | agree |
| **Tone shunt (wiper -> C6 -> R)** | .22 uF + **220 ohm** | 0.22 + **220** | 220 nF + **1 k** | **CONFLICT** |
| U1B feedback | 1 k | 1 k | 1 k (code calls it R9) | agree |
| Tone pot | 20 k G | 20 k | 20 k linear | agree (taper letter G unresolved) |
| **Level feed R** | **1 k** | **1 k** | **220 ohm** | **CONFLICT** |
| **Level pot** | **B100K** (linear) | 100 k | 100 k **audio** | Keen: "100K audio"; code keeps audio, the Cerutti "B" is recorded |
| Output buffer | .1 uF, 510 k, 2SC1815, 10 k, 100 ohm, 10 uF, 10 k | same (RB 100, RC 10 k) | same | agree |

   The code's own comment records that R8 and R11 were once 220 ohm and 1 k and were
   "corrected" to 1 k and 220 ohm, citing `TS808.md` section 12. That file is not in this
   repository. Both drawings support the *original* values. The code's "R9 unresolved (10 k
   vs 1 k)" note is a designator mix-up: the 10 k is the U1B bias resistor, which the
   code already has as 10 k.
6. **Modeled from the drawings.** The complete signal path (input buffer, clipping amp,
   tone/level, output buffer) at node level.
7. **Approximated.**
   - The JFET bypass switching is not modeled.
   - BC549 stands in for 2SC1815.
   - The op-amp is ideal with a rail.
   - The three conflicts above remain in the code.
8. **Resolution (2026-09-15).**
   - `ts808::LEGACY` keeps the netlist every existing `circuit = ts808` session and preset
     uses, bit for bit.
   - `ts808::TS808` carries the three-source values (C1 20 nF, tone shunt 220 ohm, level
     feed 1 k, audio Level). It is used by the **independent pedal slot's Green 808**
     (`pedal_ts808`), which is new, so no stored state depends on it.
   - Tested: `tests/pedal_slot.rs` (the verified tone control reaches more than 3 dB further
     at 6 kHz than the legacy one).

   **Why the catalogue voice is not corrected now.** Correcting the tone shunt, level feed and level taper
   changes the sound of every existing session and preset that uses Green 808.
   Preservation of sessions ranks above accuracy in the project priorities.
   **Proposed:** a versioned correction (a new stable id such as `ts808_rev2`, or a state
   version flag) with its own regression capture, subject to the owner's decision.

## TS9 relationship (for Green 9)

R. G. Keen's "The Technology of the Tube Screamer" (geofex.com) and several builder
comparisons state that the TS808 and TS9 differ only in the op-amp and the two output
resistors: TS808 100 ohm series / 10 k shunt, TS9 470 ohm / 100 k. Green 9 needs its own
checkpoint before implementation.

## Tests

Existing: `tests/ts808.rs`, `tests/tone_knobs.rs`, `tests/presets.rs`. New routing:
`tests/pedal_slot.rs` (added with the pedal slot).
