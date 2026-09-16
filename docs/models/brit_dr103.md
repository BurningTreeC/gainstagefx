# Brit DR103 (Hiwatt Custom 100 DR103) research log

Engineering identity: **Hiwatt DR103 Custom 100**, the brilliant channel through its
master volume, into four EL34s with 22 k grid stoppers and a tight feedback loop.
- Stable ids: `amp_hiwatt_dr103` (`circuit`), `power_dr103_el34` (`power_amp`).
- Display: Brit DR103 / DR103 EL34.

## Eight-question checkpoint (2026-09-16, before code)

1. **Revision.** The two-input DR103 as Hiwatt's own service drawings have it:
   "HIWATT DR103 PREAMPLIFIER CIRCUIT DIAGRAM, Drawn 19/05/95, Issue 4, S/No. AB0309
   Onwards", with the matching output-stage (25/04/94, Issue 1) and power-supply
   (22/11/94, Issue 3) sheets. Not the late-60s four-input amplifier (see 4), not the
   50 W DR504, not the Reeves/Hylight reissues.
2. **Original schematic found?** Yes: the three factory sheets above, in Hiwatt's own
   manual ([schematicheaven copy](https://schematicheaven.net/newamps/hiwatt_100w_dr103.pdf)).
3. **Best source.** Those sheets.
4. **Cross-check and conflicts.**
   - [Circuit Codex's DR103 page](https://circuitcodex.com/amps/dr103/) documents the
     **late-60s four-input** amplifier, and says so: "a published REDRAW taken off a
     specific amplifier, not a factory document" (Mark Huss, preamp from DR103 S/N 903).
     Differences it records against the factory sheets: four inputs rather than two;
     "V2 ... a 220 kohm-plate / 2.2 kohm-cathode gain stage direct-coupled to a cathode
     follower on a 120 kohm 2 W load" (Issue 4: 100 k and 1 k, 100 k follower load); a
     470 k log master (Issue 4: 220 k lin); **an ECC81 phase inverter** where the factory
     sheet labels every preamp valve ECC83.
   - [Ampbooks' Hiwatt phase inverter page](https://www.ampbooks.com/mobile/classic-circuits/hiwatt-phase-inverter/):
     "The 10k ohm feedback resistor is effectively placed in parallel with the 2.2k ohm
     tail resistor ... in series with the 22k ohm cathode resistor"; the feedback also
     reaches "the right side PI grid" through the 100 n capacitor, and the presence
     control sends part of the high-frequency feedback to **the cathode follower's grid
     before the inverter**, which is positive feedback.
   - The Great Wall '79 preset's amplifier is a 1970s DR103, which is between these two
     revisions. That is recorded on the preset, not modelled.
5. **Signal path and values (Issue 4 / Issue 1).**

```text
BRILLIANT input -- 220k plate V1b, 1k5 cathode shared with V1a, bypassed by 100 uF
  plate -- 1000pF -- BRILLIANT VOLUME 470k log        (NORMAL: 22 nF into its own 470k log)
  both wipers mix through 470k each into V2a
V2a: 100k plate (140 V on the sheet), 1k cathode
V2b: cathode follower, direct-coupled, 100k cathode load, 47 nF across its own 1k
tone stack from the follower: 1000pF and 220pF to TREBLE 220k lin, 220k and 22k around it,
  100k slope, 47 nF to BASS 470k log, 47 nF to MIDDLE 100k lin, 1000pF to ground
  treble wiper -- 22k -- MASTER VOLUME 220k lin
V3a: 100k plate, 1k5 cathode        V3b: cathode follower, 220k cathode load
  follower -- 47 nF -- phase inverter
V4 long-tailed pair: 82k and 91k plates; grid A returns through 100k + 1M, grid B through
  1M, both to the junction of the 22k cathode resistor and the 2k2 tail; 100 nF from that
  junction to grid B; 47 nF from each plate to the output valves
output: 22k grid stoppers (the Hiwatt signature), 100k grid leaks to a -38 V fixed bias,
  100 ohm screen stoppers, four EL34, Hiwatt TH7549/2 transformer, 4/8/16 ohm taps
feedback: 10k from the 16 ohm tap to the 22k/2k2 junction
presence: 100k lin with 100 nF and 470R/10nF, into the driver's grid (positive feedback)
supply: silicon bridge, 2 x 220 uF 350 V, standby to HT1; 100R 5W to HT2; 470R 10W to the
  screens; 1k to HT3; 47k and 22k to the preamp rails; bias -38 V from its own winding
```

6. **To be modeled exactly.** The brilliant channel from the jack to the master wiper and
   on through V3a and its follower; the whole tone stack; and in the power stage the
   inverter with its 22k/2k2 tail and 1.1M/1M grid returns, the 47 nF couplings, the 22k
   stoppers, the 100 ohm screen stoppers, the four EL34s on fixed bias, and the 10k
   feedback loop.
7. **Approximated and estimated.**
   - **The presence control is not modelled.** It works by feeding high-frequency
     feedback from the output transformer back into the *preamplifier's* cathode
     follower, and the preamplifier and power stage are separate netlists here. Leaving
     it out is a documented omission, not a substitution.
   - Supplies: the sheets give one preamp voltage (V2a's plate at 140 V) and no others.
     HT1 480 V ESTIMATED (Circuit Codex reads the sheet the same way), and the droppers
     are the sheet's.
   - The normal channel is not built; its volume is at zero and its 470 k mixer loads the
     brilliant channel, which is built.
   - Output transformer: 1.7 k plate to plate, APPROXIMATED from the Marshall-class data;
     no Hiwatt winding data was found. The 16 ohm tap's 10 k feedback is built as 7.07 k
     from the 8 ohm tap, which passes the same current.
   - Phase inverter valve: ECC83 per the factory sheet (an ECC81 on the late-60s amp).
8. **Why.** The presence loop crosses the model's preamp/power boundary; no transformer
   data; the inverter's exact grid-return wiring was read from Ampbooks' description of
   this amplifier rather than from the scan, which is not legible at that point.

## Measured (`tests/dr103.rs`, `examples/dr103_op.rs`)

- V2a's plate lands at 140.2 V against the sheet's 140 V, which is what the preamp rail
  (300 V) was set from.
- The driver's cathode follower sits at 62.7 V; Ampbooks reads about 73 V on a real one.
  That voltage is handed to the power stage as `PowerSpec::driver_volts`, because the two
  are separate netlists and the coupling between them is a direct one.
- The inverter then idles at: grids 62.1 V, cathodes 64.2 V, **bias -2.15 V** (published:
  -2.1 V), **286 V plate to cathode** (published: 285 V), **1.40 mA** a triode (published:
  1.56 mA).
- Small signal, Volume at half: 45.2 dB, against the Brit 800's 49.4 dB at the same
  position -- this preamplifier is the quieter of the two, as it should be.
- The stack: treble 31 dB of range at 4 kHz, middle 21 dB at 600 Hz, and the bass control
  only 4.6 dB at 40 Hz. The 100 k middle track dominates a 470 k bass track; that is the
  drawing's own arrangement, not a fault in the model. The late-60s amplifier's 22 k
  middle would behave differently.
- At the calibration point (Volume and Master at half, a guitar's level) the whole
  amplifier makes 50 % distortion: with both controls at half this is a very loud
  amplifier, and its power stage is doing the distorting. Its presets sit well below that.

## Solver work this needed

- `Netlist::input_at`: an input that carries a direct voltage as well as a signal, so a
  block that is direct-coupled to the one in front of it can be handed that voltage.
- One part that is not on the drawing: a 1 uF into 1 M at the preamplifier's output, so
  what leaves the block is the signal about zero like every other block's, with the
  driver's 63 V handed on as `driver_volts` instead. The corner is 0.16 Hz.
- `PowerSpec::pi_couple = 0` (direct coupling) and `driver_volts`, with the grid returns
  becoming the two resistors from the driver rather than a leak chain.
