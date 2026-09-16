# Big Muff (Electro-Harmonix Big Muff Pi, 1973 Ram's Head) research log

Engineering identity: **Electro-Harmonix Big Muff Pi, 1973 "violet" Ram's Head (V2
family)**. Also defined but not selectable: 1971 Triangle (V1) and Colorsound Supa
Tonebender voicings. Stable id `bigmuff` (existing `circuit` value) and
`pedal_bigmuff_ramshead` (pedal slot). Display name: **Ram Fuzz** (owner request 2026-09-15), after
the collector name of the 1973 circuit; the stable ids are unchanged.

## Checkpoint (2026-09-15, routing only)

1. **Revision.** Ram's Head V2, 1973 violet-era values as encoded in
   `bigmuff::RAMS_HEAD`:
   - 12 k collectors, 470 k feedback with 470 pF;
   - 0.1 uF clip and coupling caps, 10 k base resistors, 560 ohm sustain foot;
   - tone 33 k + 4 nF / 33 k + 12 nF;
   - output 390 k / 10 k / 2.7 k.
2. **Manufacturer schematic?** None published. The accepted primary evidence is traced
   units.
3. **Best source.** Kit Rae's traced-schematic archive,
   [Big Muff Pi versions and schematics](http://www.bigmuffpage.com/Big_Muff_Pi_versions_schematics_part3.html)
   and the preceding parts. It documents V1-V13 with many Ram's Head variants as schematic
   images. The netlist's "eighteen traced variants" statement matches the scope of this
   archive.
4. **Cross-check.** *Not performed value-by-value this session.* The archive pages were
   reached, but their values are in images, and the model is being routed, not modified.
5. **Risk recorded.** Kit Rae's pages show Ram's Head variants with different
   tone-stack resistors (e.g. 39 k) and base resistors (8.2 k vs 10 k). "The" Ram's Head
   is therefore a choice of one traced unit. Which traced unit `RAMS_HEAD` matches must be
   confirmed before any modification.
6-8. Modeling, approximation and justification are as in `src/circuits/bigmuff.rs`. No
   change is made here.
