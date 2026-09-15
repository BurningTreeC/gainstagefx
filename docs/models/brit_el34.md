# Brit EL34 research checkpoint (2026-09-14)

1. **Target:** Marshall JCM800 2203, 100W EL34 export version, 24-Apr-1981
   drawings; not US 6550, 50W 2204, split-channel 2210 or modern reissue.
2. **Original schematic found:** original Marshall manufacturer drawings,
   [preamp including PI](https://www.drtube.com/schematics/marshall/jcm800pr.gif)
   and [power/PSU](https://www.drtube.com/schematics/marshall/jcm800pw.gif).
3. **Best primary sources:** those drawings and the
   [Marshall manual](https://www.drtube.com/schematics/marshall/jcm800-mv-ld-manual.pdf).
   Search covered revision schematics, service manuals, NFB/presence/PI, tube
   characteristics, transformer impedance, DCR, leakage and magnetizing inductance.
4. **Cross-check:** [1988 manufacturer 2203 drawing](https://el34world.com/charts/Schematics/files/Marshall/Marshall_jcm800_lead_mstvol_100w_2203.pdf)
   independently confirms LTP values, .022uF coupling, 220k leaks, individual 5k6
   grid stoppers and 1k screens, 100k NFB from the 4-ohm tap and 22k/.1uF presence.
   This later revision labels OT C2668; do not identify it as the 1981 transformer.
5. **Key values:** ECC83 inverter 82k/100k plates, 1M grid leaks, 470-ohm common
   cathode resistor, 10k tail, 4k7 lower tail, .1uF undriven-grid coupling, 47pF
   between plates, .022uF input and output coupling. Four EL34 fixed bias -42V.
   Drawing nominal plate 470V, screen 468V, inverter 330V. Master 1M log.
   Presence 22k/.1uF; NFB 100k fed from 4 ohms. No resonance/depth control.
6. **Circuit-derived:** explicit inverter, grid coupling/bias, pentodes, feedback
   and presence in one existing MNA solve, with a complete transformer network.
7. **Approximations:** use existing PentodeSpec::EL34 (Koren parameter fit, not a
   new measured fit); no tube mismatch. Per-side parallel tubes use half of each
   individual stopper/screen resistance (2k8 / 500 ohms). Existing shared primary
   copper approximation retained, so DCR below is not a full winding-loss model.
   RC Thevenin supply replaces rectifier/choke: 100 ohms/50uF plate reservoir,
   100 ohms/50uF screen reservoir (resistances estimated; two 100uF series cans on
   the 1981 drawing give 50uF). Fixed bias source. No RF/heater/power-switch model.
8. **Why:** reuse the stable bounded solver architecture; no source for exact
   vintage transformer parasitics/core or supply impedances was authenticated.
   These limitations preclude claiming an exact 2203 reproduction.

## Transformer and tube references

[Hammond 1750U primary datasheet](https://www.hammfg.com/files/parts/pdf/1750U.pdf)
for a replacement intended for JMP/JCM800 EL34: 1700-ohm CT, 4/8/16-ohm taps,
100W; half-primary DCR 15.36/16.56 ohms; primary L=8.85H and leakage=7.97mH
(test conditions 1kHz/1V). [Manufacturer product table](https://www.hammfg.com/part/1750U)
cross-checks the rating/ratio. This is PUBLISHED-PARAMETER DERIVED replacement iron,
not evidence of the original Dagnall winding. With 4-ohm output the turns ratio is
sqrt(1700/4)=20.615528 and secondary leakage is 7.97mH/425=18.75294uH.

[JJ EL34 data](https://www.jj-electronic.com/images/stories/product/power_tubes/pdf/el34_e34l.pdf)
provides the type's primary technical reference; the existing parametric valve
fit is an approximation, not a digitized match to every curve. Saturating core
knee estimated from 100W into 4 ohms (28.284V peak) at 70Hz; sharpness 6 is an
empirical solver profile. Master rest .30 is a chosen playing position.

Boundary: preamp hands over before the master; this module includes master, PI,
power tubes, bias, NFB/presence, supplies and OT. No new Brit 800 preamp is implied.
Tests must check idle stability, finite hard drive, distinct spectrum vs Cali,
reactive/NFB sensitivity later, and the unchanged Matched fixture.
