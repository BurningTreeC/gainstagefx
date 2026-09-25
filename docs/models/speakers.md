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
| Jazz 12 | `spk_roland_30_103d` | 6.68 | 0.448 | 44.05 | 1.221 | 6.19 | 85 | 9.73 | 28.0 | 10.72 | 490.9 | size/impedance DOCUMENTED; rest ESTIMATED; voicing FITTED to a measurement (below) |
| Brit K85 | `spk_celestion_g12k85` | 7.0 | 0.448 | 44.05 | 1.221 | 6.19 | 85 | 9.73 | 28.0 | 14.16 | 490.9 | Re/Fs PUBLISHED (for the G12K-100); rest ESTIMATED as for the Celestions (below) |

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
| roland_30_103d | 510/+6.54/1.95, 3642/+8.58/2.76, 10172/+12.00/2.52 | 10344/0.50 | 1.16 (90 Hz-12 kHz, near-field, see below) |
| g12k85 | 2469/+11.85/1.14, 3577/+8.80/1.94, 5195/+7.03/5.73 | 6631/0.50 | 1.98 |

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

## Jazz 12: the Roland 30-103D (2026-09-24)

The JC-120's own driver, added so the Jazz presets stop playing through a Jensen
C12N in a Fender cabinet.

1. **Device / revision.** Roland **30-103D**, 12 inch, 8 ohm: Roland part 041-019
   in the 1979 third-edition and later JC-120/JC-160 service notes, and
   **30-103D-3** (part 22410204, "8OHM") for the JC-120JT and the newer JC-120UT in
   the 2000 notes. The older UT briefly used a **12-4782 (G1208F01)** instead, and the
   JC-120A R&P a **C1230 (307454)**; neither is modelled.
2. **Primary data found?** Size and impedance only (the service notes above, local
   copies in `docs/schematics/roland_jc120*.pdf`). No Thiele/Small data, sensitivity or
   response plot has been published by Roland or any supplier. A 60 W rating appears in
   dealer listings of pulled drivers (SECONDARY).
