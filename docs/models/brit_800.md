# Brit 800 (Marshall JCM800 2203 preamp) research log

Engineering identity: **Marshall JCM800 Lead Series 2203, 100 W master volume**, preamp
section per the original Marshall drawing dated 24-4-81 ("MASTER VOLUME PREAMP, used on
2103, 2203, 2104, 2204"). Stable id `amp_jcm800_2203` (the `circuit` parameter, appended).
Display: Brit 800. Matched power stage: Brit EL34 (`power_2203_el34`, see
[brit_el34.md](brit_el34.md)).

Status: **IMPLEMENTED 2026-09-15** (`src/circuits/brit800.rs`, `Gain::Brit800`,
`Circuit::Brit800`), tested in `tests/brit800.rs`.

## Eight-question checkpoint

1. **Revision.** 1981 2203 drawings; HIGH input with the LOW jack empty.
   This is not the 2204 (50 W), not the standard-lead 1959/1987 preamp on the upper half of
   the same sheet, and not the 1988 board revision (whose one addition is noted below).
2. **Original schematic found?** Yes. Marshall manufacturer drawings:
   - [preamp](https://www.drtube.com/schematics/marshall/jcm800pr.gif), which carries the
     manufacturer's own average-voltage table;
   - [power/PSU](https://www.drtube.com/schematics/marshall/jcm800pw.gif);
   - the project-local `docs/schematics/jcm_800_2203.pdf`, a redraw with measured voltages.
3. **Best source.** The 1981 drawings above.
4. **Cross-check.**
   - Marshall's [2203 preamp drawing, issue 4, 19-5-88](https://el34world.com/charts/Schematics/files/Marshall/Marshall_jcm800_lead_mstvol_100w_2203.pdf):
     every signal-path value agrees with the 1981 sheet. It carries reference designators,
     which the code uses.
   - The local redraw agrees on topology and on all resistors and capacitors. It differs
     on the treble pot (250 k vs 220 k) and treble cap (500 pF vs 470 pF), and gives the
     middle pot as 25 k.
5. **Signal path and values.**

```text
HIGH jack -- R2 1M to ground -- R3 68k -- V1 grid (ECC83)
  V1: R4 100k plate to node 5; R1 2k7 cathode with C1 .68 uF
  plate -- C3 .022 uF -- (LOW jack switch) -- R5 470k || C4 470 pF -- VR1 Preamp Volume 1M log
           C5 1 nF from the top of VR1 to its wiper (bright); wiper -> V2 grid
V2: R7 100k plate to node 5; R6 10k cathode, unbypassed
  plate -- C7 .022 uF -- R10 470k || C8 470 pF -- V3 grid; R11 470k grid to ground
V3: R12 100k plate to node 6; R9 820 ohm cathode, unbypassed
  plate direct -> V4 grid (cathode follower)
V4: plate to node 6; R13 100k cathode to ground
  tone stack from the V4 cathode:
    C10 470 pF -> VR3 Treble 220k lin (top); wiper -> Master Volume 1M log -> power stage
    R15 33k -> slope node; C11 .022 uF -> Treble bottom / VR5 Bass 1M top (variable resistor)
    Bass bottom -> VR4 Middle 22k lin top; Middle bottom to ground; C12 .022 uF -> Middle wiper
supplies: node 6 -- 10k 1W -- node 5 (R8), each with 50 uF (C21 50+50); node 6 fed from node 10 via 10k 1W
```

   **Resolved conflicts.**
   - Middle pot. The 1981 sheet reads "2?K LIN". The 1988 Marshall drawing prints
     **22k lin**, and that value is used. The redraw's 25 k is recorded.
   - Treble pot. Both Marshall drawings give 220k lin (the 1981 value is partly legible
     and agrees with 1988); used. The redraw's 250 k is recorded.
   - Bass pot law. The 1988 drawing prints **1M log**; the 1981 law is illegible. The
     1988 law is used.
   - The 1988 drawing adds C2 100 pF across the first stage (plate to cathode). It is
     not on the 1981 sheet and is not built.
   - Rails. The 1981 sheet's own table ("all voltages are average only", 100 W
     master-volume column) gives node 5 = 280 V, node 6 = 290 V and node 10 = 330 V.
     This **replaces** the secondary forum measurement (286/293 V) the first checkpoint
     used.
6. **Exactly modeled.**
   - All four triode sections, with coupling, divider, bright and cathode networks.
   - The cathode follower driving the passive stack, in one nonlinear netlist, so grid
     current at V4 loads V3's plate.
   - Both droppers and 50 uF reservoirs from the drawing, so preamp current sets the rail
     nodes.
   - The handoff at the treble wiper into the power stage's master pot (1 M load).
7. **Approximated.**
   - Node 10 is an ideal 330 V source (the table value). The power supply and phase
     inverter behind it are not in this netlist.
   - LOW jack and jack-switch contact resistance are omitted.
   - The ECC83 is the catalogue's generic Koren fit.
   - The master pot's load on the stack is a fixed 1 M. The pot itself is in the power
     netlist, joined through the project's fixed-impedance handoff, as for the other
     amplifiers.
8. **Why.** Separate preamp and power netlists are the project's cost structure (see
   `voice::build_power`). The table voltages are the manufacturer's own reference for the
   operating point.

## Verification (`tests/brit800.rs`)

Operating point against the 1981 table:

| Node | Table | Model |
|---|---|---|
| 5 | 280 V | 282.3 V |
| 6 | 290 V | 291.6 V |
| V1 plate / cathode | 210 V / 1.75 V | 216.1 V / 1.8 V |
| V2 plate / cathode | 250 V / 2.6 V | 255.9 V / 2.6 V |
| V3 plate / cathode | 165 V / 1.0 V | 170.4 V / 1.0 V |
| V4 cathode | 167 V | 170.7 V |

Also tested:
- the stack controls move their bands in the labelled direction;
- the Preamp Volume spans more than 30 dB, and the bright cap tilts low settings;
- the preamp exceeds 15 % distortion at a guitar's level (0.122 V) with the volume full
  (calibrated figure: 24.7 % through the Brit EL34 stage);
- Matched resolves to Brit EL34;
- realtime safety (no allocation) at 44.1–192 kHz.
