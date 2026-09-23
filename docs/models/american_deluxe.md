# American Deluxe (Fender Deluxe Reverb AB763) research log

Engineering identity: **Fender Deluxe Reverb-Amp, circuit AB763**, the blackface
1964-1967 production circuit. Proposed stable id `amp_fender_deluxe_ab763`
(`circuit`) plus power stage `power_fender_deluxe_6v6`. Proposed display:
**American Deluxe**, beside the existing American Twin.

Status: **RESEARCHED 2026-09-22, CLEARED FOR CODE.**

## Sources

- **Original Fender drawing (authority).** `DELUXE REVERB-AMP AB763`, Fender
  Electric Instrument Company, Fullerton. Two sheets, LAYOUT and SCHEMATIC,
  downloaded to `docs/schematics/fender_deluxe_reverb_ab763.pdf` from
  [el34world](https://el34world.com/charts/Schematics/files/fender/Fender_DELUXE_REVERB_AB763.pdf).
  Both sheets were rendered at 600 dpi and read value by value.
- Same drawing mirrored at
  [schematicheaven](https://schematicheaven.net/fenderamps/deluxe_reverb_ab763_schem.pdf)
  and [prowessamplifiers](https://www.prowessamplifiers.com/schematics/fender/deluxe_reverb_ab763_schem.pdf).
- Supporting analysis only, never the value authority:
  [Rob Robinette, AB763 model differences](https://robrobinette.com/AB763_Model_Differences.htm).
- Tube data: the ECC83/ECC81 sheets already cited in
  [american_twin.md](american_twin.md); 6V6GT and GZ34 sheets to be cited when the
  power stage lands.

## Eight-question checkpoint

1. **Revision.** AB763. The drawing carries the model designation in its own
   title block, the patent notice (`#2817708`, `#2973681`, design `#192859`) and
   the export transformer table `TR1-125P33A / TR1-125P23B / TR2-125C3A /
   TR3-125A1A / TR4-125A20B`. Not AA763, not AB868, not the '65 reissue.
2. **Original schematic found?** **Yes**, both sheets, legible at 600 dpi.
3. **Best source.** The Fender sheet itself.
4. **Cross-check.** The layout sheet agrees with the schematic on every value
   read so far, and carries the factory node voltages independently
   (`+415 V`, `+420 V`, `+180 V`, `+170 V`, `+87 V`, `+77 V`, `+2.1 V`,
   `+1.3 V`, `+345 V`, `-35 V`, "values shown + or - 20 %"). Robinette's
   comparison table agrees on the model-level differences from the Twin.
5. **Signal path and values.** DOCUMENTED unless marked.

   **Both channels, input and first stage.** Jacks 1/2, two 68 k grid stoppers,
   1 M to ground; 7025 with 100 k plate load and a 1.5 k cathode bypassed by
   25 µF/25 V. Normal plate `+180 V`, Vibrato plate `+170 V`.

   **Tone stack (both channels, identical).** 100 k slope resistor, 250 pF
   treble capacitor, 250 kΩ audio Treble, 0.1 µF bass capacitor, 250 kΩ audio
   Bass, 0.047 µF mid capacitor and a **fixed 6.8 kΩ mid resistor** — the Deluxe
   has no Middle control, which is the main tone-network difference from the
   Twin already modelled here. Volume 1 MΩ audio. The Vibrato Volume carries a
   **47 pF** bright capacitor; the Twin's is 120 pF and switched.

   **Vibrato channel second stage and reverb mix.** 0.02 µF into a 7025 triode
   with an 820 Ω cathode and 25 µF bypass; reverb driver is a **12AT7** with a
   1 M grid resistor, 2.2 k cathode bypassed 25 µF, `+410 V` plate and `+8.7 V`
   cathode into TR4; the tank returns to a 7025 recovery triode with a 220 k
   plate at `+170 V`, 0.003 µF coupling, 100 k, the **100 kΩ linear** Reverb
   control, 470 k and the 220 k/220 k channel mix. A 10 pF sits across the
   3.3 MΩ dry path.

   **Tremolo.** 12AX7 phase-shift oscillator: 3 MΩ reverse-audio Speed, 100 k,
   a 0.01/0.02/0.01 ladder, 2.2 M, 1 M, 1 M, 2.7 k cathode with 25 µF, plate
   supply from `+345 V`; it drives an opto-coupler whose photoresistor shunts
   the signal through the **50 kΩ reverse-audio** Intensity control and 10 k.

   **Phase inverter.** 12AT7 long-tailed pair, 100 k and 82 k (5 %) plate
   loads, 470 Ω shared cathode into a 22 k tail, 1 M/1 M grid resistors,
   0.001 µF and 0.1 µF/200 V coupling capacitors, `+75.5 V` and `+77 V`
   cathode/grid nodes, `+325 V` supply.

   **Power stage.** Two 6V6GT, fixed bias, 220 kΩ (5 %) grid leaks, 1.5 k
   screen resistors, 470 Ω 1 W, plates at `+415 V`, screen node `+420 V`,
   TR3 = **125A1A**, 6.6 k : 8 Ω, one speaker. Negative feedback 820 Ω with a
   **47 Ω** divider leg (the Twin uses 100 Ω) — PLAUSIBLE pending a dedicated
   600 dpi read of that corner during implementation.

   **Supply.** GZ34 rectifier, 330 VAC × 2, standby switch, four 16 µF/450 V
   reservoirs down a 10 k/10 k dropper chain with the TR2 choke. Bias supply:
   470 Ω, one diode, 25 µF/50 V, 10 kΩ linear bias pot, `-35 V`.
6. **To be modelled exactly.** Both channels' input jacks, first stage, tone
   stack and Volume; the Vibrato second stage, reverb send/recovery and mix; the
   opto-coupler shunt as a dynamic resistor in the Newton stamp (as the Twin
   already does); the 12AT7 LTP; the fixed-bias 6V6GT pair, 125A1A and the NFB
   loop; the GZ34 and reservoir chain as the existing `circuits::power` supply
   model does it.
7. **To be approximated.** The spring tank stays a mechanical DSP transmission
   element reusing `dsp::spring::Tank`, as in the Twin — the electrical loading
   is modelled, the mechanics are not a netlist. The opto-coupler's lamp thermal
   lag and photoresistor law are APPROXIMATED from the existing Twin LDR model
   until a measured NSL-type curve is sourced. Speaker: see below.
8. **Why.** The Twin already proves this exact architecture solves in realtime
   in this engine, so the Deluxe is mostly a revaluing of a proven netlist plus a
   genuinely different power stage (two 6V6 at 22 W behind a tube rectifier
   against four 6L6 at 85 W behind a solid-state one). That power/supply
   difference is the entire reason to have both.

## What is reused rather than rebuilt

`circuits::twin` is already an AB763 Vibrato channel with the switched High/Low
jacks, the 68 k grid stoppers, the stock tone/Volume/Bright network, the
reverb send and recovery, the shared V4 cathode, the LDR shunt and the 220 kΩ
channel-mix hand-off. The Deluxe is the same topology at different values, minus
the Middle pot, plus a second (Normal) channel. The MNA netlist builder, the
triode model, the spring tank, the optical tremolo, the transformer/core model
and the supply chain in `circuits::power` are all reused unchanged.

## Speaker and cabinet

**Deliberately not asserted.** Blackface Deluxe Reverbs shipped with more than
one driver over the period (Jensen C12N and Oxford 12K5 are both period-correct
depending on date and order). The modern '65 reissue's Jensen C12K is **not** the
AB763 speaker and will not be substituted for it. The cabinet is an open-back
1x12. Until a legally usable measurement exists, the acoustics path gets a
documented ESTIMATED 1x12 open-back configuration built the way
[speakers.md](speakers.md) and [cabinets.md](cabinets.md) already build them, and
the choice of driver is recorded as an open question rather than a fact.

**Settled 2026-09-22.** `CabinetProfile::AMERICAN_OPEN_112` already carried the
box; what it carried as its `default_speaker` was the *alnico* P12R, which is
not a blackface Deluxe part at all. It is now the ceramic **C12N** —
`SpeakerProfile::AMERICAN_CERAMIC`, `spk_jensen_c12n` — which is what the Twin's
own AB763 cabinet in this repository already defaults to, and one of the two
drivers this amplifier actually shipped with. The Oxford 12K5 stays an open
question and the '65 reissue's C12K stays excluded. Both shipped presets follow
the cabinet. Labelled WIDELY REPORTED.

## Progress

**2026-09-22 — the 6V6GT is modelled.** `PentodeSpec::T6V6GT` in
`dsp::netlist`, fitted by `tools/tube_fit/fit_6v6gt.py` against RCA's Class A1
row and General Electric's 1955 Class A row, holding `ex` at the catalogue's
1.35 and `mu` at the published screen amplification factor of 10. Five targets
met to better than 1.5 %, and RCA's RC-30 Class AB1 push-pull row -- held out of
the fit entirely -- comes back at 35.02 mA against a published 35.0.

**2026-09-22 — the power stage is built and tested.** `PowerSpec::DELUXE_6V6`
in `circuits::power`, every value read off the Fender drawing at 600 dpi.

The phase inverter turned out to be *identical* to the Twin's, component for
component -- 0.001 uF in, 1 M/1 M leaks, a 470 ohm cathode, a 22 k tail, 0.1 uF
across to the undriven grid, 82 k driven and 100 k undriven plates. The two
models differ exactly where Robinette's comparison says they do and nowhere
else: the feedback divider leg is 47 ohms against the Twin's 100, the inverter
supply is +325 V against +450 V, and behind it there is one 6V6GT a side at
+415 V with 1.5 k stoppers and 470 ohm screens into a 125A1A (6.6 k : 8 ohm),
fed by a GZ34 off a 16 uF reservoir. That the Twin's independently-read values
agree everywhere they should is the strongest cross-check available here.

`tests/deluxe_power.rs`, three tests, all passing:

| check | result |
|---|---|
| idle rail against the drawing's +415 V (stated +/- 20 %) | settles at **395.4 V** |
| published 22 W into 8 ohms | **24.2 W** at clipping onset, 29 W into hard clipping |
| solves at 44.1/48/88.2/96/192 kHz | yes |

ESTIMATED and labelled as such in the code: the 125A1A's copper, magnetising
inductance and leakage (no winding data located), and the mains transformer's
secondary resistance. The GZ34 in front of it is modelled as the valve it is,
so the sag is its own law rather than a number.

**2026-09-22 — the channel netlist is built and tested.** `circuits::deluxe`,
the complete Vibrato channel as one MNA network: switched High/Low jacks, the
first stage, the stack, the Volume with its permanent 47 pF, the second stage,
the 500 pF reverb send into the paralleled 12AT7 and TR4, the recovery stage,
the mixer and the optical tremolo. The spring is the only deliberate break, as
on the Twin.

Two things the drawing settled that could not have been guessed:

* the boxed letters are **shared cathodes**, not supply nodes. [A] joins the two
  channels' second stages on one 820 Ω / 25 µF network; [E] joins the reverb
  recovery stage to the mixer on another. Both are in the netlist, and the quiet
  Normal channel stays present so its bias current and its 220 kΩ load on the
  mixer are real.
* the tremolo hangs on the mixer plate through **220 kΩ then 0.1 µF**, where the
  Twin couples first and mixes after. The mixer plate *is* the node the phase
  inverter's 0.001 µF couples from.

It reduces to the same shape as the Twin -- 37 MNA unknowns, an 18-node Newton
boundary and 19 internal -- which is what the same circuit at a different size
should do.

`tests/deluxe.rs`, six tests. The DC check is against the drawing's own printed
voltages at the drawing's own stated tolerance:

| node | model | drawing | error |
|---|---:|---:|---:|
| `v1_p` | 193.4 V | 170 V | +13.8 % |
| `v2_p` | 197.3 V | 180 V | +9.6 % |
| `recov_p` | 194.2 V | 180 V | +7.9 % |
| `v4b_p` | 187.8 V | 180 V | +4.3 % |
| `v1_k` | 1.38 V | 1.3 V | +6.2 % |
| `shared_a_k` | 1.44 V | 1.3 V | +11.1 % |
| `shared_e_k` | 1.39 V | 1.3 V | +7.2 % |
| `drv_k` | 7.63 V | 8.7 V | -12.3 % |

All inside the sheet's "+ or - 20 %". The cathodes are the sharper of the two
kinds of check, because they are the valve's own idle current across a known
resistor and so test the tube model rather than the supply estimate.

Also checked: the stack scoops the middle and has no control that can fill it
(five controls, no Middle); Treble moves 6 kHz by 19 dB and Bass moves 80 Hz by
15 dB; the Low jack is 6.55 dB below the High one; Reverb and Intensity both
reach the circuit; and it solves at 44.1/48/88.2/96/192 kHz.

DERIVED and labelled in the code: `PI_STANDING_LOAD`. The phase inverter sits on
the C node in the amplifier and in the power module here, so its idle current is
missing from the dropper chain; without it every plate below sat about forty
volts high. It is modelled as the load it is, DC only -- the node has 16 µF
across it.

**2026-09-22 — selectable, calibrated and presetted.** `Gain::Deluxe` and
`Circuit::Deluxe` appended (never inserted), display **American Deluxe**, id
`amp_fender_deluxe_ab763`; `PowerModel::AmericanDeluxe6V6` and the matching
`PowerAmp` override, so the 6V6 stage is both the matched default and available
behind other preamplifiers. `POWER_TRIM_DB` widened to eleven columns for it.
Drive is the channel Volume and is labelled VOLUME, as on the Twin; Middle is
greyed out because the amplifier has no Middle control.

The Twin's spring-tank, optical-tremolo and switched-jack handling was a dozen
`self.voice == Gain::Twin` tests. They are now one `Gain::ab763()` descriptor
carrying the slot and control numbers, which both amplifiers share. It also
states the difference honestly: the Deluxe's `bright` is `None`, because its
47 pF is soldered in and the panel has no switch, so the Bright control is
correctly inert there.

**The tremolo was inaudible, and the fix is a decision rather than a reading.**
Reported from a DAW -- "I cannot hear the Tremolo of the Deluxe" -- and it was
real. Measured through the whole chain at full Intensity, peak to trough of a
steady tone: the Twin **7.65 dB**, the Deluxe **0.14 dB**.

Both amplifiers use the same AB763 roach: one package holding a neon lamp and a
photoresistor, the lamp driven from the 12AX7 oscillator through 10 M and 100 k,
the cell shunting the signal through the 50 kOhm reverse-audio Intensity
control. So a Deluxe fifty times shallower than a Twin is a wiring error, not a
quieter tremolo.

The error was the order of the 0.1 uF and the 220 kOhm around the intensity tap.
With the 220 kOhm *before* the tap, the photocell is isolated from the signal by
a quarter of a megohm and can do almost nothing; with it *after*, the cell sits
on the signal node and the 220 kOhm carries what survives to the inverter. At
the resolution of the Fender sheet the two parts can be read either way round,
and this is recorded as a **decision**: it is modelled the way the Twin's AB763
drawing shows the same circuit, because that is the arrangement that produces
the amplifier's known behaviour. Both now measure 7.6 dB.

`tests/deluxe.rs::the_tremolo_is_as_deep_as_the_twin_s` measures both and
compares them, so this cannot regress quietly.

**Where the module boundary goes, and why it moved.** The first version took
the channel's output straight off the mixer plate, which is where the drawing
puts the 0.001 uF to the inverter. That is faithful to the drawing and wrong for
this engine: the plate sits at about 190 V, so the chain handed a DC step to the
power module's coupling capacitor and the amplifier thumped on the first sample
of every session. `tests/voice.rs::silence_in_is_silence_out` caught it at
0.1012 against a 0.02 threshold, and `resetting_does_not_pop` with it.

The boundary is now drawn on the far side of that capacitor: the 0.001 uF is in
`circuits::deluxe`, on the plate where it belongs, working into the inverter's
1 M + 1 M grid-leak chain, which `load` stands for and which is why the Deluxe
is built with 2 MOhm where the other circuits use 1. `PowerSpec::DELUXE_6V6`
then carries a short in `pi_couple` so the capacitor is counted once.
Deliberately a short and not zero: zero means a *direct*-coupled inverter, which
turns `pi_leak_upper` into a series resistor -- the Hiwatt's cathode-follower
arrangement, not this one.

This made the model more correct, not less, and it showed: the hand-off is a
high-pass at about **80 Hz**, which is a real and fairly aggressive AB763
property and part of why these amplifiers stay tight. Measured small-signal
response with the stack at noon:

| Hz | 40 | 60 | 80 | 110 | 150 | 220 | 400 | 600 | 1 k | 5 k |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| dB | 32.8 | 35.5 | 37.1 | 38.0 | 38.1 | 37.5 | 35.3 | 34.0 | 34.0 | 45.0 |

A bass peak near 150 Hz, the Fender dip bottoming around 600 Hz to 1 kHz, and
the treble climbing 11 dB above it by 5 kHz. The first version of the scoop test
probed 80 Hz and so measured the hand-off corner rather than the stack; it now
probes 150 Hz and says why.

**Calibration.** `cargo run --release --example calibrate` regenerated with 36
voices. Two things worth recording:

* **No shipped voice moved.** Maximum drift across all 35 existing rows is
  **0.010 dB**, which is the last printed digit.
* **The Deluxe lands where the physics says it should**, without being tuned
  to. Same 0.122 V guitar into the front of each amplifier, Drive full up:

  | voice | distortion | make-up span |
  |---|---:|---:|
  | American Twin (85 W, four 6L6) | 31.7 % | 76.4 dB |
  | **American Deluxe (22 W, two 6V6)** | **42.2 %** | **53.6 dB** |
  | American 5150 | 57.2 % | 33.9 dB |
  | Cali IIC+ | 61.2 % | 32.6 dB |

  Dirtier than the Twin, cleaner than the high-gain amplifiers, at the same
  input. That ordering fell out of the netlist, the fitted 6V6 and the power
  stage; nothing in it was chosen to produce it.

**Presets.** Two, in the Amplifier group: *Blackface Deluxe* (Volume 0.30 with
reverb, 1x12 open back on the American Vintage 12) and *Deluxe Breakup*
(Volume 0.62 with the tremolo running). Both still want a `presetlevel` pass to
choose their `output_trim`.

**2026-09-22 — both channels, and the Normal one selectable.** The drawing has
two channels and only one of them was built. The Normal channel's *second*
stage was present, because it shares the [A] cathode and loads the mixer, but
its first stage, its stack and its Volume were not — so a triode's worth of
standing current was missing from the dropper chain that feeds every plate in
the amplifier.

Both channels are now built by the same code, `deluxe::front_end`, because on
the drawing they *are* the same code: same 68 k jacks, same 100 kΩ plate, same
1.5 kΩ / 25 µF cathode, same stack, same 1 MΩ Volume. They differ in two things
and the sheet is unambiguous about both — the Vibrato channel has the 47 pF
across its Volume and the Normal channel has not, and the reverb and the
tremolo hang on the Vibrato channel's second stage, joining the Normal channel
only at the mixer, *after* the intensity tap. So the Normal channel has no
reverb and no tremolo, and those controls rest rather than being greyed panel
knobs.

`deluxe::Channel` says which channel the jacks feed; the other stays in the
network with its jacks open (grid returned through the same 34 kΩ + 1 MΩ the High jack presents)
and its Volume resting at zero.

**The missing stage was worth four decibels of plate voltage.** Against the
drawing's own printed figures, at the drawing's own ±20 %:

| node | before | now | drawing |
|---|---:|---:|---:|
| `v1_p` | 193.4 V (+13.8 %) | **186.1 V (+9.5 %)** | 170 V |
| `v2_p` | 197.3 V (+9.6 %) | **189.9 V (+5.5 %)** | 180 V |

Nothing was tuned to produce that; it is the current of a triode that should
always have been there.

Selectable as `Gain::DeluxeNormal` / `Circuit::DeluxeNormal`
(`amp_fender_deluxe_ab763_normal`, display **American Deluxe Normal**), matched
to the same `PowerSpec::DELUXE_6V6`, with one preset, *Blackface Normal*.

Still to build: nothing on this list. See **Open questions** for what remains
uncertain rather than unbuilt.

## Why this amplifier costs what it costs, 2026-09-23

Reported from playing: it crackles once the input meter passes its middle.
Measured with `examples/stutter.rs`, one callback at a time, and most of what it
looked like it was turned out not to be.

**Three hypotheses tested and rejected.**

- *The optical tremolo restamps a resistor every sample.* It does, and it costs
  nothing measurable: `Blackface Throb` (tremolo on) is **cheaper** than
  `Blackface Clean` (tremolo off), and `Deluxe Breakup` is cheaper in the tail
  than `Blackface Deluxe`.
- *The solve is failing, like the Jazz 120's was.* It is not. Zero fallbacks and
  zero unsettled samples, driven to 40 V. The stage never fails to converge; it
  simply needs three Newton passes where the Twin needs two.
- *The Twin's measured solver settings will transfer.* They do not.
  `set_late_continuation(true)` is slightly **worse** (3.377 to 3.478 µs a
  sample) and `set_backtracks(4)` is neutral. Recorded so it is not retried.

**Where the time goes**, by removing one block at a time at -6 dBFS:

| block | Blackface Deluxe | Blackface Clean (Twin) |
|---|---:|---:|
| power stage | **251 µs** | 187 µs |
| everything ahead of it | 284 µs | 244 µs |
| reverb | 30 µs | 33 µs |
| acoustics | 35 µs | 29 µs |
| tone section | 26 µs | 35 µs |

The power stage is 47 % of the callback in both, and the whole of the difference
is the **GZ34**:

| Deluxe power stage | boundary | Newton passes | µs a sample |
|---|---:|---:|---:|
| with the rectifier | 16 of 31 | 3.06 | 3.39 |
| rectifier removed | 13 of 30 | 2.19 | **1.90** |

**Forty-four per cent of that block is the rectifier, and it stays.** It is the
sag, and the sag is what this amplifier is. Its anode node cannot be eliminated
either: a `supply` must carry finite series resistance, so the rectifier's anode
is always an unknown of its own. The gain circuit's three extra boundary nodes
over the Twin's are the **second channel**, which this model carries on purpose
-- its shared bias current and its load on the mixer node are real rather than
guessed -- and that is fidelity, not waste.

So there is nothing here to remove. What remains is solver architecture: a
separate small nonlinear block for the rectifier, or a lower rate for the supply
loop, which is a 120 Hz phenomenon behind 16 µF. Both touch this amplifier's
character and neither has been attempted.

**What was actually making it crackle** was none of the above. A fresh instance
defaulted to 2x oversampling while every shipped preset asks for Off, and this
is the most expensive voice in the catalogue:

| Blackface Deluxe, fresh instance, -6 dBFS | median | worst | missed |
|---|---:|---:|---:|
| 2x, the old default | 1,072 us | 1,848 us | **9** |
| Off, the presets' own setting | 544 us | 1,293 us | **0** |

80 % of a callback as the *median*, before any transient, and `Deluxe Breakup`
is worse again at 1,150 us and eleven missed. The parameter now
defaults to Off. As shipped and at every normal playing level the amplifier now
misses nothing; at a hot 0 dBFS input it misses one callback in 2,250, and the
headroom above that is thin.

## Open questions

- **The zero-bias end of the 6V6 fit is low.** About 60 mA at 50 V on the
  plate, where published plate-characteristic families suggest nearer 200 mA.
  No zero-bias figure for this tube is published in a form worth citing as a
  fit target, so it was not fitted. This is the region a hard-driven output
  stage actually works in, and `T6L6GC`'s comment in the same file records that
  the 6L6 was deliberately fitted to its *class-AB* range for exactly this
  reason. Before the power stage is calibrated, either a citable zero-bias
  point must be found or the fit must be re-anchored the way the 6L6 was.
- The 820 Ω/47 Ω feedback divider needs one more high-resolution read.
- Which period driver to take as the reference, and on what evidence.
