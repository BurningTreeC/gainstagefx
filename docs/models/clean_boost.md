# Clean Boost (MXR MicroAmp) research log

Engineering identity: **MXR M-133 MicroAmp**. Stable ids `pedal_mxr_microamp` (pedal
slot) and `pedal_mxr_microamp_circuit`. Display: **Clean Boost**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::clean_boost`; pedal slot and circuit list),
on the Yellow Dist's standing (a published analysis with a parts list and its own
arithmetic). Its one knob is the slot's **drive**; the level knob is greyed
(`Pedal::has_level`, `PedalControls::level` an `Option`).

**Measured** (`examples/drive_pedals_op.rs`, held by `tests/drive_pedals.rs`): +0.5 dB at
the stop (the op-amp's unity into the 470 / 10 k divider and the load), +5.9 dB at half
rotation -- the reverse-log track's 50 k region, as ElectroSmash describes it -- and +26.2
at full against its 26.2; flat within half a decibel from 50 Hz to 15 kHz.

## Eight-question checkpoint

1. **Revision.** The M-133 as MXR made it and Dunlop reissues it: one TL061, one pot.
   PCB revisions differ in "minor component modifications due to manufacturing component
   resourcing" (the analysis); none is a different circuit.
2. **Original schematic found?** No MXR drawing.
   - Used: [ElectroSmash's "MXR MicroAmp Analysis"](https://electrosmash.mas-effects.com/mxr-microamp),
     schematic and parts list.
3. **Best source.** That analysis.
4. **Cross-check.** Its arithmetic: "Gv max (R5 = 0) = 20.6 (26.2 dB)", "Gv min (R5 =
   500 K) = 1 (0 dB)"; its gain-against-R5 table (21.7 at 0, 2.06 at 50 k, 1.11 at
   500 k -- the op-amp alone); C2's "60.4 kHz", C3's "12.5 Hz" at full gain, C5's
   "1.06 Hz"; "everything occurs between 0 and 50 K", which is why R5 is reverse log.
5. **Signal path and values.**

```text
power: 9 V through D1 1N4001; R7 / R8 100k with C4 1u -> 4.5 V
input: R1 22M to ground; C1 0.1u; R2 10M to 4.5 V; R3 1k -> TL061 +in
gain: R4 56k || C2 47p from out to -in; from -in, R5 500k reverse log (a rheostat,
   wiper to the end nearer C3), R6 2.7k, C3 4.7u to ground
output: C5 15u, R9 470, R10 10k to ground, out
```

6. **To be modeled exactly.** Every part above.
7. **To be approximated, and why.**
   - **TL061** rail-limited at 3 V either way of 4.5 V. APPROXIMATED.
   - **R5's law**: reverse log as the analysis says, built as a C track
     (`Taper::AntiAudio`): in circuit `500 k x audio(1 - p)`, a tenth at half rotation,
     which is the analysis's 50 k region.
   - D1 as the battery's few tenths of a volt: not built (the rail is 9 V).
8. **Why.** One stage, every value, and a gain table to measure against. Its one knob is
   GAIN on the box: the drive knob in the slot, with no level control; selected as the
   circuit it is the Drive knob with `drive_is_channel_volume`, as the Treble Boost's is.
