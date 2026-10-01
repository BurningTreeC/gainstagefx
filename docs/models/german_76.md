# German 76 (Telefunken V76) research log

Engineering identity: **V76 microphone amplifier** ("Mikrofonverstärker V 76"), the
German broadcast standard built by Tonographie Apparatebau v. Willisen & Co.,
Wuppertal-Elberfeld, "nach einer Entwicklung im Institut für Rundfunktechnik", in
service from 1958, sold under the TAB and Telefunken names. Stable id
`pre_telefunken_v76` (`circuit`). Display: **German 76**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::german_76`; microphone preamplifiers), from
the IRT's own drawing and description, found after the first pass had stopped on a
redraw.

## Eight-question checkpoint

1. **Revision.** The V76 of the IRT's **Braunbuch-Beschreibung V 76, Ausgabe 1, 16.1.1959**
   (11 sheets) and its drawing **S 1176** ("Mikrofonverstärker", IRT, dated 15.1.59):
   three EF804S, one E83F, a twelve-position gain switch, a "Gerade / 3 kHz" switch and a
   four-position low cut. Of its two versions, the **V76/80** (low cut at 80 Hz, 300 Hz
   and 80 + 300 Hz; choke 87 BV Nr. 516, 100 H, C21 / C22 25 nF, C24 4 nF) is built; the
   V76/120's values are tabulated beside them. Not the later V76m and not the V72.
2. **Original schematic found?** **Yes, now.** The IRT's Braunbuch PDF
   (`irt.de/IRT/publikationen/braunbuch/V76.PDF`, from the Wayback Machine, which also has
   the description and the acceptance figures; a copy is in `docs/schematics/`,
   git-ignored). The drawing is a 300 ppi scan, legible part by part. The Supersonic
   redraw (Wagner Microphones) used in the first pass agrees on values but got two things
   wrong (below) and could not show the gain switch's decks clearly.
3. **Best source.** S 1176 for the circuit; the Braunbuch's sheets 2-10 for how the
   switches work and for the figures to meet.
4. **Cross-check.** The Braunbuch's acceptance conditions (Blatt 5-8), measured from a
   200 ohm generator into a 300 ohm load at 1 kHz and +6 dB out:
   - gain **76, 70, 64, 58, 52, 46, 40, 34 dB** (6 dB steps, +-0.5 dB) and below them
     **24, 18, 9 and 3 dB** (+-0.5 dB);
   - flat from 60 Hz to 15 kHz within +-0.5 dB, +0.5 / -1.5 dB at 40 Hz, at least 12 dB
     down at 15 Hz, steadily falling above 15 kHz;
   - "3 kHz": at most 0.6 dB down at 1 kHz, 5-7 dB at 10 kHz, 6-8 dB at 15 kHz;
   - "80 Hz": at most 3 dB at 80 Hz, at least 12 dB at 40 Hz; "300 Hz": at most 4 dB at
     300 Hz, at least 14 dB at 40 Hz; "80 + 300": at least 28 dB at 40 Hz;
   - distortion at +22 dB out and 76 dB of gain at most 0.3 % / 0.2 % (k2 / k3) at 40 Hz;
   - output impedance at most 30 ohm at 1 kHz; input impedance at least 600 ohm.
   And the drawing's own node voltages and branch currents.
5. **Signal path and values** (position numbers as drawn).

```text
input: HF-Sperre (not built); the two pad decks, one in each leg: 43 / 38 3.4 k at
   3 dB, 42 / 39 1.6 k at 9 dB, 41 / 40 450 R at 18 dB AND 24 dB, nothing above; the
   third deck puts 44 200 R across the input in the first four positions (3-24 dB)
85 BV Nr. 511, 1:30, primary in two halves with the 40 Hz high-pass between them:
   upper half -- 5, 6, 7, 8 (2 uF each, in parallel) -- 9 (2 uF) with 54 300 R
   "abgegl." across it -- lower half; secondary to V1's grid, its low end to 12 10 uF
   and through 55 100 k to the 57 / 56 junction
V1 EF804S: cathode 57 3 k + 56 12 k (11 V), g3 to the cathode, 13 50 uF from the gain
   switch's wiper to the cathode; screen 58 1 M with 14 50 nF to 0 V (97 V); plate
   59 200 k + 60 100 k from B1 with 18 4 uF at their junction (67 V)
