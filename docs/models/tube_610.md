# Tube 610 (Universal Audio 610-A modular console preamplifier) research log

Engineering identity: **Universal Audio "Modular Amplifier Model 610-A"** (Bill Putnam's 610
console, early 1960s). Stable id `pre_ua_610a` (`circuit` parameter). Display: Tube 610.

Status: **IMPLEMENTED 2026-09-15** (`src/circuits/tube610.rs`), tested in `tests/tube610.rs`.

## Eight-question checkpoint

1. **Revision.** 610-A as drawn on UA drawing C-10068. Not the modern 2-610/SOLO/610/LA-610
   reissues (12AX7 + 12AT7, different EQ, output stage and input transformer; manual says
   "Variable negative feedback is applied to both of these stages"), and not the 610-B.
2. **Original schematic found?** Not UA's original (UA has a no-schematics policy).
   Found: **John Hinson's redraw "Modular Amplifier Model 610-A — Redrawn by John Hinson from
   C-10068"** (created 2003). Hinson traced it for UA from Putnam's originals (GroupDIY).
   - Located via the Gearspace thread "UA 610 Schematic" (link to recordingjunkie.com,
     now 404).
   - Retrieved from the Internet Archive capture of 19 March 2013:
     `web.archive.org/web/20130319120253/http://www.recordingjunkie.com/Documents/TechSpecs/Pre-Amps/UniversalAudio/610-A.pdf`.
   - The same drawing is attached on GroupDIY ("610-A UA 610 Preamp Schematic.pdf",
     members only) and Scribd ("Modular Channel Amplifier 610-A Schematic").
