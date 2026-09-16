# American 312 (API 312 microphone preamplifier card) research log

Engineering identity: **API (Automated Processes, Inc.) 312 mic preamp card**, as on the
original hand-drawn API "MIC PREAMP SCHEMATIC". Proposed stable id `pre_api_312`.
Display: American 312.

Status: **IMPLEMENTED 2026-09-15** (`src/circuits/american312.rs`), tested in
`tests/american312.rs`. Stable id `pre_api_312` (`circuit` parameter).

## Eight-question checkpoint

1. **Revision.** The original console-era card with a 2622 input transformer, one 2520
   discrete op-amp and a 2503 output transformer. Not the modern 500-series 312 (AP2516
   input transformer, -20 dB pad, 1:1/1:3 output switch) and not the 50th Anniversary unit.
2. **Original schematic found?** Yes. The API factory drawing "MIC PREAMP SCHEMATIC"
   (T2 2622, 2520, T1 2503), the API transformer data sheet (2 pages) and the API Model
   2520 data sheet are archived at
   [waltzingbear API schematics](http://www225.pair.com/audio/waltzingbear/Schematics/API.html).
   The same archive holds a 2520 transistor-level schematic that is a *redraw* listing
   production variants, not a factory drawing.
3. **Best source.** The factory card drawing and API's transformer and op-amp data sheets.
4. **Cross-check.**
   - API's current [312 product page](https://apiaudio.com/product/312-mic-preamp/):
     gain +34 to +69 dB, maximum output +31 dBu, ±0.5 dB 20 Hz-40 kHz, output impedance
     under 75 ohm.
   - The [Sound On Sound review](https://www.soundonsound.com/reviews/api-312): input
     transformer about 1:7, one 2520 as the variable-gain stage, quad-filar 2503.
     These describe the modern module and are used only where they agree with the card.
5. **Signal path and values (factory card).**

```text
mic HIGH/LOW -> T2 AP2622 (primaries 150/600 ohm, series or parallel; secondary 10 k)
  secondary pin 3 -> 2520 +IN; R4 5.1k + C5 220 pF from +IN to common (secondary load/Zobel)
  C6 1000 pF, C7 1000 pF to common (RF)
2520: non-inverting; R3 20k output -> -IN with C4 120 pF across;
      gain set by the external GAIN TRIM resistor at pins 9/10: R2 200 ohm from -IN,
      then C3 250 uF 4 V to common
  supplies +V/-V via CR1/CR2 (2161) with C1/C2 12.5 uF 25 V
OUT -> T1 AP2503 primary (75 ohm, RED/BRN), secondaries SEC1..SEC3 (quad-filar), 1:1/1:2/1:3
```

   Transformer data sheet (API):
   - AP2622: primary 150/600 ohm, secondary 10,000 ohm, 30 Hz-20 kHz ±0.5 dB, maximum
     level 0 dBm. Turns ratio sqrt(10000/150) = 8.16 or sqrt(10000/600) = 4.08.
   - AP2503: primary 75 ohm; secondaries 75 ohm (1:1), 300 ohm (1:2) or 600 ohm (1:3)
     minimum loads; 30 Hz-20 kHz ±0.5 dB; +30 dBm.

   2520 data sheet (API): gain > 110 dB DC; small-signal GBW "-50 mHz" (read as 50 MHz,
   legibility recorded); full output to 40 kHz; output > 7.75 V rms at ±15 V (> 11 V rms at
   ±20 V); minimum load 75 ohm; 0.2 % THD 20 Hz-20 kHz at rated output.
6. **To be modeled.**
   - The input transformer (ideal ratio plus winding/magnetising network and a saturating
     core at its 0 dBm rating).
   - The secondary network, feedback and gain network with their capacitors.
   - The 2520 as a high-gain op-amp with rail and output-current limits.
   - The output transformer into a line load.
7. **Approximated (planned).**
   - The 2520 is not built from transistors: nine transistors including PNPs, and the
     solver has no PNP device. It is a behavioral op-amp with the data sheet's swing and
     (if feasible) slew/GBW.
   - Transformer leakage/capacitance values are not published; they are fitted to the
     published ±0.5 dB bandwidth.
   - The gain-trim resistor, which is external to the card, becomes the Gain control over
     the published +34 to +69 dB range.
8. **Why.** No transistor-level factory 2520 drawing, and no PNP device in the solver.

## Implementation notes (2026-09-15)

- **Primary strapping.** The card offers HIGH and LOW. The model uses the 150 ohm (1:8.16)
  strapping for a 150 ohm microphone. This choice is stated.
- **Output.** SEC 1 + SEC 2 in series (pins 4-6), 1:2, into a 10 k line input.
- **Gain trim.** The resistance at pins 9/10 is the Gain control, 20 k to 0 on a geometric
  law, so gain moves evenly in dB. Measured 30.2 / 36.5 / 45.0 / 54.5 / 64.2 dB at
  0 / 0.25 / 0.5 / 0.75 / 1.
- **2622, ESTIMATED to API's figures:**
  - copper 20 / 800 ohm, leakage 1 mH, secondary capacitance 50 pF;
  - core henry 2.6, knee 0.029;
  - transformer alone: −0.52 dB at 30 Hz, −0.03 dB at 20 kHz, 1.3 % at 0 dBm and 30 Hz.
- **2503, ESTIMATED.** Copper 8 / 30 ohm, core henry 0.6, knee 0.08, sharpness 5.
- **Card response.** At mid gain it is −1.8 dB at 20 kHz. The transformer alone is flat,
  so the loss is the drawn R4/C5 damping network against the 2622's ~10 k secondary source
  impedance, plus C4 across R3. It is recorded, not tuned out.
- **2520.** An ideal op-amp with a 12 V rail. Clean below the rail (0.00 % at 5 mV in, full
  gain), squared off above it.
