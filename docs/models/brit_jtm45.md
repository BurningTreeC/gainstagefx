# Brit 45 (Marshall JTM45) research log

Engineering identity: the **Marshall JTM45** head of 1965 (model 1987, the lead head,
no tremolo): two ECC83 channels mixed into a gain stage and cathode follower, the
Marshall tone stack, an ECC83 long-tailed pair, two KT66s, a GZ34. The circuit Marshall
built on Fender's 5F6-A Bassman. Proposed stable ids `amp_marshall_jtm45` and
`power_jtm45_kt66`. Proposed display **Brit 45** and **Brit 45 KT66**.

Status: **IMPLEMENTED 2026-10-02** (`circuits/jtm45.rs`, `power::PowerSpec::JTM45_KT66`,
`PentodeSpec::KT66`), on Marshall's own drawings. Results below.

## Eight-question checkpoint

1. **Revision.** The 1965 head with **KT66s and a GZ34**, which is what the album rig
   accounts give for *Californication* ("a 1965 JTM45", `brit_200.md`). Earlier units
   ran 5881s; from 1966 the KT66 gave way to the EL34 and the GZ34 to silicon diodes --
   Marshall's own drawing says so in a note (below). The tremolo models (1961, 1962,
   1987/T) share the circuit and add an oscillator; the lead head has none.
2. **Original schematic found?** **Yes, two of Marshall's.**
   - The period drawing: "JTM 45 -- basic schematic for Marshall trem amps, types 1961,
     1962, 1987/T", hand-drawn, with a **valve voltage chart** "measured to chassis under
     no signal conditions with an AVO Model 8 Mk II, meter sensitivity 20,000 ohm/V",
     and the note "if V7 is not fitted then rectification is by IN4007 or similar diodes
     and output valves are EL34" -- a drawing from the 1966 changeover. It circulates as
     Marshall's (Dr.Tube, el34world: `Marshall_jtm45tr.gif`).
   - Marshall Amplification plc's CAD "Circuit diagram, JTM45" (JTM45.DGM, issue 7) with
     the company's title block: the two-channel lead head, no tremolo -- the reissue
     era's drawing of the circuit, to be checked part by part against the period one.
   Both are in `docs/schematics/marshall_jtm45/` (not committed).
3. **Best source.** The period drawing for the 1965 circuit and its voltages; the CAD
   drawing to settle what the hand lettering leaves uncertain (V1A's plate resistor is
   lettered "180K" against V1B's "100K"; the 5F6-A has 100 k on both).
4. **Cross-check.** The voltage chart: V1 plates 220 V, cathodes 1.6 V; V3 plate 190 V,
   cathode 1.1 V, the follower's cathode 190 V on 310 V; V4 (the inverter) plates 250 V,
   cathodes 40 V; the KT66s' plates 430 V and screens 440 V; the rectifier 450 V; the
   supply nodes 450 / 440 / 380 / 310 V circled on the drawing. And the Fender 5F6-A
   (the drawing in the same el34world set) for the topology it derives from.
5. **Signal path and values** (the period drawing; to be confirmed against the CAD):

```text
Inputs: two jacks per channel, 68 k / 68 k and 1 M, as the 5F6-A
V1 ECC83, both halves, shared cathode 820 R // 250 uF; plates 100 k (V1B), "180 k"
   (V1A, to be confirmed)
.02 uF to each channel's volume, 1 M; the bright channel's with 100 pF across
Mixer: 270 k from each wiper, 56 pF across one (the bright)
V3A ECC83 gain stage, 100 k plate, 820 R unbypassed cathode, direct into V3B, a
   cathode follower (plate on the 310 V node, 100 k cathode load)
Tone stack: 56 k slope, 270 pF treble cap, .02 uF bass and middle caps, TREBLE 250 k,
   BASS 1 M, MIDDLE 25 k
V4 ECC83 long-tailed pair: .02 uF in, 1 M grid leaks, 82 k and 100 k plates, 470 R and
   10 k tail; 27 k feedback from the 16 ohm tap, PRESENCE 5 k with 0.1 uF
V5, V6 KT66, 0.1 uF coupling, 220 k grid leaks, 1 k 2 W and 470 R 1 W in the screens
   (as lettered), fixed bias through 150 k from a diode off the HT winding
GZ34 (V7) rectifier, 32 + 16 uF, 20 H choke, 8.2 k 1 W and 10 k 1 W dropping to the
   preamp nodes
```

6. **To be modelled exactly.** The preamp, the stack as drawn, the inverter and its
   feedback and presence, the KT66 push-pull with its screens and fixed bias, the GZ34
   and the choke (sag), the output transformer's taps.
7. **To be approximated, and why.**
   - **The KT66**: no fit in the solver yet; a Koren pentode fit from GEC's KT66 data
     sheet (the procedure the EL34 and 6550 fits followed), ESTIMATED until then.
   - **The output transformer** (Radio Spares, then Drake): its primary impedance and
     leakage from published reissue data and the period accounts, ESTIMATED.
   - **The mains transformer's** regulation: from the chart's 450 V at the rectifier,
     DERIVED.
8. **Why.** The album preset *Californicated '99* is blocked on it (`docs/ROADMAP.md`,
   section C); the Super Bass it pairs with is built (Brit Plexi Bass).