3. **Best source.** Hinson's redraw of C-10068.
4. **Cross-check.**
   - [UTC 1963 catalogue](https://bitsavers.trailing-edge.com/components/utc/1963_UTC_Catalog.pdf):
     O-1 "Low imp. to grid", primary 50, 200/250, 500/600 ohm; secondary 50,000 ohm; ±1 dB
     30-20,000; +8 dBm; primary resistance 52 ohm.
   - [UTC data chart](https://bunkerofdoom.com/xfm/UTCCHART/utc_DATA.pdf): O-1 secondary DC
     resistance 3900 ohm.
   - [UTC terminal arrangements, 1952](https://www.worldradiohistory.com/Archive-Catalogs/Miscellaneous-Manufacturers/UTC-Terminal-Arrangements-1952.pdf):
     O-1 "500 ohms connect to 1 and 5 (3 is center tap). 200 ohms connect to 2 and 4. 50
     ohms connect to 3 and 4"; output "Connect grid to 7; grid return to 6". The drawing
     wires the mic to 3-4 and the grid to 7, so the **50 ohm winding** is used.
   - [RCA 12AY7 tentative data, 1953](https://frank.pocnet.net/sheets/049/1/12AY7.pdf):
     250 V, -4 V, μ 40, rp 22,800 ohm, gm 1750 µmho, 3 mA; -11 V for 10 µA.
   - GroupDIY threads (UA 610 help thread; UTC 5946 thread): builders measured gain "a bit
     north of 60dB"; "around the 275V range" for B+; PA-5946 is "30k ct : 600R", "a gapped
     7:1". A rewind measured 98 H / 826 ohm primary and 118 ohm secondary.
   - UA 2-610 manual: maximum gain 61 dB, maximum output +20 dBm (reissue).
5. **Signal path and values (drawing).**

```text
MIC -- S1 -- R5 47R -- T1 UTC O-1 pins 3-4 (50 ohm); sec 7 -> V1-A grid, 6 gnd; R26 1M2
V1-A 12AX7: R6 270K plate from C1-B node; R27 4K7 1W cathode, unbypassed
  plate -- C3 .047 -- V1-B grid; R28 1M2
V1-B 12AX7: R8 100K 1W plate from C1-A node; R10 1K 1W cathode, C8 .002uF
  plate -- C4 .047 -- X; X -- R11 68K -- Y; Y -- R9 39K -- V1-A cathode (feedback)
  Lo/Hi Gain switch: common -> C14 .2uF -> LF switch lower deck; Lo = Y, Hi = X
LF lower deck 0: to R13 250K (Level) top; -6: via C13 .0047 || R25 270K
V2-A 12AY7: grid = R13 wiper; R15 4K7 cathode; R14 470K plate from C2-B node
  C15 82pF from V1-B cathode to V2-A cathode (as drawn)
  plate -- C5 .047 -- V2-B grid; R16 1M2
V2-B 12AY7: R18 560R cathode; plate -> T2 UTC PA-5946 primary (30K ohm, L = 55HY min) -> B+
  feedback: V2-B plate -- C6 .047 -- R22 220K -- R21 270K || C9 .0047 -- V2-A cathode
            LF upper deck (-6 and 0) shorts R21/C9; +6 open
            HF: R23 100K -- C10 100pF -- C12 47pF -- C16 .01 -- C11 .01 || C17 .002 -- R24 2K2 gnd;
                S2 connects V2-A cathode to those nodes (-6, +3, +6); 0 open
  echo send: V2-B plate -- C7 .047 -- R19 82K -- R20 10K pot -- R33 10K -- Echo Out
supply: B+ (C10 10uF) -- R17 10K -- C2-B -- R12 10K -- C1-A -- R7 82K -- C1-B
T2 secondary 600 ohm to Line Out; tertiary not used on 610
```

6. **Exactly modeled (EQ switches at 0, gain switch at Hi, MIC input).** Every resistor and
   capacitor above in the active path, both feedback loops, the HF network hanging off
   R22, the echo-send load, the supply decoupling, the O-1 50 ohm connection and
   secondary resistance, and T2's 55 H and 7.07:1.
7. **Approximated and estimated.**
   - B+ 275 V (secondary, builder's statement); supply impedance 10 ohm.
   - O-1, ESTIMATED to UTC's figures:
     - primary copper on the 50 ohm tap: 16 ohm;
     - magnetising inductance 0.9 H and knee 0.021: −0.58 dB at 30 Hz and 1.2 % at +8 dBm;
     - secondary capacitance 40 pF: −0.61 dB at 20 kHz.
   - PA-5946: copper 826/118 ohm (rewind measurement, not original); knee 0.07 on the
     secondary, ESTIMATED.
   - 12AY7: Koren constants fitted exactly to the RCA points (`tools/tube_fit/fit_12ay7.py`).
   - Lo/Hi gain switch fixed at Hi; EQ fixed at 0/0; the Level control is the Drive knob.
   - Load 600 ohm (the transformer's design line). Source 50 ohm (the winding the drawing
     uses).
8. **Why.** Only a redraw exists publicly. Transformer internals are not published.

## Documented doubts about the redraw

A GroupDIY reviewer (thread "M610 Mic Pre amp Schematic") wrote: "the schemo is flawed. The
Lo Gain/Hi Gain switch should just short R11 and C14 go permanently to junction of C4-R11. I
seriously doubt C15 going from V1B cathode to V2A cathode." The model follows the drawing:
- At Hi, both readings take the signal from X (C4-R11).
- C15 is built as drawn. At 82 pF against kilohm cathodes it is inaudible: it sets a
  −79 dB floor at Level 0.

## Measured (`tests/tube610.rs`)

- Operating point:
  - plates V1-A 148 V, V1-B 153 V, V2-A 86 V, V2-B 270 V;
  - V2-B cathode current 5.7 mA.
- Gain 69.8 dB at full Level (builders: "a bit north of 60dB").
- Response 20 Hz +0.7 dB, 30 Hz +0.2, 10 kHz −0.3, 20 kHz −1.4.
- 0.54 % THD at 3 mV in at full Level, second harmonic dominant.