17 25 nF; V2 EF804S: 61 1 M; cathode 62 500 R unbypassed (2.3 V); screen 63 100 k from
   B1 with 19 2 uF to 0 V (165 V); plate on 86 BV Nr. 514, 400 H, from B1 (175 V)
the loop: V2's plate -- 53 40 k -- 15 2 uF -- the string 52 19 k, 51 6.7 k, 50 2.4 k,
   49 1 k, 48 470 R, 47 224 R, 46 108 R, 45 98 R to 0 V; the wiper takes a tap
interstage: 66 40 k, 67 40 k; 92 "3 kHz" hangs 20 1 nF + 68 25 k off their junction;
   93, two linked decks: Gerade through 23 35 nF; "80" through 21 25 nF, 87 100 H to 0 V,
   22 25 nF; "300" through 21, 22 and 24 4 nF; "80 + 300" both
the 15 kHz low-pass: 25 100 pF, 84 BV Nr. 517a 0.85 H, 26 100 pF
V3 EF804S: 69 80 k to 27 10 uF and 70 200 k to the 71 1 k / 72 15 k junction; cathode
   16 V, g3 to it; screen 73 1 M from B1 with 28 50 nF to the cathode (71 V); plate
   74 200 k from B1 (80 V)
30 25 nF; V4 E83F: 77 1 M; cathode 78 150 R (2.2 V); screen 80 50 k from B2, 34 2 uF to
   0 V, 79 300 k to the cathode (140 V); plate on 88 BV Nr. 515, 250 H, from B2 (170 V,
   11.7 mA); 75 80 k "abgegl." + 31 2 uF from V4's plate to V3's cathode
