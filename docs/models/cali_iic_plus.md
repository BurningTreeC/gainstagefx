# Cali IIC+ / existing Cali 6L6 checkpoint

Target: existing stock RP10A C+ 60W interpretation, not ++ or Simul-Class.
Research 2026-09-14, before routing refactor. No component values changed.

Sources searched/read:
- [Archival manufacturer-labelled preamp schematic](https://schematicheaven.net/boogieamps/boogie_mkiic_plus.pdf)
- [RP10 redraw, same document available locally](https://www.scribd.com/document/917436296/RP10-IIC-Schematic)
- [Alternate FINAL redraw](https://cdn.imagearchive.com/boogieforum/data/attach/6/6870-Mark-IIC-Schematic-FINAL-1-.pdf)
- [Manufacturer manual index](https://legacy.mesaboogie.com/support/user-manuals.html)
- [RCA 6L6GC data](https://frank.pocnet.net/sheets/049/6/6L6GC.pdf)

The local RP10/FINAL are community redraws, NOT original factory schematics.
RP10 explicitly states its reference numbers are unofficial. Online FINAL fetch
failed; local existing copy is available. Modern reissue manuals do not establish
1983 circuit values. No fully authenticated original 60W power drawing verified.

Cross-check exposed substantive gaps: RP10 shows lead return/recovery/master
before EQ, while current code connects lead_out directly to the graphic network.
RP10 power drawing shows 82.5k/90.9k PI plates, 100k/150k grid leaks, 22k tail,
250k presence network and -47V 60W bias. Current PowerSpec uses 82k/100k plates,
1M/1M leaks, 10k tail, 5k presence and -44V bias. Its master is 1M resting .30.
The RCA data does not justify code's claim that 3.6k is its stated 450V pair load.
Do NOT claim the existing power spec exactly reproduces this drawing.

Preamp modeled values include 150k/100k/82k/274k plate loads, 1.5k/1.5k/1.5k/
3.3k cathodes, early 250pF/.1uF/.047uF stack, 680k lead resistor, 270k/68k divider,
410V assumed supply. RP10 R9=91k versus RP11A 100k is retained.

DSP: existing nonlinear netlists and RLC graphic preserved bit-for-bit for Matched.
Selection boundary is the existing lead_out -> graphic -> complete power netlist.
This is an implementation boundary; omitted recovery stages mean it is not the
complete physical preamp. Fixed load, transformer/supply constants and scalar EQ
recovery remain APPROXIMATED / EMPIRICALLY TUNED pending a versioned correction.

Eight-question checkpoint: device identified; original preamp located, full exact
power unverified; best source archival scan + revision-specific redraw; cross-check
performed and conflicts recorded; values above; exact preservation of old equations;
legacy approximations retained; necessary to separate routing from sound changes.
Tests: frozen multi-signal/rate/level fixture in tests/legacy_baseline.rs.
