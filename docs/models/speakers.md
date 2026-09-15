# Speaker driver research checkpoint (2026-09-15)

Scope: electrical and mechanical driver profiles for the reactive power-amp load and
the speaker radiation stage. The fitting code is in `tools/speaker_fit/`
(`fetch_jensen.py`, `fit_speakers.py`). It downloads Jensen's chart data rather than
committing it.

## Eight-question checkpoint

1. **Exact devices / revisions.** Current-production 8 ohm versions of:
   Celestion Vintage 30 (UK), Celestion G12M-25 Greenback (current Classic series),
   Celestion G12T-75 (Classic series), and Jensen Vintage reissue P12R, P10R, C12N,
   P12N. These are **not** 1960s originals: pre-rola Greenbacks, blackback G12H,
   and original 1950s Jensens differ in cone, magnet and coil.
   16 ohm variants have their own published T/S (Jensen). The model uses the 8 ohm set
   referred to the amplifier tap (see "Referencing" below) and documents that choice.
2. **Primary data found?** Yes for Jensen: full T/S tables plus numeric SPL and impedance
   series on each specification page. For Celestion **only partial**: Fs, Re, sensitivity,
   coil diameter, magnet and an SPL plot. Celestion does not publish Qms/Qes/Vas/Bl/Mms/Le
   for these guitar drivers (their stated reason is that they are not small-signal drivers).