35 4 uF to 89 BV Nr. 512, 9:1, DC-free ("gleichstromentlastet")
supply: B450 C80 bridge, 300 V and 21 mA on 36 32 uF; 81 1 k 2 W to B2 (32, 33
   32 uF each; the E83F's 14.6 mA); 76 6 k to B1 (29 32 uF; 6.1 mA for V1-V3)
```

   **The gain switch, position by position** (the Braunbuch: "Im Verstärkungsbereich von
   76 bis 34 dB wird die erwähnte Gegenkopplung verändert ... In den vier weiteren
   Schalterstellungen wird durch Einschalten der vor dem Eingangsübertrager liegenden
   Vordämpfung die Verstärkung auf 24, 18, 9 und 3 dB herabgesetzt, wobei die
   Gegenkopplung des Vorverstärkerteils den Wert der Stellung 34 dB bzw. 40 dB behält"),
   read deck by deck at 3x and 7x:

| Position | Gain | Pad, each leg | 200 R across | Loop tap (string below the wiper) |
|---|---|---|---|---|
| 1 | 3 dB | 3.4 k | yes | top, 30 k (the 34 dB position's) |
| 2 | 9 dB | 1.6 k | yes | top, 30 k |
| 3 | 18 dB | 450 R | yes | top, 30 k |
| 4 | 24 dB | 450 R | yes | 11 k (the 40 dB position's) |
| 5-12 | 34 ... 76 dB | none | no | 30 k, 11 k, 4.3 k, 1.9 k, 900, 430, 206, 98 R |

   Positions 3 and 4 have the same pad and differ only in the loop, by the 34 and 40 dB
   positions' 6 dB -- exactly the Braunbuch's sentence, and the reason the first pass,
   reading the redraw, could not make its steps come out even.
6. **To be modeled exactly.** Everything above but the RF filter, at the V76/80's values:
   the pads, the termination, the 40 Hz network, both amplifiers and both loops, the
   interstage with both switches, the 15 kHz low-pass, both chokes, both transformers'
   ratios, the supply chain.
7. **To be approximated, and why.** Every estimate below is bounded by a figure on the
   acceptance sheet, and `examples/german_76_op.rs` prints the bound beside the value.
   - **The valves**: `PentodeSpec::EF804S` and `PentodeSpec::E83F` fitted to their sheets
     (`tools/tube_fit/fit_ef804s.py`, `fit_e83f.py`). g3 is tied to the cathode in every
     stage, which is what a Koren pentode assumes. APPROXIMATED where it shows: V1 and V3
     run their plates below their screens (67 V against 97, 80 against 71), where the
     Koren pentode hands the plate the current the real valve sends to the screen -- the
     model's V1 plate sits at 43 V and V3's at 70, and their screens correspondingly high
     (111 against 97, 77 against 71). Everything else lands: the supply within 2.1 V, the
     cathodes within 0.8 V, V2's and V4's plates and screens within 9 V.
   - **85's inductance** is not printed. The Braunbuch (Blatt 2) says what it does: with
     "two capacitors in series and a resistor across one of them" between its primary
     halves it forms the 40 Hz high-pass, "so dimensioned that the flat range passes into
     the low cut evenly, without a resonant rise". Nothing lossless does that: C9 and R54
     put a zero at 265 Hz, and with only the generator and copper to damp it the resonance
     rises 1.5 dB at 60 Hz whatever the inductance (searched 0.5-15 H, R54 100-3000 ohm,
     copper to 200 ohm: one marginal corner). A nickel core's loss across the winding does
     it. ESTIMATED **2.5 H with 3 k of core loss** (both primary-referred): every point of
     the response, and both input-impedance figures, are met from 2.25 to 2.75 H and from
     2.5 to 4 k. It costs 0.65 dB in the band, which 75 (below) takes back.
   - **85's secondary capacitance** ESTIMATED, **15 pF**, between two of the sheet's
     figures: reflected through 1:30 it is the generator's only load at the top, so flat
     to 15 kHz within 0.5 dB holds it under about 17 pF, and the 20 dB wanted at 40 kHz
     wants all of that. (The first guess, 30 pF, put 15 kHz at -1.15 dB.) A disc winding
     is built for little of it.
   - **85's knee** ESTIMATED, **0.1506 V s** on the secondary: the sheet allows ten times
     the 1 kHz distortion at 40 Hz with 34 dB of gain and +22 dB out (1.5 % k3 against
     0.2 %) -- the input transformer at its largest flux, 5.8 V rms on the secondary.
     Set so k3 there is 1.0 %.
   - **89's inductance** ESTIMATED, **150 H**, with a floor in the sheet: at +22 dB out
     the E83F swings 88 V rms, and at 40 Hz the first guess of 50 H drew a 10 mA peak of
     magnetising current from a valve idling at 11.7 mA, cutting it off each cycle (0.73 %
     k2 against 0.3). 150 H gives 0.20 %. Copper and knee ESTIMATED.
   - **The chokes' copper** DERIVED from the drawing's voltages: 86 from B1 to 175 V at
     3.6 mA, 18.6 k; 88 from B2 to 170 V at 11.7 mA, 9.3 k. 87's and 84's ESTIMATED.
   - **The gain trim**: the Braunbuch's own -- "Abgleich der Grundverstärkung ... an Pos.
     72 oder Pos. 75" -- and 75 is drawn "abgegl.". Selected by bisection for 76 dB at
     position 12: **88.75 k** against the drawn 80 k. With the lossless transformer of the
     first pass the drawn 80 k gave 75.88 dB untrimmed; the core loss is the difference.
   - **The rail**: the bridge's 300 V at 21 mA as an open-circuit voltage behind an
     ESTIMATED 400 ohm, the voltage (308.6 V) fitted for 300 V at the bridge.
   - **The switches on the panel**: the gain switch is the Drive knob's twelve steps
     (`Taper::Steps`, four decks: both pads, the termination and the loop's wiper, each a
     pot on the one control); the low cut is the panel's low switch, flat second ("80 Hz",
     "Flat", "300 Hz", "80+300"), and "3 kHz" its mid switch ("3 kHz", "Flat"). The panel's
     switch parameter gained a fourth position, appended, for the low cut.
   - **The HF-Sperre** is not built (its band is radio).
8. **Why.** The IRT's own drawing and description, with acceptance figures for every
   control.

## What the model measures

`cargo run --release --example german_76_op`; `tests/german_76.rs` holds every line.
From a 200 ohm generator into 300 ohm, gain referred to the generator.

| The sheet | Wants | The model |
|---|---|---|
| Gain, positions 1-12, 1 kHz, +6 dB out | 3, 9, 18, 24, 34 ... 76 dB, +-0.5 | 3.12, 9.15, 18.05, 24.19, 34.22, 40.36, 46.26, 52.18, 58.06, 64.08, 70.10, 76.00 |
| Response (76 dB, -64 dB in) | 60 Hz-15 kHz +-0.5; 40 Hz +0.5/-1.5; 15 Hz <= -12 | 60 Hz +0.05, 100 Hz 0.00, 10 kHz -0.18, 15 kHz -0.31; 40 Hz -0.96; 15 Hz -17.4 |
| Above the band | falling from 15 kHz; <= -20 dB 40-200 kHz | 20 kHz -1.13; **40 kHz -15.9** (not met), 100 kHz -46.9 |
| "80 Hz" | <= 3 dB at 80, >= 12 at 40 | 1.8, 15.5 |
| "300 Hz" | <= 4 dB at 300, >= 14 at 40 | 2.8, 17.1 |
| "80 + 300 Hz" | <= 4 dB at 300, >= 28 at 40 | 2.0, 32.3 |
| Filters' cost at 1 kHz | <= 1.5 dB | 0.45 at most |
| "3 kHz", re 500 Hz | <= 0.6 at 1 kHz, 5-7 at 10 kHz, 6-8 at 15 kHz | 0.31, 6.1, 7.0 |
| Input impedance (34 dB) | >= 600 ohm 60 Hz-8 kHz, >= 300 at 40 Hz and 15 kHz | 750 / 3042 / 1353; 451, 762 |
| Output impedance | <= 30 ohm at 1 kHz, <= 40 ohm 40 Hz-15 kHz | 14.2; 15.0, 14.2 |
| k2 / k3, 76 dB, +12 dB out | 0.2/0.2 at 40 Hz, 0.1/0.1 at 1 and 5 kHz | 0.060/0.001, 0.039/0.002, 0.040/0.002 |
| k2 / k3, 76 dB, +22 dB out | 0.3/0.2 at 40 Hz, 0.2/0.2 at 1 kHz | 0.201/0.007, 0.111/0.036 |
| k2 / k3, 34 dB, +22 dB out | 1.0/1.5 at 40 Hz, 0.2/0.2 at 1 and 5 kHz | 0.098/1.000 (set), 0.047/0.038, 0.047/0.037 |

**Why the gain is referred to the generator.** The padded positions decide it. Position 4
differs from position 6 only by 450 ohm in each leg and 200 ohm across the transformer:
referred to the generator that is -16.2 dB, so 40 becomes 23.8 (24 wanted); referred to the
input terminals, across which the pads drop less, position 4 would read 25.7. Positions 1-3
come out at 2.9, 8.9 and 17.8 dB the same way. With the lossless transformer of the first
pass, every one of the twelve landed within 0.25 dB untrimmed, 75 at its drawn 80 k.

**The one figure not met**: 40 kHz, -15.9 dB against at least 20 (measured at 768 kHz; at
192 kHz trapezoidal warping reads that tone nearer 47 kHz and flatters it to -19.4). The
drawing's LC low-pass and the modelled secondary capacitance are all that is built above
the band; the unit's remaining 4 dB is in what this model leaves out -- V3's Miller
capacitance, both transformers' leakage and the output transformer's winding capacitance.
It is two octaves above anything the plugin passes.

**One fix in the chain came out of this.** The make-up curve's thirty-three points are
spread by a cube law and joined by straight lines. Across a gain switch's step that line is
wrong by up to the whole step: the 46 dB position read 3.5 dB loud at the knob's middle,
half way from the 40 dB point. `Calibration::make_up_db_on` takes a switched control's
make-up from the point inside the same position instead (`Circuit::control_steps` says
which controls are switches); all twelve positions, edges included, now play within
0.07 dB of each other, and the British 47's three within 0.05.

## What the redraw got wrong

- **5-9 and 54** are not a heater-hum network: they are the 40 Hz high-pass between the
  input transformer's primary halves, as the Braunbuch describes it.
- **19 2 uF** is V2's screen bypass, not its cathode's: its polarity is drawn with + at
  the 165 V screen, and V2's cathode resistor is unbypassed.
