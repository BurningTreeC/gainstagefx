# American 5150 / existing American 6L6 High-Gain checkpoint

Target: preserve existing model, whose ULTRA labels and separate-channel stack
point to the factory 5150 II service drawing, not an authenticated original block
letter 5150. Revision ambiguity must be resolved before claiming a revised model.
Research 2026-09-14 before routing changes.

- [Factory 5150II service drawing](https://www.thetubestore.com/lib/thetubestore/schematics/Peavey/Peavey-5150-II-Schematic.pdf)
- [Alternate same drawing](https://el34world.com/charts/Schematics/files/Peavey/5150_2.pdf)
- [Original EVH alternative](https://el34world.com/charts/Schematics/files/Peavey/5150_evh.pdf)
- [Factory 5150 II manual](https://assets.peavey.com/literature/manuals/80304643.pdf)
- [RCA 6L6GC](https://frank.pocnet.net/sheets/049/6/6L6GC.pdf)

Existing six-stage lead circuit uses 39k cold cathode, 220k/100k plates,
1.82k/2.2k cathodes elsewhere, ULTRA PRE, 470k R89 feeding a substituted 33k load.
Power has ECC83 LTP, four 6L6GC, .047uF coupling, 220k leaks, 2.2k stoppers,
100-ohm screens, 39k feedback, 10k/.033uF presence and no resonance control.
330V preamp and 470/465/410V output rails, bias -46V, ratio 15.4 and RC reservoirs
are existing assumptions, not newly verified factory measurements.

Checkpoint: existing revision lineage documented; factory sheets found; primary
factory PDF best; alternate scans located, exact sheet/value reconciliation still
incomplete; relevant existing values listed; all old equations preserved exactly;
legacy missing tone/depth and source/load/supply approximations retained; correction
requires separate revision-specific validation. This exposes existing code and does
not claim a newly authenticated hardware implementation.
Tests: frozen multi-rate/level/signal baseline includes this model.
