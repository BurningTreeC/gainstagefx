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


## 2026-09-17 circuit-completion pass

The Vibrato-channel electrical path is now one MNA network from the switched
High/Low input through V2A, the stock tone/Volume/Bright network, V2B, the dry
3.3 MΩ || 10 pF path, the 500 pF/paralleled-12AT7 reverb driver, V4A recovery,
shared 820 Ω/25 µF V4 cathode network, V4B, the 50 kΩ reverse-audio Intensity
pot/LDR shunt and the 220 kΩ channel mix hand-off. The unused Normal-channel
second triode and its 220 kΩ mix branch remain electrically present so their
shared-cathode and channel-loading effects are not replaced by a guessed shunt.
The spring itself is still a mechanical DSP transmission element; its 4AB3C1B
pickup returns through an independent 2.25 kΩ electrical source port in the same
MNA solve.

The previous split `dry_mix`, `reverb_return`, `mix_recovery`, and standalone
reverb-driver simulations were removed from the active chain. A dynamic-resistor
netlist part now puts the optical LDR into the Newton stamp directly; tremolo is
not a post-stage scalar. The spurious 500 pF tone-stack shunt was removed (the
500 pF part on the Fender drawing is the reverb-send filter). The stock 120 pF
Bright capacitor is now switchable.

The modular power circuit remains separate by design so GainStageFX can switch
power amplifiers. Its Twin spec now uses the original 22 kΩ PI tail, +450 V PI
rail and the shared AB763 +460 reservoir -> 4 H/104 Ω choke -> screen B node ->
1 kΩ -> PI C node supply rather than independent screen/PI sources. The two 70 µF
series reservoir capacitors and 220 kΩ balancing resistors are represented
explicitly.

## 2026-09-20 direct-selection effects state correction

The two pick/chord recovery regressions used `Chain::set_voice(Twin)` rather
than `Chain::apply(Settings)`. Since the unified-channel change, that path left
the electrical Reverb pot at its netlist default of 0.5 even though the chain's
stored Reverb setting was 0.0. The measured tails (0.3283 and 0.4691 peak after
the 250 ms settling window) therefore included the enabled mechanical tank.
Before unification the stored zero bypassed the separate return processing;
after unification only the electrical pot determines its contribution.

Voice selection now synchronizes the stored Reverb/Intensity settings into the
Twin netlist, using the same helper as `apply`. The transformer, spring decay,
recovery triode, shared cathode, circuit values, and test thresholds are unchanged.
`direct_twin_selection_uses_the_same_dry_defaults_as_settings` compares both API
paths sample for sample beyond the tank's first return. The existing preset
measurements continue to check explicitly enabled reverb and tremolo.
