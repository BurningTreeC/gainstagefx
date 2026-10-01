# Oregon T (Sunn Model T) research log

Engineering identity: **Sunn Model T**, the 150 W valve head, in the revision Sunn drew
on 25 September 1973. Proposed stable ids `amp_sunn_model_t` (`circuit`) and
`power_sunn_model_t_6550` (`power_amp`). Proposed display: **Oregon T**.

Status: **IMPLEMENTED 2026-10-01** (`circuits::oregon_t`, power stage
`power::PowerSpec::OREGON_6550`, stable ids `amp_sunn_model_t` and
`power_sunn_model_t_6550`, displayed **Oregon T** and **Oregon 6550**).

## What it is for

*Unknown Garden '94* in [PRESETS.md](../../PRESETS.md): "Kim Thayil's amplifier of choice
for much of *Superunknown* was the Sunn Model T" -- WIDELY REPORTED
([Alto Music's album feature](https://www.altomusic.com/blogs/news/essential-elements-of-essential-classics-soundgarden-superunknown);
[Premier Guitar's Soundgarden rig rundown](https://www.premierguitar.com/rig-rundown-soundgardens-kim-thayil-chris-cornell-and-ben-shepherd)
shows Model Ts in the live rig). Which year's Model T was on the record is not stated in
any source located. The album was tracked in 1993, before Fender's 1998 reissue, so it is
an original.

## Eight-question checkpoint

1. **Revision.** Sunn's drawing **D-1029, revision A, ECN 441, 9/25/1973**: the
   three-12AX7 preamp with BRITE and NORMAL channels, a master (`VOL`) and a presence
   control, four 6550s. Earlier (1970-72) Model Ts differ; the 1998 Fender reissue
   ("Rev B" drawings exist) is a different amplifier and is out of scope.
2. **Original schematic found?** **Yes.** Sunn Musical Equipment Company, "MODEL 'T'",
   D-1029 A, with a voltage chart (no-signal and 150 W rms) and its measurement
   conditions. [schematicheaven.net/newamps/sunn_model_t.pdf](https://schematicheaven.net/newamps/sunn_model_t.pdf);
   a copy is in `docs/schematics/` (git-ignored). Legible value by value at 200 dpi.
3. **Best source.** That drawing.
4. **Cross-check.** The drawing's own **voltage chart**, which is the strongest check
   available: twenty node voltages at no signal and at 150 W into 4 ohms, under stated
   conditions (1 kHz into the BOTH IN jack, both volumes, bass, mid, treble and master
   at 10, presence at 10, input set for 24.5 V rms across 4 ohms). A model has to land
   on those before anything else about it is trusted. The published description of it
   as "inspired by the Fender Bassman 5F6A" is a WIDELY REPORTED characterisation and
   is not a check on values.
5. **Signal path and values** (Sunn's designators).

```text
J1-J5: BRITE in/out, BOTH in, NORMAL in/out
V1 12AX7A, both triodes: R1/R4 68k grid stoppers, R2/R3 1M grid leaks,
   shared cathode R5 680 ohm with C1 250 uF 25 V, plates R6/R7 100k from C (+330 V)
   C2/C3 .022 -> R8/R9 1M (VOLUME, linear) -> R10/R11 470k mixers,
   C4 470 pF across R10 (the BRITE channel's)
V2 12AX7: gain stage (pins 6-8) with R13 820 ohm bypassed, R12 100k plate;
   cathode follower (pins 1-3) with R14 100k
tone: C5 270 pF, R15 250k TREBLE, R16 1M BASS, R17 56k slope, R18 25k MID,
      C6/C7 .022 -> R19 1M VOL (master, linear)
V3 12AX7A long-tailed pair: C8 .022 in, R21/R24 1M, R22 4.7k tail, R23 470 ohm,
   C10 .1 uF, R26 10k; plates R27 82k / R28 120k from B (+400 V); C11 270 pF
   across the plates; R20 22k feedback from the 16 ohm tap; presence R25 25k
   reverse log with C9 .022 uF
C12/C13 .25 uF -> R29/R30 100k to E (bias, -55 V)
4 x 6550: R31-R34 1k grid stoppers, R35/R38/R39/R42 47 ohm 5 W in the plates,
   screens through R36/R37/R40/R41 1.5k 5 W to taps on T1's primary
   (ultralinear -- the tap labels are too small to read on this scan: PLAUSIBLE),
   A+ 500 V (no signal) / 481 V (150 W)
T1 secondary: 16/8/4/2 ohm selector S4; line out R51 33k / R52 680 ohm
supply: CR2/CR3 22-0416, L3/L4 28-3100 chokes, C17/C18 20 uF 600 V, R46/R47 10k
```

6. **To be modeled exactly.** The BRITE channel from J1 (or the jumpered pair from J3,
   which is the BOTH IN jack Sunn itself tests with), V1-V3 with every value above, the
   stack, the master, the long-tailed pair with its feedback and presence, and the four
   6550s with their plate and screen resistors -- a new `PowerSpec`, because no existing
   one has 6550s or ultralinear screens.
7. **To be approximated, and why.**
   - **6550**: the catalogue has no 6550 pentode fit; one has to be made from a data
     sheet before the power stage exists. ESTIMATED until then.
   - **T1**: the ultralinear tap fraction and the transformer's leakage and core are not
     on the drawing. The tap is ESTIMATED (typically 40 %), the rest as for the other
     power stages.
   - The supply chokes and 20 uF reservoirs are on the drawing and can be built; the
     transformer's source resistance is ESTIMATED from the chart's A+ sag, 500 V to
     481 V at 150 W.
8. **Why.** A factory drawing with a voltage chart; the only gaps are a valve fit and an
   output transformer's tap, both of which the chart constrains.

## Before code

- A 6550 fit (`PentodeSpec`) from a data sheet, checked against the chart's no-signal
  plate and screen voltages.
- Decide the tap fraction from the chart's 150 W column rather than by ear.

## Implemented, 2026-10-01

`circuits::oregon_t`, `power::PowerSpec::OREGON_6550`, `dsp::netlist::PentodeSpec::T6550`,
`tools/tube_fit/fit_6550.py`, `tests/oregon_t.rs`, `examples/oregon_t_op.rs`. Re-read at
the scan's native resolution.

### The drawing, as built

- **Preamp**: R1 / R4 68 k, R2 / R3 1 M, V1's two halves on **one** cathode, R5 680 ohm
  with C1 250 uF -- so both halves are built -- R6 / R7 100 k from C; C2 / C3 .022 into
  R8 / R9, 1 M **linear**; R10 470 k with C4 470 pF (BRITE), R11 470 k (NORMAL); V2's
  gain half R12 100 k, R13 820 ohm with an electrolytic printed "22" (unit cut off: 22 uF,
  PLAUSIBLE); the follower R14 100 k; C5 270 pF, R17 56 k, C6 / C7 .022, R15 250 k, R16
  1 M rheostat, R18 25 k (all linear, as Sunn's notes say); **R19 1 M linear, the
  master**, between the treble wiper and C8. Played from BRITE IN with NORMAL's volume at
  zero; `Input::Both` builds the chart's BOTH IN, and NORMAL's volume is a control with
  no panel knob.
- **Power stage**: the inverter, couplings and feedback as question 5. R25 is printed
  "REV LOG" and is a rheostat under C9: a **C taper**, which the solver did not have --
  `Taper::AntiAudio`, `1 - audio(1 - p)`, a tenth of the track left at half rotation.
  The screens' 1.5 k resistors go to taps on the primary (GRN/WHT and GRN), the plates
  through 47 ohm to BLU/WHT and BLU, the centre tap RED to A: **ultra-linear**, a new
  `PowerSpec::screen_tap` built as two more windings on the same secondary.
- **The 16 ohm tap**: R20 22 k is built as drawn, from the 16 ohm tap, and the stage's
  secondary is that tap (the chart's 4 ohm load on the 4 ohm tap is the same load to the
  valves). Built first as 11 k from 4 ohm, which passes the same signal current, it put
  half the resistance from the tail to ground at DC and the inverter's cathodes read
  32.8 V; the chart says 38, and 22 k gives 37.4.

### The 6550

No fit existed. `tools/tube_fit/fit_6550.py` fits Koren's constants to General
Electric's "6550-A" sheet of April 1972 (Frank's tube data sheets): the average
characteristics (250 / 250 / -14 V, 140 mA, 11,000 micromho), the Class A1 row
(400 / 225 / -16.5 V, 87 mA) and the ultra-linear row (450 V on plate and screen at
-48 V, 75 mA). `mu` comes out at 8.18, against the sheet's printed triode amplification
factor of 8. Held out: the ultra-linear row's screen current, 6.05 mA against 6.0; the
cut-off point, 1.5 mA at -40 V against 1.0; the push-pull pentode rows, 70.7 mA against
75 and 41.2 against 50. A first fit to the pentode rows alone extrapolated to 110 mA a
valve at the Model T's idle; that is why the ultra-linear row, the only published point
near a 500 V screen, is in the fit.

### Against the chart

The plate supply is A behind a resistance, fitted to A's two readings with this model's
valves drawing what they draw: **636.5 V behind 279.3 ohm** (`power::OREGON_A_OPEN`,
`OREGON_A_SOURCE`). The tap is 40 % (GE's ultra-linear figure), the transformer 2 k plate
to plate (what GE's row asks of four valves); both ESTIMATED.

| | no signal, model | chart | 150 W, model | chart |
|---|---|---|---|---|
| A | 519.0 | 519 | 481.0 | 481 |
| plate | 514.4 | 507 | 475.0 | 472 |
| screen | 507.3 | 504 | 462.9 | 459 |
| grid | -53.0 | -53 | -60.0 | -55 |
| inverter plates | 274.6 / 257.2 | 271 / 251 | 282.6 / 245.0 | 271 / 251 |
| inverter cathodes | 37.4 | 38 | 37.5 | 30 |
| C (preamp rail) | 358.5 | 351 | | |
| V1 plates / cathode | 235.6 / 1.67 | 227, 232 / 1.71 | | |
| V2 plate / cathode / follower | 207.3 / 1.24 / 207.7 | 203 / 1.3 / 205 | | |

The 150 W column is the power stage driven by a sine to 24.5 V rms across 4 ohm
(referred). The whole amplifier from BOTH IN with every control at 10 reaches it too --
24.46 V rms at 31 mV -- at the top of a narrow window: a little more input and the
inverter's grids charge C8 and the output falls back.

**A finding, recorded rather than tuned.** With GE's bogey valve on the chart's 504 V
screen and -53 V grid, each 6550 idles at about 94 mA of plate current, 48 W against
GE's design maximum of 42. The chart's own drops agree with a hot idle -- 12 V from A to
the plates, and the screens only 3 V below the plates through 1.5 k -- so the drawing's
bias is kept and the heat is noted.

### The solve

Driven far past its rating the stage first went unsettled on every sample and froze at
a DC level (from 300 mV at 96 kHz). The cause was in the pentode, not the circuit: at
its plate's edge the device returned no *screen* current either, an artefact of an early
return, harmless where a screen sits on a capacitor and a Newton two-cycle where it hangs
on a winding. An ultra-linear valve's screen now keeps conducting there
(`device::Pentode::on_winding`, built by `Netlist::pentode_on_winding`). It is not the
default for the other stages: there it changes no solution but moves the Newton path
enough to shift the frozen Twin fixture, so they are untouched. `tests/oregon_t.rs`
drives it past its rating at both rates and every presence and asserts no unsettled
sample.
