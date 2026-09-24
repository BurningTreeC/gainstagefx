# Cabinet research checkpoint (2026-09-15)

Evidence labels: **DOCUMENTED** (manufacturer/retail specification), **SECONDARY**
(builder/replica/forum statement), **DERIVED** (computed from documented values with a
stated rule), **ESTIMATED** (no source; chosen within physical bounds), **TUNED**
(chosen for behaviour, documented as such).

## Eight-question checkpoint

1. **Exact targets.** Marshall 1960A (angled), 1960B (straight), 1960AX (G12M-25),
   1960AV (G12 Vintage); Mesa/Boogie Rectifier Standard ("oversized") 4x12; Fender
   Twin Reverb combo cabinet (AB763-style, open back); Fender '65 Deluxe Reverb combo
   (open back); Marshall 1936 closed 2x12; Marshall 1912 closed 1x12; plus one generic
   oversized 4x12 with **no** hardware claim.
2. **Primary data found?** Outer dimensions and factory speaker complements: yes, for the
   Marshall and Mesa cabinets. Internal dimensions, baffle hole layout, panel thickness
   for Mesa, back-panel openings for the Fender combos: **no manufacturer drawings found.**
3. **Best sources.**
   - [Marshall 1960A/1960B](https://www.marshall.com/amps/products/cabinets/1960a-and-1960b):
     770 W x 755 H x 365 D mm, 4x G12T-75 16 ohm, 16/4 ohm mono.
   - [Marshall 1960AX/BX](https://marshall.com/amps/products/cabinets/1960ax-and-1960bx):
     same outer dimensions, 4x G12M-25 16 ohm, 100 W.
   - [Marshall 1960AV](https://www.marshall.com/us/en/product/1960av-4x12-angled-cabinet?pid=1007275):
     same outer dimensions, 4x "Celestion G12 Vintage" 16 ohm 70 W.
   - Mesa Rectifier Standard 4x12: 32.9 H x 30.1 W x 14.25 D in, closed back, 4x V30,
     void-free marine-grade Baltic birch (Mesa/Gibson product text via retailers; the
     legacy Mesa page certificate had expired when fetched).
   - Twin Reverb combo cabinet: 20 H x 26-1/8 W x 10-1/2 D in, solid pine shell, Baltic
     birch baffle and back panels (Mojotone AB763-style reproduction specification:
     SECONDARY, not Fender).
   - '65 Deluxe Reverb: 17.5 H x 24.5 W x 9.5 D in, open combo (retail specifications of
     the Fender reissue).
   - Marshall 1936: 750 W x 600 H x 310 D mm; Marshall 1912: about 500 W x 470 H x 290 D mm
     (retail specifications).
4. **Cross-checks.**
   - Marshall ply: two independent secondary sources state **5/8 in (15.9 mm) voidless
     birch**: a builder/admin on 18watt.com, and the description of a replica 1960 plan
     on Scribd. One unsourced search summary claimed 15 mm modern birch; there is no
     primary source for 18 mm.
   - The 1960AV loads "G12 Vintage 70 W" (Marshall's label), a V30-family variant rather
     than the 60 W Vintage 30 the Brit V30 profile describes. Recorded, not resolved.
5. **Values.** Table below.
6. **From physics.** Sealed-box compliance per driver `Cab = Vb_i / (rho c^2 Sd^2)`, stamped
   into the power stage's load (see `speakers.md`). Shared radiation mass of adjacent
   drivers. Driver positions and fractional propagation delay to each microphone. Piston
   directivity. Front/rear path difference of open backs with polarity inversion. Enclosure
   standing-wave frequencies `c / 2L`. Back-panel plate fundamental. Baffle-step corner
   `c / (pi W)`.
7. **Approximated.**
   - Hole layout: equal gaps across the internal width/height, from Celestion's 283 mm cutout.
   - 1960A slant: volume reduced 8 % vs 1960B; the baffle is treated as planar.
   - Leakage Q = 7.
   - Standing-wave and panel levels.
   - Open-back fractions of the Fender combos.
   - Rear-wave diffraction.
   - Baffle-step blending with distance.
8. **Why.** No manufacturer construction drawings were found. The alternative (FEM/BEM or
   impulse responses) is ruled out by the real-time and no-convolution requirements.

## Cabinet table

Internal volume = (W - 2t)(H - 2t)(D - 2t), less 2.5 L per driver (ESTIMATED displacement)
and 3 % bracing (ESTIMATED), and less 8 % for the 1960A slant.

| ID | Display | Outer W x H x D mm | Wall t mm | Drivers | Layout (centres from baffle centre, mm) | Back | Default speaker |
|---|---|---|---|---|---|---|---|
| `cab_marshall_1960a` | Brit 1960 4x12 | 770 x 755 x 365 (DOC) | 15.9 (SEC) | 4 | +-170 x +-168 (DERIVED) | closed, slant (EST 8 %) | Brit T75 (DOC) |
| `cab_mesa_recto_standard` | Cali Oversized 4x12 | 765 x 836 x 362 (DOC) | 19.0 (EST) | 4 | +-168 x +-180 (DERIVED) | closed | Brit V30 (DOC) |
| `cab_marshall_1960b` | Brit Closed 4x12 | 770 x 755 x 365 (DOC) | 15.9 (SEC) | 4 | +-170 x +-168 (DERIVED) | closed | Brit T75 (DOC) |
| `cab_marshall_1960ax` | Brit Green 4x12 | 770 x 755 x 365 (DOC) | 15.9 (SEC) | 4 | +-170 x +-168 (DERIVED) | closed, slant | Brit Green 25 (DOC) |
| `cab_marshall_1960av` | Brit V30 4x12 | 770 x 755 x 365 (DOC) | 15.9 (SEC) | 4 | +-170 x +-168 (DERIVED) | closed, slant | Brit V30 (DOC, 70 W variant) |
| `cab_generic_oversized_412` | Oversized 4x12 | 800 x 850 x 380 (EST) | 18.0 (EST) | 4 | +-172 x +-185 (DERIVED) | closed | Brit T75 (TUNED choice) |
| `cab_fender_twin_open_212` | American Open 2x12 | 664 x 508 x 267 (SEC) | 19.0 (EST) | 2 | +-151 x -30 (DERIVED/EST) | open 40 % (EST) | American Ceramic (TUNED choice) |
| `cab_fender_deluxe_open_112` | American Open 1x12 | 622 x 445 x 241 (DOC retail) | 19.0 (EST) | 1 | 0 x -20 (EST) | open 45 % (EST) | American Vintage 12 (TUNED choice) |
| `cab_marshall_1912` | Closed 1x12 | 500 x 470 x 290 (DOC retail) | 15.9 (EST) | 1 | 0 x 0 | closed | Brit V30 (TUNED choice) |
| `cab_marshall_1936` | Closed 2x12 | 750 x 600 x 310 (DOC retail) | 15.9 (EST) | 2 | +-167 x 0 (DERIVED) | closed | Brit T75 (TUNED choice) |
| `cab_roland_jc120_212` | Jazz Open 2x12 | 750 x 540 x 270 (DOC, Roland) | 18.0 (EST) | 2 | +-166 x -40 (DERIVED/EST) | open 40 % (EST) | Jazz 12 (DOC complement) |

"TUNED choice" defaults are voicing choices for the Matched speaker selection, **not**
claims about factory complements. The AB763 Twin/Deluxe original speaker fitments were not
verified from a primary source.

## DSP mapping (see `CABINET_MODEL.md` for equations)

- Load coupling: closed cabinets pass `Mounting { volume_per_driver, leakage_q, drivers }`
  to the power-stage speaker load. Open cabinets pass no box.
- Radiation, per driver `i` to each microphone: distance, fractional delay, bounded
  near-field attenuation, piston directivity, microphone polar and off-axis response.
  Relative delays are kept; the common minimum delay is removed, so no latency is added.
- Open back: a rear source per driver with inverted polarity, arriving via the nearest
  cabinet edge after the depth path, with diffraction low-pass and open-fraction gain.
- Closed-box standing waves at `c / 2 L_int` (depth, width, height) and the back-panel
  plate fundamental (15.9 mm birch, E = 12.4 GPa, rho = 680 kg/m^3; ESTIMATED material
  constants). The resonance levels are TUNED small (+-1-2 dB).
- Baffle step: low shelf of up to -6 dB below `c / (pi W)`, blended in with distance as
  `d / (d + W/2)`.

## Jazz Open 2x12: the Roland JC-120's cabinet (2026-09-24)

- **Dimensions DOCUMENTED.** Roland's JC-120/JC-160 service notes, fifth edition:
  **750 (W) x 540 (H) x 270 (D) mm without casters**, 28 kg, speakers 30 cm x 2. The 2000
  JC-120UT/JT notes give 760 x 622 x 280 mm and 31.2 kg: the same box on its casters (the
  82 mm difference is the casters). Local copies in `docs/schematics/roland_jc120*.pdf`;
  the older editions are scans without a text layer and were read by OCR.
- **Complement DOCUMENTED.** Two Roland **30-103D** (part 041-019), the Jazz 12; see
  `speakers.md`. This is the one cabinet in the table whose Matched speaker is a factory
  complement read off the manufacturer's own parts list.
- **Open back WIDELY REPORTED.** Owners describe a "fairly shallow, open-back" cabinet
  whose front and rear waves cancel at low frequency, and one documented enclosing it; one
  retail review says closed. Roland's parts list has a single backboard (089-070), and a
  1982 repair write-up (Atomium Amps) describes a stapled cabinet with an MDF back panel,
  so the back is part covered. **How much is open is ESTIMATED at 40 %**, as for the
  Fender Twin's cabinet.
- **Driver centres DERIVED/ESTIMATED.** Horizontally by this log's equal-gap rule from a
  283 mm cutout across 714 mm inside: +-166 mm. Vertically 40 mm below centre, half the
  control strip across the top of the front (ESTIMATED from photographs).
- **Panel thickness ESTIMATED** at 18 mm.

The one published measurement of the cabinet (the speaker log's) is near-field, so it
says little about the open back; the open fraction stays an estimate.
