# American Twin / existing American 6L6 Clean checkpoint

Target existing AB763-inspired Vibrato channel, not later master/ultralinear Twin.
Research 2026-09-14 before routing changes.

- [Original Fender schematic and layout](https://schematicheaven.net/fenderamps/twin_reverb_ab763_schem.pdf)
- [Alternate manufacturer scan archive](https://el34world.com/charts/Schematics/Files/Fender/Fender_Schematics.htm)
- [ECC81 manufacturer data](https://www.jj-electronic.com/images/stories/product/preamplifying_tubes/pdf/ecc81.pdf)
- [RCA 6L6GC](https://frank.pocnet.net/sheets/049/6/6L6GC.pdf)

Downloaded and visually read original schematic and layout. Cross-check agrees on
12AT7 LTP, four 6L6GC, 820-ohm feedback, 100-ohm divider, 82k/100k plates, 470-ohm
screens, 1.5k grid stoppers, 220k grid leaks, -52V bias, 460/458V rails, 125A29A OT.
Important discrepancies: original 22k PI tail and 450V PI rail vs code 10k/410V;
reservoir drawing 70uF+70uF series vs code 200uF; code synthetic master and absent
full reverb/dry recovery network. The 500pF on drawing is a reverb-send coupling
capacitor, not the code's middle-pot shunt. Existing comments overstate fidelity.

No numerical corrections in this checkpoint. Reuse entire power netlist, keep
existing channel/effects order and calibration for compatibility. Source/load
separation, assumed transformer/core and RC sag remain approximations.

Checkpoint answers: AB763; original found; original Fender sheets best; schematic
and layout cross-checked; key values above; old DSP preserved exactly; approximated
legacy sections above; deferred correction avoids unversioned session sound change.
Tests: multi-rate/level/signal frozen baseline including the Twin.
