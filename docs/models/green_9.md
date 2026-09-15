# Green 9 (Ibanez TS9 Tube Screamer) research log

Engineering identity: **Ibanez TS9 Tube Screamer**. Stable id `pedal_ts9` (pedal slot).
Display: Green 9.

## Checkpoint (2026-09-15, before implementation)

1. **Revision.** TS9 as described across its production: the TS808 circuit with a
   different output buffer network. It is not the TS9DX, not the TS10 (extra follower,
   1 k at pin 3) and not the TS808 reissue.
2. **Original schematic?** No manufacturer TS9 drawing was retrieved.
3. **Best source.**
   [R. G. Keen, "The Technology of the Tube Screamer"](http://www.geofex.com/article_folders/tstech/tsxtech.htm).
   Its model table gives output series / shunt resistors of 808: 100 ohm / 10 k and
   9, 9RI, 10, 5: 470 ohm / 100 k. The conversion instructions ("replace them with a 100 ohm
   and a 10K. Bang, instant 808.") match that table.
4. **Cross-check.** Builder comparisons found by search (PedalPCB forum, TDPRI) state the
   same two resistors and the op-amp type as the only differences.
5. **Values.** `ts808::TS9` = `ts808::TS808` with `out_series = 470 ohm`, `out_shunt = 100 k`.
   Everything else follows the verified TS808 (see `green_808.md`).
6. **Modeled exactly.** The full TS808 netlist topology with those two values.
7. **Approximated.** The op-amp type difference (TS9 production used different
   op-amps) is not represented: the op-amp model is ideal with a rail. JFET switching is
   omitted, as in Green 808.
8. **Why.** The netlist op-amp has no slew or bandwidth model yet (the same gap as the RAT
   in `MODEL_INVENTORY.md`).

Tests: `tests/pedal_slot.rs::revisions::green_9_is_its_own_circuit_but_still_a_tube_screamer`.