## What was settled in building it

- **V1A's plate resistor is 100 k**, as Marshall's CAD drawing prints it and as the
  Bassman has it; the period drawing's lettering reads "180K".
- **The bright channel**: the period drawing's legible 100 pF across the volume
  (Marshall's later CAD sheet has 500 pF there), and 500 pF across its 270 k mixer --
  the CAD prints 500, and the period drawing's "5[00]pF" is half under the capacitor's
  symbol.
- **47 pF across the inverter's plates**: both drawings draw it.
- **No grid stoppers** on the KT66s: neither drawing has them (the builder takes one
  ohm of wire).
- **The bias, -48 V, DERIVED**: the period drawing's half-wave bias supply -- 150 k from
  the 350 V winding (the chart's "350 AC" at the rectifier), 8 uF, 15 k and 56 k --
  gives about 48 V (the CAD's 180 k would give 41). The drawing circles "105V" in that
  network; as a grid voltage it would cut the KT66s off (they stop near -57 V at 440 V
  of screen), and it is not used.
- **The KT66 fit** (`tools/tube_fit/fit_kt66.py`): two sheets disagree. Marconi's rows
  are cathode-biased with their resistors given, so each row's grid voltage is its
  resistor times its current, and they hold together under one law: the fit meets
  every one within 3 % (85 mA and 6.3 mA at 250 V / -15.5 V; 81 mA at 258 V / -17.4 V;
  62.5 mA at 400 V / 300 V / -26 V; 6.3 mA/V), with GEC's 22.5 kohm for the knee. GEC's
  later sheet (M-O Valve, 1977) gives 52 mA where the fit gives 59 at 415 V / 300 V /
  -27 V, and its ultra-linear row at 425 V asks for twice the amplification factor its
  own AB1 row allows; it is reported, not fitted. **A first attempt misread the 1977
  sheet's gm test current (85 mA, at which the grid is adjusted) as the current at
  -15 V**, and fought the rows for it.

## Results (`tests/brit_jtm45.rs`, `examples/jtm45_op.rs`)

| | model | chart |
|---|---|---|
| V1 plates, cathode | 221.6 V, 1.64 V | 220, 1.6 |
| gain stage plate, cathode | 187.0 V, 1.10 V | 190, 1.1 |
| follower's cathode | 187.4 V | 190 |
| supply nodes | 373.8 V, 321.6 V | 380, 310 |
| inverter plates, cathodes | 256.1 / 247.9 V, 41.6 V | 250, 40 |
| KT66 plates | 434.4 V | 430 |
| KT66 screens | 426.8 V (node) | 440 |

Each KT66 idles at 46 mA, 20 W, under GEC's 25 W design maximum. The model's output
transformer is ideal at DC, so its plates sit at the rail: the chart's 20 V of primary
copper between the rectifier (450 V) and the plates (430 V) is not dropped, and its
screens -- above the plates on the chart, behind the choke and a 1 k here -- come out
13 V low. The unloaded rail behind the GZ34 is ESTIMATED to put the plates on the chart.
The bright cap lifts 5 kHz against 200 Hz by 14.5 dB at a quarter volume against 5.0 at
full. The same at 44.1 to 192 kHz; realtime-safe; presets *Brit 45 Blues* and, since 2026-10-02 with its evidence table in PRESETS.md, *Californicated '99*.

**Approximated, beyond the KT66 and the bias**: the output transformer (a 6.6 k plate to
plate primary into 16 ohm, ESTIMATED from the Radio Spares / Drake part's class; copper,
inductance and leakage scaled from the Brit EL34's), the windings' resistance and the
choke's copper (ESTIMATED).
