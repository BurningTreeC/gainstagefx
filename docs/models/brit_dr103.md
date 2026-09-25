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
V3a: 100k plate from H.T. SUPPLY 3, 1k5 cathode (unbypassed); its plate -- 47 nF -- V4a grid
V3b: NO SIGNAL. Plate on H.T. SUPPLY 3; grid on 1M from H.T. SUPPLY 3 and 220k to ground;
  cathode on 220k. Its cathode holds both inverter grids: V4a through 100k, V4b through 1M
V4 long-tailed pair: 82k and 91k plates; cathodes through 22k to the tail, 2k2 to ground;
  100 nF from the tail to grid B; 47 nF from each plate to the output valves
output: 22k grid stoppers (the Hiwatt signature), 100k grid leaks to a -38 V fixed bias,
  100 ohm screen stoppers, four EL34, Hiwatt TH7549/2 transformer, 4/8/16 ohm taps
feedback: 10k from the 16 ohm tap to the 22k/2k2 junction, with 470R and 10nF in series
  from that junction to ground
presence: 100k lin; one end 100 nF to the feedback junction, the other 1000 pF to V3a's
  plate, the wiper 470R to ground
supply: silicon bridge, 2 x 220 uF 350 V, standby to HT1; 100R 5W to HT2; 470R 10W to the
  screens; 1k to HT3; 47k and 22k to the preamp rails; bias -38 V from its own winding
```

6. **To be modeled exactly.** The brilliant channel from the jack to the master wiper and
   on through V3a and its follower; the whole tone stack; and in the power stage the
   inverter with its 22k/2k2 tail and 1.1M/1M grid returns, the 47 nF couplings, the 22k
   stoppers, the 100 ohm screen stoppers, the four EL34s on fixed bias, and the 10k
   feedback loop.
7. **Approximated and estimated.**
   - **The presence control** (built 2026-09-25): a loop from the feedback junction back
     to V3a's plate. To keep a loop inside one netlist, V3a and V3b are built in the power
     stage (`power::DriverSpec::HIWATT_DR103`) and the preamplifier ends at the master's
     wiper, which V3a's grid takes without drawing current. Which end of the track is
     clockwise is not on the drawing: up is the bright end. ESTIMATED.
   - Supplies: the sheets give one preamp voltage (V2a's plate at 140 V) and no others.
     HT1 480 V ESTIMATED (Circuit Codex reads the sheet the same way), and the droppers
     are the sheet's.
   - The normal channel is not built; its volume is at zero and its 470 k mixer loads the
     brilliant channel, which is built.
   - Output transformer: 1.7 k plate to plate, APPROXIMATED from the Marshall-class data;
     no Hiwatt winding data was found. The 16 ohm tap's 10 k feedback is built as 7.07 k
     from the 8 ohm tap, which passes the same current.
   - Phase inverter valve: ECC83 per the factory sheet (an ECC81 on the late-60s amp).
8. **Why.** No transformer data; the supply voltages are estimates (see below).

## The driver was wired wrong until 2026-09-25

The first build read the inverter's grid returns from Ampbooks' description rather than
from the scan, "which is not legible at that point". Rendered at 300 dpi from the same PDF
(`docs/schematics/hiwatt_100w_dr103.pdf`, page 4) it is legible, and it is not what was
built:

| | Issue 4, at 300 dpi | built 2026-09-16 |
|---|---|---|
| signal into the inverter | V3a's plate through **47 nF** | V3b's cathode, direct |
| V3b's grid | **1 M from H.T. SUPPLY 3, 220 k to ground** -- a fixed divider | 1.8 M with 22 nF across it from V3a's plate, 1 M to ground |
| V4a's grid return | **100 k** to V3b's cathode | 1.1 M |
| feedback junction | 2k2, plus **470 R + 10 nF** to ground | 2k2 |
| presence | **built** | omitted |

The "1.8 M with 22 nF" is on neither of Hiwatt's sheets, and Ampbooks may be describing
another revision. So V3b is a DC reference and not a signal stage: it holds the grids a
fixed fifth of the rail up, and the signal couples in through a capacitor from V3a. The
"grid a follower holds cannot drift" description survives -- the follower does hold the
grids -- but the grid is capacitor-coupled to its signal like everyone else's; what the
Hiwatt has that the others do not is a stiff, low-impedance *return* for it.

## The rail conflict

H.T. SUPPLY 3 feeds V3a, V3b and the inverter directly and V1/V2 through two 10 k
droppers. Two readings of it disagree:

- The **supply sheet**: 480 V reservoir, 100 R to H.T. SUPPLY 2, 1 k to H.T. SUPPLY 3 --
  about **465 V** under the preamp's ten-odd milliamps. The power stage uses this
  (`pi_supply`).
- The **preamp sheet's one voltage**, V2a's plate at 140 V, which needs the V1/V2 rail
  near **300 V** (`dr103::RAIL`) with the drawing's 100 k plate and 1 k cathode.

With 465 V, V3b's divider puts the inverter's grids at **88 V** and its bias at -1.2 V;
Ampbooks reads a real one at 73 V and -2.1 V, which the same divider gives at about 380 V.
Neither figure is measured on an Issue 4 amplifier; the model keeps the supply sheet's
and records this.

## Measured since the driver moved (2026-09-25; `tests/dr103.rs`, `examples/dr103_op.rs`)

- V2a's plate 145.8 V (sheet: 140 V).
- V3a 1.55 mA, plate 310 V; V3b's grid 83.9 V (the divider's 83.8), cathode 87.9 V; the
  inverter's grids 87.9 V, bias -1.17 V, 216 V plate to cathode.
- Presence, the power stage alone, 4 kHz against 200 Hz: -2.0 dB turned down, -2.7 dB at
  half, **+7.4 dB turned up**; 8 kHz is taken away turned down.
- The chain plays at all five rates with no unsettled solve and no allocation.
- Behind any other preamplifier the stage is `DR103_EL34_RETURN`: the input stands at
  V3a's plate through V3a's output impedance, 68 k (DERIVED), and V3a is not built.
- At the extreme -- every control up, 12 dB over nominal, 7 kHz at 48 kHz -- the whole
  amplifier leaves 33 of 9,600 samples unsettled against 29 before the change, now in the
  preamplifier rather than the power stage (`examples/dr103_limits.rs`).

## Measured before the driver moved (2026-09-16; superseded)

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
