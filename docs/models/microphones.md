# Microphone research checkpoint (2026-09-15)

Fitting code: `tools/speaker_fit/fit_mics.py`. It contains the digitised chart points and
their sources.

## Eight-question checkpoint (per family, summarised)

1. **Targets.**
   - Shure SM57 (current)
   - Sennheiser MD 421 II, "M" bass position
   - Sennheiser e 906, flat presence position
   - Sennheiser MD 409 U3 (1986 data sheet)
   - Royer R-121
   - beyerdynamic M 160 (current, E7 09.23 sheet)
   - Coles 4038
   - Neumann U 87 Ai, cardioid
   - AKG C414 (variant unresolved)
   - Neumann U 67 (reissue of 1960-71 design)
   - Neumann U 47 fet
2. **Primary data found?**
   - Response chart and polar plots: SM57, U 87 Ai.
   - Response chart and distance curves: M 160 (10 cm and 1 m), MD 409 (5 cm and 100 cm).
   - Response chart and polar plot: R-121.
   - Response chart: 4038.
   - Small response and polar plot: MD 421 II.
   - Specifications only (range, pattern, sensitivity): C414, U 67, U 47 fet.
   - e 906: the manufacturer manual PDF was blocked (HTTP 403); shape from a
     **secondary** description (RecordingHacks).