3. **Best source for the sound.** A measurement, not a data sheet:
   [tomachikeita, "ジャズコ(JC-120)リターン差しの周波数特性を測定"](https://tomachikeita.hatenablog.com/entry/2024/09/25/140335)
   (2024-09-25). A JC-120 driven through its RETURN jack -- its own flat transistor
   power stage, none of the preamp -- into its own two speakers, a calibrated measurement
   microphone (Sonarworks Reference 4) about 9 cm off the centre of one cone and about
   4 cm from the grille, REW. SECONDARY: one unit of unknown year. The author names the
   sharp peaks at **3.6 kHz and 9.2 kHz** as the amplifier's characteristic hardness.
4. **Cross-check.** None for the driver itself. The Gear Page thread "Roland JC-120 /
   JC-160 speaker frequency response" exists but is paywalled. The measured curve's
   shape is in line with the owner reports of a bright, hard 12-inch.
5. **Values used.** Table above. The curve was digitised from the plot's own REW grid
   (422 px a decade, 14.8 px a dB, the red trace read column by column) at twelfth-octave
   points; the 100 points are in `tests/support/jazz_measurement.rs`.
6. **Fitted, and how.** The Celestion recipe fits a far-field manufacturer plot against
   a lumped piston model, and does not apply to a near-field curve inside a cabinet. So
   `examples/jazz_speaker_fit.rs` draws **the plugin's own path at the measurement's
   placement** -- the voltage-driven driver's motional response, differentiated as the
   chain does, then `AcousticStage` with an ideal omni at position 0.72 (9 / 12.5 cm) and
   4 cm in the Jazz Open 2x12 -- and fits the breakup voicing (three peaks and the
   low-pass, inside the same bounds as the other profiles) by Nelder-Mead to the curve,
   smoothed over a sixth of an octave, from **90 Hz to 12 kHz**, level offset removed.
   Result **1.16 dB rms**, closer than any Celestion profile follows its plot. The model
   puts the upper peak at 3,552 Hz and the top one at 10.0 kHz, with the trough between
   (`tests/jazz_cabinet.rs`).
7. **Approximated, and two things the fit found.**
   - **Fs and Bl are priors, not fitted.** A first fit that included the low end drove Fs
     to its 45 Hz bound: below about 90 Hz a near-field measurement of a combo on a floor
     is the floor as much as the speaker. Fs is the catalogue's median (85 Hz) and Bl gives
     the catalogue's median Qts (0.80). Re is the catalogue's median (6.68 ohm); the coil,
     Mms and Qms are the Celestion priors. All ESTIMATED.
   - **Fitted through one cone, not two.** The measurement is dominated by the cone the
     microphone is in front of, so the voicing is fitted through a one-driver copy of the
     cabinet (`FIT_CABINET`) and carries nothing of the second cone. Fitting it through
     the two-driver cabinet first gave 2.0 dB rms with every peak at its +12 dB bound,
     filling comb-filter notches at 0.6, 2.0 and 3.4 kHz that the measurement does not
     have -- which is how the stage's far-cone error was found: one-pole directivity and
     rim distances left the neighbouring cone only about 8 dB down. **Fixed the same day**
     (`CABINET_MODEL.md`, "Near and far cones"): with the fix the whole two-cone model
     follows the measurement to 1.58 dB rms (2.67 before), and
     `tests/jazz_cabinet.rs` holds it.
   - The voicing is fitted at one off-centre placement through the stage's own
     off-centre dulling (the `HF_RADIUS` corner, 2.6 kHz there). Where that tuned
     approximation differs from the real cone, the voicing carries the difference to
     other placements. The top peak and the low-pass Q sit at their bounds, as five of the
     other seven profiles' peaks do.
8. **Why.** No manufacturer data exists. One careful measurement of the real amplifier,
   reproduced inside the model rather than read as if it were a data sheet, is the best
   evidence available, and it is labelled as what it is.

## Brit K85: the Celestion G12K-85 (2026-09-25)

For *Machine Rage '92*'s "1987 Peavey 4x12 cabinet with Celestion G12K-85 speakers"
(WIDELY REPORTED; see `cabinets.md`).

1. **Revision.** The G12K-85, 8 ohm, the late-1970s-to-1990s ceramic 85 W twelve.
2. **Original data found?** **No G12K-85 sheet.** Celestion's current **G12K-100**
   (its "Legacy Guitar Speakers" range) is published with Re 7 ohm, Fs 85 Hz, 99 dB, a
   44.5 mm round-copper coil, a 1.42 kg / 50 oz ceramic magnet, and an 8 ohm response
   plot. That the G12K-100 *is* the G12K-85 re-rated is WIDELY REPORTED ("Celestion never
   stopped making it and later reintroduced it as the G12K-100"), not stated by Celestion.
   The size, magnet, coil and Fs all match the G12K-85 as sold, so this is treated as the
   same driver with its data, not as a near relative -- the same standing as building the
   G12T-75 from the current Classic-series sheet. If a G12K-85 sheet turns up, it wins.
3. **Best source.** Celestion's G12K-100 brochure PDF (`celestion.com/product-brochure-pdf.php?id=902`)
   and its plot (`celestion.com/wp-content/uploads/2019/09/G12K-100-copy.jpg`).
4. **Cross-check.** Celestion's own period description, WIDELY QUOTED: the G12K-85 "is
   exactly like the G12T-75, but with a larger (50oz) ceramic magnet, and a more focused
   field". The fit agrees without being told: with the same priors and recipe, the
   G12K-100's plot needs **Bl 14.16 against the G12T-75's 10.87** -- the stronger motor a
   bigger magnet gives -- and its two main breakup peaks land at 2.47 and 3.58 kHz against
   the T-75's 2.21 and 3.51 kHz.
5. **Values.** The tables above.
6. **Fitted, and how.** Exactly the Celestion recipe (`tools/speaker_fit/fit_speakers.py`,
   entry `g12k85`), with one difference in how the plot was read: **not by eye**. The red
   trace's pixels were read against the plot's own grid lines (decades at x = 240, 493 and
   745 px, 5 dB per 25.2 px) every 1/12 octave from 21 Hz, 119 points. The tool's three
   existing Celestion fits reproduce unchanged in the same run. Voicing error 1.98 dB rms,
   300 Hz-7 kHz, at the top of the other Celestions' 1.41-1.96.
7. **Checked in the plugin** (`tests/peavey_cabinet.rs`): drawn through the acoustic stage
   on one cone of the Peavey, the presence peak is at 2.47 kHz, 8.9 dB over 1 kHz (the
   plot: 8.5), and 7 kHz is 27.9 dB under it (the plot: 29 at 6.8 kHz).
8. **Why.** The G12K-85 is the documented speaker of a documented rig, and the only
   manufacturer data for its design is the G12K-100's.