3. **Best sources.**
   - [Celestion Vintage 30 brochure](https://celestion.com/product-brochure-pdf.php?id=888)
     and [product page](https://celestion.com/product/vintage-30/)
   - [Celestion G12M Greenback brochure](https://celestion.com/product-brochure-pdf.php?id=899)
     and [product page](https://celestion.com/product/g12m-greenback/)
   - [Celestion G12T-75 product page](https://celestion.com/product/g12t-75/) and
     [spec sheet](https://www.parts-express.com/pedocs/specs/294-2272--g12t-75-16-ohm-spec-sheet.pdf)
   - Jensen specification sheets: [P12R](https://www.jensentone.com/specification-sheet/41),
     [P10R](https://www.jensentone.com/specification-sheet/36),
     [C12N](https://www.jensentone.com/specification-sheet/16),
     [P12N](https://www.jensentone.com/specification-sheet/38),
     [P12Q](https://www.jensentone.com/specification-sheet/40), and the product pages
     that embed the SPL and impedance series.
4. **Cross-checks.**
   - Jensen internal consistency: Qes = 2 pi Fs Mms Re / Bl^2 reproduces the published Qes
     within 0.5 % for all five drivers. Zmax = Re (1 + Qms/Qes) is 37.6 / 44.8 / 34.2 ohm
     vs **measured** 38.2 / 42.4 / 33.9 ohm (P12R / C12N / P12N). Pistonic sensitivity
     computed from T/S matches the published sensitivities to 0.1 dB.
   - **Conflict:** P10R 8 ohm measured impedance peak is 82 ohm where its T/S predicts 61 ohm,
     and the C12N 4 ohm sheet lists Re = 6.5 ohm (probably a typo for a 4 ohm part).
     Neither driver's 8 ohm model uses the conflicting value. Recorded, not resolved.
   - Celestion: no second primary source exists. Published T/S of drivers sold as
     analogues serve only as **plausibility bounds**, never as values:
     [Eminence Legend GB128](https://eminence.com/products/legend_gb128) (Bl 12, Qes .67,
     Mms 28 g) and WGS Retro 30 / Green Beret tables (Bl 13.85 / 11.7, Qes .75 / 1.35).
     The WGS Retro 30 Vas "16.28 cu ft" is internally inconsistent with its own Cms and Sd.
5. **Values used.** Table below.
6. **Modeled from the physics.** The lumped electro-mechanical equivalent
   `Z = Re + (s L1 || R1) + (s L2 || R2) + [Res || s Lces || 1/(s Cmes)]`, with
   `Res = Bl^2/Rms`, `Lces = Bl^2 Cms`, `Cmes = Mms/Bl^2`. It is stamped into the
   power-amplifier netlist so the tubes, feedback loop and transformer see it.
   Cone velocity is read from the motional voltage (`u = V_mot / Bl`).
7. **Approximated.** Celestion Bl, Qes and Mms are ESTIMATED. The lossy coil for Celestion
   is the mean of the Jensen 38 mm fits (C12N and P12N). Breakup/cone voicing is EMPIRICALLY TUNED
   to the manufacturer SPL plots. 16 ohm parts use 8 ohm data. No excursion or
   thermal nonlinearity yet. The enclosure and array air load are physics-derived
   approximations (see `CABINET_MODEL.md`).
8. **Why.** Celestion does not publish the parameters. A bounded, stable real-time
   solve needs a linear lumped load. Manufacturer SPL plots are the only available
   breakup evidence, and fitting them is honest about what they are.

## Method (reproducible: `tools/speaker_fit/fit_speakers.py`)

- **Voice coil.** Two lossy inductances, (L1 || R1) + (L2 || R2), are fitted to Jensen's
  measured |Z| from 250 Hz to 20 kHz. Log-rms error is 0.029 to 0.038.
  - A pure Le from the sheets is about 2x too high at 20 kHz (P12R: 63 ohm pure vs 34 ohm
    measured).
  - The first fit used LR-2 (L1 in series, L2 || R2), with log-rms error 0.043 to 0.061. It was
    replaced after stability testing (below): its series L1 rises without limit above the
    band, while the lossy pair levels off (46-59 ohm at 100 kHz).
- **Celestion estimates.** Fs and Re are the manufacturer values. Mms = 28.0 g and
  Qms = 9.73 are the **medians of the published 12-inch guitar-driver tables** above.
  Bl is solved so that the lumped piston model (with the Jensen-derived offset of
  +3.87 dB between the model and the manufacturer plots at 500-1000 Hz) reproduces the
  Celestion plot level at 500-1000 Hz.
  - A first attempt inferred Qts from the plots' low-frequency shape. It ran to the
    50 g search bound, which is not physical: the plot conditions below 500 Hz are
    unknown. It was rejected.
  - A second attempt referenced 150-500 Hz, where the plots are still rising, and was
    rejected for the same reason.
- **Breakup voicing.** The residual (manufacturer SPL minus lumped piston model, with
  a Jensen-calibrated IEC-baffle rear-wave term) is fitted from 300 Hz to 14 kHz with
  3 peaking sections and a 2nd-order low-pass. Gain is bounded to +-12 dB and Q to
  0.7-6. The rms error for 300 Hz-7 kHz is 1.41-1.96 dB. Celestion plot points were
  digitised by eye (about +-1.5 dB).

## Profile values (8 ohm data)

| Display | ID | Re | L1 mH | R1 | L2 mH | R2 | Fs | Qms | Mms g | Bl | Sd cm2 | Evidence |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Brit V30 | `spk_celestion_v30` | 7.3 | 0.448 | 44.05 | 1.221 | 6.19 | 75 | 9.73 | 28.0 | 14.71 | 490.9 | Re/Fs PUBLISHED; rest ESTIMATED |
| Brit Green 25 | `spk_celestion_g12m25` | 6.7 | 0.448 | 44.05 | 1.221 | 6.19 | 75 | 9.73 | 28.0 | 9.90 | 490.9 | Re/Fs PUBLISHED; rest ESTIMATED |
| Brit T75 | `spk_celestion_g12t75` | 6.77 | 0.448 | 44.05 | 1.221 | 6.19 | 85 | 9.73 | 28.0 | 10.87 | 490.9 | Re/Fs PUBLISHED; rest ESTIMATED |
| American Vintage 12 | `spk_jensen_p12r` | 6.05 | 0.288 | 36.81 | 0.832 | 4.28 | 80 | 13.07 | 25.2 | 5.53 | 490.9 | PUBLISHED; coil FITTED |
| American Vintage 10 | `spk_jensen_p10r` | 6.68 | 0.299 | 35.77 | 0.674 | 4.40 | 97 | 14.83 | 15.2 | 5.83 | 330.1 | PUBLISHED; coil FITTED |
| American Ceramic | `spk_jensen_c12n` | 6.05 | 0.463 | 46.77 | 1.430 | 6.67 | 113 | 7.52 | 29.9 | 10.46 | 490.9 | PUBLISHED; coil FITTED |
| American Alnico | `spk_jensen_p12n` | 6.03 | 0.433 | 41.33 | 1.013 | 5.71 | 90 | 4.36 | 30.9 | 10.62 | 490.9 | PUBLISHED; coil FITTED |

Derived Celestion Qes / Qts / Zmax: V30 0.445 / 0.426 / 167 ohm, Green 25 0.902 / 0.825 / 79 ohm,
T75 0.856 / 0.787 / 84 ohm. These fall inside the analogue-driver bounds.

Breakup voicing (peaking fc Hz / gain dB / Q; low-pass fc / Q), EMPIRICALLY TUNED:

| ID | peaks | low-pass | rms dB |
|---|---|---|---|
| v30 | 897/+4.17/1.94, 2454/+10.62/1.26, 3871/+11.82/2.44 | 5128/0.50 | 1.50 |
| g12m25 | 986/+4.51/2.52, 2489/+12.00/1.22, 3811/+12.00/2.42 | 5478/0.50 | 1.96 |
| g12t75 | 1266/-5.07/3.53, 2205/+12.00/0.99, 3510/+9.25/1.45 | 7263/0.50 | 1.41 |
| p12r | 2011/+9.06/0.88 (x2), 3734/+7.49/1.64 | 7304/1.22 | 1.60 |
| p10r | 1058/+5.48/2.22, 2526/+11.32/0.91, 4093/+12.00/1.07 | 7553/1.20 | 1.44 |
| c12n | 1108/+8.94/1.81, 2092/+12.00/2.12, 3497/+10.45/1.27 | 5829/2.13 | 1.48 |
| p12n | 1006/+8.69/2.44, 2436/+10.70/1.71, 3632/+12.00/1.45 | 6156/1.54 | 1.61 |

## Referencing the load to the amplifier

A cabinet of identical drivers wired to its nominal impedance presents the single-driver
curve scaled by the wiring factor. The amplifier is assumed to use the transformer tap
matching that nominal impedance. Referred to the tap the power spec was drawn for
(`PowerSpec::speaker`), every impedance is multiplied by `k = PowerSpec::speaker / 8`
and every capacitance divided by `k`. This is exactly an ideal multi-tap output
transformer. Feedback stays connected to the drawn tap, and voltages at the speaker
differ from the reference-tap voltage by the constant `sqrt(Z_nominal / PowerSpec::speaker)`.

## Tests required before calling this complete

- Stamped netlist impedance (AC solve) equals the analytic Z(f) at all frequencies.
- Fitted |Z| is within 15 % of Jensen measured curves 250 Hz-10 kHz.
- Resonance frequency and peak magnitude respond to the enclosure as predicted.
- The speaker-loaded power stage is stable (idle, hard drive, all rates and blocks) and
  audibly differs from the resistor. The resistive Matched path stays bit-identical.

## Stability finding: speaker load inside a feedback amplifier

Stamping the driver into the power netlists exposed high-frequency behaviour the
resistor had hidden. It was measured with a 0.25 s sine at the power-stage input.

1. **LR-2 coil at 96-192 kHz.** Brit EL34 (the only spec whose PI grid has no stopper) went
   into a sustained ultrasonic oscillation, with 150-600 V sample steps and thousands of
   fallbacks. Clean at 48 kHz.
   - With feedback disabled, every load was clean, so this is loop instability, not the
     speaker model.
   - Removing only L1 fixed it.
2. **Lossy coil.** The EL34 and Mark are clean everywhere. The Twin and 5150 are clean at
   realistic drive but still failed to converge under extreme HF overdrive (80 V at
   1 kHz, 40 V at 7 kHz into the power-stage input), swinging to ~1 kV at 192 kHz.
3. **Published tube capacitances** (JJ EL34: cg1 15.5 pF, ca 10 pF, ca/g1 1.3 pF;
   RCA 6L6GC: 10 / 6.5 / 0.6 pF), stamped with Miller multiplication, *reduced* stability
   in this otherwise incomplete HF network (low-level oscillation in Mark and 5150). They are
   **not** stamped.
4. **Primary winding capacitance**, 1.5-4.7 nF, removed every unsettled solve in those
   cases, bounded the output (at or below ~100 V under abuse) and changed moderate-drive
   output by under 0.1 %.
   - No winding capacitance is published. Hammond's 1750U tolerance (50 Hz-12 kHz, 0/-1 dB)
     only bounds it below ~11 nF.
   - Adopted rule (ESTIMATED): C = 1 / (2 pi 30 kHz Raa). This gives 3.1 nF (2203),
     2.8 nF (Twin, 5150) and 1.5 nF (Mark IIC+).
   - Stamped **only** in the speaker-loaded netlists, so the resistive legacy netlists and
     their calibration are untouched (frozen fixture still passes).

Test coverage: `tests/speaker_load.rs::speaker_loaded_power_is_bounded_and_settles_under_abuse`.
It runs 4 power stages, 3 loads, 48 and 192 kHz, and 4 drive cases, and asserts finite
output below 150 V with at most 0.1 % unsettled samples. The one remaining unsettled sample
is Twin into a V30 at 48 kHz, 40 V at 7 kHz.