3. **Sources.**
   - [Shure SM57 user guide v3.6](https://pubs.shure.com/view/guide/SM57/en-US.pdf)
   - [Sennheiser MD 421 II product sheet](https://media.sweetwater.com/store/media/md421ii_productsheet.pdf)
   - [Sennheiser e 906 manual](https://assets.sennheiser.com/global-downloads/file/14874/e906_Manual_07_2020_EN.pdf)
     (blocked) and [RecordingHacks e 906](https://recordinghacks.com/microphones/Sennheiser/e906)
   - [MD 409 U3 1986 data sheet archive](https://www.coutant.org/md409u3/index.html)
   - [Royer R-121 cut sheet](https://media.sweetwater.com/store/media/r121_R-121cutsheet.pdf)
   - [beyerdynamic M 160 data sheet](https://cdn.accentuate.io/8054880632983/12567395860524/DAT_M160_EN_A7-v1694129552515.pdf)
   - [Coles 4038 data sheet](https://coleselectroacoustics.com/wp-content/uploads/2021/12/4038Spec.pdf)
   - [Neumann U 87 Ai operating manual](https://www.neumann.com/globalassets/digizuite/16474-en-opin0134_u87ai_06-10.pdf)
   - U 67 and U 47 fet: Neumann published specifications (range, sensitivity), via retailer
     reproductions of Neumann data.
4. **Cross-checks / conflicts.**
   - **MD 409:** the 1986 data sheet says *cardioid*, while current marketing descriptions
     say supercardioid. The primary document is used (cardioid) and the conflict is
     recorded.
   - **C414:** the XLS is described as flat, the XLII as having a presence boost near 5 kHz.
     The model follows a flat-ish XLS-like curve, APPROXIMATED.
5. **Values.** Tables below.
6. **From physics.** First-order polar pattern `a + b cos(psi)` with the signed gradient
   term (figure-8 rear lobe inverts). Proximity is the gradient term's near-field
   `1 + omega_p/(j omega)`, limited by a leaky integrator. Propagation distance, delay and
   angle come from the 3-D placement (see `CABINET_MODEL.md`).
7. **Approximated.**
   - Charts digitised by eye: +-1 dB on large charts, +-2 dB on small ones.
   - Off-axis high-frequency loss is a one-pole per path, fitted to one polar reading
     (SM57) or ESTIMATED.
   - Proximity strength `k_p` comes from distance data where it exists (M 160, MD 409, the
     SM57 text) and is ESTIMATED otherwise.
   - Capsule depth behind the grille is ESTIMATED from body dimensions.
   - Subtle nonlinearity levels are TUNED, not measured.
   - C414, U 67 and U 47 fet shapes are APPROXIMATED.
8. **Why.** No phase or impulse measurements are available. Real-time and no-convolution
   requirements call for minimum-phase low-order sections.

## On-axis response fits (HP2 fc/Q; 4 peaking fc/dB/Q; LP2 fc/Q)

| Display | ID | Pattern | HP2 | Peaks | LP2 | rms dB | Evidence |
|---|---|---|---|---|---|---|---|
| Dynamic 57 | `mic_shure_sm57` | cardioid | 64.0/0.460 | 490/-1.35/1.63, 4302/+2.40/1.19, 5620/+2.00/2.81, 11129/+1.76/3.75 | 8789/1.269 | 0.40 | chart |
| Dynamic 421 | `mic_sennheiser_md421` | cardioid | 19.2/0.343 | 168/-2.88/0.40, 3714/+2.19/0.61, 4920/+1.67/1.48, 9542/-0.80/3.55 | 10800/1.169 | 0.13 | small chart |
| Dynamic 906 | `mic_sennheiser_e906` | supercardioid | 67.6/0.461 | 968/-0.26/3.46, 3536/+2.52/1.02, 10385/+2.37/1.72 (x2) | 6254/1.114 | 0.21 | SECONDARY |
| Dynamic 409 | `mic_sennheiser_md409` | cardioid (see conflict) | 47.5/0.300 | 103/-1.06/1.13, 3748/+0.61/3.15, 12094/+4.34/1.84, 12106/+4.33/1.85 | 7901/0.777 | 0.30 | chart |
| Ribbon 121 | `mic_royer_r121` | figure-8 | 17.5/0.528 | 60/+1.85/1.12, 3435/+0.52/0.54, 3568/+0.57/0.55, 14960/-1.07/2.28 | 39870/0.454 | 0.12 | chart |
| Ribbon 160 | `mic_beyer_m160` | hypercardioid | 81.6/0.495 | 183/+2.63/1.06, 4086/+1.53/0.60, 6787/+1.17/1.90, 12352/-0.84/2.84 | 17406/0.768 | 0.16 | chart |
| Ribbon 38 | `mic_coles_4038` | figure-8 | 27.6/0.798 | 125/+0.37/0.40, 3178/+0.40/0.68, 9646/-0.19/8.00, 12057/+0.37/6.23 | 16653/0.723 | 0.09 | chart |
| Condenser 87 | `mic_neumann_u87ai` | cardioid | 7.6/0.300 | 69/+0.39/1.14, 4783/-0.38/1.45, 9215/+0.76/2.24 (x2) | 14432/0.861 | 0.05 | chart |
| Condenser 414 | `mic_akg_c414` | cardioid | 16.5/0.655 | 60/+0.15/0.97, 2741/+0.18/1.00, 11414/+0.26/3.60, 11856/+0.29/5.78 | 15475/1.018 | 0.06 | APPROXIMATED |
| Tube Condenser 67 | `mic_neumann_u67` | cardioid | 8.6/0.300 | 113/+0.23/2.09, 5840/+0.39/0.51, 8516/+0.46/3.49, 15183/+0.38/7.56 | 12711/1.156 | 0.07 | APPROXIMATED |
| FET Condenser 47 | `mic_neumann_u47fet` | cardioid | 32.8/0.663 | 121/+0.53/0.40, 7621/-0.72/1.21, 13464/+1.10/2.99, 15111/+1.03/3.63 | 10637/1.274 | 0.13 | APPROXIMATED |

Pattern coefficients: cardioid a = b = 0.5; supercardioid a = 0.366, b = 0.634;
hypercardioid a = 0.25, b = 0.75 (null at 109.5 deg, consistent with M 160's ">25 dB at
110 deg"); figure-8 a = 0, b = 1.

## Placement-related parameters

| ID | Off-axis HF loss at 90 deg, 10 kHz | Proximity k_p | Capsule depth m | Saturation family |
|---|---|---|---|---|
| sm57 | 4 dB (Shure polar, 8 kHz reading) | 0.18 (Shure: +6-10 dB below 100 Hz at 6 mm) | 0.012 (EST) | dynamic |
| md421 | 5 dB (small polar, EST) | 0.25 (EST) | 0.020 (EST) | dynamic |
| e906 | 3 dB (EST) | 0.25 (EST) | 0.010 (EST) | dynamic |
| md409 | 4 dB (EST) | 0.22 (5 cm vs 100 cm curves) | 0.010 (EST) | dynamic |
| r121 | 0.5 dB (Royer: frequency-consistent figure-8) | 1.0 (pure gradient, EST) | 0.012 (EST) | ribbon |
| m160 | 2 dB (polar chart, EST) | 1.2 (10 cm vs 1 m curves) | 0.015 (EST) | ribbon |
| c4038 | 1 dB (Coles: substantially constant) | 1.0 (pure gradient, EST) | 0.030 (EST) | ribbon |
| u87ai | 6 dB (EST from manual polar) | 0.5 (EST) | 0.030 (EST) | FET condenser |
| c414 | 4 dB (EST) | 0.5 (EST) | 0.025 (EST) | FET condenser |
| u67 | 6 dB (EST) | 0.5 (EST) | 0.030 (EST) | tube condenser |
| u47fet | 6 dB (EST) | 0.5 (EST) | 0.030 (EST) | FET condenser |

`k_p` derivation for the SM57: with the source at 6 mm plus capsule depth (r = 18 mm),
+8 dB at 100 Hz for a cardioid requires `omega_p = 2 pi 460 Hz`, so
`k_p = 460 / (343 / (2 pi 0.018)) = 0.15`, rounded to 0.18 within the +6-10 dB range.
